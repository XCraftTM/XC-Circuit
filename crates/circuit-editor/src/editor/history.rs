//! Undo and redo per workspace, and whether the project has unsaved changes.
//! A step is recorded when an edit settles (no mouse button held, no text box
//! focused), so a whole drag or a whole edit is one step.
//!
//! Every recorded state gets a version number. The project is unsaved when
//! the versions differ from the ones it had when it was last saved, so
//! undoing back to the saved state makes it saved again.

use std::collections::HashMap;

use circuit_core::project::ProjectMeta;
use circuit_core::{Project, Workspace};

/// The steps kept, until [`Histories::set_limit`] says otherwise (Settings › Editor).
const LIMIT: usize = crate::editor::prefs::UNDO_STEPS;

/// The states of one thing (a workspace, or the project's settings and data).
struct History<T> {
    undo: Vec<(T, u64)>,
    redo: Vec<(T, u64)>,
    last: T,
    /// The version of `last`.
    version: u64,
}

impl<T: Clone + PartialEq> History<T> {
    fn new(now: &T, version: u64) -> Self {
        History { undo: Vec::new(), redo: Vec::new(), last: now.clone(), version }
    }

    /// Records `now` as a step if it changed, keeping the newest `limit`
    /// steps. Returns whether it did.
    fn record(&mut self, now: &T, next: &mut u64, limit: usize) -> bool {
        if *now == self.last {
            return false;
        }
        *next += 1;
        let prev = std::mem::replace(&mut self.last, now.clone());
        self.undo.push((prev, std::mem::replace(&mut self.version, *next)));
        self.keep(limit);
        self.redo.clear();
        true
    }

    /// Forgets the oldest steps beyond `limit`.
    fn keep(&mut self, limit: usize) {
        let extra = self.undo.len().saturating_sub(limit);
        self.undo.drain(..extra);
    }

    fn undo(&mut self, now: &mut T, next: &mut u64, limit: usize) -> bool {
        // An edit that has not settled yet is undone first.
        self.record(now, next, limit);
        let Some((prev, version)) = self.undo.pop() else { return false };
        self.redo.push((std::mem::replace(now, prev.clone()), self.version));
        (self.last, self.version) = (prev, version);
        true
    }

    fn redo(&mut self, now: &mut T) -> bool {
        let Some((next, version)) = self.redo.pop() else { return false };
        self.undo.push((std::mem::replace(now, next.clone()), self.version));
        (self.last, self.version) = (next, version);
        true
    }
}

/// The key that stands for the project's settings and data (such as the
/// slash commands) in undo and redo.
pub const PROJECT: &str = "@project";

pub struct Histories {
    workspaces: HashMap<String, History<Workspace>>,
    project: Option<History<ProjectMeta>>,
    /// The highest version handed out.
    next: u64,
    /// Versions when last saved: per workspace, and the project's. `None`
    /// when not known (after taking over unsaved changes), which counts as unsaved.
    saved: Option<(HashMap<String, u64>, u64)>,
    /// Counts recorded changes, undos and redos, so others can tell the
    /// project changed without comparing it.
    revision: u64,
    /// How many steps each workspace (and the project's settings) keeps.
    limit: usize,
}

impl Default for Histories {
    fn default() -> Self {
        Histories { workspaces: HashMap::new(), project: None, next: 0, saved: None, revision: 0, limit: LIMIT }
    }
}

impl Histories {
    /// Forgets every step; what is open now counts as unsaved.
    pub fn clear(&mut self) {
        *self = Histories { next: self.next, revision: self.revision + 1, limit: self.limit, ..Default::default() };
    }

    /// Changes whenever the recorded project does (a change, undo, redo, or
    /// another project).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Forgets every step and takes `project` as saved, as it is now.
    pub fn reset_saved(&mut self, project: &Project) {
        self.clear();
        self.track(project, true);
        self.mark_saved();
    }

    /// Records changes made since the last settled state.
    pub fn track(&mut self, project: &Project, settled: bool) {
        let before = self.workspaces.len();
        self.workspaces.retain(|k, _| project.workspaces.contains_key(k));
        let mut changed = self.workspaces.len() != before;
        let next = &mut self.next;
        for (key, ws) in &project.workspaces {
            match self.workspaces.get_mut(key) {
                Some(h) if settled => {
                    changed |= h.record(ws, next, self.limit);
                }
                Some(_) => {}
                None => {
                    *next += 1;
                    self.workspaces.insert(key.clone(), History::new(ws, *next));
                    changed = true;
                }
            }
        }
        if changed {
            self.revision += 1;
        }
        match &mut self.project {
            Some(h) if settled => {
                if h.record(&project.meta, next, self.limit) {
                    self.revision += 1;
                }
            }
            Some(_) => {}
            None => {
                *next += 1;
                self.project = Some(History::new(&project.meta, *next));
            }
        }
    }

    /// Keeps at most `limit` steps per workspace (and for the project's
    /// settings) from now on: Settings › Editor. Fewer than before forgets
    /// the oldest steps at once, and the memory their copies took.
    pub fn set_limit(&mut self, limit: usize) {
        if limit == self.limit {
            return;
        }
        self.limit = limit;
        for h in self.workspaces.values_mut() {
            h.keep(limit);
        }
        if let Some(h) = &mut self.project {
            h.keep(limit);
        }
    }

    /// Takes the recorded state as saved; `track` it first.
    pub fn mark_saved(&mut self) {
        let workspaces = self.workspaces.iter().map(|(k, h)| (k.clone(), h.version)).collect();
        self.saved = Some((workspaces, self.project.as_ref().map_or(0, |h| h.version)));
    }

