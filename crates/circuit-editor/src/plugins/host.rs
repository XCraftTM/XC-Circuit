//! Plugins with editor code: JavaScript plugins (an `editor.js`, see
//! [`crate::plugins::script`]) that add commands, panels and checks. They run
//! sandboxed and load right away, and load again when one of their files changes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use circuit_core::Workspace;
use eframe::egui;
use serde_json::Value;
use crate::plugins::api::{self, BotEvent, Command, PanelInfo, PluginObject};

use crate::editor::{Editor, Level};

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Failed(String),
    Loaded,
}

/// Size and modification time of a library file, to notice rebuilds.
type Stamp = (SystemTime, u64);

struct Instance {
    obj: Box<dyn PluginObject>,
    commands: Vec<Command>,
    panels: Vec<PanelInfo>,
    stamp: Stamp,
    /// Bumped on every reload, so panel widgets start fresh.
    generation: u32,
}


pub struct EditorPlugin {
    pub id: String,
    pub name: String,
    pub dir: PathBuf,
    /// The script, relative to the plugin folder.
    script: String,
    /// The files the script read the last time it loaded, to reload it when one changes.
    watch: Vec<PathBuf>,
    /// Shipped in the app's own plugins folder.
    pub bundled: bool,
    pub status: Status,
    instance: Option<Instance>,
    problems: Vec<api::Problem>,
    /// Its files when it last failed to load or crashed; it isn't tried
    /// again until one changes.
    failed: Option<Stamp>,
}

impl EditorPlugin {
    pub fn commands(&self) -> &[Command] {
        self.instance.as_ref().map_or(&[], |i| &i.commands)
    }

    pub fn panels(&self) -> &[PanelInfo] {
        self.instance.as_ref().map_or(&[], |i| &i.panels)
    }
}

pub struct PluginHost {
    pub plugins: Vec<EditorPlugin>,
    /// Each plugin's key-value store.
    pub storage: BTreeMap<String, BTreeMap<String, String>>,
    last_poll: Option<Instant>,
    seen_saves: u64,
    /// Commands plugins asked to run, handled by the app.
    pub queued: Vec<String>,
}

fn stamp(path: &Path) -> Option<Stamp> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// One stamp for several files: the latest change, and the sizes and count.
fn stamp_all(paths: &[PathBuf]) -> Option<Stamp> {
    let stamps: Vec<Stamp> = paths.iter().filter_map(|p| stamp(p)).collect();
    let latest = stamps.iter().map(|s| s.0).max()?;
    Some((latest, stamps.iter().map(|s| s.1).sum::<u64>() + ((stamps.len() as u64) << 48)))
}

fn map_level(l: api::Level) -> Level {
    match l {
        api::Level::Info => Level::Info,
        api::Level::Success => Level::Success,
        api::Level::Warn => Level::Warn,
        api::Level::Error => Level::Error,
    }
}

impl PluginHost {
    pub fn new(storage: BTreeMap<String, BTreeMap<String, String>>) -> PluginHost {
        PluginHost { plugins: Vec::new(), storage, last_poll: None, seen_saves: 0, queued: Vec::new() }
    }

    /// Matches the running scripts to the registry's plugins, loading,
    /// reloading or stopping them as needed. `bundled_plugins` is the app's
    /// own plugins folder.
    pub fn sync(&mut self, ed: &mut Editor, bundled_plugins: Option<&Path>) {
        let mut old: Vec<EditorPlugin> = std::mem::take(&mut self.plugins);
        let found: Vec<_> = ed.reg.plugins.iter().filter(|lp| lp.manifest.editor.is_some()).cloned().collect();
        for lp in &found {
            let m = &lp.manifest;
            let Some(e) = &m.editor else { continue };
            let script = if e.script.is_empty() { "editor.js".to_string() } else { e.script.clone() };
            let prev = old.iter().position(|p| p.id == m.id && p.dir == lp.dir).map(|i| old.swap_remove(i));
            let bundled = match bundled_plugins {
                Some(b) => lp.dir.starts_with(b),
                None => prev.as_ref().is_some_and(|p| p.bundled),
            };
            let mut plugin = prev.unwrap_or_else(|| EditorPlugin {
                id: m.id.clone(),
                name: m.name.clone(),
                dir: lp.dir.clone(),
                script: script.clone(),
                watch: Vec::new(),
                bundled,
                status: Status::Failed(String::new()),
                instance: None,
                problems: Vec::new(),
                failed: None,
            });
            plugin.bundled = bundled;
            plugin.name = m.name.clone();
            if plugin.script != script {
                plugin.script = script;
                plugin.watch.clear();
            }
            Self::refresh(&mut plugin, ed);
            self.plugins.push(plugin);
        }
    }

    /// Loads a script plugin, or loads it again when one of its files changed.
    fn refresh(p: &mut EditorPlugin, ed: &mut Editor) {
        let main = p.dir.join(&p.script);
        if !main.is_file() {
            p.instance = None;
            p.status = Status::Failed(format!("{} is missing", p.script));
            return;
        }
        if p.watch.is_empty() {
            p.watch = vec![main.clone()];
        }
        let st = stamp_all(&p.watch);
        if p.instance.is_none() && st.is_some() && p.failed == st {
            return;
        }
        if let Some(i) = &p.instance
            && Some(i.stamp) == st
        {
            return;
        }
        // A file being saved right now is picked up on a later poll.
        let changed = p.instance.is_some() || p.failed.is_some();
        if changed && st.is_some_and(|(t, _)| t.elapsed().unwrap_or_default() < Duration::from_millis(150)) {
            return;
        }
        let generation = p.instance.as_ref().map_or(0, |i| i.generation + 1);
        p.instance = None;
        let (result, files) = crate::plugins::script::load(&p.name, &p.dir, &p.script);
        p.watch = files;
        let st = stamp_all(&p.watch);
        match result {
            Ok(obj) => {
                let obj: Box<dyn PluginObject> = Box::new(obj);
                let commands = obj.commands().unwrap_or_default();
                let panels = obj.panels().unwrap_or_default();
                p.instance = Some(Instance { obj, commands, panels, stamp: st.unwrap_or((SystemTime::UNIX_EPOCH, 0)), generation });
                p.status = Status::Loaded;
                p.failed = None;
                if changed {
                    ed.log(Level::Info, format!("Reloaded {}", p.name));
                }
            }
            Err(e) => {
                p.failed = st;
                ed.log(Level::Error, format!("{} couldn't start: {e}", p.name));
                p.status = Status::Failed(e);
            }
        }
    }

