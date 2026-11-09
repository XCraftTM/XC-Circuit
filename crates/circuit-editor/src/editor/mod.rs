//! Editor state shared by every panel: plugins, the open project, cached
//! block shapes, selection and the log.

pub mod exchange;
pub mod folders;
pub mod history;
pub mod keymap;
pub mod prefs;
pub mod saver;
pub mod custom_ids;
pub mod values;

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use std::collections::HashSet;

use circuit_core::{BlockShape, CLIP_FORMAT, Clip, Link, Node, Project, Workspace};

use crate::editor::history::Histories;
use circuit_core::graph::ACTION_IN;
use circuit_plugins::{BlockEngine, BuildMode, Registry};
use eframe::egui::Pos2;
use serde_json::{Map, Value};

use crate::canvas::CanvasView;
use crate::canvas::picker::Picker;
use crate::ui::theme::Theme;

mod shapes;
pub use shapes::*;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Warn,
    Error,
    /// What an AI agent did through the MCP server.
    Mcp,
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
    /// The ids of the plugins that come with the app: they can't be turned
    /// off or removed.
    pub bundled_plugins: Vec<String>,
    /// Plugins that come with the app but can be turned off and removed: (id, name).
    pub optional_plugins: Vec<(String, String)>,
    /// Text a plugin put on the clipboard, for the app to copy.
    pub copy_out: Option<String>,
    /// The MCP server for AI agents: { listening, agents }, kept by the app.
    pub agent_link: serde_json::Value,
    /// The bundled `examples` folder (DemoBot and the starters), when found.
    pub examples_dir: Option<PathBuf>,
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
    /// The bot's server as plugins and agents see it (`xc.server()`), kept
    /// up to date by the hosting part of the app. Nothing secret.
    pub server_view: serde_json::Value,
    pub picker: Option<Picker>,
    pub actions: Vec<Action>,
    /// The workspace whose canvas was used last.
    pub active_workspace: Option<String>,
    /// What blocks and zones of the running bot did recently, keyed by (workspace, id).
    pub activity: HashMap<(String, String), crate::bot::Activity>,
    /// The last value each output gave while the bot ran, as a short text,
    /// and when: (workspace, block or zone, output) → value.
    pub wire_values: HashMap<(String, String, String), (String, std::time::Instant)>,
    /// Every selected block in the selected workspace (includes `selected`).
    pub multi: HashSet<String>,
    /// Selected notes: (workspace, note).
    pub selected_notes: HashSet<(String, String)>,
    /// The frame in which Settings › Shortcuts last waited for a key: until
    /// the next one, no shortcut fires (see `recording_shortcut`).
    pub recording_frame: Option<u64>,
    /// A selected wire (workspace, link), deleted with Del.
    pub selected_wire: Option<(String, Link)>,
    pub history: Histories,
    /// App preferences; the theme is styled with them.
    pub prefs: crate::editor::prefs::Prefs,
    /// Where parts of the window were drawn this frame, for the tour to point at.
    pub ui_rects: HashMap<String, eframe::egui::Rect>,
    /// While the tour runs: the canvas, picker and Inspector also note where
    /// each pin, block row, picker entry and field went, for it to make glow.
    pub tour_parts: bool,
    /// `ui_rects` of the frame before: for what's drawn after the tour (the picker).
    pub ui_rects_before: HashMap<String, eframe::egui::Rect>,
    /// Project snapshots and crash recovery copies, kept outside projects.
    pub backups: circuit_core::backup::Backups,
    /// Unsaved changes found when the project opened (left by a crash).
    pub recovery_offer: Option<circuit_core::backup::Recovery>,
    /// Successful saves so far, so others can react to a save. Counted once
    /// the files are written.
    pub saves: u64,
    /// Writes saves, backups and recovery copies on a thread of its own.
    pub saver: saver::Saver,
    /// The newest save given to `saver`.
    last_save: u64,
    /// What blocks see as `project` in `define()`: `{ data, settings }` of the open project.
    pub define_ctx: Value,
    pub embed_edit: Option<crate::builders::embed::EmbedEdit>,
    pub layout_edit: Option<crate::builders::layout::LayoutEdit>,
    pub modal_edit: Option<crate::builders::modal::ModalEdit>,
    /// Workspaces selected in the Workspaces list (Ctrl- and Shift-click), and
    /// the one a Shift-click counts from. Delete asks to delete them.
    pub ws_selection: Vec<String>,
    pub ws_anchor: Option<String>,
    /// The frame the Workspaces list was last drawn in: its selection only
    /// counts for Delete while the list is on screen.
    pub ws_list_frame: u64,
    /// Workspaces waiting for "Delete them?" to be answered.
    pub ask_delete_workspaces: Option<Vec<String>>,
    pub canvas_edit: Option<crate::builders::canvas::CanvasEdit>,
    /// The bot's name and picture, for message previews.
    pub bot_profile: crate::bot::profile::Profile,
    /// The bot's servers, channels and roles, for picking ids.
    pub servers: crate::bot::servers::Servers,
    pub slash_edit: Option<crate::builders::slash::SlashEdit>,
    /// The Export Workspaces and Import Workspaces tabs.
    pub ws_export: Option<crate::editor::exchange::ExportDraft>,
    pub ws_import: Option<crate::editor::exchange::ImportDraft>,
}

