//! The window: menu bar, dockable panels, status bar.

use std::env;
use std::path::{Path, PathBuf};

use circuit_core::Project;
use eframe::egui::{self, Frame, Id, Margin, RichText, ViewportCommand, WidgetText};
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};
use serde::{Deserialize, Serialize};

use crate::bot::{BotState, BotUi};
use crate::editor::{Action, Editor, Level};
use crate::palette::{Cmd, Item};
use crate::{canvas, dialog, palette, panels, picker};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tab {
    Workspaces,
    Canvas(String),
    Inspector,
    Console,
    Problems,
    StoredData,
    /// A native plugin's panel: plugin id, panel id.
    Plugin(String, String),
}

impl Tab {
    const PANELS: [(Tab, &'static str); 5] = [
        (Tab::Workspaces, "Workspaces"),
        (Tab::Inspector, "Inspector"),
        (Tab::Console, "Console"),
        (Tab::Problems, "Problems"),
        (Tab::StoredData, "Stored Data"),
    ];
}

const KEY_DOCK: &str = "dock";
const KEY_THEME: &str = "theme";
const KEY_PROJECT: &str = "project";
const KEY_SYSTEM_TITLEBAR: &str = "system_titlebar";
const KEY_NATIVE_ALLOW: &str = "native_allow";
const KEY_NATIVE_TRUSTED: &str = "native_trusted";
const KEY_NATIVE_STORAGE: &str = "native_storage";

pub struct CircuitApp {
    ed: Editor,
    dock: DockState<Tab>,
    examples_dir: Option<PathBuf>,
    title: String,
    show_about: bool,
    confirm_close: bool,
    allow_close: bool,
    problem_count: usize,
    bot: BotUi,
    stored: crate::stored::StoredView,
    watcher: crate::watch::PluginWatcher,
    palette: Option<palette::Palette>,
    /// Use the operating system's title bar instead of ours.
    system_titlebar: bool,
    native: crate::native::NativeHost,
    /// The app's own plugins folder.
    bundled_plugins: Option<PathBuf>,
    show_plugins: bool,
}

impl CircuitApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let (plugin_dirs, bundled_root) = find_plugin_dirs();
        let theme_id: Option<String> = cc.storage.and_then(|s| eframe::get_value(s, KEY_THEME));
        let last_project: Option<PathBuf> = cc.storage.and_then(|s| eframe::get_value(s, KEY_PROJECT));
        let saved_dock: Option<DockState<Tab>> = cc.storage.and_then(|s| eframe::get_value(s, KEY_DOCK));
        let system_titlebar: bool = cc.storage.and_then(|s| eframe::get_value(s, KEY_SYSTEM_TITLEBAR)).unwrap_or(false);
        if system_titlebar {
            cc.egui_ctx.send_viewport_cmd(ViewportCommand::Decorations(true));
        }

        let mut ed = match Editor::new(plugin_dirs, theme_id.as_deref()) {
            Ok(ed) => ed,
            Err(e) => panic!("could not start the block sandbox: {e:#}"),
        };
        ed.theme.apply(&cc.egui_ctx);

        let examples_dir = bundled_root.as_ref().map(|r| r.join("examples"));
        let bundled_plugins = bundled_root.map(|r| r.join("plugins"));
        let project = last_project
            .filter(|p| p.join(circuit_core::project::PROJECT_FILE).is_file())
            .or_else(|| examples_dir.as_ref().map(|e| e.join("DemoBot")).filter(|p| p.is_dir()));
        if let Some(p) = project {
            ed.open_project(&p);
        }
        // The saved layout is used as-is; opening the project queued its first workspace.
        let dock = match saved_dock {
            Some(mut d) => {
                let keys: Vec<String> = ed.project.as_ref().map(|p| p.workspaces.keys().cloned().collect()).unwrap_or_default();
                d.retain_tabs(|t| !matches!(t, Tab::Canvas(k) if !keys.contains(k)));
                d
            }
            None => default_layout(),
        };

        let mut native = crate::native::NativeHost::new(
            cc.storage.and_then(|s| eframe::get_value(s, KEY_NATIVE_ALLOW)).unwrap_or(false),
            cc.storage.and_then(|s| eframe::get_value(s, KEY_NATIVE_TRUSTED)).unwrap_or_default(),
            cc.storage.and_then(|s| eframe::get_value(s, KEY_NATIVE_STORAGE)).unwrap_or_default(),
        );
        native.sync(&mut ed, bundled_plugins.as_deref());