    /// Runs `f` on a plugin with an editor-backed [`api::Host`]. A plugin that
    /// fails that way is switched off until its files change.
    fn call<R>(&mut self, index: usize, ed: &mut Editor, bot_running: bool, f: impl FnOnce(&mut dyn PluginObject, &mut dyn api::Host) -> Result<R, String>) -> Option<R> {
        let p = &mut self.plugins[index];
        let inst = p.instance.as_mut()?;
        let storage = self.storage.entry(p.id.clone()).or_default();
        let mut host = EditorHost { ed, storage, queued: &mut self.queued, bot_running };
        // Its panics are caught and reported here, not crashes of the app.
        let call = || std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(inst.obj.as_mut(), &mut host))).unwrap_or_else(|p| Err(panic_message(&*p)));
        match crate::app::crash::plugin_call(call) {
            Ok(r) => Some(r),
            Err(e) => {
                ed.log(Level::Error, format!("{} crashed: {e}. It stays off until its files change.", p.name));
                p.failed = Some(inst.stamp);
                p.instance = None;
                p.status = Status::Failed(format!("crashed: {e}"));
                None
            }
        }
    }

    /// Picks up changed scripts, runs checks about once a second, and tells
    /// plugins about saves.
    pub fn tick(&mut self, ed: &mut Editor, bot_running: bool) {
        if ed.saves != self.seen_saves {
            self.seen_saves = ed.saves;
            for i in 0..self.plugins.len() {
                self.call(i, ed, bot_running, |o, h| o.on_save(h));
            }
        }
        if self.last_poll.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
            return;
        }
        self.last_poll = Some(Instant::now());
        for i in 0..self.plugins.len() {
            Self::refresh(&mut self.plugins[i], ed);
            let problems = if ed.project.is_some() { self.call(i, ed, bot_running, |o, h| o.check(h)).unwrap_or_default() } else { Vec::new() };
            self.plugins[i].problems = problems;
        }
    }

    /// Every frame: plugins' timers and what floats over the editor. Returns
    /// when a plugin wants the next frame at the latest.
    pub fn frame(&mut self, ctx: &egui::Context, ed: &mut Editor, bot_running: bool) -> Option<Duration> {
        let mut wake: Option<Duration> = None;
        for i in 0..self.plugins.len() {
            if let Some(Some(d)) = self.call(i, ed, bot_running, |o, h| o.tick(h)) {
                wake = Some(wake.map_or(d, |w| w.min(d)));
            }
            self.call(i, ed, bot_running, |o, h| o.overlay_ui(ctx, h));
        }
        wake
    }

    pub fn bot_event(&mut self, ed: &mut Editor, event: &BotEvent, bot_running: bool) {
        for i in 0..self.plugins.len() {
            self.call(i, ed, bot_running, |o, h| o.on_bot_event(event, h));
        }
    }

    pub fn run_command(&mut self, ed: &mut Editor, id: &str, bot_running: bool) -> bool {
        let Some(i) = self.plugins.iter().position(|p| p.commands().iter().any(|c| c.id == id)) else { return false };
        self.call(i, ed, bot_running, |o, h| o.run_command(id, h));
        true
    }

    /// Draws a plugin panel tab.
    pub fn panel_ui(&mut self, ui: &mut egui::Ui, ed: &mut Editor, plugin: &str, panel: &str, bot_running: bool) {
        let Some(i) = self.plugins.iter().position(|p| p.id == plugin) else {
            ui.label(format!("The plugin {plugin} isn't installed."));
            return;
        };
        let p = &self.plugins[i];
        let Some(inst) = &p.instance else {
            ui.label(tf!("{plugin} isn't running: {status}.", plugin = p.name, status = status_text(&p.status)));
            ui.label(egui::RichText::new(t!("See Plugins › Manage Plugins.")).weak());
            return;
        };
        let id = egui::Id::unique(("plugin-panel", plugin, panel, inst.generation));
        self.call(i, ed, bot_running, |o, h| ui.push_id(id, |ui| o.panel_ui(panel, ui, h)).inner);
    }

    /// Every tool plugins offer AI agents (the first plugin's, when two use a name).
    pub fn tools(&self) -> Vec<api::Tool> {
        let mut seen = std::collections::HashSet::new();
        self.plugins.iter().filter_map(|p| p.instance.as_ref()).flat_map(|i| i.obj.tools()).filter(|t| seen.insert(t.name.clone())).collect()
    }

    /// Runs a tool for an AI agent. None: no plugin has it.
    pub fn call_tool(&mut self, ed: &mut Editor, name: &str, args: &Value, bot_running: bool) -> Option<Result<Value, String>> {
        let i = self.plugins.iter().position(|p| p.instance.as_ref().is_some_and(|i| i.obj.tools().iter().any(|t| t.name == name)))?;
        // A tool's failure is its answer to the agent; the plugin stays on.
        Some(self.call(i, ed, bot_running, |o, h| Ok(o.call_tool(name, args, h))).unwrap_or_else(|| Err("the plugin crashed".into())))
    }

    pub fn panel_title(&self, plugin: &str, panel: &str) -> Option<String> {
        self.plugins.iter().find(|p| p.id == plugin)?.panels().iter().find(|x| x.id == panel).map(|x| x.title.clone())
    }

    pub fn problems(&self) -> Vec<crate::pages::panels::ProblemItem> {
        self.plugins
            .iter()
            .flat_map(|p| {
                p.problems.iter().map(|pr| {
                    let target = pr.workspace.clone().zip(pr.node.clone());
                    (p.name.clone(), pr.message.clone(), target)
                })
            })
            .collect()
    }
}

/// What a panic said.
fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "it panicked".into())
}

pub fn status_text(s: &Status) -> String {
    match s {
        Status::Failed(e) => e.clone(),
        Status::Loaded => "running".into(),
    }
}