impl Editor {
    pub fn new(plugin_dirs: Vec<PathBuf>, theme_id: Option<&str>) -> anyhow::Result<Editor> {
        let engine = BlockEngine::new()?;
        let reg = Registry::load(&plugin_dirs, &engine);
        let mut ed = Editor {
            examples_dir: None,
            reg,
            engine,
            plugin_dirs,
            bundled_plugins: Vec::new(),
            optional_plugins: Vec::new(),
            copy_out: None,
            agent_link: serde_json::Value::Null,
            theme: Theme::fallback(),
            project: None,
            edited: false,
            undo_project: false,
            reloads: 0,
            selected: None,
            log: Vec::new(),
            shapes: ShapeCache::default(),
            views: HashMap::new(),
            server_view: serde_json::Value::Null,
            picker: None,
            actions: Vec::new(),
            active_workspace: None,
            activity: HashMap::new(),
            wire_values: HashMap::new(),
            multi: HashSet::new(),
            selected_notes: HashSet::new(),
            recording_frame: None,
            selected_wire: None,
            history: Histories::default(),
            prefs: Default::default(),
            ui_rects: HashMap::new(),
            tour_parts: false,
            ui_rects_before: HashMap::new(),
            backups: circuit_core::backup::Backups::new(crate::bot::app_data_dir().join("backups")),
            recovery_offer: None,
            saves: 0,
            saver: saver::Saver::default(),
            last_save: 0,
            define_ctx: Value::Object(Map::new()),
            embed_edit: None,
            layout_edit: None,
            modal_edit: None,
            ws_selection: Vec::new(),
            ws_anchor: None,
            ws_list_frame: 0,
            ask_delete_workspaces: None,
            canvas_edit: None,
            bot_profile: Default::default(),
            servers: Default::default(),
            slash_edit: None,
            ws_export: None,
            ws_import: None,
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
        // The app's own plugins load even if a preference says otherwise.
        let off: Vec<String> = self.prefs.disabled_plugins.iter().chain(&self.prefs.removed_plugins).filter(|id| !self.bundled_plugins.contains(id)).cloned().collect();
        self.reg = Registry::load_except(&dirs, &self.engine, &off);
        self.apply_language();
        self.shapes.clear();
        let theme = self.theme.id.clone();
        self.set_theme(&theme);
        self.log_plugin_summary();
    }

    pub fn open_project(&mut self, root: &Path) {
        // Its files may still be being written.
        self.finish_saving();
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
                self.ws_export = None;
                self.ws_import = None;
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

    /// Closes the open project (save first: unsaved changes are dropped).
    /// The app shows its start page then.
    pub fn close_project(&mut self) {
        let Some(project) = self.project.take() else { return };
        self.log(Level::Info, format!("Closed {}", project.meta.name));
        self.history.clear();
        self.recovery_offer = None;
        self.edited = false;
        self.selected = None;
        self.selected_wire = None;
        self.multi.clear();
        self.views.clear();
        self.picker = None;
        self.active_workspace = None;
        self.activity.clear();
        self.embed_edit = None;
        self.layout_edit = None;
        self.modal_edit = None;
        self.ws_selection.clear();
        self.ws_anchor = None;
        self.ask_delete_workspaces = None;
        self.canvas_edit = None;
        self.slash_edit = None;
        self.ws_export = None;
        self.ws_import = None;
        // Without the project's own plugins (DemoBot has one).
        self.reload_plugins();
        self.refresh_define_ctx();
    }

    /// Saves the project as it is now. The files are written on the saving
    /// thread; the project counts as saved at once, and as unsaved again if
    /// writing fails ([`Editor::poll_saving`] says so).
    pub fn save(&mut self) {
        crate::app::watchdog::mark("saving the project");
        let Some(project) = self.project.as_mut() else { return };
        let copy = project.take_save();
        self.history.track(project, true);
        self.history.mark_saved();
        self.edited = false;
        let id = self.saver.next_id();
        self.last_save = id;
        let backup = self.prefs.backups.then_some((self.prefs.backup_keep, self.prefs.backup_days));
        self.saver.send(saver::Job::Save { id, project: copy, backups: self.backups.clone(), backup });
    }

    /// Keeps a copy of the unsaved changes for after a crash (on the saving thread).
    pub fn keep_recovery(&mut self) {
        if let Some(p) = &self.project {
            self.saver.send(saver::Job::Recovery { project: p.clone(), backups: self.backups.clone() });
        }
    }

    /// Deletes the crash recovery copy of the open project (on the saving thread).
    pub fn clear_recovery(&mut self) {
        if let Some(p) = &self.project {
            self.saver.send(saver::Job::ClearRecovery { root: p.root.clone(), backups: self.backups.clone() });
        }
    }

    /// Takes in what the saving thread has finished.
    pub fn poll_saving(&mut self) {
        for done in self.saver.finished() {
            self.saving_done(done);
        }
    }

    /// Waits for everything given to the saving thread to be written: before
    /// reading the project's files or backups, and before the app ends.
    pub fn finish_saving(&mut self) {
        if !self.saver.busy() {
            return;
        }
        crate::app::watchdog::mark("waiting for saving to finish");
        for done in self.saver.wait() {
            self.saving_done(done);
        }
    }

    fn saving_done(&mut self, done: saver::Done) {
        match done {
            saver::Done::Saved { id, root, name, removed, result, backup } => match result {
                Ok(()) => {
                    self.saves += 1;
                    self.log(Level::Success, format!("Saved {name}"));
                    if let Some(Err(e)) = backup {
                        self.log(Level::Warn, format!("Saved, but the backup couldn't be made: {e}"));
                    }
                }
                Err(e) => {
                    if let Some(p) = self.project.as_mut().filter(|p| p.root == root) {
                        // Their files are still there: deleted with the next save.
                        for stem in removed {
                            if !p.workspaces.contains_key(&stem) && !p.removed.contains(&stem) {
                                p.removed.push(stem);
                            }
                        }
                        // Unless a newer save follows, what's open isn't saved.
                        if id == self.last_save {
                            self.history.forget_saved();
                        }
                    }
                    self.log(Level::Error, format!("Save failed: {e}"));
                }
            },
            saver::Done::Recovery(Err(e)) => self.log(Level::Warn, format!("Couldn't keep a recovery copy of your unsaved changes: {e}")),
            saver::Done::Recovery(Ok(())) | saver::Done::Cleared => {}
        }
    }

    /// Puts the whole project back as it was in a snapshot and saves it. The
    /// current version is backed up first, so this can be undone.
    pub fn restore_project(&mut self, snap: &circuit_core::backup::Snapshot) -> anyhow::Result<()> {
        self.finish_saving();
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
        // As many undo steps as Settings › Editor says.
        self.history.set_limit(self.prefs.undo_limit());
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

    /// Whether the open project's mode has slash commands (Discord's does).
    pub fn has_slash_commands(&self) -> bool {
        self.mode().is_some_and(|m| m.has("slashCommands"))
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

    /// The values texts on block `node` can use: see [`crate::editor::values`].
    pub fn values_for(&mut self, ws_key: &str, node: &str) -> crate::editor::values::Scope {
        let Editor { reg, engine, project, shapes, define_ctx, .. } = self;
        shapes.begin();
        let Some(ws) = project.as_ref().and_then(|p| p.workspaces.get(ws_key)) else { return Default::default() };
        crate::editor::values::available(ws, node, reg, &mut |id| node_shape(reg, engine, shapes, ws_key, ws, id, define_ctx))
    }

    /// Call after project data (such as slash commands) or Bot › Settings change.
    pub fn refresh_define_ctx(&mut self) {
        let data = self.project.as_ref().map(|p| p.meta.data.clone()).unwrap_or_default();
        let settings = self.project.as_ref().map(|p| p.meta.settings.clone()).unwrap_or_default();
        let forms = self.project.as_ref().map(|p| crate::editor::custom_ids::forms(p, &self.reg)).unwrap_or_default();
        self.define_ctx = serde_json::json!({ "data": data, "settings": settings, "forms": forms });
        self.shapes.clear();
    }

    /// After an edit: when a form (Show Modal's design) changed, blocks that
    /// read the forms (Modal Submitted's outputs) are worked out again.
    pub fn refresh_forms(&mut self) {
        let forms = self.project.as_ref().map(|p| crate::editor::custom_ids::forms(p, &self.reg)).unwrap_or_default();
        if self.define_ctx.get("forms") != Some(&forms) {
            self.refresh_define_ctx();
        }
    }

    /// Reads the bot's name and picture from Discord for the previews (when the
    /// project has a bot token).
    pub fn fetch_bot_profile(&mut self) {
        if let Some(p) = &self.project {
            self.bot_profile.fetch(&p.root, &p.meta.mode);
            self.servers.fetch(&p.root, &p.meta.mode);
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
        let mut placed = 0;
        for ws in project.workspaces.values_mut() {
            // Blocks that became action blocks (Info blocks were value blocks).
            let mut now_actions = Vec::new();
            for (id, node) in ws.nodes.iter_mut() {
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
                if def.kind == circuit_core::BlockKind::Action {
                    now_actions.push(id.clone());
                }
            }
            // They run in the path that reads their values now: right before
            // the first block there that does.
            let values: HashSet<String> =
                ws.nodes.iter().filter(|(_, n)| self.reg.block(&n.block).is_some_and(|d| d.kind == circuit_core::BlockKind::Value)).map(|(k, _)| k.clone()).collect();
            for id in now_actions {
                placed += ws.put_in_path(&id, |n| values.contains(n));
            }
        }
        if upgraded > 0 {
            self.edited = true;
            self.log(Level::Info, format!("Upgraded {upgraded} blocks to newer versions (save to keep)"));
        }
        if placed > 0 {
            self.log(Level::Info, tf!("Put {count} Info blocks into the paths that use them: they run as steps now (save to keep)", count = placed));
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
        let view = self.views.entry(ws_key.to_string()).or_insert_with(CanvasView::at_full_size);
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
            return crate::editor::history::PROJECT.to_string();
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
                crate::builders::slash::open(self);
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
        let notes: HashSet<String> = self.workspace(key).map(|w| w.notes.keys().cloned().collect()).unwrap_or_default();
        self.selected_notes.retain(|(w, n)| w != key || notes.contains(n));
        if self.selected.as_ref().is_some_and(|(w, n)| w == key && !alive.contains(n)) {
            self.clear_selection();
        }
    }

    /// Whether a shortcut is being recorded in Settings, so none should fire.
    pub fn recording_shortcut(&self, ctx: &eframe::egui::Context) -> bool {
        self.recording_frame.is_some_and(|f| f + 1 >= ctx.cumulative_pass_nr())
    }

    /// The selected notes of a workspace.
    pub fn note_selection(&self, ws_key: &str) -> Vec<String> {
        let mut ids: Vec<String> = self.selected_notes.iter().filter(|(w, _)| w == ws_key).map(|(_, n)| n.clone()).collect();
        ids.sort();
        ids
    }

    /// Copies the selected blocks and notes to the clipboard as JSON.
    pub fn copy(&self, ctx: &eframe::egui::Context) -> bool {
        let Some(key) = self.edit_target() else { return false };
        let ids = self.selection(&key);
        let notes = self.note_selection(&key);
        let Some(ws) = self.workspace(&key) else { return false };
        if ids.is_empty() && notes.is_empty() {
            return false;
        }
        let clip = ws.extract_with_notes(&ids, &notes);
        if let Ok(text) = serde_json::to_string_pretty(&clip) {
            ctx.copy_text(text);
        }
        true
    }

    pub fn delete_selection(&mut self) {
        let notes: Vec<(String, String)> = self.selected_notes.drain().collect();
        for (w, n) in &notes {
            if let Some(ws) = self.project.as_mut().and_then(|p| p.workspaces.get_mut(w))
                && ws.notes.shift_remove(n).is_some()
            {
                self.edited = true;
            }
        }
        if !notes.is_empty() && self.selected.is_none() {
            return;
        }
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
        if clip.format != CLIP_FORMAT || (clip.nodes.is_empty() && clip.notes.is_empty()) {
            return false;
        }
        let corners = clip.nodes.values().map(|n| n.at).chain(clip.notes.values().map(|n| [n.rect[0], n.rect[1]]));
        let min = corners.fold([f32::MAX, f32::MAX], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
        let offset = match at {
            Some(p) => [p.x - min[0], p.y - min[1]],
            None => [40.0, 40.0],
        };
        let Some(ws) = self.project.as_mut().and_then(|p| p.workspaces.get_mut(&key)) else { return false };
        let (ids, notes) = ws.paste_all(&clip, offset);
        self.edited = true;
        self.selected = ids.first().map(|f| (key.clone(), f.clone()));
        self.multi = ids.into_iter().collect();
        self.selected_notes = notes.into_iter().map(|n| (key.clone(), n)).collect();
        true
    }

    pub fn duplicate(&mut self) {
        let Some(key) = self.edit_target() else { return };
        let ids = self.selection(&key);
        let notes = self.note_selection(&key);
        let Some(ws) = self.workspace(&key) else { return };
        if let Ok(text) = serde_json::to_string(&ws.extract_with_notes(&ids, &notes)) {
            self.paste(&text, None);
        }
    }

    pub fn select_all(&mut self) {
        let Some(key) = self.edit_target() else { return };
        let ids: Vec<String> = self.workspace(&key).map(|w| w.nodes.keys().cloned().collect()).unwrap_or_default();
        let notes: Vec<String> = self.workspace(&key).map(|w| w.notes.keys().cloned().collect()).unwrap_or_default();
        self.selected_notes = notes.into_iter().map(|n| (key.clone(), n)).collect();
        if let Some(first) = ids.first() {
            self.selected = Some((key, first.clone()));
            self.multi = ids.into_iter().collect();
        }
    }
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
        // Digits only, so it reads the same in every language.
        Some(t) => t.format("%Y-%m-%d, %H:%M").to_string(),
        None => t!("an unknown time").into(),
    }
}

/// How long ago a moment (in ms since 1970) was: `just now`, `5 min ago`.
pub fn ago(ms: u64) -> String {
    crate::bot::ago(std::time::Duration::from_millis(circuit_core::backup::now_ms().saturating_sub(ms)))
}
