//! Reloads plugins when their files change, so block authors see edits right away.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Edits often arrive as several events; wait for them to settle.
const SETTLE: Duration = Duration::from_millis(400);

pub struct PluginWatcher {
    watcher: Option<RecommendedWatcher>,
    rx: Receiver<PathBuf>,
    tx: std::sync::mpsc::Sender<PathBuf>,
    pending: Option<Instant>,
    watched: Vec<PathBuf>,
}

impl PluginWatcher {
    pub fn new(repaint: impl Fn() + Send + Sync + 'static) -> PluginWatcher {
        let (tx, rx) = channel();
        let sender = tx.clone();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(e) = res
                && matches!(e.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_))
            {
                for p in e.paths {
                    if is_plugin_file(&p) {
                        let _ = sender.send(p);
                        repaint();
                    }
                }
            }
        })
        .ok();
        PluginWatcher { watcher, rx, tx, pending: None, watched: Vec::new() }
    }

    /// Watches these folders (and stops watching others).
    pub fn watch(&mut self, dirs: &[PathBuf]) {
        let Some(w) = self.watcher.as_mut() else { return };
        for old in std::mem::take(&mut self.watched) {
            let _ = w.unwatch(&old);
        }
        for d in dirs.iter().filter(|d| d.is_dir()) {
            if w.watch(d, RecursiveMode::Recursive).is_ok() {
                self.watched.push(d.clone());
            }
        }
        let _ = &self.tx;
    }

    /// True once changes have settled and plugins should reload.
    pub fn poll(&mut self) -> bool {
        if self.rx.try_iter().count() > 0 {
            self.pending = Some(Instant::now());
        }
        match self.pending {
            Some(t) if t.elapsed() >= SETTLE => {
                self.pending = None;
                true
            }
            _ => false,
        }
    }

    pub fn waiting(&self) -> bool {
        self.pending.is_some()
    }
}

fn is_plugin_file(p: &Path) -> bool {
    matches!(p.extension().and_then(|e| e.to_str()), Some("js" | "cjs" | "mjs" | "ts" | "toml" | "svg" | "png" | "json"))
}