/// The editor as plugins see it.
struct EditorHost<'a> {
    ed: &'a mut Editor,
    storage: &'a mut BTreeMap<String, String>,
    queued: &'a mut Vec<String>,
    bot_running: bool,
}

impl api::Host for EditorHost<'_> {
    fn project_name(&self) -> Option<String> {
        self.ed.project.as_ref().map(|p| p.meta.name.clone())
    }

    fn project_root(&self) -> Option<PathBuf> {
        self.ed.project.as_ref().map(|p| p.root.clone())
    }

    fn workspaces(&self) -> Vec<api::WorkspaceInfo> {
        let Some(p) = &self.ed.project else { return Vec::new() };
        p.workspaces.iter().map(|(k, w)| api::WorkspaceInfo { key: k.clone(), name: w.name.clone() }).collect()
    }

    fn workspace_json(&self, key: &str) -> Option<String> {
        serde_json::to_string(self.ed.workspace(key)?).ok()
    }

    fn set_workspace_json(&mut self, key: &str, json: &str) -> Result<(), String> {
        let ws: Workspace = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let project = self.ed.project.as_mut().ok_or("no project is open")?;
        let slot = project.workspaces.get_mut(key).ok_or_else(|| format!("no workspace `{key}`"))?;
        if *slot != ws {
            *slot = ws;
            self.ed.edited = true;
        }
        Ok(())
    }

    fn active_workspace(&self) -> Option<String> {
        self.ed.active_workspace.clone()
    }

    fn selection(&self) -> Vec<String> {
        self.ed.active_workspace.as_ref().map(|k| self.ed.selection(k)).unwrap_or_default()
    }

    fn reveal(&mut self, workspace: &str, node: &str) {
        self.ed.reveal(workspace, node);
    }

    fn block_name(&self, block: &str) -> Option<String> {
        let id = block.split('@').next().unwrap_or(block);
        self.ed.reg.block(id).map(|d| d.name.clone())
    }

    fn project_data(&self, key: &str) -> Option<String> {
        self.ed.project.as_ref()?.meta.data.get(key).map(Value::to_string)
    }

    fn set_project_data(&mut self, key: &str, json: &str) -> Result<(), String> {
        let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let project = self.ed.project.as_mut().ok_or("no project is open")?;
        if project.meta.data.get(key) != Some(&value) {
            project.meta.data.insert(key.to_string(), value);
            self.ed.edited = true;
            // Blocks list it (Slash Command's commands): they see the new data.
            self.ed.refresh_define_ctx();
        }
        Ok(())
    }

    fn log(&mut self, level: api::Level, text: &str) {
        self.ed.log(map_level(level), text);
    }

    fn run_command(&mut self, id: &str) {
        self.queued.push(id.to_string());
    }

    fn bot_running(&self) -> bool {
        self.bot_running
    }

    fn storage_get(&self, key: &str) -> Option<String> {
        self.storage.get(key).cloned()
    }

    fn storage_set(&mut self, key: &str, value: &str) {
        self.storage.insert(key.to_string(), value.to_string());
    }

    fn add_block(&mut self, workspace: &str, block: &str, at: [f32; 2], values_json: &str) -> Result<String, String> {
        let id = block.split('@').next().unwrap_or(block);
        if self.ed.reg.block(id).is_none() {
            return Err(format!("no block `{id}` is installed"));
        }
        if self.ed.workspace(workspace).is_none() {
            return Err(format!("no workspace `{workspace}`"));
        }
        let values: Value = serde_json::from_str(values_json).map_err(|e| e.to_string())?;
        let node = self.ed.add_node(workspace, id, egui::pos2(at[0], at[1])).ok_or("the block couldn't be placed")?;
        if let (Some(Value::Object(values)), Some(n)) = (Some(values), self.ed.project.as_mut().and_then(|p| p.workspaces.get_mut(workspace)).and_then(|w| w.nodes.get_mut(&node))) {
            n.values.extend(values);
        }
        Ok(node)
    }

    fn select(&mut self, workspace: &str, nodes: &[String]) {
        let Some(ws) = self.ed.workspace(workspace) else { return };
        let ids: Vec<String> = nodes.iter().filter(|n| ws.nodes.contains_key(*n)).cloned().collect();
        self.ed.selected = ids.first().map(|n| (workspace.to_string(), n.clone()));
        self.ed.multi = ids.into_iter().collect();
    }

    fn block_types(&self, query: &str) -> Value {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut found: Vec<&circuit_core::BlockDef> = self
            .ed
            .reg
            .blocks
            .values()
            .filter(|d| d.deprecated.is_none())
            .filter(|d| {
                let hay = format!("{} {} {} {} {} {}", d.id, d.name, d.aliases.join(" "), d.tags.join(" "), d.category.as_deref().unwrap_or(""), d.description).to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .collect();
        found.sort_by(|a, b| a.id.cmp(&b.id));
        Value::Array(
            found
                .into_iter()
                .map(|d| serde_json::json!({ "type": d.id, "name": d.name, "category": d.category, "kind": kind_name(d), "description": d.description }))
                .collect(),
        )
    }

    fn block_type(&self, block: &str) -> Option<Value> {
        let d = self.ed.reg.block(block.split('@').next().unwrap_or(block))?;
        let fields: Vec<Value> = d
            .fields
            .iter()
            .map(|(k, f)| {
                let mut v = serde_json::to_value(f).unwrap_or(Value::Null);
                if let Some(o) = v.as_object_mut() {
                    o.insert("key".into(), Value::String(k.clone()));
                    o.insert("takesWire".into(), Value::Bool(f.pin != circuit_core::block::PinMode::Inline));
                    if let Some(format) = value_format(&f.type_name) {
                        o.insert("format".into(), Value::String(format.into()));
                    }
                }
                v
            })
            .collect();
        let outputs: Vec<Value> = d.outputs.iter().map(|(k, o)| serde_json::json!({ "key": k, "type": o.type_name, "label": o.label, "description": o.description })).collect();
        Some(serde_json::json!({
            "type": d.id,
            "block": format!("{}@{}", d.id, d.major_version()),
            "name": d.name,
            "kind": kind_name(d),
            "category": d.category,
            "description": d.description,
            "docs": d.docs,
            "flows": d.base_flows(),
            "fields": fields,
            "outputs": outputs,
            "hasDefine": d.has_define,
        }))
    }

    fn block_pins(&mut self, workspace: &str, node: &str) -> Result<Value, String> {
        let shape = self.shape(workspace, node)?;
        let ws = self.ed.workspace(workspace).ok_or_else(|| format!("no workspace `{workspace}`"))?;
        let n = &ws.nodes[node];
        let wired = |pin: &str, into: bool| -> Vec<String> {
            ws.links.iter().filter(|l| if into { l.to.node == node && l.to.pin == pin } else { l.from.node == node && l.from.pin == pin }).map(|l| l.to_string()).collect()
        };
        let fields: Vec<Value> = shape
            .visible_fields()
            .map(|(k, f)| {
                let value = n.values.get(k).cloned().or_else(|| f.default.clone()).unwrap_or(Value::Null);
                serde_json::json!({
                    "key": k,
                    "label": f.label(k),
                    "type": f.ty.to_string(),
                    "value": value,
                    "required": f.required,
                    "takesWire": f.pin != circuit_core::block::PinMode::Inline,
                    "isPin": f.pin == circuit_core::block::PinMode::Only || n.pins.contains(k),
                    "advanced": f.advanced,
                    "choices": f.choices,
                    "description": f.description,
                    "format": value_format(&f.type_name),
                    "wires": wired(k, true),
                })
            })
            .collect();
        let outputs: Vec<Value> = shape.outputs.iter().map(|(k, o)| serde_json::json!({ "key": k, "label": o.label(k), "type": o.ty.to_string(), "description": o.description, "wires": wired(k, false) })).collect();
        let flows: Vec<Value> = shape.flows.iter().map(|f| serde_json::json!({ "key": f, "wires": wired(f, false) })).collect();
        Ok(serde_json::json!({
            "id": node,
            "block": n.block,
            "name": self.block_name(&n.block),
            "at": n.at,
            "actionIn": shape.action_in.then(|| wired(circuit_core::graph::ACTION_IN, true)),
            "flows": flows,
            "fields": fields,
            "outputs": outputs,
        }))
    }

    fn connect(&mut self, workspace: &str, from: &str, to: &str) -> Result<(), String> {
        let link = circuit_core::graph::Link { from: pin_ref(from)?, to: pin_ref(to)? };
        if link.from.node == link.to.node {
            return Err("a block can't be wired to itself".into());
        }
        let from_shape = self.shape(workspace, &link.from.node)?;
        let to_shape = self.shape(workspace, &link.to.node)?;
        use circuit_core::types::PinType;
        let from_ty = if from_shape.flows.contains(&link.from.pin) {
            PinType::Action
        } else if let Some(o) = from_shape.output(&link.from.pin) {
            o.ty.clone()
        } else {
            let mut have: Vec<&str> = from_shape.flows.iter().map(String::as_str).collect();
            have.extend(from_shape.outputs.iter().map(|(k, _)| k.as_str()));
            return Err(format!("block {} has no output `{}` (it has: {})", link.from.node, link.from.pin, have.join(", ")));
        };
        let (to_ty, multi) = if link.to.pin == circuit_core::graph::ACTION_IN {
            if !to_shape.action_in {
                return Err(format!("block {} is an event: nothing runs it", link.to.node));
            }
            (PinType::Action, true)
        } else {
            match to_shape.field(&link.to.pin) {
                Some(f) if f.pin == circuit_core::block::PinMode::Inline => return Err(format!("the field `{}` of block {} can't take a wire: set its value instead", link.to.pin, link.to.node)),
                Some(f) => (f.ty.clone(), f.multi),
                None => {
                    let mut have = vec![circuit_core::graph::ACTION_IN.to_string()];
                    have.extend(to_shape.visible_fields().filter(|(_, f)| f.pin != circuit_core::block::PinMode::Inline).map(|(k, _)| k.clone()));
                    return Err(format!("block {} has no input `{}` (it has: {})", link.to.node, link.to.pin, have.join(", ")));
                }
            }
        };
        if from_ty.is_action() != to_ty.is_action() {
            return Err("an Action (flow) pin connects only to another block's `in`, and values only to fields".into());
        }
        if from_ty.compat(&to_ty, &self.ed.reg) == circuit_core::types::Compat::No {
            return Err(format!("a {from_ty} doesn't fit into a {to_ty}"));
        }
        let reg = &self.ed.reg;
        let project = self.ed.project.as_mut().ok_or("no project is open")?;
        let ws = project.workspaces.get_mut(workspace).ok_or_else(|| format!("no workspace `{workspace}`"))?;
        let value_block = |id: &str| ws.nodes.get(id).and_then(|n| reg.block(&n.block)).is_some_and(|d| d.kind == circuit_core::block::BlockKind::Value);
        if ws.flow(value_block).would_loop(&link) {
            return Err("that wire would make a loop".into());
        }
        let action = from_ty.is_action();
        let (to_node, to_pin) = (link.to.node.clone(), link.to.pin.clone());
        ws.connect(link, !action && !multi);
        if !action
            && let Some(n) = ws.nodes.get_mut(&to_node)
            && !n.pins.contains(&to_pin)
        {
            n.pins.push(to_pin);
        }
        self.ed.edited = true;
        Ok(())
    }

    fn add_workspace(&mut self, name: &str, folder: Option<&str>) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("a workspace needs a name".into());
        }
        let project = self.ed.project.as_mut().ok_or("no project is open")?;
        let key = project.new_workspace_key(name);
        // A folder in a folder is a path: "Tickets/Archive" (spaces around a / don't count).
        let folder = folder.map(|f| f.split(crate::editor::folders::SEP).map(str::trim).filter(|n| !n.is_empty()).collect::<Vec<_>>().join("/")).filter(|f| !f.is_empty());
        if let Some(f) = &folder
            && !project.meta.folders.contains(f)
        {
            project.meta.folders.push(f.clone());
        }
        project.workspaces.insert(key.clone(), Workspace { folder, ..Workspace::new(name) });
        self.ed.edited = true;
        self.ed.actions.push(crate::editor::Action::OpenWorkspace(key.clone()));
        Ok(key)
    }

    fn problems(&mut self) -> Value {
        let items = crate::pages::panels::collect_problems(self.ed);
        Value::Array(
            items
                .into_iter()
                .map(|(source, message, at)| serde_json::json!({ "source": source, "message": message, "workspace": at.as_ref().map(|a| &a.0), "block": at.as_ref().map(|a| &a.1) }))
                .collect(),
        )
    }

    fn console(&self, lines: usize) -> Value {
        let log = &self.ed.log;
        Value::Array(
            log[log.len().saturating_sub(lines)..]
                .iter()
                .map(|l| {
                    let level = match l.level {
                        Level::Info => "info",
                        Level::Success => "success",
                        Level::Warn => "warn",
                        Level::Error => "error",
                        Level::Mcp => "mcp",
                    };
                    serde_json::json!({ "time": l.time, "level": level, "text": l.text, "workspace": l.target.as_ref().map(|t| &t.0), "block": l.target.as_ref().map(|t| &t.1) })
                })
                .collect(),
        )
    }

    fn translate(&self, text: &str) -> String {
        crate::i18n::tr(text).into_owned()
    }

    fn copy_text(&mut self, text: &str) {
        self.ed.copy_out = Some(text.to_string());
    }

    fn block_rects(&self, workspace: &str) -> Value {
        let Some(ws) = self.ed.workspace(workspace) else { return serde_json::json!({}) };
        let rects = self.ed.views.get(workspace).map(|v| v.block_rects()).unwrap_or_default();
        Value::Object(
            rects
                .into_iter()
                .filter(|(id, _)| ws.nodes.contains_key(id))
                .map(|(id, r)| (id, serde_json::json!([r.min.x.round(), r.min.y.round(), r.width().round(), r.height().round()])))
                .collect(),
        )
    }

    fn field_value(&self, block: &str, field: &str, value: &Value) -> Result<Value, String> {
        let def = self.ed.reg.block(block.split('@').next().unwrap_or(block)).ok_or_else(|| format!("no block `{block}` is installed"))?;
        let Some(f) = def.fields.get(field) else { return Ok(value.clone()) };
        if value.is_null() {
            return Ok(Value::Null);
        }
        Ok(match f.type_name.as_str() {
            "embed" => {
                let discord = value.get("embeds").is_some()
                    || value["color"].is_number()
                    || value["image"].is_object()
                    || value["thumbnail"].is_object()
                    || value["author"].get("icon_url").is_some()
                    || value["footer"].get("icon_url").is_some()
                    || value["timestamp"].is_string();
                let data = if discord { crate::builders::embed::EmbedData::from_discord(value).map_err(|e| format!("not an embed: {e}"))? } else { crate::builders::embed::EmbedData::from_value(value) };
                data.to_value()
            }
            "layout" => crate::builders::layout::from_json(value).map_err(|e| format!("not a layout: {e}"))?,
            "modal" => crate::builders::modal::from_json(value).map_err(|e| format!("not a form: {e}"))?,
            "canvas" => serde_json::from_value::<crate::builders::canvas::CanvasData>(value.clone()).map_err(|e| format!("not a canvas: {e}"))?.to_value(),
            _ => value.clone(),
        })
    }

    fn open_builder(&mut self, workspace: &str, node: &str, field: &str) -> Result<String, String> {
        let n = self.ed.workspace(workspace).ok_or_else(|| format!("no workspace `{workspace}`"))?.nodes.get(node).cloned().ok_or_else(|| format!("workspace `{workspace}` has no block `{node}`"))?;
        let def = self.ed.reg.block(&n.block).ok_or_else(|| format!("block {node} is a `{}`, which isn't installed", n.block))?;
        // The field asked for, or the block's first one with a builder.
        let builder_of = |t: &str| matches!(t, "embed" | "layout" | "modal" | "canvas" | "slash-command-ref");
        let key = if field.is_empty() { def.fields.iter().find(|(_, f)| builder_of(&f.type_name)).map(|(k, _)| k.clone()) } else { Some(field.to_string()) };
        let key = key.ok_or_else(|| format!("block {node} ({}) has nothing a builder edits", def.name))?;
        let f = def.fields.get(&key).ok_or_else(|| format!("block {node} has no field `{key}`"))?;
        Ok(match f.type_name.as_str() {
            "embed" => {
                crate::builders::embed::open(self.ed, workspace, node, &key);
                "Embed Builder"
            }
            "layout" => {
                crate::builders::layout::open(self.ed, workspace, node, &key);
                "Layout Builder"
            }
            "modal" => {
                crate::builders::modal::open(self.ed, workspace, node, &key);
                "Modal Builder"
            }
            "canvas" => {
                crate::builders::canvas::open(self.ed, workspace, node, &key);
                "Canvas Builder"
            }
            "slash-command-ref" => {
                crate::builders::slash::open(self.ed);
                "Slash Commands"
            }
            other => return Err(format!("the field `{key}` is a {other}: no builder edits it")),
        }
        .to_string())
    }

    fn agent_clients(&self) -> Value {
        crate::mcp::install::clients()
    }

    fn agent_install(&mut self, client: &str, remove: bool) -> Result<String, String> {
        let done = if remove { crate::mcp::install::remove(client) } else { crate::mcp::install::install(client) };
        match &done {
            Ok(what) => self.ed.log(Level::Success, what.clone()),
            Err(e) => self.ed.log(Level::Warn, e.clone()),
        }
        done
    }

    fn agent_snippet(&self) -> String {
        crate::mcp::install::snippet()
    }

    fn agent_link(&self) -> Value {
        self.ed.agent_link.clone()
    }
}

