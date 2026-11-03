//! The window: menu bar, dockable panels, status bar.

use crate::prefs::SideView;
use std::env;
use std::path::{Path, PathBuf};

use circuit_core::Project;
use eframe::egui::{self, Frame, Id, Margin, RichText, ViewportCommand, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockArea, DockState, Node, NodeIndex, TabViewer, Tree};
use serde::{Deserialize, Serialize};

use crate::bot::{BotState, BotUi};
use crate::i18n::n;
use crate::editor::{Action, Editor, Level};
use crate::palette::{Cmd, Item};
use crate::{canvas, dialog, palette, panels, picker};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tab {
    Canvas(String),
    Inspector,
    Console,
    Problems,
    /// The Settings page.
    Settings,
    /// Backups and restoring.
    Backups,
    /// A native plugin's panel: plugin id, panel id.
    Plugin(String, String),
    /// Editors that used to be dialogs. Each is open while its draft exists.
    EmbedBuilder,
    LayoutBuilder,
    SlashCommands,
    BotSettings,
    Export,
    Plugins,
    WorkspaceExport,
    WorkspaceImport,
}

/// Where a tab belongs in the window.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Right,
    Bottom,
    Center,
}

impl Tab {
    fn side(&self) -> Side {
        match self {
            Tab::Inspector | Tab::Plugin(..) => Side::Right,
            Tab::Console | Tab::Problems => Side::Bottom,
            _ => Side::Center,
        }
    }

    /// The editors that open in their own floating window.
    fn floats(&self) -> bool {
        matches!(self, Tab::EmbedBuilder | Tab::LayoutBuilder | Tab::SlashCommands | Tab::BotSettings | Tab::Export | Tab::Plugins | Tab::WorkspaceExport | Tab::WorkspaceImport)
    }

