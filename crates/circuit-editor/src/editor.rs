//! Editor state shared by every panel: plugins, the open project, cached
//! block shapes, selection and the log.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use std::collections::HashSet;

use circuit_core::{BlockShape, CLIP_FORMAT, Clip, Link, Node, Project, Workspace};

use crate::history::Histories;
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

/// The mode of a project with only the core blocks: no Discord, no npm packages.
pub const NO_MODE: &str = "none";

pub struct Editor {
    /// Block names and descriptions in English, to translate from.
    pub reg: Registry,
    pub engine: BlockEngine,
    pub plugin_dirs: Vec<PathBuf>,
    pub theme: Theme,
    pub project: Option<Project>,
    /// Set by every edit, so the history looks for changes on the next
    /// settled frame. Whether there are unsaved changes is `unsaved()`.
    pub edited: bool,
    /// Undo and Redo act on the project's settings and data (slash commands,
    /// bot settings) instead of a workspace: while their tab has focus.
    pub undo_project: bool,
    /// Counts plugin reloads, so others can tell the blocks changed.
    pub reloads: u64,
    /// Selected node as (workspace key, node id).
    pub selected: Option<(String, String)>,
    pub log: Vec<LogLine>,
    pub shapes: ShapeCache,
    pub views: HashMap<String, CanvasView>,
    pub picker: Option<Picker>,
    pub actions: Vec<Action>,
    /// The workspace whose canvas was used last.
    pub active_workspace: Option<String>,
    /// What blocks and zones of the running bot did recently, keyed by (workspace, id).
    pub activity: HashMap<(String, String), crate::bot::Activity>,
    /// Every selected block in the selected workspace (includes `selected`).
    pub multi: HashSet<String>,
    /// A selected wire (workspace, link), deleted with Del.
    pub selected_wire: Option<(String, Link)>,
    pub history: Histories,
    /// App preferences; the theme is styled with them.
    pub prefs: crate::prefs::Prefs,
    /// Where parts of the window were drawn this frame, for the tour to point at.
    pub ui_rects: HashMap<String, eframe::egui::Rect>,
    /// Project snapshots and crash recovery copies, kept outside projects.
    pub backups: circuit_core::backup::Backups,
    /// Unsaved changes found when the project opened (left by a crash).
    pub recovery_offer: Option<circuit_core::backup::Recovery>,
    /// Successful saves so far, so others can react to a save.
    pub saves: u64,
    /// What blocks see as `project` in `define()`: `{ data }` of the open project.
    pub define_ctx: Value,
    pub embed_edit: Option<crate::embed::EmbedEdit>,
    pub layout_edit: Option<crate::layout::LayoutEdit>,
    /// The bot's name and picture, for message previews.
    pub bot_profile: crate::bot_profile::Profile,
    pub slash_edit: Option<crate::slash::SlashEdit>,
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
            edited: false,
            undo_project: false,
            reloads: 0,
            selected: None,
            log: Vec::new(),
            shapes: ShapeCache::default(),
            views: HashMap::new(),
            picker: None,
            actions: Vec::new(),
            active_workspace: None,
            activity: HashMap::new(),
            multi: HashSet::new(),
            selected_wire: None,
            history: Histories::default(),
            prefs: Default::default(),
            ui_rects: HashMap::new(),
            backups: circuit_core::backup::Backups::new(crate::bot::app_data_dir().join("backups")),
            recovery_offer: None,
            saves: 0,
            define_ctx: Value::Object(Map::new()),
            embed_edit: None,
            layout_edit: None,
            bot_profile: Default::default(),
            slash_edit: None,
        };
        ed.set_theme(theme_id.unwrap_or("graphite"));
        ed.log_plugin_summary();
        Ok(ed)
    }

    pub fn log(&mut self, level: Level, text: impl Into<String>) {
        self.push_log(LogLine::new(level, text));
    }

    /// Adds a console line, keeping the newest 2000 (the oldest 500 go at once).
    pub fn push_log(&mut self, line: LogLine) {
        self.log.push(line);
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
        self.prefs.style(&mut self.theme);
    }

    /// Re-applies the preferences to the current theme, after they changed.
    pub fn restyle(&mut self) {
        let id = self.theme.id.clone();
        self.set_theme(&id);
    }

    /// Plugin folders to watch for changes, including the project's own.
    pub fn watch_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = self.plugin_dirs.clone();
        if let Some(p) = &self.project {
            dirs.push(p.root.join("plugins"));
        }
        dirs
    }

    /// Reloads every plugin, including the open project's own `plugins/` folder.
    /// Shows the interface in `prefs.language`.
    pub fn apply_language(&mut self) {
        crate::i18n::set_language(&self.reg, &self.prefs.language);
    }

    pub fn reload_plugins(&mut self) {
        self.reloads += 1;
        let mut dirs = self.plugin_dirs.clone();
        if let Some(p) = &self.project {
            dirs.push(p.root.join("plugins"));
        }
        match BlockEngine::new() {
            Ok(engine) => self.engine = engine,
            Err(e) => return self.log(Level::Error, format!("could not restart the block sandbox: {e:#}")),
        }
        self.reg = Registry::load(&dirs, &self.engine);
        self.apply_language();
        self.shapes.clear();
        let theme = self.theme.id.clone();
        self.set_theme(&theme);
        self.log_plugin_summary();
    }

    pub fn open_project(&mut self, root: &Path) {
        match Project::load(root) {
            Ok(project) => {
                self.log(Level::Success, format!("Opened {}", project.root.display()));
                for (stem, why) in &project.unreadable {
                    self.log(Level::Error, format!("Couldn't read workspaces/{stem}.json, so it isn't open. The file is left as it is: {why}"));
                }
                let first = project.workspaces.keys().next().cloned();
                self.recovery_offer = self.backups.read_recovery(&project);
                self.history.reset_saved(&project);
                self.project = Some(project);
                self.edited = false;
                self.selected = None;
                self.multi.clear();
                self.views.clear();
                self.reload_plugins();
                self.refresh_define_ctx();
                self.upgrade_blocks();
                self.upgrade_texts();
                self.fetch_bot_profile();
                if let Some(first) = first {
                    self.actions.push(Action::OpenWorkspace(first));
                }
            }
            Err(e) => self.log(Level::Error, format!("Could not open {}: {e:#}", root.display())),
        }
    }

    pub fn save(&mut self) {
        let Some(project) = self.project.as_mut() else { return };
        match project.save() {
            Ok(()) => {
                self.history.track(project, true);
                self.history.mark_saved();
                self.edited = false;
                self.saves += 1;
                let msg = format!("Saved {}", project.meta.name);
                self.backups.clear_recovery(&project.root);
                let backup = self.prefs.backups.then(|| {
                    let made = self.backups.snapshot(project, "Saved");
                    self.backups.prune(&project.root, self.prefs.backup_keep, self.prefs.backup_days, circuit_core::backup::now_ms());
                    made
                });
                self.log(Level::Success, msg);
                if let Some(Err(e)) = backup {
                    self.log(Level::Warn, format!("Saved, but the backup couldn't be made: {e:#}"));
                }
            }
            Err(e) => self.log(Level::Error, format!("Save failed: {e:#}")),
        }
    }

    /// Puts the whole project back as it was in a snapshot and saves it. The
    /// current version is backed up first, so this can be undone.
    pub fn restore_project(&mut self, snap: &circuit_core::backup::Snapshot) -> anyhow::Result<()> {
        let (meta, workspaces) = self.backups.load(snap)?;
        let Some(project) = self.project.as_mut() else { anyhow::bail!("no project is open") };
        self.backups.snapshot(project, "Before restore")?;
        let gone: Vec<String> = project.workspaces.keys().filter(|k| !workspaces.contains_key(*k)).cloned().collect();
        for key in &gone {
            project.remove_workspace(key);
            self.actions.push(Action::CloseWorkspaceTabs(key.clone()));
        }
        project.meta = meta;
        for (key, ws) in workspaces {
            project.removed.retain(|r| *r != key);
            project.workspaces.insert(key, ws);
        }
        self.history.clear();
        self.views.clear();
        self.clear_selection();
        self.shapes.clear();
        self.edited = true;
        self.save();
        self.refresh_define_ctx();
        let when = crate::editor::local_time(snap.info.saved_at);
        self.log(Level::Success, format!("Restored the project as it was on {when}"));
        if let Some(first) = self.project.as_ref().and_then(|p| p.workspaces.keys().next().cloned()) {
            self.actions.push(Action::OpenWorkspace(first));
        }
        Ok(())
    }

    /// Puts one workspace back as it was in a snapshot, as an unsaved change.
    pub fn restore_workspace(&mut self, snap: &circuit_core::backup::Snapshot, key: &str) -> anyhow::Result<()> {
        let (_, mut workspaces) = self.backups.load(snap)?;
        let ws = workspaces.shift_remove(key).ok_or_else(|| anyhow::anyhow!("the backup has no workspace `{key}`"))?;
        let Some(project) = self.project.as_mut() else { anyhow::bail!("no project is open") };
        project.removed.retain(|r| r != key);
        let name = ws.name.clone();
        project.workspaces.insert(key.to_string(), ws);
        self.shapes.forget_workspace(key);
        self.edited = true;
        self.actions.push(Action::OpenWorkspace(key.to_string()));
        self.log(Level::Info, format!("Restored the workspace {name} from {} (not saved yet)", crate::editor::local_time(snap.info.saved_at)));
        Ok(())
    }

    /// Takes over the unsaved changes found after a crash, as unsaved changes.
    pub fn apply_recovery(&mut self, rec: circuit_core::backup::Recovery) {
        let Some(project) = self.project.as_mut() else { return };
        let gone: Vec<String> = project.workspaces.keys().filter(|k| !rec.workspaces.contains_key(*k)).cloned().collect();
        for key in &gone {
            project.remove_workspace(key);
            self.actions.push(Action::CloseWorkspaceTabs(key.clone()));
        }
        project.meta = rec.meta;
        for (key, ws) in rec.workspaces {
            project.removed.retain(|r| *r != key);
            project.workspaces.insert(key, ws);
        }
        self.shapes.clear();
        self.history.clear();
        self.edited = true;
        self.log(Level::Success, "Your unsaved changes are back. Save to keep them.");
    }

    /// Whether the project has changes that aren't saved, as of the last
    /// settled frame.
    pub fn unsaved(&self) -> bool {
        self.project.is_some() && self.history.unsaved()
    }

    /// Records changes made since the last settled frame right away.
    pub fn check_changes(&mut self) {
        if let Some(p) = &self.project {
            self.history.track(p, true);
        }
        self.edited = false;
    }

    /// Whether there are unsaved changes, counting ones not recorded yet.
    pub fn unsaved_now(&mut self) -> bool {
        self.check_changes();
        self.unsaved()
    }

    /// Treats the project as it is now as saved, without saving it (for the
    /// tour's practice project, which isn't kept).
    pub fn forget_changes(&mut self) {
        self.check_changes();
        self.history.mark_saved();
    }

    pub fn mode(&self) -> Option<&BuildMode> {
        let id = self.project.as_ref().map(|p| p.meta.mode.as_str()).unwrap_or("discord-bot");
        if id == NO_MODE {
            return None;
        }
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
            Node { block: def.reference(), at: [at.x, at.y], values: def.default_values(), pins: def.default_pins(), ..Default::default() },
        );
        self.edited = true;
        self.select_only(ws_key, &id);
        Some(id)
    }

    /// The values texts on block `node` can use: see [`crate::values`].
    pub fn values_for(&mut self, ws_key: &str, node: &str) -> crate::values::Scope {
        let Editor { reg, engine, project, shapes, define_ctx, .. } = self;
        shapes.begin();
        let Some(ws) = project.as_ref().and_then(|p| p.workspaces.get(ws_key)) else { return Default::default() };
        crate::values::available(ws, node, reg, &mut |id| node_shape(reg, engine, shapes, ws_key, ws, id, define_ctx))
    }

    /// Call after project data (such as slash commands) changes.
    pub fn refresh_define_ctx(&mut self) {
        let data = self.project.as_ref().map(|p| p.meta.data.clone()).unwrap_or_default();
        self.define_ctx = serde_json::json!({ "data": data });
        self.shapes.clear();
    }

    /// Reads the bot's name and picture from Discord for the previews (when the
    /// project has a bot token).
    pub fn fetch_bot_profile(&mut self) {
        if let Some(p) = &self.project {
            self.bot_profile.fetch(&p.root, &p.meta.mode);
        }
    }

    /// Texts written with `{name}` placeholders use `${name}` now (`${…}` holds
    /// JavaScript): every text field of every block, and the texts of designs
    /// such as embeds, are upgraded.
    fn upgrade_texts(&mut self) {
        let Some(project) = self.project.as_mut() else { return };
        fn walk(v: &mut Value, count: &mut usize) {
            match v {
                Value::String(s) => {
                    if let Some(new) = circuit_core::template::upgrade(s) {
                        *s = new;
                        *count += 1;
                    }
                }
                Value::Array(a) => a.iter_mut().for_each(|x| walk(x, count)),
                Value::Object(m) => m.values_mut().for_each(|x| walk(x, count)),
                _ => {}
            }
        }
        let mut count = 0;
        for ws in project.workspaces.values_mut() {
            for node in ws.nodes.values_mut() {
                let Some(def) = self.reg.block(&node.block) else { continue };
                for (key, f) in &def.fields {
                    if f.templated()
                        && let Some(v) = node.values.get_mut(key)
                    {
                        walk(v, &mut count);
                    }
                }
            }
        }
        if count > 0 {
            self.edited = true;
            self.log(Level::Info, tf!("Changed {count} texts from {name} to ${name}, which holds JavaScript now (save to keep)", count = count));
        }
    }

    /// Upgrades blocks saved with an older major version, using the block's `migrate()`.
    fn upgrade_blocks(&mut self) {
        let Some(project) = self.project.as_mut() else { return };
        let mut upgraded = 0;
        for ws in project.workspaces.values_mut() {
            for node in ws.nodes.values_mut() {
                let Some(def) = self.reg.block(&node.block) else { continue };
                let saved: u64 = node.block.split('@').nth(1).and_then(|v| v.parse().ok()).unwrap_or(def.major_version());
                if saved >= def.major_version() {
                    continue;
                }
                if def.has_migrate
                    && let Some(key) = self.reg.engine_keys.get(&def.id)
                {
                    match self.engine.migrate(key, &node.values, &saved.to_string()) {
                        Ok(values) => node.values = values,
                        Err(e) => {
                            self.log.push(LogLine::new(Level::Warn, format!("Could not upgrade {}: {e:#}", def.name)));
                            continue;
                        }
                    }
                }
                node.block = def.reference();
                upgraded += 1;
            }
        }
        if upgraded > 0 {
            self.edited = true;
            self.log(Level::Info, format!("Upgraded {upgraded} blocks to newer versions (save to keep)"));
        }
    }

    pub fn select_only(&mut self, ws_key: &str, id: &str) {
        self.selected = Some((ws_key.to_string(), id.to_string()));
        self.multi = HashSet::from([id.to_string()]);
    }

    /// Opens a block's workspace, selects the block and centers the canvas on it.
    pub fn reveal(&mut self, ws_key: &str, id: &str) {
        self.actions.push(Action::OpenWorkspace(ws_key.to_string()));
        self.select_only(ws_key, id);
        let view = self.views.entry(ws_key.to_string()).or_insert_with(|| CanvasView { zoom: 1.0, ..Default::default() });
        view.focus = Some(id.to_string());
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.multi.clear();
    }

    /// Selected block ids in a workspace.
    pub fn selection(&self, ws_key: &str) -> Vec<String> {
        match &self.selected {
            Some((w, n)) if w == ws_key => {
                let mut ids: Vec<String> = self.multi.iter().cloned().collect();
                if !ids.contains(n) {
                    ids.push(n.clone());
                }
                ids.sort();
                ids
            }
            _ => Vec::new(),
        }
    }

    /// The workspace edit commands act on.
    fn edit_target(&self) -> Option<String> {
        self.active_workspace.clone().filter(|k| self.workspace(k).is_some())
    }

    /// What Undo and Redo act on: a workspace key, or `history::PROJECT`.
    pub fn undo_key(&self) -> String {
        if self.undo_project && self.project.is_some() {
            return crate::history::PROJECT.to_string();
        }
        self.active_workspace.clone().unwrap_or_default()
    }

    /// Undoes the last change to the project's settings and data.
    fn undo_project_data(&mut self, redo: bool) -> bool {
        self.check_changes();
        let Some(meta) = self.project.as_mut().map(|p| &mut p.meta) else { return false };
        let done = if redo { self.history.redo_project(meta) } else { self.history.undo_project(meta) };
        if done {
            // Open editors show the data as it is now.
            if self.slash_edit.is_some() {
                crate::slash::open(self);
            }
            self.shapes.clear();
            self.refresh_define_ctx();
            self.edited = true;
        }
        done
    }

    /// Returns whether something was undone.
    pub fn undo(&mut self) -> bool {
        if self.undo_project {
            return self.undo_project_data(false);
        }
        let Some(key) = self.edit_target() else { return false };
        let Some(ws) = self.project.as_mut().and_then(|p| p.workspaces.get_mut(&key)) else { return false };
        let done = self.history.undo(&key, ws);
        if done {
            self.after_history_jump(&key);
        }
        done
    }

    /// Returns whether something was redone.
    pub fn redo(&mut self) -> bool {
        if self.undo_project {
            return self.undo_project_data(true);
        }
        let Some(key) = self.edit_target() else { return false };
        let Some(ws) = self.project.as_mut().and_then(|p| p.workspaces.get_mut(&key)) else { return false };
        let done = self.history.redo(&key, ws);
        if done {
            self.after_history_jump(&key);
        }
        done
    }

    fn after_history_jump(&mut self, key: &str) {
        self.edited = true;
        self.shapes.forget_workspace(key);
        let alive: HashSet<String> = self.workspace(key).map(|w| w.nodes.keys().cloned().collect()).unwrap_or_default();
        self.multi.retain(|n| alive.contains(n));
        if self.selected.as_ref().is_some_and(|(w, n)| w == key && !alive.contains(n)) {
            self.clear_selection();
        }
    }

    /// Copies the selected blocks to the clipboard as JSON.
    pub fn copy(&self, ctx: &eframe::egui::Context) -> bool {
        let Some(key) = self.edit_target() else { return false };
        let ids = self.selection(&key);
        let Some(ws) = self.workspace(&key) else { return false };
        if ids.is_empty() {
            return false;
        }
        let clip = ws.extract(&ids);
        if let Ok(text) = serde_json::to_string_pretty(&clip) {
            ctx.copy_text(text);
        }
        true
    }

    pub fn delete_selection(&mut self) {
        if self.selected.is_none()
            && let Some((key, link)) = self.selected_wire.take()
        {
            if let Some(ws) = self.project.as_mut().and_then(|p| p.workspaces.get_mut(&key)) {
                let before = ws.links.len();
                ws.links.retain(|l| *l != link);
                self.edited |= ws.links.len() != before;
            }
            return;
        }
        let Some(key) = self.edit_target() else { return };
        let ids = self.selection(&key);
        let Some(ws) = self.project.as_mut().and_then(|p| p.workspaces.get_mut(&key)) else { return };
        for id in &ids {
            ws.remove_node(id);
        }
        if !ids.is_empty() {
            self.edited = true;
            self.clear_selection();
        }
    }

    /// Pastes blocks copied from XC Circuit. Pasted at `at` (scene position) or next to the originals.
    pub fn paste(&mut self, text: &str, at: Option<Pos2>) -> bool {
        let Some(key) = self.edit_target() else { return false };
        let Ok(clip) = serde_json::from_str::<Clip>(text) else { return false };
        if clip.format != CLIP_FORMAT || clip.nodes.is_empty() {
            return false;
        }
        let min = clip.nodes.values().fold([f32::MAX, f32::MAX], |m, n| [m[0].min(n.at[0]), m[1].min(n.at[1])]);
        let offset = match at {
            Some(p) => [p.x - min[0], p.y - min[1]],
            None => [40.0, 40.0],
        };
        let Some(ws) = self.project.as_mut().and_then(|p| p.workspaces.get_mut(&key)) else { return false };
        let ids = ws.paste(&clip, offset);
        self.edited = true;
        self.selected = ids.first().map(|f| (key.clone(), f.clone()));
        self.multi = ids.into_iter().collect();
        true
    }

    pub fn duplicate(&mut self) {
        let Some(key) = self.edit_target() else { return };
        let ids = self.selection(&key);
        let Some(ws) = self.workspace(&key) else { return };
        if let Ok(text) = serde_json::to_string(&ws.extract(&ids)) {
            self.paste(&text, None);
        }
    }

    pub fn select_all(&mut self) {
        let Some(key) = self.edit_target() else { return };
        let ids: Vec<String> = self.workspace(&key).map(|w| w.nodes.keys().cloned().collect()).unwrap_or_default();
        if let Some(first) = ids.first() {
            self.selected = Some((key, first.clone()));
            self.multi = ids.into_iter().collect();
        }
    }
}