        let repaint = cc.egui_ctx.clone();
        let mut watcher = crate::watch::PluginWatcher::new(move || repaint.request_repaint());
        watcher.watch(&ed.watch_dirs());
        CircuitApp {
            ed,
            dock,
            examples_dir,
            title: String::new(),
            show_about: false,
            confirm_close: false,
            allow_close: false,
            problem_count: 0,
            bot: BotUi::new(),
            stored: Default::default(),
            watcher,
            palette: None,
            system_titlebar,
            native,
            bundled_plugins,
            show_plugins: false,
        }
    }

    fn apply_actions(&mut self) {
        for action in std::mem::take(&mut self.ed.actions) {
            match action {
                Action::OpenWorkspace(key) => {
                    self.ed.active_workspace = Some(key.clone());
                    open_canvas(&mut self.dock, key);
                }
                Action::CloseWorkspaceTabs(key) => {
                    self.dock.retain_tabs(|t| *t != Tab::Canvas(key.clone()));
                    self.ed.views.remove(&key);
                    if self.ed.selected.as_ref().is_some_and(|(w, _)| *w == key) {
                        self.ed.selected = None;
                    }
                }
            }
        }
    }

    fn toggle_panel(&mut self, tab: Tab) {
        if self.dock.find_tab(&tab).is_some() {
            self.dock.retain_tabs(|t| *t != tab);
            return;
        }
        if matches!(tab, Tab::Plugin(..))
            && let Some(path) = self.dock.find_tab(&Tab::Inspector)
        {
            self.dock.set_focused_node_and_surface(path.node_path());
            self.dock.push_to_focused_leaf(tab);
            return;
        }
        let surface = self.dock.main_surface_mut();
        match tab {
            Tab::Workspaces => {
                surface.split_left(NodeIndex::root(), 0.17, vec![tab]);
            }
            Tab::Inspector | Tab::Plugin(..) => {
                surface.split_right(NodeIndex::root(), 0.78, vec![tab]);
            }
            _ => {
                surface.split_below(NodeIndex::root(), 0.75, vec![tab]);
            }
        }
    }

    fn open_project_dialog(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().set_title("Open a Circuit project folder").pick_folder() {
            self.switch_project(&dir);
        }
    }

    fn new_project_dialog(&mut self) {
        let Some(dir) = rfd::FileDialog::new().set_title("Choose an empty folder for the new project").pick_folder() else { return };
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "New Bot".into());
        let mode = self.ed.reg.modes.first().map(|m| m.id.clone()).unwrap_or_else(|| "discord-bot".into());
        match Project::create(&dir, &name, &mode) {
            Ok(_) => self.switch_project(&dir),
            Err(e) => self.ed.log(Level::Error, format!("Could not create the project: {e:#}")),
        }
    }

    fn switch_project(&mut self, dir: &Path) {
        if self.ed.dirty {
            self.ed.save();
        }
        self.bot.stop(&mut self.ed);
        self.dock.retain_tabs(|t| !matches!(t, Tab::Canvas(_)));
        self.ed.open_project(dir);
        self.native.sync(&mut self.ed, self.bundled_plugins.as_deref());
        self.watcher.watch(&self.ed.watch_dirs());
        self.stored = Default::default();
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.ed.theme.clone();
        let custom = !self.system_titlebar;
        let bar = ui.max_rect();
        if custom {
            crate::titlebar::drag_region(ui, bar);
        }
        // The search box sits in the middle, where the window title would be.
        let search = egui::Rect::from_center_size(bar.center(), egui::vec2(crate::titlebar::SEARCH_W, 24.0));
        let label = match &self.ed.project {
            Some(p) => format!("Search {}", p.meta.name),
            None => "Search".into(),
        };
        if crate::titlebar::search_box(ui, search, &theme, &label) {
            self.palette = Some(Default::default());
        }
        let row = |ui: &mut egui::Ui, layout| ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(layout));
        let mut left = row(ui, egui::Layout::left_to_right(egui::Align::Center));
        egui::MenuBar::new().ui(&mut left, |ui| {
            ui.add(egui::Image::new(egui::include_image!("../assets/icon.svg")).fit_to_exact_size(egui::vec2(18.0, 18.0)));
            ui.add_space(4.0);
            ui.menu_button("File", |ui| {
                if ui.button("New Project…").clicked() {
                    self.new_project_dialog();
                }
                if ui.button("Open Project…").clicked() {
                    self.open_project_dialog();
                }
                if let Some(examples) = self.examples_dir.clone()
                    && ui.button("Open Example: DemoBot").clicked()
                {
                    self.switch_project(&examples.join("DemoBot"));
                }
                ui.separator();
                if ui.add_enabled(self.ed.project.is_some(), egui::Button::new("Save").shortcut_text("Ctrl+S")).clicked() {
                    self.ed.save();
                    self.bot.restart_if_running(&mut self.ed, ui.ctx());
                }
                if ui.button("Reload Plugins").clicked() {
                    self.reload_plugins(ui.ctx());
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
            });
            ui.menu_button("Edit", |ui| {
                let key = self.ed.active_workspace.clone().unwrap_or_default();
                if ui.add_enabled(self.ed.history.can_undo(&key) || self.ed.dirty, egui::Button::new("Undo").shortcut_text("Ctrl+Z")).clicked() {
                    self.ed.undo();
                }
                if ui.add_enabled(self.ed.history.can_redo(&key), egui::Button::new("Redo").shortcut_text("Ctrl+Y")).clicked() {
                    self.ed.redo();
                }
                ui.separator();
                let has_sel = !self.ed.selection(&key).is_empty();
                if ui.add_enabled(has_sel, egui::Button::new("Cut").shortcut_text("Ctrl+X")).clicked() && self.ed.copy(ui.ctx()) {
                    self.ed.delete_selection();
                }
                if ui.add_enabled(has_sel, egui::Button::new("Copy").shortcut_text("Ctrl+C")).clicked() {
                    self.ed.copy(ui.ctx());
                }
                if ui.add(egui::Button::new("Paste").shortcut_text("Ctrl+V")).clicked()
                    && let Ok(text) = arboard::Clipboard::new().and_then(|mut c| c.get_text())
                {
                    self.ed.paste(&text, None);
                }
                if ui.add_enabled(has_sel, egui::Button::new("Duplicate").shortcut_text("Ctrl+D")).clicked() {
                    self.ed.duplicate();
                }
                if ui.add_enabled(has_sel, egui::Button::new("Delete").shortcut_text("Del")).clicked() {
                    self.ed.delete_selection();
                }
                ui.separator();
                if ui.add(egui::Button::new("Select all").shortcut_text("Ctrl+A")).clicked() {
                    self.ed.select_all();
                }
            });
            ui.menu_button("View", |ui| {
                ui.label(RichText::new("PANELS").size(10.5).color(theme.text_muted));
                for (tab, name) in Tab::PANELS {
                    let mut open = self.dock.find_tab(&tab).is_some();
                    if ui.checkbox(&mut open, name).clicked() {
                        self.toggle_panel(tab);
                    }
                }
                ui.separator();
                ui.label(RichText::new("THEME").size(10.5).color(theme.text_muted));
                let themes: Vec<(String, String)> = self.ed.reg.themes.iter().map(|t| (t.id.clone(), t.name.clone())).collect();
                for (id, name) in themes {
                    if ui.radio(self.ed.theme.id == id, name).clicked() {
                        self.ed.set_theme(&id);
                        self.ed.theme.apply(ui.ctx());
                    }
                }
                ui.separator();
                if ui.checkbox(&mut self.system_titlebar, "Use system title bar").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Decorations(self.system_titlebar));
                }
                if ui.add(egui::Button::new("Command Palette…").shortcut_text("Ctrl+P")).clicked() {
                    self.palette = Some(Default::default());
                }
                if ui.button("Reset layout").clicked() {
                    self.reset_layout();
                }
            });
            let has_project = self.ed.project.is_some();
            ui.menu_button("Bot", |ui| {
                let running = self.bot.state != BotState::Stopped;
                if ui.add_enabled(has_project, egui::Button::new(if running { "Restart" } else { "Run" }).shortcut_text("F5")).clicked() {
                    self.bot.start(&mut self.ed, ui.ctx());
                }
                if ui.add_enabled(running, egui::Button::new("Stop").shortcut_text("Shift+F5")).clicked() {
                    self.bot.stop(&mut self.ed);
                }
                ui.separator();
                if ui.add_enabled(has_project, egui::Button::new("Slash Commands…")).clicked() {
                    crate::slash::open(&mut self.ed);
                }
                if ui.add_enabled(has_project, egui::Button::new("Settings…")).clicked() {
                    self.bot.open_settings(&self.ed);
                }
                if ui.add_enabled(has_project, egui::Button::new("Export…")).clicked() {
                    self.bot.open_export(&self.ed);
                }
            });
            ui.menu_button("Plugins", |ui| {
                let mut any = false;
                for i in 0..self.native.plugins.len() {
                    let p = &self.native.plugins[i];
                    if p.commands().is_empty() && p.panels().is_empty() {
                        continue;
                    }
                    any = true;
                    ui.label(RichText::new(p.name.to_uppercase()).size(10.5).color(theme.text_muted));
                    let (id, commands, panels) = (p.id.clone(), p.commands().to_vec(), p.panels().to_vec());
                    for c in commands {
                        let mut b = ui.button(&c.title);
                        if !c.description.is_empty() {
                            b = b.on_hover_text(&c.description);
                        }
                        if b.clicked() {
                            self.native.queued.push(c.id.clone());
                        }
                    }
                    for panel in panels {
                        let tab = Tab::Plugin(id.clone(), panel.id.clone());
                        let mut open = self.dock.find_tab(&tab).is_some();
                        if ui.checkbox(&mut open, format!("{} panel", panel.title)).clicked() {
                            self.toggle_panel(tab);
                        }
                    }
                    ui.separator();
                }
                if !any {
                    ui.label(RichText::new("No native plugins are running").color(theme.text_muted));
                    ui.separator();
                }
                if ui.button("Manage Plugins…").clicked() {
                    self.show_plugins = true;
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About XC™ Circuit").clicked() {
                    self.show_about = true;
                }
            });
        });

        // Window buttons and Run / Stop on the right.
        let ui = &mut row(ui, egui::Layout::right_to_left(egui::Align::Center));
        if custom {
            ui.spacing_mut().item_spacing.x = 0.0;
            crate::titlebar::window_buttons(ui, &theme);
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.add_space(10.0);
        }
        let running = self.bot.state != BotState::Stopped;
        if running && ui.button("Stop").on_hover_text("Stop the bot (Shift+F5)").clicked() {
            self.bot.stop(&mut self.ed);
        }
        let label = match self.bot.state {
            BotState::Stopped => "Run",
            BotState::Starting => "Starting…",
            BotState::Running => "Restart",
        };
        let run = egui::Button::new(RichText::new(label).strong().color(egui::Color32::from_rgb(4, 33, 13))).fill(theme.action);
        if ui.add_enabled(self.ed.project.is_some(), run).on_hover_text("Run the bot (F5)").clicked() {
            self.bot.start(&mut self.ed, ui.ctx());
        }
    }

    fn reload_plugins(&mut self, ctx: &egui::Context) {
        self.ed.reload_plugins();
        self.ed.refresh_define_ctx();
        self.ed.theme.apply(ctx);
        self.native.sync(&mut self.ed, self.bundled_plugins.as_deref());
    }

    /// Bot events, checks and queued commands for native plugins.
    fn run_native(&mut self, ctx: &egui::Context) {
        let running = self.bot.state != BotState::Stopped;
        for ev in std::mem::take(&mut self.bot.events) {
            self.native.bot_event(&mut self.ed, &ev, running);
        }
        self.native.tick(&mut self.ed, running);
        for id in std::mem::take(&mut self.native.queued) {
            match id.as_str() {
                "bot.run" => self.run_command(Cmd::Run, ctx),
                "bot.stop" => self.run_command(Cmd::Stop, ctx),
                "file.save" => self.run_command(Cmd::Save, ctx),
                _ => {
                    if !self.native.run_command(&mut self.ed, &id, running) {
                        self.ed.log(Level::Warn, format!("No command `{id}`"));
                    }
                }
            }
        }
        if self.native.plugins.iter().any(|p| p.status == crate::native::Status::Loaded) {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
    }

    fn reset_layout(&mut self) {
        let canvases: Vec<Tab> = self.dock.iter_all_tabs().map(|(_, t)| t.clone()).filter(|t| matches!(t, Tab::Canvas(_))).collect();
        self.dock = default_layout();
        for t in canvases {
            if let Tab::Canvas(k) = t {
                open_canvas(&mut self.dock, k);
            }
        }
    }

    /// Everything the command palette offers right now.
    fn palette_items(&self) -> Vec<Item> {
        let has_project = self.ed.project.is_some();
        let running = self.bot.state != BotState::Stopped;
        let key = self.ed.active_workspace.clone().unwrap_or_default();
        let has_sel = !self.ed.selection(&key).is_empty();
        let mut items = Vec::new();
        if has_project {
            items.push(Item::new("Bot", if running { "Restart bot" } else { "Run bot" }, Cmd::Run).shortcut("F5"));
        }
        if running {
            items.push(Item::new("Bot", "Stop bot", Cmd::Stop).shortcut("Shift+F5"));
        }
        if has_project {
            items.push(Item::new("Bot", "Slash Commands…", Cmd::SlashCommands));
            items.push(Item::new("Bot", "Bot Settings…", Cmd::Settings));
            items.push(Item::new("Bot", "Export…", Cmd::Export));
            items.push(Item::new("File", "Save", Cmd::Save).shortcut("Ctrl+S"));
        }
        items.push(Item::new("File", "New Project…", Cmd::NewProject));
        items.push(Item::new("File", "Open Project…", Cmd::OpenProject));
        if self.examples_dir.is_some() {
            items.push(Item::new("File", "Open Example: DemoBot", Cmd::OpenExample));
        }
        items.push(Item::new("File", "Reload Plugins", Cmd::ReloadPlugins));
        if self.ed.history.can_undo(&key) {
            items.push(Item::new("Edit", "Undo", Cmd::Undo).shortcut("Ctrl+Z"));
        }
        if self.ed.history.can_redo(&key) {
            items.push(Item::new("Edit", "Redo", Cmd::Redo).shortcut("Ctrl+Y"));
        }
        if has_sel {
            items.push(Item::new("Edit", "Duplicate", Cmd::Duplicate).shortcut("Ctrl+D"));
            items.push(Item::new("Edit", "Delete", Cmd::Delete).shortcut("Del"));
        }
        if has_project {
            items.push(Item::new("Edit", "Select all", Cmd::SelectAll).shortcut("Ctrl+A"));
        }
        for (tab, name) in Tab::PANELS {
            let verb = if self.dock.find_tab(&tab).is_some() { "Close" } else { "Show" };
            items.push(Item::new("View", format!("{verb} {name} panel"), Cmd::TogglePanel(tab)));
        }
        for t in &self.ed.reg.themes {
            if t.id != self.ed.theme.id {
                items.push(Item::new("Theme", format!("Use {} theme", t.name), Cmd::Theme(t.id.clone())));
            }
        }
        items.push(Item::new("View", "Reset layout", Cmd::ResetLayout));
        items.push(Item::new("Help", "About XC™ Circuit", Cmd::About));
        items.push(Item::new("Plugins", "Manage Plugins…", Cmd::ManagePlugins));
        for p in &self.native.plugins {
            for c in p.commands() {
                items.push(Item::new("Plugin", c.title.clone(), Cmd::PluginCommand(c.id.clone())).detail(p.name.clone()));
            }
            for panel in p.panels() {
                let tab = Tab::Plugin(p.id.clone(), panel.id.clone());
                let verb = if self.dock.find_tab(&tab).is_some() { "Close" } else { "Show" };
                items.push(Item::new("Plugin", format!("{verb} {} panel", panel.title), Cmd::TogglePluginPanel(p.id.clone(), panel.id.clone())).detail(p.name.clone()));
            }
        }
        if let Some(p) = &self.ed.project {
            for (wk, ws) in &p.workspaces {
                items.push(Item::new("Workspace", ws.name.clone(), Cmd::OpenWorkspace(wk.clone())));
            }
            for (wk, ws) in &p.workspaces {
                for (id, node) in &ws.nodes {
                    let block = node.block.split('@').next().unwrap_or(&node.block);
                    let name = self.ed.reg.block(block).map(|d| d.name.clone()).unwrap_or_else(|| block.to_string());
                    items.push(Item::new("Block", name, Cmd::RevealBlock(wk.clone(), id.clone())).detail(format!("{} · #{id}", ws.name)));
                }
            }
        }
        items
    }

    fn run_command(&mut self, cmd: Cmd, ctx: &egui::Context) {
        match cmd {
            Cmd::NewProject => self.new_project_dialog(),
            Cmd::OpenProject => self.open_project_dialog(),
            Cmd::OpenExample => {
                if let Some(examples) = self.examples_dir.clone() {
                    self.switch_project(&examples.join("DemoBot"));
                }
            }
            Cmd::Save => {
                self.ed.save();
                self.bot.restart_if_running(&mut self.ed, ctx);
            }
            Cmd::ReloadPlugins => self.reload_plugins(ctx),
            Cmd::Undo => self.ed.undo(),
            Cmd::Redo => self.ed.redo(),
            Cmd::Duplicate => self.ed.duplicate(),
            Cmd::Delete => self.ed.delete_selection(),
            Cmd::SelectAll => self.ed.select_all(),
            Cmd::TogglePanel(tab) => self.toggle_panel(tab),
            Cmd::Theme(id) => {
                self.ed.set_theme(&id);
                self.ed.theme.apply(ctx);
            }
            Cmd::ResetLayout => self.reset_layout(),
            Cmd::Run => self.bot.start(&mut self.ed, ctx),
            Cmd::Stop => self.bot.stop(&mut self.ed),
            Cmd::SlashCommands => crate::slash::open(&mut self.ed),
            Cmd::Settings => self.bot.open_settings(&self.ed),
            Cmd::Export => self.bot.open_export(&self.ed),
            Cmd::About => self.show_about = true,
            Cmd::ManagePlugins => self.show_plugins = true,
            Cmd::PluginCommand(id) => self.native.queued.push(id),
            Cmd::TogglePluginPanel(plugin, panel) => self.toggle_panel(Tab::Plugin(plugin, panel)),
            Cmd::OpenWorkspace(key) => self.ed.actions.push(Action::OpenWorkspace(key)),
            Cmd::RevealBlock(ws, id) => self.ed.reveal(&ws, &id),
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let t = self.ed.theme.clone();
        let ctx = ui.ctx().clone();
        ui.spacing_mut().item_spacing.x = 2.0;
        let bar = ui.max_rect();
        let (dot, state) = match self.bot.state {
            BotState::Stopped => (t.status_text.gamma_multiply(0.45), "Bot stopped"),
            BotState::Starting => (egui::Color32::from_rgb(0xFF, 0xB4, 0x54), "Bot starting"),
            BotState::Running => (t.action, "Bot running"),
        };
        let has_project = self.ed.project.is_some();
        let mut left = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let running = self.bot.state != BotState::Stopped;
        let tip = if running { "Stop the bot (Shift+F5)" } else { "Run the bot (F5)" };
        if status_item(&mut left, &t, Some(dot), state).on_hover_text(tip).clicked() && has_project {
            if running { self.bot.stop(&mut self.ed) } else { self.bot.start(&mut self.ed, &ctx) }
        }
        match self.ed.project.as_ref().map(|p| (p.meta.name.clone(), p.meta.mode.clone())) {
            Some((name, mode_id)) => {
                if status_item(&mut left, &t, None, &name).on_hover_text("Bot settings").clicked() {
                    self.bot.open_settings(&self.ed);
                }
                let mode = self.ed.reg.mode(&mode_id).map(|m| (m.name.clone(), m.description.clone()));
                let (mode_name, about) = mode.unwrap_or((mode_id, String::new()));
                let r = status_item(&mut left, &t, None, &mode_name);
                if !about.is_empty() {
                    r.on_hover_text(about);
                }
            }
            None => {
                if status_item(&mut left, &t, None, "No project").on_hover_text("Open a project").clicked() {
                    self.open_project_dialog();
                }
            }
        }
        let counts = format!("{} plugins · {} blocks", self.ed.reg.plugins.len(), self.ed.reg.blocks.len());
        if status_item(&mut left, &t, None, &counts).on_hover_text("Manage plugins").clicked() {
            self.show_plugins = true;
        }
        if self.problem_count > 0 {
            let label = format!("⚠ {} problem{}", self.problem_count, if self.problem_count == 1 { "" } else { "s" });
            if status_item(&mut left, &t, None, &label).on_hover_text("Show the Problems panel").clicked() {
                match self.dock.find_tab(&Tab::Problems) {
                    Some(path) => {
                        let _ = self.dock.set_active_tab(path);
                    }
                    None => self.toggle_panel(Tab::Problems),
                }
            }
        }

        let mut right = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(egui::Layout::right_to_left(egui::Align::Center)));
        if status_item(&mut right, &t, None, &format!("Theme: {}", t.name)).on_hover_text("Switch to the next theme").clicked() {
            let ids: Vec<String> = self.ed.reg.themes.iter().map(|x| x.id.clone()).collect();
            if let Some(next) = ids.iter().position(|id| *id == t.id).map(|i| ids[(i + 1) % ids.len()].clone()) {
                self.ed.set_theme(&next);
                self.ed.theme.apply(&ctx);
            }
        }
        let saved = if self.ed.dirty { "Unsaved changes" } else { "Saved" };
        if status_item(&mut right, &t, None, saved).on_hover_text("Save (Ctrl+S)").clicked() && has_project {
            self.ed.save();
            self.bot.restart_if_running(&mut self.ed, &ctx);
        }
    }

    /// Undo, clipboard and selection shortcuts, unless a text box has focus.
    fn edit_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.text_edit_focused() || self.ed.picker.is_some() {
            return;
        }
        use egui::{Key, Modifiers};
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        if ctx.input_mut(|i| i.consume_key(cmd_shift, Key::Z) || i.consume_key(cmd, Key::Y)) {
            self.ed.redo();
        } else if ctx.input_mut(|i| i.consume_key(cmd, Key::Z)) {
            self.ed.undo();
        }
        if ctx.input_mut(|i| i.consume_key(cmd, Key::D)) {
            self.ed.duplicate();
        }
        if ctx.input_mut(|i| i.consume_key(cmd, Key::A)) {
            self.ed.select_all();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete)) {
            self.ed.delete_selection();
        }
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Copy => {
                    self.ed.copy(ctx);
                }
                egui::Event::Cut => {
                    if self.ed.copy(ctx) {
                        self.ed.delete_selection();
                    }
                }
                egui::Event::Paste(text) => {
                    let at = self.ed.active_workspace.as_ref().and_then(|k| self.ed.views.get(k)).and_then(|v| v.pointer);
                    self.ed.paste(&text, at);
                }
                _ => {}
            }
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let theme = self.ed.theme.clone();
        if self.show_about {
            let info = format!(
                "{} plugins · {} blocks · {} themes loaded",
                self.ed.reg.plugins.len(),
                self.ed.reg.blocks.len(),
                self.ed.reg.themes.len()
            );
            let mut ok = false;
            let close = dialog::show(
                ctx,
                &theme,
                dialog::Spec { id: "about", title: "About XC™ Circuit", subtitle: None, icon: None, width: 420.0 },
                |ui, part| match part {
                    dialog::Part::Body => {
                        ui.vertical_centered(|ui| {
                            ui.add_space(6.0);
                            ui.add(egui::Image::new(egui::include_image!("../assets/icon.svg")).fit_to_exact_size(egui::vec2(76.0, 76.0)));
                            ui.add_space(10.0);
                            ui.label(RichText::new("XC™ Circuit").size(22.0).strong().color(theme.text));
                            ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).size(12.5).color(theme.text_muted));
                            ui.add_space(10.0);
                            ui.label(RichText::new("Build bots by connecting blocks.").size(13.5).color(theme.text));
                            ui.add_space(4.0);
                            ui.label(RichText::new(&info).size(11.5).color(theme.text_muted));
                            ui.add_space(6.0);
                        });
                    }
                    dialog::Part::Footer => ok = dialog::primary_button(ui, &theme, "Close").clicked(),
                },
            );
            if ok || close {
                self.show_about = false;
            }
        }
        if self.confirm_close {
            let name = self.ed.project.as_ref().map(|p| p.meta.name.clone()).unwrap_or_default();
            let title = format!("Save changes to {name}?");
            let icon = self.ed.reg.icons.get("alert").cloned();
            let (mut save, mut discard, mut cancel) = (false, false, false);
            let close = dialog::show(
                ctx,
                &theme,
                dialog::Spec { id: "confirm-close", title: &title, subtitle: None, icon: icon.as_deref(), width: 460.0 },
                |ui, part| match part {
                    dialog::Part::Body => {
                        ui.label(RichText::new("Your changes are lost if you close without saving.").size(13.5).color(theme.text));
                        if self.bot.state != BotState::Stopped {
                            ui.add_space(4.0);
                            ui.label(RichText::new("The running bot will be stopped.").size(12.0).color(theme.text_muted));
                        }
                    }
                    dialog::Part::Footer => {
                        save = dialog::primary_button(ui, &theme, "Save and close").clicked();
                        discard = dialog::secondary_button(ui, &theme, "Don't save").clicked();
                        cancel = dialog::secondary_button(ui, &theme, "Cancel").clicked();
                    }
                },
            );
            if save || discard {
                if save {
                    self.ed.save();
                }
                self.allow_close = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            } else if cancel || close {
                self.confirm_close = false;
            }
        }
    }
}