    const PANELS: [(Tab, &'static str); 3] = [(Tab::Inspector, n("Inspector")), (Tab::Console, n("Console")), (Tab::Problems, n("Problems"))];
}

const KEY_DOCK: &str = "dock";
const KEY_THEME: &str = "theme";
const KEY_PROJECT: &str = "project";
const KEY_SYSTEM_TITLEBAR: &str = "system_titlebar";
const KEY_NATIVE_ALLOW: &str = "native_allow";
const KEY_ACTIVITY_BAR: &str = "activity_bar";
const KEY_PREFS: &str = "prefs";
const KEY_NATIVE_TRUSTED: &str = "native_trusted";
const KEY_NATIVE_STORAGE: &str = "native_storage";

pub struct CircuitApp {
    ed: Editor,
    dock: DockState<Tab>,
    examples_dir: Option<PathBuf>,
    title: String,
    show_about: bool,
    /// File › New Project: the starter to begin from (0 = empty), while choosing.
    new_project: Option<crate::new_project::NewProject>,
    confirm_close: bool,
    allow_close: bool,
    problem_count: usize,
    /// The project's problems and what they were found for (project revision,
    /// plugin reloads, language): found again only when one changes.
    problems_found: Option<((u64, u64, String), Vec<panels::ProblemItem>)>,
    /// The dock's size last frame, so side panels keep theirs when it changes.
    dock_size: Option<egui::Vec2>,
    bot: BotUi,
    stored: crate::stored::StoredView,
    watcher: crate::watch::PluginWatcher,
    palette: Option<palette::Palette>,
    /// Use the operating system's title bar instead of ours.
    native: crate::native::NativeHost,
    /// The app's own plugins folder.
    bundled_plugins: Option<PathBuf>,
    /// The Plugins tab's unsaved changes, while it is open.
    plugins: Option<crate::plugin_manager::Draft>,
    settings: crate::settings::SettingsView,
    restore: crate::restore::RestoreView,
    /// When unsaved changes were last copied for crash recovery.
    last_recovery: std::time::Instant,
    /// When changes were last looked for.
    last_check: std::time::Instant,
    /// Shows the window once it has drawn (no white flash at startup).
    reveal: crate::reveal::Reveal,
    /// With XC_FRAME_STATS set: frames drawn since the last report, and when it was.
    frame_stats: Option<(u32, std::time::Instant)>,
    /// The First Start page, while it is open.
    setup: Option<crate::setup::Setup>,
    /// The guided tour, while it runs, and the layout to put back afterwards.
    tour: Option<crate::tour::Tour>,
    dock_before_tour: Option<DockState<Tab>>,
}

impl CircuitApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let (plugin_dirs, bundled_root) = find_plugin_dirs();
        let theme_id: Option<String> = cc.storage.and_then(|s| eframe::get_value(s, KEY_THEME));
        let last_project: Option<PathBuf> = cc.storage.and_then(|s| eframe::get_value(s, KEY_PROJECT));
        let saved_dock: Option<DockState<Tab>> = cc.storage.and_then(|s| eframe::get_value(s, KEY_DOCK));
        let prefs: crate::prefs::Prefs = cc.storage.and_then(|s| eframe::get_value(s, KEY_PREFS)).unwrap_or_else(|| crate::prefs::Prefs {
            // Settings saved before there was a Settings page.
            system_titlebar: cc.storage.and_then(|s| eframe::get_value(s, KEY_SYSTEM_TITLEBAR)).unwrap_or(false),
            activity_bar: cc.storage.and_then(|s| eframe::get_value(s, KEY_ACTIVITY_BAR)).unwrap_or(true),
            ..Default::default()
        });
        if prefs.system_titlebar {
            cc.egui_ctx.send_viewport_cmd(ViewportCommand::Decorations(true));
        }
        cc.egui_ctx.set_zoom_factor(prefs.ui_scale);

        let mut ed = match Editor::new(plugin_dirs, theme_id.as_deref()) {
            Ok(ed) => ed,
            Err(e) => panic!("could not start the block sandbox: {e:#}"),
        };
        ed.prefs = prefs;
        ed.apply_language();
        ed.prefs.apply_node();
        ed.restyle();
        ed.theme.apply(&cc.egui_ctx);
        // What draws the app, and whether it had to fall back.
        let (graphics, software) = crate::graphics::describe(cc);
        if let Ok(why) = std::env::var(crate::graphics::REASON_VAR) {
            ed.log(Level::Warn, format!("{why}; drawing with {graphics} instead"));
        }
        ed.log(Level::Info, format!("Graphics: {graphics}"));
        eprintln!("graphics: {graphics}");
        if software {
            ed.log(Level::Warn, "No graphics card driver could be used, so the app is drawn in software, which is slower.");
        }

        let examples_dir = bundled_root.as_ref().map(|r| r.join("examples"));
        let bundled_plugins = bundled_root.map(|r| r.join("plugins"));
        let project = last_project
            .filter(|p| ed.prefs.reopen_last_project && p.join(circuit_core::project::PROJECT_FILE).is_file())
            .or_else(|| examples_dir.as_ref().map(|e| e.join("DemoBot")).filter(|p| p.is_dir()));
        if let Some(p) = project {
            ed.open_project(&p);
        }
        // The saved layout is used as-is; opening the project queued its first workspace.
        let dock = match saved_dock {
            Some(mut d) => {
                let keys: Vec<String> = ed.project.as_ref().map(|p| p.workspaces.keys().cloned().collect()).unwrap_or_default();
                if repair_layout(&mut d) {
                    ed.log(Level::Warn, t!("The panel layout was damaged, so it was put back together."));
                }
                close_tabs(&mut d, |t| matches!(t, Tab::Canvas(k) if !keys.contains(k)));
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
        let setup = (!ed.prefs.setup_done).then(crate::setup::Setup::new);
        CircuitApp {
            ed,
            dock,
            examples_dir,
            title: String::new(),
            show_about: false,
            new_project: None,
            confirm_close: false,
            allow_close: false,
            problem_count: 0,
            problems_found: None,
            dock_size: None,
            bot: BotUi::new(),
            stored: Default::default(),
            watcher,
            palette: None,
            native,
            bundled_plugins,
            plugins: None,
            setup,
            tour: None,
            dock_before_tour: None,
            settings: Default::default(),
            restore: Default::default(),
            last_recovery: std::time::Instant::now(),
            last_check: std::time::Instant::now(),
            reveal: crate::reveal::Reveal::new(cc),
            frame_stats: std::env::var_os("XC_FRAME_STATS").map(|_| (0, std::time::Instant::now())),
        }
    }

    /// The editors that used to be dialogs are tabs: one is shown while its
    /// draft exists, and closed once it is saved or cancelled.
    fn sync_dialog_tabs(&mut self, ctx: &egui::Context) {
        let wanted = [
            (Tab::EmbedBuilder, self.ed.embed_edit.is_some(), egui::vec2(1060.0, 760.0)),
            (Tab::LayoutBuilder, self.ed.layout_edit.is_some(), egui::vec2(1240.0, 800.0)),
            (Tab::SlashCommands, self.ed.slash_edit.is_some(), egui::vec2(1320.0, 720.0)),
            (Tab::BotSettings, self.bot.settings.is_some(), egui::vec2(540.0, 600.0)),
            (Tab::Export, self.bot.export.is_some(), egui::vec2(560.0, 620.0)),
            (Tab::Plugins, self.plugins.is_some(), egui::vec2(580.0, 560.0)),
            (Tab::WorkspaceExport, self.ed.ws_export.is_some(), egui::vec2(660.0, 680.0)),
            (Tab::WorkspaceImport, self.ed.ws_import.is_some(), egui::vec2(660.0, 680.0)),
        ];
        for (tab, open, size) in wanted {
            match (open, self.dock.find_tab(&tab)) {
                (true, None) => open_floating(&mut self.dock, ctx, tab, size),
                (false, Some(path)) => {
                    self.dock.remove_tab(path);
                }
                _ => {}
            }
        }
    }

    /// Opens the Plugins tab, or focuses it.
    fn open_plugins(&mut self) {
        match self.dock.find_tab(&Tab::Plugins) {
            Some(path) => {
                let _ = self.dock.set_active_tab(path);
            }
            None => self.plugins = Some(crate::plugin_manager::Draft::new(&self.native)),
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
                    close_tabs(&mut self.dock, |t| *t == Tab::Canvas(key.clone()));
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
            close_tabs(&mut self.dock, |t| *t == tab);
            return;
        }
        place_panel(&mut self.dock, tab);
    }

    fn open_project_dialog(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().set_title(t!("Open a Circuit project folder")).pick_folder() {
            self.switch_project(&dir);
        }
    }

    /// File › New Project: the kind of project, its name and place, and what
    /// it starts from.
    fn new_project_dialog(&mut self) {
        let location = self
            .ed
            .prefs
            .new_project_location
            .as_ref()
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| {
                // Next to the open project, unless that is one of the app's examples.
                let open = self.ed.project.as_ref().map(|p| p.root.as_path()).filter(|r| self.examples_dir.as_ref().is_none_or(|e| !r.starts_with(e)));
                crate::new_project::default_location(open)
            });
        self.new_project = Some(crate::new_project::NewProject::new(&self.ed.reg, location));
    }

    fn new_project_chooser(&mut self, ctx: &egui::Context, theme: &crate::theme::Theme) {
        use crate::new_project::Outcome;
        let Some(mut state) = self.new_project.take() else { return };
        let templates = crate::new_project::templates(self.examples_dir.as_deref(), &self.ed.reg);
        match crate::new_project::show(ctx, theme, &mut state, &self.ed.reg, &templates) {
            None => self.new_project = Some(state),
            Some(Outcome::Cancel) => {}
            Some(Outcome::MorePlugins) => self.open_plugins(),
            Some(Outcome::Create { dir, name, mode, template }) => {
                self.ed.prefs.new_project_location = Some(state.location.trim().to_string());
                let made = match template {
                    Some(from) => create_from_template(&from, &dir, &name),
                    None => Project::create(&dir, &name, &mode).map(|_| ()),
                };
                match made {
                    Ok(()) => self.switch_project(&dir),
                    Err(e) => {
                        self.ed.log(Level::Error, format!("Could not create the project: {e:#}"));
                        self.new_project = Some(state);
                    }
                }
            }
        }
    }

    fn switch_project(&mut self, dir: &Path) {
        if self.ed.unsaved_now() {
            self.ed.save();
        }
        self.bot.stop(&mut self.ed);
        close_tabs(&mut self.dock, |t| matches!(t, Tab::Canvas(_)));
        self.ed.open_project(dir);
        self.native.sync(&mut self.ed, self.bundled_plugins.as_deref());
        self.watcher.watch(&self.ed.watch_dirs());
        self.stored = Default::default();
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.ed.theme.clone();
        let custom = !self.ed.prefs.system_titlebar;
        let bar = ui.max_rect();
        if custom {
            crate::titlebar::drag_region(ui, bar);
        }
        // The search box sits in the middle, where the window title would be.
        let search = egui::Rect::from_center_size(bar.center(), egui::vec2(crate::titlebar::SEARCH_W, 24.0));
        let label = match &self.ed.project {
            Some(p) => tf!("Search {project}", project = p.meta.name),
            None => t!("Search").into(),
        };
        self.ed.ui_rects.insert("search".into(), search);
        if crate::titlebar::search_box(ui, search, &theme, &label) {
            self.palette = Some(Default::default());
        }
        let row = |ui: &mut egui::Ui, layout| ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(layout));
        let mut left = row(ui, egui::Layout::left_to_right(egui::Align::Center));
        if self.tour.is_some() {
            left.add(egui::Image::new(egui::include_image!("../assets/icon.svg")).fit_to_exact_size(egui::vec2(18.0, 18.0)));
            left.add_space(6.0);
            left.label(RichText::new(t!("Tour")).strong().color(theme.accent));
        } else {
        // The menus fill the bar's height, so they're easy to hit. (Set before
        // the menu bar: it sizes its row from this.)
        left.spacing_mut().interact_size.y = bar.height();
        egui::MenuBar::new().ui(&mut left, |ui| {
            ui.spacing_mut().button_padding.x = 10.0;
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add(egui::Image::new(egui::include_image!("../assets/icon.svg")).fit_to_exact_size(egui::vec2(18.0, 18.0)));
            ui.add_space(4.0);
            ui.menu_button(t!("File"), |ui| {
                if ui.button(t!("New Project…")).clicked() {
                    self.new_project_dialog();
                }
                if ui.button(t!("Open Project…")).clicked() {
                    self.open_project_dialog();
                }
                if let Some(examples) = self.examples_dir.clone()
                    && ui.button(t!("Open Example: DemoBot")).clicked()
                {
                    self.switch_project(&examples.join("DemoBot"));
                }
                ui.separator();
                if ui.add_enabled(self.ed.project.is_some(), egui::Button::new(t!("Save")).shortcut_text("Ctrl+S")).clicked() {
                    self.ed.save();
                    self.bot.restart_if_running(&mut self.ed, ui.ctx());
                }
                if ui.add_enabled(self.ed.project.is_some(), egui::Button::new(t!("Import workspaces…"))).clicked() {
                    crate::exchange::pick_import(&mut self.ed);
                }
                if ui.add_enabled(self.ed.project.as_ref().is_some_and(|p| !p.workspaces.is_empty()), egui::Button::new(t!("Export workspaces…"))).clicked() {
                    crate::exchange::open_export(&mut self.ed, crate::exchange::Preselect::All);
                }
                ui.separator();
                if ui.add_enabled(self.ed.project.is_some(), egui::Button::new(t!("Restore from backup…"))).clicked() {
                    open_center(&mut self.dock, Tab::Backups);
                }
                if ui.add(egui::Button::new(t!("Settings…")).shortcut_text("Ctrl+,")).clicked() {
                    self.open_settings();
                }
                if ui.button(t!("Reload Plugins")).clicked() {
                    self.reload_plugins(ui.ctx());
                }
                ui.separator();
                if ui.button(t!("Exit")).clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
            });
            ui.menu_button(t!("Edit"), |ui| {
                let key = self.ed.undo_key();
                if ui.add_enabled(self.ed.history.can_undo(&key) || self.ed.edited, egui::Button::new(t!("Undo")).shortcut_text("Ctrl+Z")).clicked() {
                    self.undo();
                }
                if ui.add_enabled(self.ed.history.can_redo(&key), egui::Button::new(t!("Redo")).shortcut_text("Ctrl+Y")).clicked() {
                    self.redo();
                }
                ui.separator();
                let has_sel = !self.ed.selection(&key).is_empty();
                if ui.add_enabled(has_sel, egui::Button::new(t!("Cut")).shortcut_text("Ctrl+X")).clicked() && self.ed.copy(ui.ctx()) {
                    self.ed.delete_selection();
                }
                if ui.add_enabled(has_sel, egui::Button::new(t!("Copy")).shortcut_text("Ctrl+C")).clicked() {
                    self.ed.copy(ui.ctx());
                }
                if ui.add(egui::Button::new(t!("Paste")).shortcut_text("Ctrl+V")).clicked()
                    && let Ok(text) = arboard::Clipboard::new().and_then(|mut c| c.get_text())
                {
                    self.ed.paste(&text, None);
                }
                if ui.add_enabled(has_sel, egui::Button::new(t!("Duplicate")).shortcut_text("Ctrl+D")).clicked() {
                    self.ed.duplicate();
                }
                if ui.add_enabled(has_sel, egui::Button::new(t!("Delete")).shortcut_text("Del")).clicked() {
                    self.ed.delete_selection();
                }
                ui.separator();
                if ui.add(egui::Button::new(t!("Select all")).shortcut_text("Ctrl+A")).clicked() {
                    self.ed.select_all();
                }
            });
            ui.menu_button(t!("View"), |ui| {
                ui.label(RichText::new(t!("SIDE BAR")).size(10.5).color(theme.text_muted));
                for view in SideView::ALL {
                    if ui.radio(self.ed.prefs.side_view == Some(view), view.name()).clicked() {
                        self.ed.prefs.side_view = Some(view);
                    }
                }
                if ui.radio(self.ed.prefs.side_view.is_none(), t!("Hidden")).clicked() {
                    self.ed.prefs.side_view = None;
                }
                ui.separator();
                ui.label(RichText::new(t!("PANELS")).size(10.5).color(theme.text_muted));
                for (tab, name) in Tab::PANELS {
                    let name = crate::i18n::t(name);
                    let mut open = self.dock.find_tab(&tab).is_some();
                    if ui.checkbox(&mut open, name).clicked() {
                        self.toggle_panel(tab);
                    }
                }
                ui.separator();
                ui.label(RichText::new(t!("THEME")).size(10.5).color(theme.text_muted));
                let themes: Vec<(String, String)> = self.ed.reg.themes.iter().map(|t| (t.id.clone(), t.name.clone())).collect();
                for (id, name) in themes {
                    if ui.radio(self.ed.theme.id == id, name).clicked() {
                        self.ed.set_theme(&id);
                        self.ed.theme.apply(ui.ctx());
                    }
                }
                ui.separator();
                ui.checkbox(&mut self.ed.prefs.activity_bar, t!("Show activity bar"));
                if ui.checkbox(&mut self.ed.prefs.system_titlebar, t!("Use system title bar")).clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Decorations(self.ed.prefs.system_titlebar));
                }
                if ui.add(egui::Button::new(t!("Command Palette…")).shortcut_text("Ctrl+P")).clicked() {
                    self.palette = Some(Default::default());
                }
                if ui.button(t!("Reset layout")).clicked() {
                    self.reset_layout();
                }
            });
            let has_project = self.ed.project.is_some();
            ui.menu_button(t!("Bot"), |ui| {
                let running = self.bot.state != BotState::Stopped;
                if ui.add_enabled(has_project, egui::Button::new(if running { t!("Restart") } else { t!("Run") }).shortcut_text("F5")).clicked() {
                    self.bot.start(&mut self.ed, ui.ctx());
                }
                if ui.add_enabled(running, egui::Button::new(t!("Stop")).shortcut_text("Shift+F5")).clicked() {
                    self.bot.stop(&mut self.ed);
                }
                ui.separator();
                if self.ed.has_slash_commands() && ui.add_enabled(has_project, egui::Button::new(t!("Slash Commands…"))).clicked() {
                    crate::slash::open(&mut self.ed);
                }
                if ui.add_enabled(has_project, egui::Button::new(t!("Settings…"))).clicked() {
                    self.bot.open_settings(&mut self.ed);
                }
                if ui.add_enabled(has_project, egui::Button::new(t!("Export…"))).clicked() {
                    self.bot.open_export(&self.ed);
                }
            });
            ui.menu_button(t!("Plugins"), |ui| {
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
                    ui.label(RichText::new(t!("No native plugins are running")).color(theme.text_muted));
                    ui.separator();
                }
                if ui.button(t!("Manage Plugins…")).clicked() {
                    self.open_plugins();
                }
            });
            ui.menu_button(t!("Help"), |ui| {
                if ui.button(t!("Welcome and setup…")).clicked() {
                    self.setup = Some(crate::setup::Setup::new());
                }
                if ui.add_enabled(self.tour.is_none(), egui::Button::new(t!("Take the tour"))).clicked() {
                    self.start_tour();
                }
                if ui.button(t!("About XC™ Circuit")).clicked() {
                    self.show_about = true;
                }
            });
        });

        }

        // Window buttons and Run / Stop on the right.
        let ui = &mut row(ui, egui::Layout::right_to_left(egui::Align::Center));
        if custom {
            ui.spacing_mut().item_spacing.x = 0.0;
            crate::titlebar::window_buttons(ui, &theme);
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.add_space(10.0);
        }
        let running = self.bot.state != BotState::Stopped;
        if running && ui.button(t!("Stop")).on_hover_text(t!("Stop the bot (Shift+F5)")).clicked() {
            self.bot.stop(&mut self.ed);
        }
        let label = match self.bot.state {
            BotState::Stopped => t!("Run"),
            BotState::Starting => t!("Starting…"),
            BotState::Running => t!("Restart"),
        };
        let run = egui::Button::new(RichText::new(label).strong().color(egui::Color32::from_rgb(4, 33, 13)));
        let run = ui.add_enabled_ui(self.ed.project.is_some(), |ui| crate::dialog::styled_button(ui, &theme, run, theme.action, egui::Stroke::NONE)).inner;
        self.ed.ui_rects.insert("run".into(), run.rect);
        if run.on_hover_text(t!("Run the bot (F5)")).clicked() {
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

    /// The activity bar: an icon per side bar view. Clicking one shows that view
    /// in the side bar; clicking the one shown folds the side bar away.
    fn activity(&mut self, ui: &mut egui::Ui) {
        let t = self.ed.theme.clone();
        let line = ui.max_rect();
        ui.painter().vline(line.right() - 0.5, line.y_range(), egui::Stroke::new(1.0, t.line));
        ui.spacing_mut().item_spacing.y = 2.0;
        for view in SideView::ALL {
            let shown = self.ed.prefs.side_view == Some(view);
            if self.activity_item(ui, view.icon(), view.name(), shown) {
                self.ed.prefs.side_view = if shown { None } else { Some(view) };
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            if self.activity_item(ui, "settings", t!("Settings (Ctrl+,)"), self.dock.find_tab(&Tab::Settings).is_some()) {
                self.toggle_settings();
            }
        });
    }

    /// The side bar: the view picked in the activity bar, under its name.
    fn side_bar(&mut self, ui: &mut egui::Ui, view: SideView) {
        let t = self.ed.theme.clone();
        let line = ui.max_rect();
        ui.painter().vline(line.right() - 0.5, line.y_range(), egui::Stroke::new(1.0, t.line));
        ui.horizontal(|ui| {
            ui.set_min_height(34.0);
            ui.add_space(14.0);
            ui.label(RichText::new(view.name().to_uppercase()).size(11.0).strong().color(t.text_muted));
        });
        self.ed.ui_rects.insert("side-bar".into(), ui.max_rect());
        // The view draws in a box of the side bar's size: whatever it holds, the
        // side bar keeps the width it was given (or dragged to).
        let area = ui.available_rect_before_wrap();
        let mut view_ui = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::top_down(egui::Align::Min)));
        view_ui.set_clip_rect(area.intersect(ui.clip_rect()));
        ui.allocate_rect(area, egui::Sense::hover());
        let ui = &mut view_ui;
        match view {
            SideView::Workspaces => crate::sidebar::panel(ui, &mut self.ed),
            SideView::StoredData => {
                let running = self.bot.state != BotState::Stopped;
                crate::stored::panel(ui, &mut self.ed, &mut self.stored, running);
            }
            SideView::Plugins => match crate::plugin_manager::side(ui, &t, &self.ed, &self.native) {
                Some(crate::plugin_manager::SideAsk::Manage) => self.open_plugins(),
                Some(crate::plugin_manager::SideAsk::Reload) => self.reload_plugins(ui.ctx()),
                Some(crate::plugin_manager::SideAsk::OpenFolder) => {
                    let dir = crate::bot::app_data_dir().join("plugins");
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = open::that(dir);
                }
                None => {}
            },
        }
    }

    /// One activity bar icon. Returns true when clicked.
    fn activity_item(&self, ui: &mut egui::Ui, icon: &str, tip: &str, active: bool) -> bool {
        let t = &self.ed.theme;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ACTIVITY_W, 40.0), egui::Sense::click());
        let resp = resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand);
        if active {
            ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(2.0, rect.height())), 0.0, t.accent);
        }
        let color = if active || resp.hovered() { t.text } else { t.text_muted };
        if let Some(path) = self.ed.reg.icons.get(icon) {
            egui::Image::from_uri(crate::editor::file_uri(path)).tint(color).paint_at(ui, egui::Rect::from_center_size(rect.center(), egui::vec2(20.0, 20.0)));
        }
        resp.clicked()
    }

    /// Starts the guided tour on a fresh practice project, with a reduced layout.
    fn start_tour(&mut self) {
        if self.tour.is_some() {
            return;
        }
        let dir = crate::bot::app_data_dir().join("tour");
        if let Err(e) = crate::tour::create_project(&dir) {
            self.ed.log(Level::Error, format!("Couldn't prepare the tour: {e:#}"));
            return;
        }
        self.bot.stop(&mut self.ed);
        let before = self.ed.project.as_ref().map(|p| p.root.clone());
        self.dock_before_tour = Some(self.dock.clone());
        self.switch_project(&dir);
        self.dock = tour_layout();
        self.ed.log.clear();
        self.tour = Some(crate::tour::Tour { step: 0, before });
    }

    fn end_tour(&mut self, exit: crate::tour::Exit) {
        use crate::tour::Exit;
        let Some(tour) = self.tour.take() else { return };
        self.bot.stop(&mut self.ed);
        // The practice project isn't kept.
        self.ed.forget_changes();
        let target = match exit {
            Exit::DemoBot => self.examples_dir.as_ref().map(|e| e.join("DemoBot")),
            Exit::Return | Exit::NewProject => tour.before,
        };
        match target {
            Some(dir) => self.switch_project(&dir),
            None => {
                close_tabs(&mut self.dock, |t| matches!(t, Tab::Canvas(_)));
            }
        }
        if let Some(dock) = self.dock_before_tour.take() {
            let keys: Vec<String> = self.ed.project.as_ref().map(|p| p.workspaces.keys().cloned().collect()).unwrap_or_default();
            self.dock = dock;
            close_tabs(&mut self.dock, |t| matches!(t, Tab::Canvas(k) if !keys.contains(k)));
            if let Some(first) = keys.first() {
                open_canvas(&mut self.dock, first.clone());
            }
        }
        if exit == Exit::NewProject {
            self.new_project_dialog();
        }
    }

    fn open_settings(&mut self) {
        open_center(&mut self.dock, Tab::Settings);
    }

    /// The activity bar's gear: opens Settings, or closes it when it is the tab in view.
    fn toggle_settings(&mut self) {
        let focused = self.dock.find_active_focused().is_some_and(|(_, t)| *t == Tab::Settings);
        if focused {
            close_tabs(&mut self.dock, |t| *t == Tab::Settings);
        } else {
            self.open_settings();
        }
    }

    fn settings_asks(&mut self, asks: Vec<crate::settings::Ask>, ctx: &egui::Context) {
        use crate::settings::Ask;
        for ask in asks {
            match ask {
                Ask::BotSettings => self.bot.open_settings(&mut self.ed),
                Ask::SlashCommands => crate::slash::open(&mut self.ed),
                Ask::Export => self.bot.open_export(&self.ed),
                Ask::ManagePlugins => self.open_plugins(),
                Ask::ReloadPlugins => self.reload_plugins(ctx),
                Ask::AllowNative(on) => {
                    self.native.allow = on;
                    self.native.sync(&mut self.ed, self.bundled_plugins.as_deref());
                }
                Ask::ResetLayout => self.reset_layout(),
                Ask::NodeSetup => self.setup = Some(crate::setup::Setup::node_only()),
                Ask::OpenBackups => open_center(&mut self.dock, Tab::Backups),
            }
        }
    }

    /// After a crash: offers the unsaved changes that were left behind.
    fn recovery_dialog(&mut self, ctx: &egui::Context, theme: &crate::theme::Theme) {
        let Some(rec) = &self.ed.recovery_offer else { return };
        let Some(project) = &self.ed.project else { return };
        let changes = circuit_core::backup::compare(&rec.workspaces, &project.workspaces);
        let changed: Vec<String> = changes
            .iter()
            .filter_map(|(_, name, c)| match c {
                circuit_core::backup::Change::Same => None,
                circuit_core::backup::Change::Changed => Some(format!("{name} (changed)")),
                circuit_core::backup::Change::OnlyInBackup => Some(format!("{name} (new)")),
                circuit_core::backup::Change::OnlyNow => Some(format!("{name} (deleted)")),
            })
            .collect();
        let when = format!("{} ({})", crate::editor::local_time(rec.saved_at), crate::editor::ago(rec.saved_at));
        let icon = self.ed.reg.icons.get("clock").cloned();
        let (mut restore, mut discard) = (false, false);
        dialog::show(
            ctx,
            theme,
            dialog::Spec { id: "recovery", title: t!("Unsaved changes found"), subtitle: Some(t!("XC Circuit didn't close normally last time")), icon: icon.as_deref(), width: 500.0 },
            |ui, part| match part {
                dialog::Part::Body => {
                    ui.label(RichText::new(format!("Changes to {} from {when} weren't saved.", project.meta.name)).size(13.5).color(theme.text));
                    ui.add_space(6.0);
                    if changed.is_empty() {
                        ui.label(RichText::new(t!("They change the project's settings.")).size(12.5).color(theme.text_muted));
                    } else {
                        for c in &changed {
                            ui.label(RichText::new(format!("• {c}")).size(12.5).color(theme.text_muted));
                        }
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new(t!("Bring them back as unsaved changes, then save to keep them.")).size(12.5).color(theme.text_muted));
                }
                dialog::Part::Footer => {
                    restore = dialog::primary_button(ui, theme, t!("Bring them back")).clicked();
                    discard = dialog::secondary_button(ui, theme, t!("Discard")).clicked();
                }
            },
        );
        if restore && let Some(rec) = self.ed.recovery_offer.take() {
            self.ed.apply_recovery(rec);
        } else if discard {
            self.ed.recovery_offer = None;
            if let Some(p) = &self.ed.project {
                self.ed.backups.clear_recovery(&p.root);
            }
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
        let key = self.ed.undo_key();
        let has_sel = !self.ed.selection(&key).is_empty();
        let mut items = Vec::new();
        if has_project {
            items.push(Item::new(t!("Bot"), if running { t!("Restart bot") } else { t!("Run bot") }, Cmd::Run).shortcut("F5"));
        }
        if running {
            items.push(Item::new(t!("Bot"), t!("Stop bot"), Cmd::Stop).shortcut("Shift+F5"));
        }
        if has_project {
            if self.ed.has_slash_commands() {
                items.push(Item::new(t!("Bot"), t!("Slash Commands…"), Cmd::SlashCommands));
            }
            items.push(Item::new(t!("Bot"), t!("Bot Settings…"), Cmd::Settings));
            items.push(Item::new(t!("Bot"), "Export…", Cmd::Export));
            items.push(Item::new(t!("File"), "Save", Cmd::Save).shortcut("Ctrl+S"));
            items.push(Item::new(t!("File"), t!("Import workspaces…"), Cmd::ImportWorkspaces));
            items.push(Item::new(t!("File"), t!("Export workspaces…"), Cmd::ExportWorkspaces));
        }
        items.push(Item::new(t!("File"), t!("New Project…"), Cmd::NewProject));
        items.push(Item::new(t!("File"), t!("Open Project…"), Cmd::OpenProject));
        if self.examples_dir.is_some() {
            items.push(Item::new(t!("File"), t!("Open Example: DemoBot"), Cmd::OpenExample));
        }
        items.push(Item::new(t!("File"), t!("Reload Plugins"), Cmd::ReloadPlugins));
        if self.ed.history.can_undo(&key) {
            items.push(Item::new(t!("Edit"), "Undo", Cmd::Undo).shortcut("Ctrl+Z"));
        }
        if self.ed.history.can_redo(&key) {
            items.push(Item::new(t!("Edit"), "Redo", Cmd::Redo).shortcut("Ctrl+Y"));
        }
        if has_sel {
            items.push(Item::new(t!("Edit"), "Duplicate", Cmd::Duplicate).shortcut("Ctrl+D"));
            items.push(Item::new(t!("Edit"), "Delete", Cmd::Delete).shortcut("Del"));
        }
        if has_project {
            items.push(Item::new(t!("Edit"), t!("Select all"), Cmd::SelectAll).shortcut("Ctrl+A"));
        }
        for view in SideView::ALL {
            if self.ed.prefs.side_view != Some(view) {
                items.push(Item::new(t!("View"), tf!("Show {name} in the side bar", name = view.name()), Cmd::SideView(Some(view))));
            }
        }
        if self.ed.prefs.side_view.is_some() {
            items.push(Item::new(t!("View"), t!("Hide the side bar"), Cmd::SideView(None)));
        }
        for (tab, name) in Tab::PANELS {
            let label = if self.dock.find_tab(&tab).is_some() { tf!("Close {name} panel", name = crate::i18n::t(name)) } else { tf!("Show {name} panel", name = crate::i18n::t(name)) };
            items.push(Item::new(t!("View"), label, Cmd::TogglePanel(tab)));
        }
        for t in &self.ed.reg.themes {
            if t.id != self.ed.theme.id {
                items.push(Item::new(t!("Theme"), format!("Use {} theme", t.name), Cmd::Theme(t.id.clone())));
            }
        }
        items.push(Item::new(t!("View"), t!("Reset layout"), Cmd::ResetLayout));
        items.push(Item::new(t!("Help"), t!("About XC™ Circuit"), Cmd::About));
        items.push(Item::new(t!("File"), "Settings", Cmd::OpenSettings).shortcut("Ctrl+,"));
        items.push(Item::new(t!("Help"), t!("Take the tour"), Cmd::Tour));
        if has_project {
            items.push(Item::new(t!("File"), t!("Restore from backup…"), Cmd::Backups));
        }
        items.push(Item::new(t!("Plugins"), t!("Manage Plugins…"), Cmd::ManagePlugins));
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
                items.push(Item::new(t!("Workspace"), ws.name.clone(), Cmd::OpenWorkspace(wk.clone())));
            }
            for (wk, ws) in &p.workspaces {
                for (id, node) in &ws.nodes {
                    let block = node.block.split('@').next().unwrap_or(&node.block);
                    let def = self.ed.reg.block(block);
                    let name = def.map(|d| d.name.clone()).unwrap_or_else(|| block.to_string());
                    items.push(Item::new(t!("Block"), name, Cmd::RevealBlock(wk.clone(), id.clone())).detail(format!("{} · #{id}", ws.name)).content(block_texts(def, &node.values)));
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
            Cmd::Undo => self.undo(),
            Cmd::Redo => self.redo(),
            Cmd::Duplicate => self.ed.duplicate(),
            Cmd::Delete => self.ed.delete_selection(),
            Cmd::SelectAll => self.ed.select_all(),
            Cmd::TogglePanel(tab) => self.toggle_panel(tab),
            Cmd::SideView(view) => self.ed.prefs.side_view = view,
            Cmd::Theme(id) => {
                self.ed.set_theme(&id);
                self.ed.theme.apply(ctx);
            }
            Cmd::ResetLayout => self.reset_layout(),
            Cmd::Run => self.bot.start(&mut self.ed, ctx),
            Cmd::Stop => self.bot.stop(&mut self.ed),
            Cmd::SlashCommands => crate::slash::open(&mut self.ed),
            Cmd::Settings => self.bot.open_settings(&mut self.ed),
            Cmd::Export => self.bot.open_export(&self.ed),
            Cmd::ImportWorkspaces => crate::exchange::pick_import(&mut self.ed),
            Cmd::ExportWorkspaces => crate::exchange::open_export(&mut self.ed, crate::exchange::Preselect::All),
            Cmd::About => self.show_about = true,
            Cmd::OpenSettings => self.open_settings(),
            Cmd::Tour => self.start_tour(),
            Cmd::Backups => open_center(&mut self.dock, Tab::Backups),
            Cmd::ManagePlugins => self.open_plugins(),
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
            BotState::Stopped => (t.status_text.gamma_multiply(0.45), t!("Bot stopped")),
            BotState::Starting => (egui::Color32::from_rgb(0xFF, 0xB4, 0x54), t!("Bot starting")),
            BotState::Running => (t.action, t!("Bot running")),
        };
        let has_project = self.ed.project.is_some();
        let mut left = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let running = self.bot.state != BotState::Stopped;
        let tip = if running { t!("Stop the bot (Shift+F5)") } else { t!("Run the bot (F5)") };
        if status_item(&mut left, &t, Some(dot), state).on_hover_text(tip).clicked() && has_project {
            if running { self.bot.stop(&mut self.ed) } else { self.bot.start(&mut self.ed, &ctx) }
        }
        match self.ed.project.as_ref().map(|p| (p.meta.name.clone(), p.meta.mode.clone())) {
            Some((name, mode_id)) => {
                if status_item(&mut left, &t, None, &name).on_hover_text(t!("Bot settings")).clicked() {
                    self.bot.open_settings(&mut self.ed);
                }
                let mode = self.ed.reg.mode(&mode_id).map(|m| (m.name.clone(), m.description.clone()));
                let (mode_name, about) = match mode {
                    Some(m) => m,
                    None if mode_id == crate::editor::NO_MODE => (t!("Core blocks").to_string(), t!("No build mode: only the core blocks, no npm packages").to_string()),
                    None => (mode_id, String::new()),
                };
                let r = status_item(&mut left, &t, None, &mode_name);
                if !about.is_empty() {
                    r.on_hover_text(about);
                }
            }
            None => {
                if status_item(&mut left, &t, None, t!("No project")).on_hover_text(t!("Open a project")).clicked() {
                    self.open_project_dialog();
                }
            }
        }
        let counts = tf!("{plugins} plugins · {blocks} blocks", plugins = self.ed.reg.plugins.len(), blocks = self.ed.reg.blocks.len());
        if status_item(&mut left, &t, None, &counts).on_hover_text(t!("Manage plugins")).clicked() {
            self.open_plugins();
        }
        if self.problem_count > 0 {
            let label = if self.problem_count == 1 { t!("⚠ 1 problem").to_string() } else { tf!("⚠ {count} problems", count = self.problem_count) };
            if status_item(&mut left, &t, None, &label).on_hover_text(t!("Show the Problems panel")).clicked() {
                match self.dock.find_tab(&Tab::Problems) {
                    Some(path) => {
                        let _ = self.dock.set_active_tab(path);
                    }
                    None => self.toggle_panel(Tab::Problems),
                }
            }
        }

        let mut right = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(egui::Layout::right_to_left(egui::Align::Center)));
        if status_item(&mut right, &t, None, &tf!("Theme: {name}", name = t.name)).on_hover_text(t!("Switch to the next theme")).clicked() {
            let ids: Vec<String> = self.ed.reg.themes.iter().map(|x| x.id.clone()).collect();
            if let Some(next) = ids.iter().position(|id| *id == t.id).map(|i| ids[(i + 1) % ids.len()].clone()) {
                self.ed.set_theme(&next);
                self.ed.theme.apply(&ctx);
            }
        }
        let saved = if self.ed.unsaved() { t!("Unsaved changes") } else { t!("Saved") };
        if status_item(&mut right, &t, None, saved).on_hover_text(t!("Save (Ctrl+S)")).clicked() && has_project {
            self.ed.save();
            self.bot.restart_if_running(&mut self.ed, &ctx);
        }
    }

    fn undo(&mut self) {
        if self.ed.undo() {
            self.reopen_bot_settings();
        }
    }

    fn redo(&mut self) {
        if self.ed.redo() {
            self.reopen_bot_settings();
        }
    }

    /// The Bot Settings tab shows the settings as they are after undo or redo.
    fn reopen_bot_settings(&mut self) {
        if self.ed.undo_project && self.bot.settings.is_some() {
            self.bot.open_settings(&mut self.ed);
        }
    }

    /// Undo, clipboard and selection shortcuts, unless a text box has focus.
    fn edit_shortcuts(&mut self, ctx: &egui::Context) {
        // The settings and data tabs undo changes to them; others a workspace.
        self.ed.undo_project = self.dock.find_active_focused().is_some_and(|(_, t)| matches!(t, Tab::SlashCommands | Tab::BotSettings));
        if ctx.text_edit_focused() || self.ed.picker.is_some() {
            return;
        }
        use egui::{Key, Modifiers};
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        if ctx.input_mut(|i| i.consume_key(cmd_shift, Key::Z) || i.consume_key(cmd, Key::Y)) {
            self.redo();
        } else if ctx.input_mut(|i| i.consume_key(cmd, Key::Z)) {
            self.undo();
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
        if self.setup.is_none() && self.tour.is_none() {
            self.recovery_dialog(ctx, &theme);
        }
        self.new_project_chooser(ctx, &theme);
        if self.show_about {
            let info = tf!("{plugins} plugins · {blocks} blocks · {themes} themes loaded", plugins = self.ed.reg.plugins.len(), blocks = self.ed.reg.blocks.len(), themes = self.ed.reg.themes.len());
            let mut ok = false;
            let close = dialog::show(
                ctx,
                &theme,
                dialog::Spec { id: "about", title: t!("About XC™ Circuit"), subtitle: None, icon: None, width: 420.0 },
                |ui, part| match part {
                    dialog::Part::Body => {
                        ui.vertical_centered(|ui| {
                            ui.add_space(6.0);
                            ui.add(egui::Image::new(egui::include_image!("../assets/icon.svg")).fit_to_exact_size(egui::vec2(76.0, 76.0)));
                            ui.add_space(10.0);
                            ui.label(RichText::new(t!("XC™ Circuit")).size(22.0).strong().color(theme.text));
                            ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).size(12.5).color(theme.text_muted));
                            ui.add_space(10.0);
                            ui.label(RichText::new(t!("Build bots by connecting blocks.")).size(13.5).color(theme.text));
                            ui.add_space(4.0);
                            ui.label(RichText::new(&info).size(11.5).color(theme.text_muted));
                            ui.add_space(6.0);
                        });
                    }
                    dialog::Part::Footer => ok = dialog::primary_button(ui, &theme, t!("Close")).clicked(),
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
                        ui.label(RichText::new(t!("Your changes are lost if you close without saving.")).size(13.5).color(theme.text));
                        if self.bot.state != BotState::Stopped {
                            ui.add_space(4.0);
                            ui.label(RichText::new(t!("The running bot will be stopped.")).size(12.0).color(theme.text_muted));
                        }
                    }
                    dialog::Part::Footer => {
                        save = dialog::primary_button(ui, &theme, t!("Save and close")).clicked();
                        discard = dialog::secondary_button(ui, &theme, t!("Don't save")).clicked();
                        cancel = dialog::secondary_button(ui, &theme, t!("Cancel")).clicked();
                    }
                },
            );
            if save || discard {
                if save {
                    self.ed.save();
                } else if let Some(p) = &self.ed.project {
                    // Chosen on purpose: no recovery offer next time.
                    self.ed.backups.clear_recovery(&p.root);
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
        self.reveal.frame(&ctx);
        // XC_FRAME_STATS=1 prints how many frames a second are drawn, to find
        // what keeps the app busy.
        if let Some((frames, since)) = &mut self.frame_stats {
            *frames += 1;
            if since.elapsed() >= std::time::Duration::from_secs(1) {
                eprintln!("frames: {frames}/s");
                (*frames, *since) = (0, std::time::Instant::now());
            }
        }
        self.ed.ui_rects.clear();
        // The tour's practice project is never worth a "save changes?" question.
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.tour.is_none() && self.ed.unsaved_now() {
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
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
            self.open_settings();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::P) || i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::P)) {
            self.palette = if self.palette.is_some() { None } else { Some(Default::default()) };
            self.ed.picker = None;
        }
        self.bot.poll(&mut self.ed, &ctx);
        if let Some(Err(e)) = self.ed.bot_profile.poll() {
            self.ed.log(crate::editor::Level::Warn, tf!("Couldn't load the bot's name and picture: {error}", error = e));
        }
        if self.ed.bot_profile.busy() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
        self.run_native(&ctx);
        if self.watcher.poll() {
            self.ed.log(Level::Info, t!("Plugin files changed, reloading"));
            self.reload_plugins(&ctx);
        } else if self.watcher.waiting() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.edit_shortcuts(&ctx);
        // A block that just ran glows and fades out, so repaint while one does.
        // Failed blocks stay marked, but their mark doesn't move: no repaints.
        self.ed.activity.retain(|_, a| a.error.is_some() || a.at.elapsed().as_secs_f32() < 1.2);
        if self.ed.activity.values().any(|a| a.error.is_none()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }

        let title = match &self.ed.project {
            Some(p) => format!("{}{} — XC™ Circuit", if self.ed.unsaved() { "● " } else { "" }, p.meta.name),
            None => "XC™ Circuit".into(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        let theme = self.ed.theme.clone();
        let (margin, height) = if self.ed.prefs.system_titlebar { (Margin::symmetric(8, 4), None) } else { (Margin { left: 8, right: 0, top: 0, bottom: 0 }, Some(crate::titlebar::HEIGHT)) };
        let mut top = egui::Panel::top("menu").frame(Frame::new().fill(theme.frame).inner_margin(margin));
        if let Some(h) = height {
            top = top.exact_size(h);
        }
        top.show(ui, |ui| {
            crate::theme::compact(ui.style_mut());
            self.menu_bar(ui)
        });
        egui::Panel::bottom("status")
            .exact_size(24.0)
            .frame(Frame::new().fill(theme.status_bar).inner_margin(Margin::symmetric(4, 0)))
            .show(ui, |ui| self.status_bar(ui));
        if self.ed.prefs.activity_bar && self.setup.is_none() && self.tour.is_none() {
            egui::Panel::left("activity")
                .exact_size(ACTIVITY_W)
                .resizable(false)
                .frame(Frame::new().fill(theme.frame).inner_margin(Margin::symmetric(0, 6)))
                .show(ui, |ui| self.activity(ui));
        }
        if self.setup.is_none()
            && self.tour.is_none()
            && let Some(view) = self.ed.prefs.side_view
        {
            egui::Panel::left("side-bar")
                .resizable(true)
                .default_size(270.0)
                .size_range(200.0..=560.0)
                .frame(Frame::new().fill(theme.panel))
                .show(ui, |ui| self.side_bar(ui, view));
        }
        let key = (self.ed.history.revision(), self.ed.reloads, self.ed.prefs.language.clone());
        if self.problems_found.as_ref().is_none_or(|(k, _)| *k != key) {
            self.problems_found = Some((key, panels::collect_problems(&self.ed)));
        }
        let mut problems = self.problems_found.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();
        problems.extend(self.native.problems());
        self.problem_count = problems.len();
        // Filled, so the dock's rounded panel corners show the frame color and not
        // whatever is behind the window's content.
        if let Some(mut setup) = self.setup.take() {
            let done = egui::CentralPanel::default().frame(Frame::new().fill(theme.frame)).show(ui, |ui| crate::setup::show(ui, &mut self.ed, &mut setup)).inner;
            match done {
                Some(crate::setup::Done::Finished { tour }) => {
                    self.ed.prefs.setup_done = true;
                    if tour {
                        self.start_tour();
                    }
                }
                Some(crate::setup::Done::Closed) => {}
                None => self.setup = Some(setup),
            }
        } else {
        egui::CentralPanel::default().frame(Frame::new().fill(theme.frame)).show(ui, |ui| {
            let style = theme.dock_style(ui.style());
            let running = self.bot.state != BotState::Stopped;
            self.sync_dialog_tabs(ui.ctx());
            let focused_tab = self.dock.find_active_focused().map(|(_, t)| t.clone());
            let mut viewer = Viewer {
                focused_tab,
                bot: &mut self.bot,
                bundled_plugins: self.bundled_plugins.as_deref(),
                plugins: &mut self.plugins,
                ed: &mut self.ed,
                problem_count: self.problem_count,
                problems,
                running,
                native: &mut self.native,
                settings: &mut self.settings,
                restore: &mut self.restore,
                asks: Vec::new(),
            };
            let size = ui.available_size();
            if let Some(before) = self.dock_size
                && before != size
                && before.min_elem() > 100.0
                && size.min_elem() > 100.0
            {
                keep_panel_sizes(self.dock.main_surface_mut(), NodeIndex::root(), size - before);
            }
            self.dock_size = Some(size);
            DockArea::new(&mut self.dock)
                .style(style)
                .show_add_buttons(false)
                .show_leaf_close_all_buttons(false)
                .show_leaf_collapse_buttons(false)
                .show_inside(ui, &mut viewer);
            let asks = std::mem::take(&mut viewer.asks);
            if repair_layout(&mut self.dock) {
                self.ed.log(Level::Warn, t!("The panel layout was damaged, so it was put back together."));
            }
            self.settings_asks(asks, ui.ctx());
        });
        }

        if self.setup.is_none()
            && let Some(mut tour) = self.tour.take()
        {
            let running = self.bot.state != BotState::Stopped;
            let ask = crate::tour::show(&ctx, &mut self.ed, &mut tour, running);
            self.tour = Some(tour);
            match ask {
                Some(crate::tour::Ask::Run) => self.bot.start(&mut self.ed, &ctx),
                Some(crate::tour::Ask::SetUpNode) => self.setup = Some(crate::setup::Setup::node_only()),
                Some(crate::tour::Ask::End(exit)) => self.end_tour(exit),
                None => {}
            }
        }
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
        if !self.ed.prefs.system_titlebar {
            crate::titlebar::frame_edges(&ctx, &theme);
        }

        // Record an undo step once an edit has settled.
        // Only looked for after an edit or input (not the pointer just moving),
        // or now and then for changes made some other way, since comparing a
        // big project takes a moment.
        let settled = !ctx.input(|i| i.pointer.any_down()) && !ctx.text_edit_focused();
        let input = ctx.input(|i| i.events.iter().any(|e| !matches!(e, egui::Event::PointerMoved(_) | egui::Event::MouseMoved(_))));
        let due = self.ed.edited || input || self.last_check.elapsed() >= CHECK_EVERY;
        if settled && due {
            self.ed.check_changes();
            self.last_check = std::time::Instant::now();
        }
        // Unsaved changes are copied every 30 seconds, for crash recovery.
        if self.ed.unsaved() && settled && self.tour.is_none() && self.last_recovery.elapsed() >= RECOVERY_EVERY {
            self.last_recovery = std::time::Instant::now();
            if let Some(p) = &self.ed.project
                && let Err(e) = self.ed.backups.write_recovery(p)
            {
                self.ed.log(Level::Warn, format!("Couldn't keep a recovery copy of your unsaved changes: {e:#}"));
            }
        }
        if self.ed.unsaved() {
            ctx.request_repaint_after(RECOVERY_EVERY);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.ed.theme.frame.to_normalized_gamma_f32()
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.bot.stop(&mut self.ed);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Mid-tour, keep the user's own layout and project.
        let dock = self.dock_before_tour.as_ref().filter(|_| self.tour.is_some()).unwrap_or(&self.dock);
        eframe::set_value(storage, KEY_DOCK, dock);
        eframe::set_value(storage, KEY_THEME, &self.ed.theme.id);
        eframe::set_value(storage, KEY_NATIVE_ALLOW, &self.native.allow);
        eframe::set_value(storage, KEY_PREFS, &self.ed.prefs);
        eframe::set_value(storage, KEY_NATIVE_TRUSTED, &self.native.trusted);
        eframe::set_value(storage, KEY_NATIVE_STORAGE, &self.native.storage);
        let project = match &self.tour {
            Some(t) => t.before.clone(),
            None => self.ed.project.as_ref().map(|p| p.root.clone()),
        };
        if let Some(root) = project {
            eframe::set_value(storage, KEY_PROJECT, &root);
        }
    }
}

const ACTIVITY_W: f32 = 44.0;
/// How often unsaved changes are copied for crash recovery.
const RECOVERY_EVERY: std::time::Duration = std::time::Duration::from_secs(30);
/// How often changes are looked for without an edit or input.
const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

struct Viewer<'a> {
    ed: &'a mut Editor,
    /// The tab the user last clicked into, marked with an accent line.
    focused_tab: Option<Tab>,
    bot: &'a mut BotUi,
    bundled_plugins: Option<&'a Path>,
    plugins: &'a mut Option<crate::plugin_manager::Draft>,
    running: bool,
    problem_count: usize,
    problems: Vec<panels::ProblemItem>,
    native: &'a mut crate::native::NativeHost,
    settings: &'a mut crate::settings::SettingsView,
    restore: &'a mut crate::restore::RestoreView,
    asks: Vec<crate::settings::Ask>,
}

