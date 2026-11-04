//! Plugins with editor code: JavaScript plugins (an `editor.js`, see
//! [`crate::script`]) and native Rust plugins (`.dll` / `.so` libraries built
//! with the `xc-circuit-plugin` crate). Both add commands, panels and checks
//! the same way.
//!
//! Scripts run sandboxed and load right away. Native plugins are off until
//! the user allows native plugins, and plugins outside the app's own folder
//! must also be allowed one by one, because native code can do anything the
//! editor can.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CStr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use circuit_core::Workspace;
use eframe::egui;
use libloading::Library;
use serde_json::Value;
use xc_circuit_plugin::{self as api, BotEvent, Command, PanelInfo, PluginObject};

use crate::editor::{Editor, Level};

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Native plugins are turned off.
    Off,
    /// Waiting for the user to allow this plugin.
    NeedsApproval,
    /// The library hasn't been built (or shipped) yet.
    NotBuilt(String),
    /// Built with a different compiler, egui or profile than the editor.
    Incompatible(String),
    Failed(String),
    Loaded,
}

/// Size and modification time of a library file, to notice rebuilds.
type Stamp = (SystemTime, u64);

struct Instance {
    obj: Box<dyn PluginObject>,
    commands: Vec<Command>,
    panels: Vec<PanelInfo>,
    library: PathBuf,
    stamp: Stamp,
    /// Bumped on every reload, so panel widgets start fresh.
    generation: u32,
}

/// How a plugin's editor code runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// JavaScript in a sandbox.
    Script,
    /// A compiled Rust library.
    Native,
}

pub struct NativePlugin {
    pub id: String,
    pub name: String,
    pub dir: PathBuf,
    pub kind: Kind,
    library_name: String,
    /// The script, relative to the plugin folder.
    script: String,
    /// The files the script read the last time it loaded, to reload it when one changes.
    watch: Vec<PathBuf>,
    /// Shipped in the app's own plugins folder.
    pub bundled: bool,
    pub status: Status,
    instance: Option<Instance>,
    problems: Vec<api::Problem>,
    /// The library that last failed to load or crashed; it isn't retried
    /// until the file changes.
    failed: Option<Stamp>,
}

impl NativePlugin {
    pub fn commands(&self) -> &[Command] {
        self.instance.as_ref().map_or(&[], |i| &i.commands)
    }

    pub fn panels(&self) -> &[PanelInfo] {
        self.instance.as_ref().map_or(&[], |i| &i.panels)
    }
}

pub struct NativeHost {
    pub allow: bool,
    /// Plugin folders the user allowed.
    pub trusted: BTreeSet<String>,
    pub plugins: Vec<NativePlugin>,
    /// Each plugin's key-value store.
    pub storage: BTreeMap<String, BTreeMap<String, String>>,
    /// Loaded libraries are never unloaded: egui may still hold data whose
    /// code lives in them.
    libs: Vec<Library>,
    copies: PathBuf,
    loads: u32,
    last_poll: Option<Instant>,
    seen_saves: u64,
    /// Commands plugins asked to run, handled by the app.
    pub queued: Vec<String>,
}

impl Drop for NativeHost {
    fn drop(&mut self) {
        for p in &mut self.plugins {
            p.instance = None;
        }
        for lib in self.libs.drain(..) {
            std::mem::forget(lib);
        }
    }
}

