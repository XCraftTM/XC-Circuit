//! Saving on a thread of its own: the project's files, its backup and the
//! crash recovery copy are written there, one after another, so a slow disk
//! (or a virus scanner checking each new file) never holds up the window.
//!
//! The editor hands over a copy of the project and carries on; what happened
//! comes back to it ([`Saver::finished`]) in the order the jobs were given.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use circuit_core::Project;
use circuit_core::backup::Backups;

pub enum Job {
    /// Saves the project, then makes a backup of it if `backup` says how
    /// many to keep (the newest, and of the last days).
    Save { id: u64, project: Project, backups: Backups, backup: Option<(usize, u32)> },
    /// Writes the crash recovery copy of unsaved changes.
    Recovery { project: Project, backups: Backups },
    /// Deletes the crash recovery copy.
    ClearRecovery { root: PathBuf, backups: Backups },
}

pub enum Done {
    Saved {
        id: u64,
        root: PathBuf,
        name: String,
        /// Workspaces whose files the save was to delete.
        removed: Vec<String>,
        result: Result<(), String>,
        /// The backup, when one was asked for and the save worked.
        backup: Option<Result<(), String>>,
    },
    Recovery(Result<(), String>),
    Cleared,
}

pub struct Saver {
    jobs: Option<Sender<Job>>,
    done: Receiver<Done>,
    thread: Option<JoinHandle<()>>,
    /// Jobs given and not finished yet.
    pending: usize,
    next_id: u64,
}

impl Default for Saver {
    fn default() -> Self {
        let (jobs, inbox) = channel::<Job>();
        let (outbox, done) = channel();
        let thread = std::thread::Builder::new()
            .name("saving".into())
            .spawn(move || {
                for job in inbox {
                    if outbox.send(run(job)).is_err() {
                        break;
                    }
                }
            })
            .expect("starting the saving thread");
        Saver { jobs: Some(jobs), done, thread: Some(thread), pending: 0, next_id: 0 }
    }
}

impl Saver {
    /// A number for the next save, to tell its answer apart.
    pub fn next_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn send(&mut self, job: Job) {
        // The thread only ends when this is dropped, so it's always there.
        if let Some(jobs) = &self.jobs
            && jobs.send(job).is_ok()
        {
            self.pending += 1;
        }
    }

    /// Whether something is still being written.
    pub fn busy(&self) -> bool {
        self.pending > 0
    }

    /// What has finished, without waiting.
    pub fn finished(&mut self) -> Vec<Done> {
        let done: Vec<Done> = self.done.try_iter().collect();
        self.pending = self.pending.saturating_sub(done.len());
        done
    }

    /// Waits until everything given is written, and says how it went.
    pub fn wait(&mut self) -> Vec<Done> {
        let mut done = Vec::new();
        while self.pending > 0 {
            match self.done.recv() {
                Ok(d) => {
                    done.push(d);
                    self.pending -= 1;
                }
                Err(_) => {
                    self.pending = 0;
                }
            }
        }
        done
    }
}

impl Drop for Saver {
    /// Whatever was given is still written.
    fn drop(&mut self) {
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(job: Job) -> Done {
    match job {
        Job::Save { id, mut project, backups, backup } => {
            let removed = project.removed.clone();
            let result = project.save().map_err(|e| format!("{e:#}"));
            let backup = match (&result, backup) {
                (Ok(()), Some((keep, days))) => {
                    backups.clear_recovery(&project.root);
                    let made = backups.snapshot(&project, "Saved").map(|_| ()).map_err(|e| format!("{e:#}"));
                    backups.prune(&project.root, keep, days, circuit_core::backup::now_ms());
                    Some(made)
                }
                (Ok(()), None) => {
                    backups.clear_recovery(&project.root);
                    None
                }
                (Err(_), _) => None,
            };
            Done::Saved { id, root: project.root, name: project.meta.name, removed, result, backup }
        }
        Job::Recovery { project, backups } => Done::Recovery(backups.write_recovery(&project).map_err(|e| format!("{e:#}"))),
        Job::ClearRecovery { root, backups } => {
            backups.clear_recovery(&root);
            Done::Cleared
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xc-saver-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn jobs_are_written_in_order_and_answered() {
        let dir = temp("order");
        let backups = Backups::new(dir.join("backups"));
        let mut project = Project::create(dir.join("bot"), "Bot", "none").unwrap();
        let mut saver = Saver::default();
        saver.send(Job::Recovery { project: project.clone(), backups: backups.clone() });
        project.meta.name = "Renamed".into();
        let id = saver.next_id();
        saver.send(Job::Save { id, project: project.take_save(), backups: backups.clone(), backup: Some((5, 14)) });
        assert!(saver.busy());
        let done = saver.wait();
        assert!(!saver.busy());
        assert!(matches!(done[0], Done::Recovery(Ok(()))));
        let Done::Saved { id: got, name, result, backup, .. } = &done[1] else { panic!("the save comes second") };
        assert_eq!((*got, name.as_str()), (id, "Renamed"));
        assert!(result.is_ok() && matches!(backup, Some(Ok(()))));
        assert!(std::fs::read_to_string(dir.join("bot").join("circuit.json")).unwrap().contains("Renamed"), "written to disk");
        assert!(backups.read_recovery(&project).is_none(), "the save clears the recovery copy");
        assert_eq!(backups.list(&project.root).len(), 1, "and makes a backup");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_save_says_why_and_which_files_it_was_to_delete() {
        let dir = temp("fails");
        let mut project = Project::create(dir.join("bot"), "Bot", "none").unwrap();
        project.remove_workspace("main");
        // A file where the project's folder should be.
        project.root = dir.join("not-a-folder");
        std::fs::write(&project.root, "").unwrap();
        let mut saver = Saver::default();
        let id = saver.next_id();
        saver.send(Job::Save { id, project: project.take_save(), backups: Backups::new(dir.join("backups")), backup: Some((5, 14)) });
        assert!(project.removed.is_empty(), "handed over with the copy");
        let Done::Saved { result, removed, backup, .. } = saver.wait().remove(0) else { panic!() };
        assert!(result.is_err());
        assert_eq!(removed, vec!["main".to_string()]);
        assert!(backup.is_none(), "no backup of a project that wasn't saved");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dropping_it_still_writes_what_was_given() {
        let dir = temp("drop");
        let mut project = Project::create(dir.join("bot"), "Bot", "none").unwrap();
        project.meta.name = "Kept".into();
        {
            let mut saver = Saver::default();
            let id = saver.next_id();
            saver.send(Job::Save { id, project: project.take_save(), backups: Backups::new(dir.join("backups")), backup: None });
        }
        assert!(std::fs::read_to_string(dir.join("bot").join("circuit.json")).unwrap().contains("Kept"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