impl TabViewer for Viewer<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> Id {
        Id::new(("tab", &*tab))
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        match tab {
            Tab::Inspector => t!("Inspector").into(),
            Tab::Console => t!("Console").into(),
            Tab::Problems => tf!("Problems  {count}", count = self.problem_count).into(),
            Tab::Settings => t!("Settings").into(),
            Tab::Backups => t!("Backups").into(),
            Tab::Plugin(plugin, panel) => self.native.panel_title(plugin, panel).unwrap_or_else(|| panel.clone()).into(),
            Tab::Canvas(key) => self.ed.workspace(key).map(|w| w.name.clone()).unwrap_or_else(|| key.clone()).into(),
            Tab::EmbedBuilder => t!("Embed Builder").into(),
            Tab::LayoutBuilder => t!("Layout Builder").into(),
            Tab::SlashCommands => t!("Slash Commands").into(),
            Tab::BotSettings => t!("Bot settings").into(),
            Tab::Export => t!("Export bot").into(),
            Tab::Plugins => t!("Plugins").into(),
            Tab::WorkspaceExport => t!("Export workspaces").into(),
            Tab::WorkspaceImport => t!("Import workspaces").into(),
        }
    }

    fn on_tab_button(&mut self, tab: &mut Tab, response: &egui::Response) {
        if self.focused_tab.as_ref() == Some(tab) {
            let r = response.rect;
            let line = egui::Rect::from_min_max(r.left_top(), egui::pos2(r.right(), r.top() + 2.0));
            response.ctx.layer_painter(response.layer_id).rect_filled(line, 0.0, self.ed.theme.accent);
        }
    }

    fn on_close(&mut self, tab: &mut Tab) -> OnCloseResponse {
        // Closing an editor's tab cancels it.
        match tab {
            Tab::EmbedBuilder => self.ed.embed_edit = None,
            Tab::LayoutBuilder => self.ed.layout_edit = None,
            Tab::SlashCommands => self.ed.slash_edit = None,
            Tab::BotSettings => self.bot.settings = None,
            Tab::Export => self.bot.export = None,
            Tab::Plugins => *self.plugins = None,
            Tab::WorkspaceExport => self.ed.ws_export = None,
            Tab::WorkspaceImport => self.ed.ws_import = None,
            _ => {}
        }
        OnCloseResponse::Close
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        let name = match tab {
            Tab::Canvas(_) => Some("canvas"),
            Tab::Inspector => Some("inspector"),
            Tab::Console => Some("console"),
            _ => None,
        };
        if let Some(name) = name {
            self.ed.ui_rects.insert(name.into(), ui.max_rect());
        }
        match tab {
            Tab::Inspector => panels::inspector(ui, self.ed),
            Tab::Console => panels::console(ui, self.ed),
            Tab::Problems => panels::problems(ui, self.ed, std::mem::take(&mut self.problems)),
            Tab::Settings => crate::settings::ui(ui, self.ed, self.native, self.settings, &mut self.asks),
            Tab::Backups => crate::restore::ui(ui, self.ed, self.restore, self.running),
            Tab::Plugin(plugin, panel) => {
                let (plugin, panel) = (plugin.clone(), panel.clone());
                self.native.panel_ui(ui, self.ed, &plugin, &panel, self.running);
            }
            Tab::Canvas(key) => {
                let key = key.clone();
                canvas::show(ui, self.ed, &key);
            }
            Tab::EmbedBuilder => crate::embed::tab(ui, self.ed),
            Tab::LayoutBuilder => crate::layout::tab(ui, self.ed),
            Tab::SlashCommands => crate::slash::tab(ui, self.ed),
            Tab::BotSettings => self.bot.settings_tab(ui, self.ed),
            Tab::Export => self.bot.export_tab(ui, self.ed),
            Tab::WorkspaceExport => crate::exchange::export_tab(ui, self.ed),
            Tab::WorkspaceImport => crate::exchange::import_tab(ui, self.ed),
            Tab::Plugins => {
                use crate::plugin_manager::Ask;
                let theme = self.ed.theme.clone();
                let Some(draft) = self.plugins.as_mut() else { return };
                match crate::plugin_manager::tab(ui, &theme, self.ed, self.native, draft) {
                    Some(Ask::Save(close)) => {
                        let draft = self.plugins.take().unwrap();
                        self.native.allow = draft.allow;
                        self.native.sync(self.ed, self.bundled_plugins);
                        for id in &draft.trust {
                            self.native.trust(id, self.ed);
                        }
                        if !close {
                            *self.plugins = Some(crate::plugin_manager::Draft::new(self.native));
                        }
                    }
                    Some(Ask::Cancel) => *self.plugins = None,
                    None => {}
                }
            }
        }
    }

    fn scroll_bars(&self, _tab: &Tab) -> [bool; 2] {
        [false, false]
    }
}