impl eframe::App for CircuitApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) && self.ed.dirty && !self.allow_close {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.confirm_close = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.ed.save();
            self.bot.restart_if_running(&mut self.ed, &ctx);
        }
        if self.ed.project.is_some() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
            self.bot.start(&mut self.ed, &ctx);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::F5)) {
            self.bot.stop(&mut self.ed);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::P) || i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::P)) {
            self.palette = if self.palette.is_some() { None } else { Some(Default::default()) };
            self.ed.picker = None;
        }
        self.bot.poll(&mut self.ed);
        self.run_native(&ctx);
        if self.watcher.poll() {
            self.ed.log(Level::Info, "Plugin files changed, reloading");
            self.reload_plugins(&ctx);
        } else if self.watcher.waiting() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.edit_shortcuts(&ctx);
        if !self.ed.activity.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
            self.ed.activity.retain(|_, a| a.error.is_some() || a.at.elapsed().as_secs_f32() < 1.2);
        }

        let title = match &self.ed.project {
            Some(p) => format!("{}{} — XC™ Circuit", if self.ed.dirty { "● " } else { "" }, p.meta.name),
            None => "XC™ Circuit".into(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        let theme = self.ed.theme.clone();
        let (margin, height) = if self.system_titlebar { (Margin::symmetric(8, 4), None) } else { (Margin { left: 8, right: 0, top: 0, bottom: 0 }, Some(crate::titlebar::HEIGHT)) };
        let mut top = egui::Panel::top("menu").frame(Frame::new().fill(theme.frame).inner_margin(margin));
        if let Some(h) = height {
            top = top.exact_size(h);
        }
        top.show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status")
            .exact_size(24.0)
            .frame(Frame::new().fill(theme.status_bar).inner_margin(Margin::symmetric(4, 0)))
            .show(ui, |ui| self.status_bar(ui));
        let mut problems = panels::collect_problems(&self.ed);
        problems.extend(self.native.problems());
        self.problem_count = problems.len();
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let style = theme.dock_style(ui.style());
            let running = self.bot.state != BotState::Stopped;
            let mut viewer = Viewer { ed: &mut self.ed, problem_count: self.problem_count, problems, stored: &mut self.stored, running, native: &mut self.native };
            DockArea::new(&mut self.dock)
                .style(style)
                .show_add_buttons(false)
                .show_leaf_close_all_buttons(false)
                .show_leaf_collapse_buttons(false)
                .show_inside(ui, &mut viewer);
        });

        picker::show(&ctx, &mut self.ed);
        if let Some(mut state) = self.palette.take() {
            let items = self.palette_items();
            let (cmd, close) = palette::show(&ctx, &theme, &mut state, &items);
            if !close {
                self.palette = Some(state);
            }
            if let Some(cmd) = cmd {
                self.run_command(cmd, &ctx);
            }
        }
        self.apply_actions();
        self.dialogs(&ctx);
        self.bot.windows(&ctx, &mut self.ed);
        crate::embed::window(&ctx, &mut self.ed);
        crate::slash::window(&ctx, &mut self.ed);
        if self.show_plugins {
            use crate::plugin_manager::Ask;
            match crate::plugin_manager::window(&ctx, &theme, &self.ed, &self.native) {
                Some(Ask::SetAllow(on)) => {
                    self.native.allow = on;
                    self.native.sync(&mut self.ed, self.bundled_plugins.as_deref());
                }
                Some(Ask::Trust(id)) => self.native.trust(&id, &mut self.ed),
                Some(Ask::Close) => self.show_plugins = false,
                None => {}
            }
        }
        if !self.system_titlebar {
            crate::titlebar::frame_edges(&ctx, &theme);
        }

        // Record an undo step once an edit has settled.
        let settled = !ctx.input(|i| i.pointer.any_down()) && !ctx.text_edit_focused();
        if let Some(p) = &self.ed.project {
            self.ed.history.track(p, settled);
        }
    }

    fn on_exit(&mut self) {
        self.bot.stop(&mut self.ed);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, KEY_DOCK, &self.dock);
        eframe::set_value(storage, KEY_THEME, &self.ed.theme.id);
        eframe::set_value(storage, KEY_SYSTEM_TITLEBAR, &self.system_titlebar);
        eframe::set_value(storage, KEY_NATIVE_ALLOW, &self.native.allow);
        eframe::set_value(storage, KEY_NATIVE_TRUSTED, &self.native.trusted);
        eframe::set_value(storage, KEY_NATIVE_STORAGE, &self.native.storage);
        if let Some(p) = &self.ed.project {
            eframe::set_value(storage, KEY_PROJECT, &p.root);
        }
    }
}

