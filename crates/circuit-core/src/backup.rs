//! Backups of a project, kept outside it (so they never end up in Git or in an
//! export), and the crash recovery copy of unsaved changes.
//!
//! Layout under the backup folder:
//! ```text
//! <project id>/
//!   recovery.json              unsaved changes, while there are any
//!   1762101012345/             one snapshot per save (milliseconds since 1970)
//!     info.json
//!     circuit.json
//!     workspaces/<key>.json
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::graph::Workspace;
use crate::project::{Project, ProjectMeta, WORKSPACE_DIR, write_json};

/// The whole project (settings and data, shared and local) in a snapshot.
/// Its name is the snapshots' own: they are read only from here.
const PROJECT_FILE: &str = "circuit.json";

const INFO: &str = "info.json";
const RECOVERY: &str = "recovery.json";
const DAY_MS: u64 = 86_400_000;

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// FNV-1a of the project folder's full path: the backup folder of a project
/// that has no id of its own yet (the layout before).
pub fn project_id(root: &Path) -> String {
    let path = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let hash = path.to_string_lossy().bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    format!("{hash:016x}")
}

/// What a snapshot holds, written next to it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub saved_at: u64,
    pub project: String,
    pub root: String,
    /// Workspace key and name.
    pub workspaces: Vec<(String, String)>,
    /// Why it was made: "Saved", "Before restore", …
    pub reason: String,
    /// Of the project's content, to skip saves that changed nothing.
    pub hash: String,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub dir: PathBuf,
    pub info: SnapshotInfo,
    /// Bytes on disk.
    pub size: u64,
}

/// Unsaved changes, written while editing and offered after a crash.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recovery {
    pub saved_at: u64,
    pub meta: ProjectMeta,
    pub workspaces: IndexMap<String, Workspace>,
}

/// The content that a snapshot or recovery copy stands for, as one hash.
fn content_hash(meta: &ProjectMeta, workspaces: &IndexMap<String, Workspace>) -> String {
    let text = serde_json::to_string(&(meta, workspaces)).unwrap_or_default();
    let h = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    format!("{h:016x}")
}