/// Gives a change in the dock's size (`grow`) to the canvases, so the panels
/// beside and below them (Inspector, Console …) keep their size. Splits are
/// fractions, so on their own every part would grow and shrink with the dock.
fn keep_panel_sizes(tree: &mut Tree<Tab>, index: NodeIndex, grow: egui::Vec2) {
    if index.0 >= tree.len() {
        return;
    }
    let (horizontal, rect, fraction) = match &tree[index] {
        Node::Horizontal(s) => (true, s.rect, s.fraction),
        Node::Vertical(s) => (false, s.rect, s.fraction),
        _ => return,
    };
    let (size, along) = if horizontal { (rect.width(), grow.x) } else { (rect.height(), grow.y) };
    let (left, right) = (index.left(), index.right());
    // The side with the canvases takes it all; otherwise both share it.
    let to_left = match (has_center(tree, left), has_center(tree, right)) {
        (true, false) => along,
        (false, true) => 0.0,
        _ => along * fraction,
    };
    if along != 0.0 && size.is_finite() && size > 1.0 {
        let f = ((fraction * size + to_left) / (size + along)).clamp(0.05, 0.95);
        if let Node::Horizontal(s) | Node::Vertical(s) = &mut tree[index] {
            s.fraction = f;
        }
    }
    let (grow_left, grow_right) = if horizontal {
        (egui::vec2(to_left, grow.y), egui::vec2(along - to_left, grow.y))
    } else {
        (egui::vec2(grow.x, to_left), egui::vec2(grow.x, along - to_left))
    };
    keep_panel_sizes(tree, left, grow_left);
    keep_panel_sizes(tree, right, grow_right);
}