impl EditorHost<'_> {
    /// A block's shape as it is now (a block's `define` can change it).
    fn shape(&mut self, workspace: &str, node: &str) -> Result<std::sync::Arc<circuit_core::block::BlockShape>, String> {
        let Editor { reg, engine, shapes, project, define_ctx, .. } = &mut *self.ed;
        let ws = project.as_ref().ok_or("no project is open")?.workspaces.get(workspace).ok_or_else(|| format!("no workspace `{workspace}`"))?;
        let n = ws.nodes.get(node).ok_or_else(|| format!("workspace `{workspace}` has no block `{node}`"))?;
        shapes.begin();
        crate::editor::node_shape(reg, engine, shapes, workspace, ws, node, define_ctx).ok_or_else(|| format!("block {node} is a `{}`, which isn't installed", n.block))
    }
}

/// `3.next` as a pin of a block.
fn pin_ref(text: &str) -> Result<circuit_core::graph::PinRef, String> {
    match text.trim().split_once('.') {
        Some((node, pin)) if !node.is_empty() && !pin.is_empty() => Ok(circuit_core::graph::PinRef::new(node, pin)),
        _ => Err(format!("`{text}` isn't a pin: write it as block.pin, such as 3.next or 4.in")),
    }
}

/// How to write the value of a field a builder edits, for AI agents.
fn value_format(type_name: &str) -> Option<&'static str> {
    Some(match type_name {
        "embed" => "An object: { color: \"#RRGGBB\", title, url, description, author: { name, icon, url }, fields: [{ name, value, inline }], image (a URL), thumbnail (a URL), footer: { text, icon }, timestamp: true for when it's sent }. Discord's own embed JSON (or a message with embeds) works too. Texts can hold ${…}.",
        "layout" => "Discord's Components V2 JSON: { components: [ … ] }, such as a container { type: 17, accent_color, components: [ { type: 10, content: \"## Title\" }, { type: 14 }, { type: 1, components: [ { type: 2, style: 1, label, custom_id } ] } ] }. Text 10, section 9 (with an accessory: thumbnail 11 or button 2), media gallery 12, file 13, separator 14, row 1. A whole message or a list of components works too. Texts can hold ${…}.",
        "modal" => "Discord's modal JSON: { title (up to 45), custom_id (what Modal Submitted matches), components: [ … ] }, up to 5 components: questions { type: 18, label, description, component } around a text input { type: 4, custom_id (the answer's id), style: 1 one line or 2 several, placeholder, value, min_length, max_length, required }, a select menu { type: 3, custom_id, options: [{ label, value }] } (or 5 users, 6 roles, 7 both, 8 channels) or a file upload { type: 19, custom_id, min_values, max_values }; and texts { type: 10, content }. Texts can hold ${…}.",
        "canvas" => "A picture: { width, height, background: \"#RRGGBB\", layers: [ … ] }, bottom layer first. Layers: { type: \"rect\" | \"circle\" | \"image\" | \"text\" | \"bar\", x, y, w, h, color, color2 and angle (a gradient), radius, opacity (0-100), outline, outline_width; image: src (a URL or ${…}), shape (rect, rounded, circle), fit (cover, contain, stretch); text: text, font (regular, bold, mono), size, align, shadow, shrink; bar: value, max, back }. The field's default is a full example.",
        _ => return None,
    })
}

