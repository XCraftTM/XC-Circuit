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
}