    /// Whether the recorded state differs from the saved one.
    pub fn unsaved(&self) -> bool {
        let Some((workspaces, project)) = &self.saved else { return true };
        workspaces.len() != self.workspaces.len()
            || self.workspaces.iter().any(|(k, h)| workspaces.get(k) != Some(&h.version))
            || self.project.as_ref().is_some_and(|h| h.version != *project)
    }

    pub fn undo(&mut self, key: &str, ws: &mut Workspace) -> bool {
        let Some(h) = self.workspaces.get_mut(key) else { return false };
        self.revision += 1;
        h.undo(ws, &mut self.next, self.limit)
    }

    pub fn redo(&mut self, key: &str, ws: &mut Workspace) -> bool {
        let Some(h) = self.workspaces.get_mut(key) else { return false };
        self.revision += 1;
        h.redo(ws)
    }

    /// Undo for the project's settings and data.
    pub fn undo_project(&mut self, meta: &mut ProjectMeta) -> bool {
        let Some(h) = self.project.as_mut() else { return false };
        self.revision += 1;
        h.undo(meta, &mut self.next, self.limit)
    }

    pub fn redo_project(&mut self, meta: &mut ProjectMeta) -> bool {
        let Some(h) = self.project.as_mut() else { return false };
        self.revision += 1;
        h.redo(meta)
    }

    pub fn can_undo(&self, key: &str) -> bool {
        match key {
            PROJECT => self.project.as_ref().is_some_and(|h| !h.undo.is_empty()),
            _ => self.workspaces.get(key).is_some_and(|h| !h.undo.is_empty()),
        }
    }

    pub fn can_redo(&self, key: &str) -> bool {
        match key {
            PROJECT => self.project.as_ref().is_some_and(|h| !h.redo.is_empty()),
            _ => self.workspaces.get(key).is_some_and(|h| !h.redo.is_empty()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use circuit_core::Node;

    /// The steps kept follow Settings › Editor: only the newest ones, and
    /// fewer chosen later forgets the oldest at once.
    #[test]
    fn keeps_as_many_steps_as_chosen() {
        let dir = std::env::temp_dir().join(format!("xc-history-limit-{}", std::process::id()));
        let mut p = Project::create(&dir, "H", "m").unwrap();
        let mut h = Histories::default();
        h.set_limit(3);
        h.reset_saved(&p);
        let undos = |h: &mut Histories, p: &mut Project| {
            let ws = p.workspaces.get_mut("main").unwrap();
            let mut n = 0;
            while h.undo("main", ws) {
                n += 1;
            }
            while h.redo("main", ws) {}
            n
        };
        for i in 0..5 {
            p.workspaces.get_mut("main").unwrap().nodes.insert(format!("n{i}"), Node { block: "x@1".into(), ..Default::default() });
            h.track(&p, true);
        }
        assert_eq!(undos(&mut h, &mut p), 3, "the newest three");
        assert_eq!(p.workspaces["main"].nodes.len(), 5, "redone again");
        h.set_limit(2);
        assert_eq!(undos(&mut h, &mut p), 2, "fewer: the oldest are forgotten");
        h.set_limit(10);
        p.workspaces.get_mut("main").unwrap().nodes.clear();
        h.track(&p, true);
        assert_eq!(undos(&mut h, &mut p), 3, "more: room for new steps");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn records_settled_steps_and_restores_them() {
        let dir = std::env::temp_dir().join(format!("xc-history-{}", std::process::id()));
        let mut p = Project::create(&dir, "H", "m").unwrap();
        let mut h = Histories::default();
        h.reset_saved(&p);
        assert!(!h.unsaved());

        let ws = p.workspaces.get_mut("main").unwrap();
        ws.nodes.insert("a".into(), Node { block: "x@1".into(), ..Default::default() });
        h.track(&p, false); // still dragging: nothing recorded
        assert!(!h.can_undo("main"));
        let before = h.revision();
        h.track(&p, true);
        assert!(h.can_undo("main"));
        assert!(h.unsaved());
        assert!(h.revision() > before, "a change moves the revision on");
        let after = h.revision();
        h.track(&p, true);
        assert_eq!(h.revision(), after, "nothing changed, same revision");

        let ws = p.workspaces.get_mut("main").unwrap();
        assert!(h.undo("main", ws));
        assert!(ws.nodes.is_empty());
        assert!(!h.unsaved(), "undone back to the saved state");
        assert!(h.redo("main", ws));
        assert_eq!(ws.nodes.len(), 1);
        assert!(h.unsaved());
        h.mark_saved();
        assert!(!h.unsaved());

        // The project's settings and data are tracked and undone too.
        p.meta.settings.insert("devServer".into(), "1".into());
        h.track(&p, true);
        assert!(h.unsaved() && h.can_undo(PROJECT));
        assert!(h.undo_project(&mut p.meta));
        assert!(p.meta.settings.is_empty());
        assert!(!h.unsaved());

        // A new or deleted workspace is a change.
        p.workspaces.insert("more".into(), Workspace::new("More"));
        h.track(&p, true);
        assert!(h.unsaved());
        p.workspaces.shift_remove("more");
        h.track(&p, true);
        assert!(!h.unsaved());
        p.workspaces.shift_remove("main");
        h.track(&p, true);
        assert!(h.unsaved());

        // Taken-over changes count as unsaved until saved.
        h.clear();
        h.track(&p, true);
        assert!(h.unsaved());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
