//! Editor state shared by every panel: plugins, the open project, cached
//! block shapes, selection and the log.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use circuit_core::{BlockShape, Node, Project, Workspace};
use circuit_core::graph::ACTION_IN;
use circuit_plugins::{BlockEngine, BuildMode, Registry};
use eframe::egui::Pos2;
use serde_json::{Map, Value};

use crate::canvas::CanvasView;
use crate::picker::Picker;
use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Warn,
    Error,
}

#[derive(Clone, Debug)]
pub struct LogLine {
    pub time: String,
    pub level: Level,
    pub text: String,
    /// The block that wrote it, as (workspace key, node id).
    pub target: Option<(String, String)>,
}

impl LogLine {
    pub fn new(level: Level, text: impl Into<String>) -> Self {
        LogLine { time: clock(), level, text: text.into(), target: None }
    }
}

/// Work the dock can only do after it has finished drawing this frame.
#[derive(Clone, Debug)]
pub enum Action {
    OpenWorkspace(String),
    CloseWorkspaceTabs(String),
}

pub struct Editor {
    pub reg: Registry,
    pub engine: BlockEngine,
    pub plugin_dirs: Vec<PathBuf>,
    pub theme: Theme,
    pub project: Option<Project>,
    pub dirty: bool,
    /// Selected node as (workspace key, node id).
    pub selected: Option<(String, String)>,
    pub log: Vec<LogLine>,
    pub shapes: HashMap<(String, String), (u64, BlockShape)>,
    pub views: HashMap<String, CanvasView>,
    pub picker: Option<Picker>,
    pub actions: Vec<Action>,
    /// The workspace whose canvas was used last.
    pub active_workspace: Option<String>,
    /// What blocks and zones of the running bot did recently, keyed by (workspace, id).
    pub activity: HashMap<(String, String), crate::bot::Activity>,
}

impl Editor {
    pub fn new(plugin_dirs: Vec<PathBuf>, theme_id: Option<&str>) -> anyhow::Result<Editor> {
        let engine = BlockEngine::new()?;
        let reg = Registry::load(&plugin_dirs, &engine);
        let mut ed = Editor {
            reg,
            engine,
            plugin_dirs,
            theme: Theme::fallback(),
            project: None,
            dirty: false,
            selected: None,
            log: Vec::new(),
            shapes: HashMap::new(),
            views: HashMap::new(),
            picker: None,
            actions: Vec::new(),
            active_workspace: None,
            activity: HashMap::new(),
        };
        ed.set_theme(theme_id.unwrap_or("graphite"));
        ed.log_plugin_summary();
        Ok(ed)
    }

    pub fn log(&mut self, level: Level, text: impl Into<String>) {
        self.log.push(LogLine::new(level, text));
        if self.log.len() > 2000 {
            self.log.drain(..500);
        }
    }

    fn log_plugin_summary(&mut self) {
        let summary = format!(
            "Loaded {} plugins, {} blocks, {} pin types, {} themes",
            self.reg.plugins.len(),
            self.reg.blocks.len(),
            self.reg.types.len(),
            self.reg.themes.len()
        );
        self.log(Level::Info, summary);
        let problems = self.reg.problems.len();
        if problems > 0 {
            self.log(Level::Warn, format!("{problems} plugin problems, see the Problems panel"));
        }
    }

    pub fn set_theme(&mut self, id: &str) {
        self.theme = self
            .reg
            .theme(id)
            .or_else(|| self.reg.themes.first())
            .map(Theme::from_def)
            .unwrap_or_else(Theme::fallback);
    }

    /// Reloads every plugin, including the open project's own `plugins/` folder.
    pub fn reload_plugins(&mut self) {
        let mut dirs = self.plugin_dirs.clone();
        if let Some(p) = &self.project {
            dirs.push(p.root.join("plugins"));
        }
        match BlockEngine::new() {
            Ok(engine) => self.engine = engine,
            Err(e) => return self.log(Level::Error, format!("could not restart the block sandbox: {e:#}")),
        }
        self.reg = Registry::load(&dirs, &self.engine);
        self.shapes.clear();
        let theme = self.theme.id.clone();
        self.set_theme(&theme);
        self.log_plugin_summary();
    }