/// Whether a part of the layout holds canvases (or other middle tabs).
fn has_center(tree: &Tree<Tab>, index: NodeIndex) -> bool {
    if index.0 >= tree.len() {
        return false;
    }
    match &tree[index] {
        Node::Leaf(leaf) => leaf.tabs().iter().any(|t| t.side() == Side::Center),
        Node::Horizontal(_) | Node::Vertical(_) => has_center(tree, index.left()) || has_center(tree, index.right()),
        Node::Empty => false,
    }
}

/// The tour's reduced window: canvas, Inspector and Console only.
fn tour_layout() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Console]);
    let surface = dock.main_surface_mut();
    let [_bottom, top] = surface.split_above(NodeIndex::root(), 0.74, vec![Tab::Canvas(crate::tour::WORKSPACE.into())]);
    surface.split_right(top, 0.72, vec![Tab::Inspector]);
    dock
}

fn default_layout() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Console, Tab::Problems]);
    let surface = dock.main_surface_mut();
    // Root starts as the bottom panel; the top area holds (canvases) | Inspector.
    // The side bar, outside the dock, has Workspaces. A split's fraction is
    // the share of its left/top child.
    surface.split_above(NodeIndex::root(), 0.76, vec![Tab::Inspector]);
    dock
}

/// Opens a tab in a window of its own over the middle of the app. It can be
/// docked from there like any other tab.
fn open_floating(dock: &mut DockState<Tab>, ctx: &egui::Context, tab: Tab, size: egui::Vec2) {
    let area = ctx.content_rect();
    let size = size.min(area.size() - egui::vec2(48.0, 48.0)).max(egui::vec2(320.0, 240.0));
    let surface = dock.add_window(vec![tab]);
    if let Some(w) = dock.get_window_state_mut(surface) {
        w.set_position(area.center() - size / 2.0).set_size(size);
    }
}