/// Block shapes, kept until what they are made from changes.
#[derive(Default)]
pub struct ShapeCache {
    blocks: HashMap<(String, String), (u64, BlockShape)>,
    /// Per workspace: the names each block gets from earlier blocks.
    known: HashMap<String, Known>,
    /// Counts passes over the blocks; see `begin`.
    pass: u64,
}

/// The names blocks in one workspace get from earlier blocks, and what they
/// were worked out from.
#[derive(Default)]
struct Known {
    /// The pass in which `structure` was last checked.
    pass: u64,
    /// A hash of the wires, the blocks, and the settings of event and
    /// variable blocks: everything the names depend on.
    structure: u64,
    /// A hash of the project data that `define()` sees.
    project: u64,
    names: HashMap<String, Vec<String>>,
}

impl ShapeCache {
    /// Starts a pass over the blocks (a frame, or a lookup): the workspace's
    /// structure is checked once per pass, not once per block.
    pub fn begin(&mut self) {
        self.pass += 1;
    }

    pub fn clear(&mut self) {
        self.blocks.clear();
        self.known.clear();
    }

    pub fn forget_workspace(&mut self, key: &str) {
        self.blocks.retain(|(w, _), _| w != key);
        self.known.remove(key);
    }

    pub fn forget(&mut self, ws_key: &str, node: &str) {
        self.blocks.remove(&(ws_key.to_string(), node.to_string()));
    }