fn dir_size(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

#[derive(Clone)]
pub struct Backups {
    base: PathBuf,
}

impl Backups {
    pub fn new(base: impl Into<PathBuf>) -> Backups {
        Backups { base: base.into() }
    }

    /// The folder holding one project's backups.
    pub fn folder(&self, root: &Path) -> PathBuf {
        let by_path = self.base.join(project_id(root));
        // By the project's id, so its backups stay with it when its folder
        // is moved or renamed. Those made while it went by its folder's path
        // come along the first time.
        match crate::project::id_of(root) {
            Some(id) => {
                let by_id = self.base.join(id);
                if !by_id.exists() && by_path.is_dir() {
                    let _ = fs::rename(&by_path, &by_id);
                }
                by_id
            }
            None => by_path,
        }
    }

    /// Saves a copy of the project as it is in memory. Returns `None` when the
    /// newest snapshot already has exactly this content.
    pub fn snapshot(&self, project: &Project, reason: &str) -> Result<Option<Snapshot>> {
        let hash = content_hash(&project.meta, &project.workspaces);
        if self.entries(&project.root).first().is_some_and(|(_, info)| info.hash == hash) {
            return Ok(None);
        }
        let mut at = now_ms();
        let folder = self.folder(&project.root);
        // Left over when a snapshot was cut short (a crash while saving).
        for e in fs::read_dir(&folder).into_iter().flatten().flatten() {
            if e.file_name().to_string_lossy().ends_with(".partial") {
                let _ = fs::remove_dir_all(e.path());
            }
        }
        while folder.join(format!("{at:013}")).exists() {
            at += 1;
        }
        let dir = folder.join(format!("{at:013}"));
        let tmp = folder.join(format!("{at:013}.partial"));
        fs::create_dir_all(tmp.join(WORKSPACE_DIR))?;
        write_json(&tmp.join(PROJECT_FILE), &project.meta)?;
        for (key, ws) in &project.workspaces {
            write_json(&tmp.join(WORKSPACE_DIR).join(format!("{key}.json")), ws)?;
        }
        let info = SnapshotInfo {
            saved_at: at,
            project: project.meta.name.clone(),
            root: project.root.display().to_string(),
            workspaces: project.workspaces.iter().map(|(k, w)| (k.clone(), w.name.clone())).collect(),
            reason: reason.to_string(),
            hash,
        };
        write_json(&tmp.join(INFO), &info)?;
        // Complete or not there at all.
        fs::rename(&tmp, &dir)?;
        let size = dir_size(&dir);
        Ok(Some(Snapshot { dir, info, size }))
    }

    /// Snapshots of a project, newest first, with their size on disk.
    /// Unfinished or unreadable ones are left out.
    pub fn list(&self, root: &Path) -> Vec<Snapshot> {
        self.entries(root).into_iter().map(|(dir, info)| Snapshot { size: dir_size(&dir), dir, info }).collect()
    }

    /// Snapshot folders and what they hold, newest first, without adding up
    /// their sizes.
    fn entries(&self, root: &Path) -> Vec<(PathBuf, SnapshotInfo)> {
        let mut out: Vec<(PathBuf, SnapshotInfo)> = fs::read_dir(self.folder(root))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir() && e.file_name().to_string_lossy().chars().all(|c| c.is_ascii_digit()))
            .filter_map(|e| {
                let dir = e.path();
                let info: SnapshotInfo = serde_json::from_str(&fs::read_to_string(dir.join(INFO)).ok()?).ok()?;
                Some((dir, info))
            })
            .collect();
        out.sort_by_key(|(_, info)| std::cmp::Reverse(info.saved_at));
        out
    }

    /// Keeps the newest `keep_last`, plus the newest of each of the last
    /// `keep_days` days; deletes the rest. Returns how many were deleted.
    pub fn prune(&self, root: &Path, keep_last: usize, keep_days: u32, now: u64) -> usize {
        let mut days_kept = std::collections::HashSet::new();
        let mut deleted = 0;
        for (i, (dir, info)) in self.entries(root).iter().enumerate() {
            let day = info.saved_at / DAY_MS;
            let recent_day = now.saturating_sub(info.saved_at) < keep_days as u64 * DAY_MS;
            let keep = i < keep_last || (recent_day && days_kept.insert(day));
            if !keep && fs::remove_dir_all(dir).is_ok() {
                deleted += 1;
            }
        }
        deleted
    }

    /// The project's settings and workspaces as they were in a snapshot.
    pub fn load(&self, snap: &Snapshot) -> Result<(ProjectMeta, IndexMap<String, Workspace>)> {
        let meta: ProjectMeta = serde_json::from_str(&fs::read_to_string(snap.dir.join(PROJECT_FILE))?)?;
        let mut workspaces = IndexMap::new();
        for (key, _) in &snap.info.workspaces {
            let path = snap.dir.join(WORKSPACE_DIR).join(format!("{key}.json"));
            let ws: Workspace = serde_json::from_str(&fs::read_to_string(&path)?).with_context(|| format!("reading {}", path.display()))?;
            workspaces.insert(key.clone(), ws);
        }
        Ok((meta, workspaces))
    }

    /// Writes the unsaved state of the project, replacing the previous copy.
    pub fn write_recovery(&self, project: &Project) -> Result<()> {
        let folder = self.folder(&project.root);
        fs::create_dir_all(&folder)?;
        let rec = Recovery { saved_at: now_ms(), meta: project.meta.clone(), workspaces: project.workspaces.clone() };
        write_json(&folder.join(RECOVERY), &rec)
    }

    /// Unsaved changes left behind (by a crash), if they differ from what is saved.
    pub fn read_recovery(&self, project: &Project) -> Option<Recovery> {
        let text = fs::read_to_string(self.folder(&project.root).join(RECOVERY)).ok()?;
        let rec: Recovery = serde_json::from_str(&text).ok()?;
        let same = content_hash(&rec.meta, &rec.workspaces) == content_hash(&project.meta, &project.workspaces);
        if same {
            self.clear_recovery(&project.root);
            return None;
        }
        Some(rec)
    }

    pub fn clear_recovery(&self, root: &Path) {
        let _ = fs::remove_file(self.folder(root).join(RECOVERY));
    }
}

/// How a workspace in a backup compares with the open project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Same,
    Changed,
    /// In the backup, not in the project now.
    OnlyInBackup,
    /// In the project now, not in the backup.
    OnlyNow,
}

