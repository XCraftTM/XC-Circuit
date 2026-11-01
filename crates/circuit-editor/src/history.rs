//! Undo and redo per workspace. A step is recorded when an edit settles (no mouse
//! button held, no text box focused), so a whole drag or a whole edit is one step.

use std::collections::HashMap;

use circuit_core::{Project, Workspace};

const LIMIT: usize = 200;

struct History {
    undo: Vec<Workspace>,
    redo: Vec<Workspace>,
    last: Workspace,
}

#[derive(Default)]
pub struct Histories(HashMap<String, History>);

impl Histories {
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Records changes made since the last settled state.
    pub fn track(&mut self, project: &Project, settled: bool) {
        self.0.retain(|k, _| project.workspaces.contains_key(k));
        for (key, ws) in &project.workspaces {
            let h = self.0.entry(key.clone()).or_insert_with(|| History { undo: Vec::new(), redo: Vec::new(), last: ws.clone() });
            if settled && *ws != h.last {
                h.undo.push(std::mem::replace(&mut h.last, ws.clone()));
                if h.undo.len() > LIMIT {
                    h.undo.remove(0);
                }
                h.redo.clear();
            }
        }
    }

    pub fn undo(&mut self, key: &str, ws: &mut Workspace) -> bool {
        let Some(h) = self.0.get_mut(key) else { return false };
        // An edit that has not settled yet is undone first.
        let target = if *ws != h.last { Some(h.last.clone()) } else { h.undo.pop() };
        let Some(prev) = target else { return false };
        h.redo.push(std::mem::replace(ws, prev.clone()));
        h.last = prev;
        true
    }

    pub fn redo(&mut self, key: &str, ws: &mut Workspace) -> bool {
        let Some(h) = self.0.get_mut(key) else { return false };
        let Some(next) = h.redo.pop() else { return false };
        h.undo.push(std::mem::replace(ws, next.clone()));
        h.last = next;
        true
    }

    pub fn can_undo(&self, key: &str) -> bool {
        self.0.get(key).is_some_and(|h| !h.undo.is_empty())
    }

    pub fn can_redo(&self, key: &str) -> bool {
        self.0.get(key).is_some_and(|h| !h.redo.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use circuit_core::Node;

    #[test]
    fn records_settled_steps_and_restores_them() {
        let dir = std::env::temp_dir().join(format!("xc-history-{}", std::process::id()));
        let mut p = Project::create(&dir, "H", "m").unwrap();
        let mut h = Histories::default();
        h.track(&p, true);

        let ws = p.workspaces.get_mut("main").unwrap();
        ws.nodes.insert("a".into(), Node { block: "x@1".into(), ..Default::default() });
        h.track(&p, false); // still dragging: nothing recorded
        assert!(!h.can_undo("main"));
        h.track(&p, true);
        assert!(h.can_undo("main"));

        let ws = p.workspaces.get_mut("main").unwrap();
        assert!(h.undo("main", ws));
        assert!(ws.nodes.is_empty());
        assert!(h.redo("main", ws));
        assert_eq!(ws.nodes.len(), 1);
    }
}