    fn get(&self, ws_key: &str, node: &str) -> Option<&BlockShape> {
        self.blocks.get(&(ws_key.to_string(), node.to_string())).map(|(_, s)| s)
    }

    /// The workspace's names, emptied when what they depend on changed.
    fn known(&mut self, reg: &Registry, ws_key: &str, ws: &Workspace, project: &Value) -> &mut Known {
        let pass = self.pass;
        let k = self.known.entry(ws_key.to_string()).or_default();
        if k.pass != pass || k.names.is_empty() {
            let (structure, project) = (structure_hash(reg, ws), text_hash(project));
            if structure != k.structure || project != k.project {
                k.names.clear();
                (k.structure, k.project) = (structure, project);
            }
            k.pass = pass;
        }
        k
    }
}

fn text_hash(v: &impl serde::Serialize) -> u64 {
    let mut h = DefaultHasher::new();
    serde_json::to_string(v).unwrap_or_default().hash(&mut h);
    h.finish()
}

/// What the names blocks get from earlier blocks depend on.
fn structure_hash(reg: &Registry, ws: &Workspace) -> u64 {
    let mut h = DefaultHasher::new();
    for l in &ws.links {
        (&l.from.node, &l.from.pin, &l.to.node, &l.to.pin).hash(&mut h);
    }
    for (id, n) in &ws.nodes {
        (id, &n.block, &n.zone).hash(&mut h);
        let event = reg.block(&n.block).is_some_and(|d| d.kind == circuit_core::BlockKind::Event);
        if event || n.block_id() == "core.set_variable" {
            serde_json::to_string(&n.values).unwrap_or_default().hash(&mut h);
        }
    }
    ws.zones.keys().for_each(|z| z.hash(&mut h));
    h.finish()
}