pub fn library_file(name: &str) -> String {
    if cfg!(windows) { format!("{name}.dll") } else { format!("lib{name}.so") }
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

/// Where a plugin's library is looked for: its own folder, its `native/`
/// folder, then next to the editor (where `cargo build` puts workspace plugins).
fn find_library(dir: &Path, name: &str) -> Option<PathBuf> {
    let file = library_file(name);
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
    [Some(dir.to_path_buf()), Some(dir.join("native")), exe_dir].into_iter().flatten().map(|d| d.join(&file)).find(|p| p.is_file())
}

fn map_level(l: api::Level) -> Level {
    match l {
        api::Level::Info => Level::Info,
        api::Level::Success => Level::Success,
        api::Level::Warn => Level::Warn,
        api::Level::Error => Level::Error,
    }
}

impl NativeHost {
    pub fn new(allow: bool, trusted: BTreeSet<String>, storage: BTreeMap<String, BTreeMap<String, String>>) -> NativeHost {
        // A per-user folder: in a shared one (/tmp on Linux) someone else could
        // swap a library between copying and loading it.
        let copies = dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("xc-circuit").join("native");
        // Copies left by earlier sessions; ones still in use by another window stay.
        if let Ok(entries) = std::fs::read_dir(&copies) {
            for e in entries.flatten() {
                let _ = std::fs::remove_file(e.path());
            }
        }
        NativeHost { allow, trusted, plugins: Vec::new(), storage, libs: Vec::new(), copies, loads: 0, last_poll: None, seen_saves: 0, queued: Vec::new() }
    }

    fn key(dir: &Path) -> String {
        dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf()).to_string_lossy().into_owned()
    }

    pub fn is_trusted(&self, p: &NativePlugin) -> bool {
        p.bundled || self.trusted.contains(&Self::key(&p.dir))
    }

    pub fn trust(&mut self, id: &str, ed: &mut Editor) {
        if let Some(p) = self.plugins.iter().find(|p| p.id == id) {
            self.trusted.insert(Self::key(&p.dir));
        }
        self.sync(ed, None);
    }

    /// Matches the loaded libraries to the registry's plugins, loading,
    /// reloading or unloading as needed. `bundled_plugins` is the app's own
    /// plugins folder.
    pub fn sync(&mut self, ed: &mut Editor, bundled_plugins: Option<&Path>) {
        let mut old: Vec<NativePlugin> = std::mem::take(&mut self.plugins);
        let found: Vec<_> = ed.reg.plugins.iter().filter(|lp| lp.manifest.native.is_some() || lp.manifest.editor.is_some()).cloned().collect();
        for lp in &found {
            let m = &lp.manifest;
            // A plugin with both runs its native part.
            let (kind, library, script) = match (&m.native, &m.editor) {
                (Some(n), _) => (Kind::Native, n.library.clone(), String::new()),
                (None, Some(e)) => (Kind::Script, String::new(), if e.script.is_empty() { "editor.js".to_string() } else { e.script.clone() }),
                (None, None) => continue,
            };
            let prev = old.iter().position(|p| p.id == m.id && p.kind == kind && p.dir == lp.dir).map(|i| old.swap_remove(i));
            let bundled = match bundled_plugins {
                Some(b) => lp.dir.starts_with(b),
                None => prev.as_ref().is_some_and(|p| p.bundled),
            };
            let mut plugin = prev.unwrap_or_else(|| NativePlugin {
                id: m.id.clone(),
                name: m.name.clone(),
                dir: lp.dir.clone(),
                kind,
                library_name: library.clone(),
                script: script.clone(),
                watch: Vec::new(),
                bundled,
                status: Status::Off,
                instance: None,
                problems: Vec::new(),
                failed: None,
            });
            plugin.bundled = bundled;
            plugin.name = m.name.clone();
            plugin.library_name = library;
            if plugin.script != script {
                plugin.script = script;
                plugin.watch.clear();
            }
            self.refresh(&mut plugin, ed);
            self.plugins.push(plugin);
        }
    }

    /// Loads the plugin if it's allowed and its library or script is new or changed.
    fn refresh(&mut self, p: &mut NativePlugin, ed: &mut Editor) {
        if p.kind == Kind::Script {
            Self::refresh_script(p, ed);
            return;
        }
        if !self.allow {
            p.instance = None;
            p.status = Status::Off;
            return;
        }
        if !self.is_trusted(p) {
            p.instance = None;
            p.status = Status::NeedsApproval;
            return;
        }
        let Some(lib) = find_library(&p.dir, &p.library_name) else {
            p.instance = None;
            p.status = Status::NotBuilt(library_file(&p.library_name));
            return;
        };
        let st = stamp(&lib);
        if p.instance.is_none() && st.is_some() && p.failed == st {
            return;
        }
        if let Some(i) = &p.instance
            && i.library == lib
            && Some(i.stamp) == st
        {
            return;
        }
        // A rebuilt library may still be being written; it is picked up on a later poll.
        let rebuilt = p.instance.is_some() || p.failed.is_some();
        if rebuilt && st.is_some_and(|(t, _)| t.elapsed().unwrap_or_default() < Duration::from_millis(600)) {
            return;
        }
        let generation = p.instance.as_ref().map_or(0, |i| i.generation + 1);
        let reload = p.instance.is_some();
        p.instance = None;
        match self.load(&lib) {
            Ok(obj) => {
                let commands = obj.commands().unwrap_or_default();
                let panels = obj.panels().unwrap_or_default();
                p.instance = Some(Instance { obj, commands, panels, library: lib.clone(), stamp: st.unwrap_or((SystemTime::UNIX_EPOCH, 0)), generation });
                p.status = Status::Loaded;
                p.failed = None;
                ed.log(Level::Info, format!("{} native plugin {}", if reload { "Reloaded" } else { "Loaded" }, p.name));
            }
            Err(Status::Incompatible(why)) => {
                p.failed = st;
                ed.log(Level::Warn, format!("{} wasn't loaded: it was built differently from XC Circuit ({why})", p.name));
                p.status = Status::Incompatible(why);
            }
            Err(status) => {
                p.failed = st;
                if let Status::Failed(e) = &status {
                    ed.log(Level::Error, format!("{} failed to load: {e}", p.name));
                }
                p.status = status;
            }
        }
    }

    /// Loads a script plugin, or loads it again when one of its files changed.
    fn refresh_script(p: &mut NativePlugin, ed: &mut Editor) {
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
        let (result, files) = crate::script::load(&p.name, &p.dir, &p.script);
        p.watch = files;
        let st = stamp_all(&p.watch);
        match result {
            Ok(obj) => {
                let obj: Box<dyn PluginObject> = Box::new(obj);
                let commands = obj.commands().unwrap_or_default();
                let panels = obj.panels().unwrap_or_default();
                p.instance = Some(Instance { obj, commands, panels, library: main, stamp: st.unwrap_or((SystemTime::UNIX_EPOCH, 0)), generation });
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

    fn load(&mut self, lib_path: &Path) -> Result<Box<dyn PluginObject>, Status> {
        // Load a copy, so the original can be rebuilt while the editor runs.
        std::fs::create_dir_all(&self.copies).map_err(|e| Status::Failed(e.to_string()))?;
        self.loads += 1;
        let file = lib_path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let copy = self.copies.join(format!("{}-{}-{file}", std::process::id(), self.loads));
        std::fs::copy(lib_path, &copy).map_err(|e| Status::Failed(format!("could not copy {}: {e}", lib_path.display())))?;

        // SAFETY: loading runs the library's initializers. This only happens
        // for plugins the user allowed.
        let lib = unsafe { Library::new(&copy) }.map_err(|e| Status::Failed(e.to_string()))?;
        let result = (|| {
            // SAFETY: the fingerprint symbol is a C function returning a static C string.
            let fp = unsafe { lib.get::<api::FingerprintFn>(api::FINGERPRINT_SYMBOL) }
                .map_err(|_| Status::Failed("not an XC Circuit plugin (xc_circuit_plugin_fingerprint is missing)".into()))?;
            let theirs = unsafe { CStr::from_ptr(fp()) }.to_string_lossy().into_owned();
            if let Some(why) = api::compare_fingerprints(&api::fingerprint(), &theirs) {
                return Err(Status::Incompatible(why));
            }
            // SAFETY: the fingerprints match, so the plugin was built with this
            // compiler and these crates and shares their types.
            let create = unsafe { lib.get::<api::CreateFn>(api::CREATE_SYMBOL) }.map_err(|e| Status::Failed(e.to_string()))?;
            create().map_err(|e| Status::Failed(format!("panicked while starting: {e}")))
        })();
        self.libs.push(lib);
        result
    }

    /// Runs `f` on a plugin with an editor-backed [`api::Host`]. A plugin that
    /// panics is switched off until it is rebuilt.
    fn call<R>(&mut self, index: usize, ed: &mut Editor, bot_running: bool, f: impl FnOnce(&mut dyn PluginObject, &mut dyn api::Host) -> Result<R, String>) -> Option<R> {
        let p = &mut self.plugins[index];
        let inst = p.instance.as_mut()?;
        let storage = self.storage.entry(p.id.clone()).or_default();
        let mut host = EditorHost { ed, storage, queued: &mut self.queued, bot_running };
        match f(inst.obj.as_mut(), &mut host) {
            Ok(r) => Some(r),
            Err(e) => {
                let until = if p.kind == Kind::Script { "its files change" } else { "it's rebuilt" };
                ed.log(Level::Error, format!("{} crashed: {e}. It stays off until {until}.", p.name));
                p.failed = Some(inst.stamp);
                p.instance = None;
                p.status = Status::Failed(format!("crashed: {e}"));
                None
            }
        }
    }

    /// Picks up rebuilt libraries, runs checks about once a second, and tells
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
            let mut p = std::mem::replace(&mut self.plugins[i], placeholder());
            self.refresh(&mut p, ed);
            self.plugins[i] = p;
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
            ui.label(format!("{} isn't running: {}.", p.name, status_text(&p.status)));
            ui.label(egui::RichText::new("See Plugins › Manage Plugins.").weak());
            return;
        };
        let id = egui::Id::new(("plugin-panel", plugin, panel, inst.generation));
        self.call(i, ed, bot_running, |o, h| ui.push_id(id, |ui| o.panel_ui(panel, ui, h)).inner);
    }

    pub fn panel_title(&self, plugin: &str, panel: &str) -> Option<String> {
        self.plugins.iter().find(|p| p.id == plugin)?.panels().iter().find(|x| x.id == panel).map(|x| x.title.clone())
    }

    pub fn problems(&self) -> Vec<crate::panels::ProblemItem> {
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

fn placeholder() -> NativePlugin {
    NativePlugin {
        id: String::new(),
        name: String::new(),
        dir: PathBuf::new(),
        kind: Kind::Native,
        library_name: String::new(),
        script: String::new(),
        watch: Vec::new(),
        bundled: false,
        status: Status::Off,
        instance: None,
        problems: Vec::new(),
        failed: None,
    }
}

pub fn status_text(s: &Status) -> String {
    match s {
        Status::Off => "native plugins are turned off".into(),
        Status::NeedsApproval => "waiting for your approval".into(),
        Status::NotBuilt(file) => format!("{file} hasn't been built"),
        Status::Incompatible(why) => format!("built differently from XC Circuit: {why}"),
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

    fn play_sound(&mut self, bytes: Vec<u8>, volume: f32) {
        crate::sound::play(bytes, volume);
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command as Process;

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
        let mut host = NativeHost::new(false, BTreeSet::new(), BTreeMap::new());
        host.sync(&mut ed, None);
        assert_eq!((host.plugins[0].kind, &host.plugins[0].status), (Kind::Script, &Status::Loaded), "{:?}", ed.log);
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

    /// Builds the example plugin, puts it in a plugin folder of its own and
    /// drives it through the editor-side host.
    #[test]
    fn loads_and_runs_the_example_plugin() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        // Built together with the editor, so both use the same build of egui.
        let built = Process::new(env!("CARGO")).args(["build", "-p", "xc-stats", "-p", "circuit-editor"]).current_dir(&root).status().unwrap();
        assert!(built.success());

        let tmp = std::env::temp_dir().join(format!("xc-native-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let plugin_dir = tmp.join("plugins").join("xc.stats");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::copy(root.join("plugins/xc.stats/plugin.toml"), plugin_dir.join("plugin.toml")).unwrap();
        let lib = library_file("xc_stats");
        std::fs::copy(root.join("target/debug").join(&lib), plugin_dir.join(&lib)).unwrap();

        let mut ed = Editor::new(vec![tmp.join("plugins")], None).unwrap();
        Project::create(tmp.join("bot"), "Test Bot", "none").unwrap();
        ed.open_project(&tmp.join("bot"));
        let ws: Workspace = serde_json::from_value(serde_json::json!({
            "name": "Main",
            "nodes": { "a": { "block": "core.on_start@1", "at": [0, 0] }, "b": { "block": "core.log@1", "at": [0, 0] }, "c": { "block": "core.math@1", "at": [0, 0] } },
            "links": ["a.next > b.in"]
        }))
        .unwrap();
        ed.project.as_mut().unwrap().workspaces.insert("main".into(), ws);

        // Off until allowed; outside the app's folder it needs approval too.
        let mut host = NativeHost::new(false, BTreeSet::new(), BTreeMap::new());
        host.sync(&mut ed, Some(&root.join("plugins")));
        assert_eq!(host.plugins[0].status, Status::Off);
        host.allow = true;
        host.sync(&mut ed, Some(&root.join("plugins")));
        assert_eq!(host.plugins[0].status, Status::NeedsApproval);
        host.trust("xc.stats", &mut ed);
        assert_eq!(host.plugins[0].status, Status::Loaded, "{:?}", ed.log);

        assert_eq!(host.plugins[0].commands()[0].id, "stats.summary");
        assert_eq!(host.plugins[0].panels()[0].title, "Project Stats");
        assert!(host.run_command(&mut ed, "stats.summary", false));
        assert!(ed.log.iter().any(|l| l.text == "Test Bot: 1 workspaces, 3 blocks, 1 wires, 0 Try Zones"), "{:?}", ed.log);

        host.tick(&mut ed, false);
        let problems = host.problems();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].1.contains("(#c) isn't connected"), "{problems:?}");
        assert_eq!(problems[0].2, Some(("main".to_string(), "c".to_string())));

        let ev = BotEvent::Block { workspace: "main".into(), node: "b".into(), phase: "exit".into(), ms: Some(2.0), error: None };
        host.bot_event(&mut ed, &ev, true);
        assert_eq!(host.storage["xc.stats"]["allTimeRuns"], "1");

        // The panel draws with egui from inside the library.
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| host.panel_ui(ui, &mut ed, "xc.stats", "stats", true));
            out.textures_delta.clear();
        }
        assert_eq!(host.plugins[0].status, Status::Loaded);
    }
}