    pub fn open_project(&mut self, root: &Path) {
        match Project::load(root) {
            Ok(project) => {
                self.log(Level::Success, format!("Opened {}", project.root.display()));
                let first = project.workspaces.keys().next().cloned();
                self.project = Some(project);
                self.dirty = false;
                self.selected = None;
                self.views.clear();
                self.reload_plugins();
                if let Some(first) = first {
                    self.actions.push(Action::OpenWorkspace(first));
                }
            }
            Err(e) => self.log(Level::Error, format!("Could not open {}: {e:#}", root.display())),
        }
    }

    pub fn save(&mut self) {
        let Some(project) = &self.project else { return };
        match project.save() {
            Ok(()) => {
                self.dirty = false;
                let msg = format!("Saved {}", project.meta.name);
                self.log(Level::Success, msg);
            }
            Err(e) => self.log(Level::Error, format!("Save failed: {e:#}")),
        }
    }

    pub fn mode(&self) -> Option<&BuildMode> {
        let id = self.project.as_ref().map(|p| p.meta.mode.as_str()).unwrap_or("discord-bot");
        self.reg.mode(id).or_else(|| self.reg.modes.first())
    }

    pub fn workspace(&self, key: &str) -> Option<&Workspace> {
        self.project.as_ref()?.workspaces.get(key)
    }

    /// Places a new block, with its default values and pins.
    pub fn add_node(&mut self, ws_key: &str, block_id: &str, at: Pos2) -> Option<String> {
        let def = self.reg.block(block_id)?.clone();
        let ws = self.project.as_mut()?.workspaces.get_mut(ws_key)?;
        let id = ws.next_node_id();
        ws.nodes.insert(
            id.clone(),
            Node { block: def.reference(), at: [at.x, at.y], values: def.default_values(), pins: def.default_pins(), zone: None },
        );
        self.dirty = true;
        self.selected = Some((ws_key.to_string(), id.clone()));
        Some(id)
    }
}

/// The effective shape of a node, re-running `define()` only when its values or
/// connected types change.
pub fn node_shape(
    reg: &Registry,
    engine: &BlockEngine,
    cache: &mut HashMap<(String, String), (u64, BlockShape)>,
    ws_key: &str,
    ws: &Workspace,
    node_id: &str,
) -> Option<BlockShape> {
    let node = ws.nodes.get(node_id)?;
    let def = reg.block(&node.block)?;
    let connected = connected_types(reg, cache, ws_key, ws, node_id);

    let mut h = DefaultHasher::new();
    node.block.hash(&mut h);
    Value::Object(node.values.clone()).to_string().hash(&mut h);
    Value::Object(connected.clone()).to_string().hash(&mut h);
    let hash = h.finish();

    let key = (ws_key.to_string(), node_id.to_string());
    if let Some((cached_hash, shape)) = cache.get(&key)
        && *cached_hash == hash
    {
        return Some(shape.clone());
    }

    let define = if def.has_define {
        let engine_key = reg.engine_keys.get(&def.id)?;
        engine.define(engine_key, &node.values, &connected).ok()
    } else {
        None
    };
    let shape = BlockShape::build(def, &node.values, define.as_ref());
    cache.insert(key, (hash, shape.clone()));
    Some(shape)
}

/// For each connected input pin of a node, the type string of the output feeding it.
fn connected_types(
    reg: &Registry,
    cache: &HashMap<(String, String), (u64, BlockShape)>,
    ws_key: &str,
    ws: &Workspace,
    node_id: &str,
) -> Map<String, Value> {
    let mut out = Map::new();
    for link in ws.links.iter().filter(|l| l.to.node == node_id && l.to.pin != ACTION_IN) {
        let Some(src) = ws.nodes.get(&link.from.node) else { continue };
        let cached = cache.get(&(ws_key.to_string(), link.from.node.clone())).and_then(|(_, s)| s.output(&link.from.pin)).map(|o| o.type_name.clone());
        let ty = cached.or_else(|| reg.block(&src.block).and_then(|d| d.outputs.get(&link.from.pin)).map(|o| o.type_name.clone()));
        if let Some(ty) = ty {
            out.insert(link.to.pin.clone(), Value::String(ty));
        }
    }
    out
}

/// An image URI for a local file: `file:///C:/…` on Windows, `file:///home/…` on Linux.
pub fn file_uri(path: &Path) -> String {
    let p = path.to_string_lossy().replace('\\', "/");
    if p.starts_with('/') { format!("file://{p}") } else { format!("file:///{p}") }
}

fn clock() -> String {
    let secs = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // Local offset is not available without extra crates; show UTC time of day.
    let s = secs % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}