/// Names a block gets from earlier blocks, which need no {name} input
/// (unless one is wired already).
fn known_names(reg: &Registry, engine: &BlockEngine, cache: &mut ShapeCache, ws_key: &str, ws: &Workspace, node_id: &str, project: &Value) -> Vec<String> {
    if let Some(names) = cache.known(reg, ws_key, ws, project).names.get(node_id) {
        return names.clone();
    }
    // Only event blocks' shapes are needed for them, and events have nothing
    // before them, so this goes one level deep.
    let upstream = crate::values::upstream(ws, node_id, reg);
    let mut known = crate::values::known_names(ws, &upstream, reg, &mut |id| {
        let is_event = ws.nodes.get(id).and_then(|n| reg.block(&n.block)).is_some_and(|d| d.kind == circuit_core::BlockKind::Event);
        if is_event { node_shape(reg, engine, cache, ws_key, ws, id, project) } else { None }
    });
    known.retain(|name| ws.links_into(node_id, name).next().is_none());
    let mut known: Vec<String> = known.into_iter().collect();
    known.sort();
    cache.known(reg, ws_key, ws, project).names.insert(node_id.to_string(), known.clone());
    known
}

/// The effective shape of a node, re-running `define()` only when its values or
/// connected types change. Call `cache.begin()` before a pass over the blocks.
pub fn node_shape(reg: &Registry, engine: &BlockEngine, cache: &mut ShapeCache, ws_key: &str, ws: &Workspace, node_id: &str, project: &Value) -> Option<BlockShape> {
    let node = ws.nodes.get(node_id)?;
    let def = reg.block(&node.block)?;
    let connected = connected_types(reg, cache, ws_key, ws, node_id);
    let known = known_names(reg, engine, cache, ws_key, ws, node_id, project);

    let mut h = DefaultHasher::new();
    node.block.hash(&mut h);
    serde_json::to_string(&node.values).unwrap_or_default().hash(&mut h);
    serde_json::to_string(&connected).unwrap_or_default().hash(&mut h);
    known.hash(&mut h);
    if def.has_define {
        cache.known(reg, ws_key, ws, project).project.hash(&mut h);
    }
    let hash = h.finish();

    let key = (ws_key.to_string(), node_id.to_string());
    if let Some((cached_hash, shape)) = cache.blocks.get(&key)
        && *cached_hash == hash
    {
        return Some(shape.clone());
    }

    let define = if def.has_define {
        let engine_key = reg.engine_keys.get(&def.id)?;
        engine.define(engine_key, &node.values, &connected, project).ok()
    } else {
        None
    };
    let shape = BlockShape::build_with(def, &node.values, define.as_ref(), &known.into_iter().collect());
    cache.blocks.insert(key, (hash, shape.clone()));
    Some(shape)
}