fn kind_name(d: &circuit_core::BlockDef) -> String {
    format!("{:?}", d.kind).to_lowercase()
}

#[cfg(test)]
mod tests {
    use circuit_core::Project;

    use super::*;

    /// Script plugins load without approval and reload when their file changes.
    #[test]
    fn runs_script_plugins_and_reloads_them() {
        let tmp = std::env::temp_dir().join(format!("xc-script-host-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let dir = tmp.join("plugins").join("t.script");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plugin.toml"), "id = \"t.script\"\nname = \"Script Test\"\nversion = \"1.0.0\"\n\n[editor]\nscript = \"editor.js\"\n").unwrap();
        let script = |v: &str| format!("xc.command({{ id: 't.say', title: 'Say', run() {{ xc.log('{v}', xc.project().name) }} }});\nxc.panel({{ id: 'p', title: 'Panel {v}', render(ui) {{ ui.text('{v}') }} }});");
        std::fs::write(dir.join("editor.js"), script("v1")).unwrap();

        let mut ed = Editor::new(vec![tmp.join("plugins")], None).unwrap();
        Project::create(tmp.join("bot"), "Test Bot", "none").unwrap();
        ed.open_project(&tmp.join("bot"));
        let mut host = PluginHost::new(BTreeMap::new());
        host.sync(&mut ed, None);
        assert_eq!(host.plugins[0].status, Status::Loaded, "{:?}", ed.log);
        assert_eq!(host.panel_title("t.script", "p").as_deref(), Some("Panel v1"));
        assert!(host.run_command(&mut ed, "t.say", false));
        assert_eq!(ed.log.last().unwrap().text, "v1 Test Bot");

        // Saved again: loaded again, with the new commands and panels.
        std::thread::sleep(Duration::from_millis(50));
        std::fs::write(dir.join("editor.js"), script("v2")).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        host.sync(&mut ed, None);
        assert!(ed.log.iter().any(|l| l.text == "Reloaded Script Test"), "{:?}", ed.log);
        assert_eq!(host.panel_title("t.script", "p").as_deref(), Some("Panel v2"));
        host.run_command(&mut ed, "t.say", false);
        assert_eq!(ed.log.last().unwrap().text, "v2 Test Bot");

        // A broken file: the error is said once, and the plugin waits for the next save.
        std::fs::write(dir.join("editor.js"), "xc.command(").unwrap();
        std::thread::sleep(Duration::from_millis(200));
        host.sync(&mut ed, None);
        assert!(matches!(&host.plugins[0].status, Status::Failed(e) if e.contains("at editor.js:2") && !e.contains("eval_script")), "{:?}", host.plugins[0].status);
        let errors = ed.log.len();
        host.sync(&mut ed, None);
        assert_eq!(ed.log.len(), errors);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Slash commands an agent saves reach the blocks at once (Slash Command lists them).
    #[test]
    fn saved_project_data_reaches_the_blocks() {
        let tmp = std::env::temp_dir().join(format!("xc-agent-data-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let plugins = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let mut ed = Editor::new(vec![plugins], None).unwrap();
        Project::create(tmp.join("bot"), "Data Bot", "discord-bot").unwrap();
        ed.open_project(&tmp.join("bot"));
        let mut host = PluginHost::new(BTreeMap::new());
        host.sync(&mut ed, None);
        let commands = serde_json::json!({ "commands": [{ "name": "ticket-panel", "description": "Posts the panel", "contexts": ["guild"] }] });
        host.call_tool(&mut ed, "set_slash_commands", &commands, false).expect("a tool").unwrap();
        assert_eq!(ed.define_ctx["data"]["slashCommands"][0]["name"], "ticket-panel", "blocks see the new commands: {}", ed.define_ctx);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// The AI agents plugin's tools, on a real project with the bundled blocks.
    #[test]
    fn agents_build_a_workspace_with_tools() {
        let tmp = std::env::temp_dir().join(format!("xc-agent-tools-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let plugins = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let mut ed = Editor::new(vec![plugins], None).unwrap();
        Project::create(tmp.join("bot"), "Agent Bot", "discord-bot").unwrap();
        ed.open_project(&tmp.join("bot"));
        let mut host = PluginHost::new(BTreeMap::new());
        host.sync(&mut ed, None);
        let names: Vec<String> = host.tools().into_iter().map(|t| t.name).collect();
        for want in ["get_project", "get_workspace", "find_block_types", "add_block", "connect", "run_bot"] {
            assert!(names.iter().any(|n| n == want), "{want} in {names:?} · {:?}", ed.log);
        }
        let mut call = |name: &str, args: Value| host.call_tool(&mut ed, name, &args, false).expect("a tool");

        let project = call("get_project", serde_json::json!({})).unwrap();
        assert_eq!(project["name"], "Agent Bot");
        let ws = project["workspaces"][0]["key"].as_str().unwrap().to_string();
        let found = call("find_block_types", serde_json::json!({ "query": "log" })).unwrap();
        assert!(found["blockTypes"].as_array().unwrap().iter().any(|b| b["type"] == "core.log"), "{found}");
        let info = call("get_block_type", serde_json::json!({ "type": "core.log" })).unwrap();
        assert!(info["fields"].as_array().unwrap().iter().any(|f| f["key"] == "message"), "{info}");

        let event = call("add_block", serde_json::json!({ "workspace": ws, "type": "discord.on_message" })).unwrap();
        let event_id = event["id"].as_str().unwrap().to_string();
        let log = call("add_block", serde_json::json!({ "workspace": ws, "type": "core.log", "after": event_id, "values": { "message": "Hi ${user}" } })).unwrap();
        let log_id = log["id"].as_str().unwrap().to_string();
        assert!(log["at"][0].as_f64().unwrap() > event["at"][0].as_f64().unwrap(), "placed right of the block it follows");
        call("connect", serde_json::json!({ "workspace": ws, "from": format!("{event_id}.next"), "to": format!("{log_id}.in") })).unwrap();
        let wrong = call("connect", serde_json::json!({ "workspace": ws, "from": format!("{event_id}.nope"), "to": format!("{log_id}.in") })).unwrap_err();
        assert!(wrong.contains("has no output `nope`"), "{wrong}");
        assert!(!wrong.contains("editor.js"), "the agent gets the message without the plugin's stack: {wrong}");
        let backwards = call("connect", serde_json::json!({ "workspace": ws, "from": format!("{event_id}.next"), "to": format!("{log_id}.message") })).unwrap_err();
        assert!(backwards.contains("Action"), "{backwards}");

        let seen = call("get_workspace", serde_json::json!({ "workspace": ws })).unwrap();
        assert_eq!(seen["wires"], serde_json::json!([format!("{event_id}.next > {log_id}.in")]));
        assert_eq!(seen["blocks"][1]["values"]["message"], "Hi ${user}");
        call("set_block_values", serde_json::json!({ "workspace": ws, "block": log_id, "values": { "message": "Bye" } })).unwrap();
        let bad = call("set_block_values", serde_json::json!({ "workspace": ws, "block": log_id, "values": { "nope": 1 } })).unwrap_err();
        assert!(bad.contains("no field \"nope\""), "{bad}");
        call("disconnect", serde_json::json!({ "workspace": ws, "from": format!("{event_id}.next"), "to": format!("{log_id}.in") })).unwrap();
        call("delete_blocks", serde_json::json!({ "workspace": ws, "blocks": [log_id] })).unwrap();
        let after = call("get_workspace", serde_json::json!({ "workspace": ws })).unwrap();
        assert_eq!(after["blocks"].as_array().unwrap().len(), 1);
        assert!(ed.edited, "the changes are edits the editor saves and can undo");
        let made = host.call_tool(&mut ed, "create_workspace", &serde_json::json!({ "name": "Moderation", "folder": "Events" }), false).unwrap().unwrap();
        let moderation = made["key"].as_str().unwrap().to_string();
        assert!(ed.workspace(&moderation).is_some());
        // A folder in a folder, as a path.
        let nested = host.call_tool(&mut ed, "create_workspace", &serde_json::json!({ "name": "Archive", "folder": "Events / Old " }), false).unwrap().unwrap();
        assert_eq!(ed.workspace(nested["key"].as_str().unwrap()).unwrap().folder.as_deref(), Some("Events/Old"));

        // Many things in one call: a flow built and wired by a script.
        let code = format!(
            r#"const ws = "{moderation}";
            const a = tools.add_block({{ workspace: ws, type: "discord.on_message" }});
            const b = tools.add_block({{ workspace: ws, type: "core.log", after: a.id, values: {{ message: "seen" }} }});
            print("made", a.id, b.id);
            tools.connect({{ workspace: ws, from: a.id + ".next", to: b.id + ".in" }});
            return xc.workspace(ws).links;"#
        );
        let ran = host.call_tool(&mut ed, "run_script", &serde_json::json!({ "code": code }), false).unwrap().unwrap();
        assert_eq!(ran["result"].as_array().map(Vec::len), Some(1), "{ran}");
        assert!(ran["output"][0].as_str().unwrap().starts_with("made "), "{ran}");
        let stopped = host.call_tool(&mut ed, "run_script", &serde_json::json!({ "code": "print('half'); tools.connect({ workspace: 'nope', from: '1.next', to: '2.in' });" }), false).unwrap().unwrap_err();
        assert!(stopped.contains("The script stopped: No workspace \"nope\"") && stopped.contains("half"), "{stopped}");
        let broken = host.call_tool(&mut ed, "run_script", &serde_json::json!({ "code": "return (" }), false).unwrap().unwrap_err();
        assert!(broken.contains("doesn't parse"), "{broken}");

        let mut call = |name: &str, args: Value| host.call_tool(&mut ed, name, &args, false).expect("a tool");
        // A note around blocks, changed, then gone.
        let seen = call("get_workspace", serde_json::json!({ "workspace": moderation })).unwrap();
        let ids: Vec<Value> = seen["blocks"].as_array().unwrap().iter().map(|b| b["id"].clone()).collect();
        let note = call("add_note", serde_json::json!({ "workspace": moderation, "title": "Logs messages", "text": "Every message", "around": ids })).unwrap();
        assert_eq!(note["id"], "n1");
        let x0 = seen["blocks"][0]["at"][0].as_f64().unwrap();
        assert!(note["rect"][0].as_f64().unwrap() < x0 && note["rect"][2].as_f64().unwrap() > 400.0, "it frames both blocks: {note}");
        call("set_note", serde_json::json!({ "workspace": moderation, "note": "n1", "title": "Message log", "color": "#3DFF6E" })).unwrap();
        let notes = call("get_workspace", serde_json::json!({ "workspace": moderation })).unwrap()["notes"].clone();
        assert_eq!((notes[0]["title"].as_str(), notes[0]["color"].as_str(), notes[0]["text"].as_str()), (Some("Message log"), Some("#3DFF6E"), Some("Every message")));
        call("delete_note", serde_json::json!({ "workspace": moderation, "note": "n1" })).unwrap();
        assert_eq!(call("get_workspace", serde_json::json!({ "workspace": moderation })).unwrap()["notes"], serde_json::json!([]));

        // Slash commands follow Discord's naming rules; a space starts a subcommand.
        let cmd = |name: &str| serde_json::json!({ "name": name, "description": "d", "options": [{ "name": "why", "type": "string", "description": "Why" }] });
        call("set_slash_commands", serde_json::json!({ "commands": [cmd("ping"), cmd("ticket open"), cmd("mod user ban-hammer_2")] })).unwrap();
        for bad in ["Ping", "two  spaces", " ping", "a b c d", "hi!", &"x".repeat(33)] {
            let err = call("set_slash_commands", serde_json::json!({ "commands": [cmd(bad)] })).unwrap_err();
            assert!(err.starts_with("Not saved: "), "{bad:?}: {err}");
        }
        let bad_option = call("set_slash_commands", serde_json::json!({ "commands": [{ "name": "ping", "description": "d", "options": [{ "name": "the reason" }] }] })).unwrap_err();
        assert!(bad_option.contains("option \"the reason\""), "{bad_option}");
        let kept = call("get_slash_commands", serde_json::json!({})).unwrap();
        assert_eq!(kept.to_string().matches("\"name\":\"ticket open\"").count(), 1, "a rejected list leaves the saved one: {kept}");

        // Builders: an embed given as Discord's JSON is kept the builder's way, and its builder opens.
        let info = call("get_block_type", serde_json::json!({ "type": "discord.build_embed" })).unwrap();
        assert!(info["fields"].as_array().unwrap().iter().any(|f| f["format"].as_str().is_some_and(|t| t.contains("Discord's own embed JSON"))), "{info}");
        let embed = call("add_block", serde_json::json!({ "workspace": moderation, "type": "discord.build_embed", "values": { "embed": { "title": "Hi", "color": 0x00D8F0, "footer": { "text": "Bot", "icon_url": "https://x.example/i.png" } } } })).unwrap();
        let stored = embed["fields"].as_array().unwrap().iter().find(|f| f["key"] == "embed").unwrap()["value"].clone();
        assert_eq!((stored["color"].as_str(), stored["footer"]["icon"].as_str()), (Some("#00D8F0"), Some("https://x.example/i.png")), "{stored}");
        let bad_layout = call("add_block", serde_json::json!({ "workspace": moderation, "type": "discord.build_layout", "values": { "layout": { "content": "hi" } } })).unwrap_err();
        assert!(bad_layout.contains("not a layout"), "{bad_layout}");
        // A form, as Discord's modal JSON; text inputs in rows become questions.
        let form = serde_json::json!({ "title": "Apply", "custom_id": "apply", "components": [{ "type": 1, "components": [{ "type": 4, "custom_id": "why", "label": "Why?", "style": 2 }] }] });
        let modal = call("add_block", serde_json::json!({ "workspace": moderation, "type": "discord.show_modal", "values": { "form": form } })).unwrap();
        let stored = modal["fields"].as_array().unwrap().iter().find(|f| f["key"] == "form").unwrap()["value"].clone();
        assert_eq!(stored["components"][0], serde_json::json!({ "type": 18, "label": "Why?", "component": { "type": 4, "custom_id": "why", "style": 2 } }), "{stored}");
        let bad_form = call("add_block", serde_json::json!({ "workspace": moderation, "type": "discord.show_modal", "values": { "form": "hi" } })).unwrap_err();
        assert!(bad_form.contains("not a form"), "{bad_form}");
        let opened = call("open_builder", serde_json::json!({ "workspace": moderation, "block": modal["id"] })).unwrap();
        assert!(opened.to_string().contains("Modal Builder"), "{opened}");
        // Placing: a second block after the same one goes below the first, not on it.
        let start = call("add_block", serde_json::json!({ "workspace": moderation, "type": "core.on_start", "x": 40, "y": 2000 })).unwrap();
        let first = call("add_block", serde_json::json!({ "workspace": moderation, "type": "core.log", "after": start["id"] })).unwrap();
        let second = call("add_block", serde_json::json!({ "workspace": moderation, "type": "core.log", "after": start["id"] })).unwrap();
        assert_eq!(first["at"][0], second["at"][0], "{first} {second}");
        assert!(second["at"][1].as_f64().unwrap() >= first["at"][1].as_f64().unwrap() + 100.0, "below the first: {first} {second}");
        assert!(first.get("warning").is_none());
        let on_top = call("add_block", serde_json::json!({ "workspace": moderation, "type": "core.log", "x": first["at"][0], "y": first["at"][1] })).unwrap();
        assert!(on_top["warning"].as_str().is_some_and(|w| w.contains(first["id"].as_str().unwrap())), "{on_top}");
        let opened = call("open_builder", serde_json::json!({ "workspace": moderation, "block": embed["id"] })).unwrap();
        assert_eq!(opened, "Opened the Embed Builder.");
        assert!(ed.embed_edit.is_some(), "the Embed Builder is open on it");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