struct Viewer<'a> {
    ed: &'a mut Editor,
    stored: &'a mut crate::stored::StoredView,
    running: bool,
    problem_count: usize,
    problems: Vec<panels::ProblemItem>,
    native: &'a mut crate::native::NativeHost,
}

impl TabViewer for Viewer<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> Id {
        Id::new(("tab", &*tab))
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        match tab {
            Tab::Workspaces => "Workspaces".into(),
            Tab::Inspector => "Inspector".into(),
            Tab::Console => "Console".into(),
            Tab::Problems => format!("Problems  {}", self.problem_count).into(),
            Tab::StoredData => "Stored Data".into(),
            Tab::Plugin(plugin, panel) => self.native.panel_title(plugin, panel).unwrap_or_else(|| panel.clone()).into(),
            Tab::Canvas(key) => self.ed.workspace(key).map(|w| w.name.clone()).unwrap_or_else(|| key.clone()).into(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match tab {
            Tab::Workspaces => panels::workspaces(ui, self.ed),
            Tab::Inspector => panels::inspector(ui, self.ed),
            Tab::Console => panels::console(ui, self.ed),
            Tab::Problems => panels::problems(ui, self.ed, std::mem::take(&mut self.problems)),
            Tab::StoredData => crate::stored::panel(ui, self.ed, self.stored, self.running),
            Tab::Plugin(plugin, panel) => {
                let (plugin, panel) = (plugin.clone(), panel.clone());
                self.native.panel_ui(ui, self.ed, &plugin, &panel, self.running);
            }
            Tab::Canvas(key) => {
                let key = key.clone();
                canvas::show(ui, self.ed, &key);
            }
        }
    }

    fn scroll_bars(&self, _tab: &Tab) -> [bool; 2] {
        [false, false]
    }
}

fn default_layout() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Console, Tab::Problems, Tab::StoredData]);
    let surface = dock.main_surface_mut();
    // Root starts as the bottom panel; the top area holds Workspaces | (canvases) | Inspector.
    // A split's fraction is the share of its left/top child.
    let [_bottom, top] = surface.split_above(NodeIndex::root(), 0.76, vec![Tab::Inspector]);
    surface.split_left(top, 0.17, vec![Tab::Workspaces]);
    dock
}