/// For each connected input pin of a node, the type string of the output feeding it.
fn connected_types(
    reg: &Registry,
    cache: &ShapeCache,
    ws_key: &str,
    ws: &Workspace,
    node_id: &str,
) -> Map<String, Value> {
    let mut out = Map::new();
    for link in ws.links.iter().filter(|l| l.to.node == node_id && l.to.pin != ACTION_IN) {
        let Some(src) = ws.nodes.get(&link.from.node) else { continue };
        let cached = cache.get(ws_key, &link.from.node).and_then(|s| s.output(&link.from.pin)).map(|o| o.type_name.clone());
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
    chrono::Local::now().format("%H:%M:%S").to_string()
}

/// A moment (milliseconds since 1970) in local time: `Mon 2 Nov 2026, 15:30`.
pub fn local_time(ms: u64) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_millis_opt(ms as i64).single() {
        Some(t) => t.format("%a %-d %b %Y, %H:%M").to_string(),
        None => "an unknown time".into(),
    }
}

/// How long ago a moment was: `just now`, `5 minutes ago`, `3 days ago`.
pub fn ago(ms: u64) -> String {
    let secs = circuit_core::backup::now_ms().saturating_sub(ms) / 1000;
    let (n, unit) = match secs {
        0..60 => return "just now".into(),
        60..3_600 => (secs / 60, "minute"),
        3_600..86_400 => (secs / 3_600, "hour"),
        _ => (secs / 86_400, "day"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Undo in the Slash Commands tab takes back a saved change to the
    /// commands, and the open builder shows them as they were.
    #[test]
    fn undo_takes_back_project_data() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("xc-undo-data-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Project::create(&dir, "U", "discord-bot").unwrap();
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&dir);
        let commands = serde_json::json!([{ "name": "ping", "description": "Pong", "options": [] }]);
        ed.project.as_mut().unwrap().meta.data.insert("slashCommands".into(), commands);
        assert!(ed.unsaved_now());
        crate::slash::open(&mut ed);
        assert_eq!(ed.slash_edit.as_ref().unwrap().commands.len(), 1);

        ed.undo_project = true;
        assert_eq!(ed.undo_key(), crate::history::PROJECT);
        assert!(ed.undo());
        assert!(ed.project.as_ref().unwrap().meta.data.is_empty());
        assert!(ed.slash_edit.as_ref().unwrap().commands.is_empty(), "the builder shows the commands as they are now");
        assert!(!ed.unsaved_now(), "back as saved");
        assert!(ed.redo());
        assert_eq!(ed.slash_edit.as_ref().unwrap().commands.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every bundled block's define() runs in the sandbox with its defaults
    /// and with each choice of its select fields, and gives a usable shape.
    #[test]
    fn older_texts_upgrade_to_javascript() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        assert!(!ed.unsaved_now(), "the example is up to date");
        let nodes = &mut ed.project.as_mut().unwrap().workspaces.get_mut("rank-command").unwrap().nodes;
        nodes.get_mut("4").unwrap().values.insert("text".into(), "Hi {user}, {#3.embed.title} x{3}".into());
        nodes.get_mut("3").unwrap().values["embed"]["title"] = "Rank of {user}".into();
        ed.upgrade_texts();
        let nodes = &ed.project.as_ref().unwrap().workspaces["rank-command"].nodes;
        assert_eq!(nodes["4"].values["text"], "Hi ${user}, ${#3.embed.title} x{3}");
        assert_eq!(nodes["3"].values["embed"]["title"], "Rank of ${user}", "texts inside designs too");
        assert!(ed.unsaved_now());
    }

    #[test]
    fn every_block_defines_cleanly() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        assert!(ed.reg.problems.is_empty(), "{:?}", ed.reg.problems);
        assert!(ed.reg.blocks.len() > 60 && ed.reg.blocks.values().filter(|b| b.has_define).count() > 15, "the bundled blocks loaded");
        let project = serde_json::json!({ "name": "Test", "mode": "discord-bot", "settings": {}, "data": { "slashCommands": [{ "name": "ping", "description": "Pong", "options": [] }] } });
        let mut failures = Vec::new();
        for def in ed.reg.blocks.values() {
            let defaults: Map<String, Value> = def.fields.iter().filter_map(|(k, f)| f.default.clone().map(|d| (k.clone(), d))).collect();
            let mut cases = vec![defaults.clone()];
            for (k, f) in &def.fields {
                for (_, choice, _) in f.choice_list() {
                    let mut v = defaults.clone();
                    v.insert(k.clone(), Value::String(choice));
                    cases.push(v);
                }
            }
            for values in cases {
                if def.has_define {
                    let key = &ed.reg.engine_keys[&def.id];
                    match ed.engine.define(key, &values, &Map::new(), &project) {
                        Ok(d) => {
                            let shape = circuit_core::BlockShape::build(def, &values, Some(&d));
                            assert!(!shape.fields.is_empty() || !shape.outputs.is_empty() || def.kind != circuit_core::BlockKind::Value, "{} has no shape", def.id);
                        }
                        Err(e) => failures.push(format!("{} with {:?}: {e:#}", def.id, values)),
                    }
                } else {
                    circuit_core::BlockShape::build(def, &values, None);
                }
            }
        }
        assert!(failures.is_empty(), "define() failed:\n{}", failures.join("\n"));
    }

    /// The starter templates and the example load, use blocks that exist, and
    /// every wire connects an output (or flow) to an input that the block has.
    #[test]
    fn templates_are_wired_correctly() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(root.join("examples/templates")).unwrap().flatten().map(|e| e.path()).collect();
        dirs.push(root.join("examples/DemoBot"));
        assert!(dirs.len() >= 6, "five templates and the example");
        let mut wrong = Vec::new();
        for dir in dirs {
            let project = Project::load(&dir).unwrap();
            assert!(project.unreadable.is_empty(), "{}: {:?}", dir.display(), project.unreadable);
            let meta = serde_json::to_value(&project.meta).unwrap();
            let mut cache = ShapeCache::default();
            for (key, ws) in &project.workspaces {
                let place = format!("{} › {key}", dir.file_name().unwrap().to_string_lossy());
                let mut shapes = HashMap::new();
                for id in ws.nodes.keys() {
                    match node_shape(&ed.reg, &ed.engine, &mut cache, key, ws, id, &meta) {
                        Some(s) => {
                            shapes.insert(id.clone(), s);
                        }
                        None => wrong.push(format!("{place}: #{id} uses a missing block {}", ws.nodes[id].block)),
                    }
                }
                for link in &ws.links {
                    let from_ok = if ws.zones.contains_key(&link.from.node) {
                        ["done", "error", "message", "details"].contains(&link.from.pin.as_str())
                    } else {
                        shapes.get(&link.from.node).is_some_and(|s| s.output(&link.from.pin).is_some() || s.flows.contains(&link.from.pin))
                    };
                    let to_ok = shapes.get(&link.to.node).is_some_and(|s| link.to.pin == circuit_core::graph::ACTION_IN || s.field(&link.to.pin).is_some());
                    if !from_ok || !to_ok {
                        wrong.push(format!("{place}: {}.{} > {}.{}", link.from.node, link.from.pin, link.to.node, link.to.pin));
                    }
                }
            }
        }
        assert!(wrong.is_empty(), "wrong wires:\n{}", wrong.join("\n"));
    }
}