/// Per workspace: key, name and how it differs.
pub fn compare(backup: &IndexMap<String, Workspace>, now: &IndexMap<String, Workspace>) -> Vec<(String, String, Change)> {
    let mut out: Vec<(String, String, Change)> = backup
        .iter()
        .map(|(k, w)| {
            let change = match now.get(k) {
                Some(n) if n == w => Change::Same,
                Some(_) => Change::Changed,
                None => Change::OnlyInBackup,
            };
            (k.clone(), w.name.clone(), change)
        })
        .collect();
    out.extend(now.iter().filter(|(k, _)| !backup.contains_key(*k)).map(|(k, w)| (k.clone(), w.name.clone(), Change::OnlyNow)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xc-backup-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// Backups go by the project's id: they stay with a project whose folder
    /// was moved, and those from before it had an id come along.
    #[test]
    fn backups_follow_the_project() {
        let root = temp("follow");
        let backups = Backups::new(temp("follow-store"));
        let p = Project::create(&root, "Bot", "none").unwrap();
        // Made while backups went by the folder's path.
        let old = backups.base.join(project_id(&root));
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("marker.txt"), "from before").unwrap();
        let folder = backups.folder(&root);
        assert_eq!(folder.file_name().unwrap().to_string_lossy(), p.meta.id);
        assert!(folder.join("marker.txt").is_file() && !old.exists(), "the earlier backups came along");
        backups.snapshot(&p, "Saved").unwrap().expect("a snapshot");

        // The folder moved: the same backups.
        let moved = temp("follow-moved");
        fs::rename(&root, &moved).unwrap();
        assert_eq!(backups.folder(&moved), folder);
        assert_eq!(backups.list(&moved).len(), 1);
    }

    #[test]
    fn snapshots_skip_unchanged_saves_and_load_back() {
        let root = temp("proj");
        let backups = Backups::new(temp("store"));
        let mut p = Project::create(&root, "Bot", "none").unwrap();
        let first = backups.snapshot(&p, "Saved").unwrap().expect("first snapshot");
        assert!(backups.snapshot(&p, "Saved").unwrap().is_none(), "nothing changed, no new snapshot");
        // A snapshot cut short earlier is cleared away by the next one.
        let partial = backups.folder(&root).join("0000000000001.partial");
        fs::create_dir_all(&partial).unwrap();
        p.workspaces.get_mut("main").unwrap().name = "Renamed".into();
        backups.snapshot(&p, "Saved").unwrap().expect("a change makes a new one");
        assert!(!partial.exists());
        let list = backups.list(&root);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].info.workspaces[0].1, "Renamed", "newest first");
        let (_, old) = backups.load(&list[1]).unwrap();
        assert_eq!(old["main"].name, "Main");
        assert_eq!(list[1].dir, first.dir);
        let diff = compare(&old, &p.workspaces);
        assert_eq!(diff[0].2, Change::Changed);
    }

    #[test]
    fn pruning_keeps_the_newest_and_one_per_day() {
        let root = temp("prune");
        let backups = Backups::new(temp("prune-store"));
        let p = Project::create(&root, "Bot", "none").unwrap();
        let folder = backups.folder(&root);
        let now = 100 * DAY_MS + 12 * 3_600_000;
        // Three a day for 30 days.
        for day in 0..30u64 {
            for hour in [1u64, 8, 15] {
                let at = now - day * DAY_MS - hour * 3_600_000;
                let dir = folder.join(format!("{at:013}"));
                fs::create_dir_all(&dir).unwrap();
                let info = SnapshotInfo { saved_at: at, project: "Bot".into(), root: String::new(), workspaces: vec![], reason: "Saved".into(), hash: format!("{at}") };
                fs::write(dir.join(INFO), serde_json::to_string(&info).unwrap()).unwrap();
            }
        }
        let _ = p;
        backups.prune(&root, 5, 14, now);
        let left = backups.list(&root);
        // The newest 5 (from the last two days), plus one for each other day of the last 14.
        let days: std::collections::HashSet<u64> = left.iter().map(|s| s.info.saved_at / DAY_MS).collect();
        assert!(left.len() >= 14 && left.len() <= 5 + 14, "{}", left.len());
        assert!(days.len() >= 14);
        assert!(left.iter().all(|s| now - s.info.saved_at < 15 * DAY_MS), "nothing older than the kept days");
    }

    #[test]
    fn recovery_is_offered_only_when_it_differs() {
        let root = temp("recover");
        let backups = Backups::new(temp("recover-store"));
        let mut p = Project::create(&root, "Bot", "none").unwrap();
        p.workspaces.get_mut("main").unwrap().name = "Unsaved name".into();
        backups.write_recovery(&p).unwrap();
        let saved = Project::load(&root).unwrap();
        let rec = backups.read_recovery(&saved).expect("unsaved changes found");
        assert_eq!(rec.workspaces["main"].name, "Unsaved name");
        // Once saved, the copy is the same as the files and isn't offered.
        p.save().unwrap();
        assert!(backups.read_recovery(&Project::load(&root).unwrap()).is_none());
    }
}