/// Opens a side panel next to the open tabs of its kind (the bottom panels
/// together, Inspector with plugin panels, …). Only when none is open does it
/// get a new area on its side.
fn place_panel(dock: &mut DockState<Tab>, tab: Tab) {
    let side = tab.side();
    if side == Side::Center {
        return open_center(dock, tab);
    }
    let group = dock.find_tab_from(|t| t.side() == side).map(|p| p.node_path());
    match group {
        Some(np) if dock.leaf_mut(np).is_ok() => {
            if let Ok(leaf) = dock.leaf_mut(np) {
                leaf.append_tab(tab.clone());
            }
        }
        _ if dock.main_surface().is_empty() => *dock.main_surface_mut() = Tree::new(vec![tab.clone()]),
        _ => {
            let surface = dock.main_surface_mut();
            match side {
                Side::Right => {
                    surface.split_right(NodeIndex::root(), 0.78, vec![tab.clone()]);
                }
                _ => {
                    surface.split_below(NodeIndex::root(), 0.75, vec![tab.clone()]);
                }
            }
        }
    }
    if let Some(path) = dock.find_tab(&tab) {
        let _ = dock.set_active_tab(path);
        dock.set_focused_node_and_surface(path.node_path());
    }
}

/// Closes every tab that matches, one at a time. (egui_dock's `retain_tabs`
/// can break the layout: when an area empties and its neighbour is split,
/// it moves only the neighbour's top node and leaves its parts behind.)
fn close_tabs(dock: &mut DockState<Tab>, matches: impl Fn(&Tab) -> bool) {
    while let Some(path) = dock.find_tab_from(&matches) {
        dock.remove_tab(path);
    }
}