/// Focuses a workspace's canvas tab, or opens it next to the other canvases.
fn open_canvas(dock: &mut DockState<Tab>, key: String) {
    let tab = Tab::Canvas(key);
    if let Some(path) = dock.find_tab(&tab) {
        let _ = dock.set_active_tab(path);
        dock.set_focused_node_and_surface(path.node_path());
        return;
    }
    let target = dock
        .find_tab_from(|t| matches!(t, Tab::Canvas(_)))
        .map(|p| p.node_path())
        .or_else(|| dock.find_tab(&Tab::Inspector).map(|p| p.node_path()));
    match target {
        Some(np) if dock.find_tab_from(|t| matches!(t, Tab::Canvas(_))).is_some() => {
            if let Ok(leaf) = dock.leaf_mut(np) {
                leaf.append_tab(tab.clone());
            }
        }
        Some(np) => {
            // No canvas yet: split the area left of the Inspector.
            let surface = dock.main_surface_mut();
            surface.split_left(np.node, 0.78, vec![tab.clone()]);
        }
        None => dock.push_to_first_leaf(tab.clone()),
    }
    if let Some(path) = dock.find_tab(&tab) {
        let _ = dock.set_active_tab(path);
    }
}

/// Plugin folders: `XC_CIRCUIT_PLUGINS`, the bundled `plugins/` next to the program
/// (or in a parent folder while developing), and the user's plugin folder.
/// Also returns the folder that holds the bundled `plugins/`.
fn find_plugin_dirs() -> (Vec<PathBuf>, Option<PathBuf>) {
    let mut dirs = Vec::new();
    if let Ok(list) = env::var("XC_CIRCUIT_PLUGINS") {
        dirs.extend(env::split_paths(&list));
    }
    let starts = [env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)), env::current_dir().ok()];
    let bundled_root = starts
        .into_iter()
        .flatten()
        .find_map(|start| start.ancestors().find(|a| a.join("plugins").join("xc.core").join("plugin.toml").is_file()).map(Path::to_path_buf));
    if let Some(root) = &bundled_root {
        dirs.push(root.join("plugins"));
    }
    if let Some(data) = dirs::data_dir() {
        let name = if cfg!(windows) { "XC Circuit" } else { "xc-circuit" };
        dirs.push(data.join(name).join("plugins"));
    }
    dirs.dedup();
    (dirs, bundled_root)
}

/// A status bar entry: highlights on hover, can be clicked, and its text
/// can't be selected.
fn status_item(ui: &mut egui::Ui, theme: &crate::theme::Theme, dot: Option<egui::Color32>, text: &str) -> egui::Response {
    let font = egui::FontId::proportional(11.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, theme.status_text);
    let dot_w = if dot.is_some() { 14.0 } else { 0.0 };
    let size = egui::vec2(galley.size().x + dot_w + 16.0, ui.available_height());
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect.shrink2(egui::vec2(0.0, 2.0)), 4.0, crate::theme::mix(theme.status_bar, theme.status_text, 0.14));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let mut x = rect.left() + 8.0;
    if let Some(c) = dot {
        ui.painter().circle_filled(egui::pos2(x + 4.0, rect.center().y), 4.0, c);
        x += dot_w;
    }
    ui.painter().galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, theme.status_text);
    resp
}