/// Whether a layout tree is whole: every split has two halves, every part
/// hangs off a split, and every area has a tab.
fn tree_ok(tree: &Tree<Tab>) -> bool {
    let nodes: Vec<_> = tree.iter().collect();
    let at = |i: NodeIndex| nodes.get(i.0).copied();
    nodes.iter().enumerate().all(|(i, node)| {
        let index = NodeIndex(i);
        node.is_empty()
            || (index.parent().is_none_or(|p| at(p).is_some_and(|p| p.is_parent()))
                && (!node.is_parent() || (at(index.left()).is_some_and(|n| !n.is_empty()) && at(index.right()).is_some_and(|n| !n.is_empty())))
                && (!node.is_leaf() || node.tabs_count() > 0))
    })
}

/// Rebuilds a broken layout from the default one, with the same tabs open.
/// Returns whether it had to.
fn repair_layout(dock: &mut DockState<Tab>) -> bool {
    if dock.iter_surfaces().all(|s| s.node_tree().is_none_or(tree_ok)) {
        return false;
    }
    let tabs: Vec<Tab> = dock.iter_all_tabs().map(|(_, t)| t.clone()).filter(|t| !t.floats()).collect();
    let mut fresh = default_layout();
    close_tabs(&mut fresh, |t| !tabs.contains(t));
    for tab in tabs {
        if fresh.find_tab(&tab).is_none() {
            place_panel(&mut fresh, tab);
        }
    }
    *dock = fresh;
    true
}

/// Focuses a workspace's canvas tab, or opens it next to the other canvases.
fn open_canvas(dock: &mut DockState<Tab>, key: String) {
    open_center(dock, Tab::Canvas(key));
}

/// Focuses a tab of the main area (canvases, Settings), or opens it there.
fn open_center(dock: &mut DockState<Tab>, tab: Tab) {
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

/// A new project at `dir` from a template folder: its workspaces, slash
/// commands and settings, under the folder's name.
fn create_from_template(template: &Path, dir: &Path, name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(!dir.join(circuit_core::project::PROJECT_FILE).exists(), "there is already a project in that folder");
    let mut project = Project::load(template)?;
    project.root = dir.to_path_buf();
    project.meta.name = name.to_string();
    project.save()?;
    let ignore = dir.join(".gitignore");
    if !ignore.exists() {
        std::fs::write(ignore, ".circuit/\n")?;
    }
    Ok(())
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

/// The texts typed into a block, for the search: (field label, text). Texts
/// inside embeds, layouts and lists count; picked choices don't.
fn block_texts(def: Option<&circuit_core::block::BlockDef>, values: &serde_json::Map<String, serde_json::Value>) -> Vec<(String, String)> {
    fn strings(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::String(s) if !s.trim().is_empty() => out.push(s.clone()),
            serde_json::Value::Array(items) => items.iter().for_each(|i| strings(i, out)),
            serde_json::Value::Object(map) => map.values().for_each(|i| strings(i, out)),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    for (key, value) in values {
        let field = def.and_then(|d| d.fields.get(key));
        if field.is_some_and(|f| f.choices.is_some()) {
            continue;
        }
        let label = field.map(|f| f.label(key)).unwrap_or_else(|| circuit_core::block::title_case(key));
        let mut found = Vec::new();
        strings(value, &mut found);
        texts.extend(found.into_iter().map(|t| (label.clone(), t)));
    }
    texts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaves(dock: &DockState<Tab>) -> Vec<Vec<Tab>> {
        dock.main_surface().iter().filter_map(|n| n.tabs().map(|t| t.to_vec())).collect()
    }

    #[test]
    fn panels_open_next_to_their_kind() {
        let mut dock = default_layout();
        close_tabs(&mut dock, |t| *t == Tab::Problems);
        place_panel(&mut dock, Tab::Problems);
        let bottom = leaves(&dock).into_iter().find(|l| l.contains(&Tab::Console)).unwrap();
        assert!(bottom.contains(&Tab::Problems), "it joins Console: {bottom:?}");
        assert_eq!(leaves(&dock).len(), 2, "no new area");

        // With the whole bottom group closed, it gets a new area below.
        close_tabs(&mut dock, |t| t.side() == Side::Bottom);
        assert_eq!(leaves(&dock).len(), 1);
        place_panel(&mut dock, Tab::Problems);
        assert_eq!(leaves(&dock).len(), 2);
        assert!(tree_ok(dock.main_surface()));
    }

    #[test]
    fn closing_every_bottom_tab_keeps_the_layout_whole() {
        // The bottom area's neighbour is a split (Workspaces | Inspector), the
        // case where retain_tabs breaks the tree.
        let mut dock = default_layout();
        close_tabs(&mut dock, |t| t.side() == Side::Bottom);
        assert!(tree_ok(dock.main_surface()));
        let mut broken = default_layout();
        broken.retain_tabs(|t| t.side() != Side::Bottom);
        if !tree_ok(broken.main_surface()) {
            assert!(repair_layout(&mut broken), "a broken layout is rebuilt");
            assert!(tree_ok(broken.main_surface()));
            let open: Vec<Tab> = broken.iter_all_tabs().map(|(_, t)| t.clone()).collect();
            assert!(open.contains(&Tab::Inspector));
            assert!(!open.contains(&Tab::Console), "with the same tabs open");
        }
        assert!(!repair_layout(&mut dock), "a whole layout is left alone");
    }

    #[test]
    fn projects_start_from_templates() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let base = std::env::temp_dir().join(format!("xc-templates-{}", std::process::id()));
        for entry in std::fs::read_dir(root.join("examples/templates")).unwrap().flatten() {
            let dir = base.join(entry.file_name());
            std::fs::create_dir_all(&dir).unwrap();
            create_from_template(&entry.path(), &dir, "My Bot").unwrap();
            let made = Project::load(&dir).unwrap();
            let template = Project::load(entry.path()).unwrap();
            assert_eq!(made.meta.name, "My Bot");
            assert_eq!(made.workspaces.len(), template.workspaces.len());
            assert_eq!(made.meta.data, template.meta.data, "slash commands come along");
            assert!(dir.join(".gitignore").is_file());
            assert!(create_from_template(&entry.path(), &dir, "Again").is_err(), "an existing project isn't overwritten");
        }
        let _ = std::fs::remove_dir_all(base);
    }
}
