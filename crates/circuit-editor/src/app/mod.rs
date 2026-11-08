//! The window: menu bar, dockable panels, status bar.

pub mod crash;
pub mod graphics;
pub mod icon;
pub mod reveal;
pub mod titlebar;
pub mod updater;

use crate::editor::prefs::SideView;
use std::env;
use std::path::{Path, PathBuf};

use circuit_core::Project;
use eframe::egui::{self, Frame, Margin, RichText, ViewportCommand};
use crate::ui::dock::{Dir, Dock, DockViewer, Edge, Node};
use serde::{Deserialize, Serialize};

use crate::bot::{BotState, BotUi};
use crate::i18n::n;
use crate::editor::{Action, Editor, Level};
use crate::editor::keymap::{self, Action as Hotkey};
use crate::ui::palette::{Cmd, Item};
use crate::{canvas, ui::dialog, ui::palette, pages::panels, canvas::picker};

mod activity;
mod chrome;
mod commands;
mod dialogs;
mod dock;
use dock::*;
mod files;
use files::*;
pub(crate) mod fps;
mod menus;
mod plugins;
mod projects;
#[cfg(test)]
mod a11y;
#[cfg(test)]
mod tests;

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
    /// A plugin's panel: plugin id, panel id.
    Plugin(String, String),
    /// Editors that used to be dialogs. Each is open while its draft exists.
    EmbedBuilder,
    LayoutBuilder,
    ModalBuilder,
    CanvasBuilder,
    SlashCommands,
    BotSettings,
    Export,
    /// Bot › Server…: where the bot is uploaded to.
    Hosting,
    /// A Calagopus server's state, power buttons and live console.
    ServerConsole,
    Plugins,
    WorkspaceExport,
    WorkspaceImport,
    /// The start page, while no project is open.
    Start,
    /// Workspaces, Stored Data or Plugins, dragged out of the activity bar.
    Side(SideView),
}

/// Where a tab belongs in the window.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Side {
    Left,
    Right,
    Bottom,
    Center,
}

impl Tab {
    fn side(&self) -> Side {
        match self {
            Tab::Side(_) => Side::Left,
            Tab::Inspector | Tab::Plugin(..) => Side::Right,
            Tab::Console | Tab::Problems => Side::Bottom,
            _ => Side::Center,
        }
    }

    const PANELS: [(Tab, &'static str); 3] = [(Tab::Inspector, n("Inspector")), (Tab::Console, n("Console")), (Tab::Problems, n("Problems"))];

    /// The key of a tab that can be an icon in the activity bar: Workspaces,
    /// Stored Data, Plugins and the panels, the server console too (not
    /// workspaces, builders or pages).
    pub(super) fn side_key(&self) -> Option<String> {
        Some(match self {
            Tab::Side(view) => view.key().into(),
            Tab::Inspector => "inspector".into(),
            Tab::Console => "console".into(),
            Tab::Problems => "problems".into(),
            Tab::ServerConsole => "server-console".into(),
            Tab::Plugin(plugin, panel) => format!("plugin:{plugin}:{panel}"),
            _ => return None,
        })
    }

    /// The tab an activity bar key names.
    pub(super) fn from_side_key(key: &str) -> Option<Tab> {
        if let Some(view) = SideView::from_key(key) {
            return Some(Tab::Side(view));
        }
        Some(match key {
            "inspector" => Tab::Inspector,
            "console" => Tab::Console,
            "problems" => Tab::Problems,
            "server-console" => Tab::ServerConsole,
            _ => {
                let (plugin, panel) = key.strip_prefix("plugin:")?.split_once(':')?;
                Tab::Plugin(plugin.into(), panel.into())
            }
        })
    }
}

/// The layout as egui_dock saved it, read once (see [`crate::ui::dock::legacy`]).
const KEY_DOCK: &str = "dock";
const KEY_LAYOUT: &str = "layout";
const KEY_THEME: &str = "theme";
const KEY_PROJECT: &str = "project";
const KEY_SYSTEM_TITLEBAR: &str = "system_titlebar";
const KEY_ACTIVITY_BAR: &str = "activity_bar";
const KEY_PREFS: &str = "prefs";
/// Plugins' own key-value stores (the name is from when native plugins had them too).
const KEY_PLUGIN_STORAGE: &str = "native_storage";

pub struct CircuitApp {
    ed: Editor,
    dock: Dock<Tab>,
    title: String,
    show_about: bool,
    /// File › New Project: the starter to begin from (0 = empty), while choosing.
    new_project: Option<crate::pages::new_project::NewProject>,
    confirm_close: bool,
    allow_close: bool,
    problem_count: usize,
    /// The project's problems and what they were found for (project revision,
    /// plugin reloads, language): found again only when one changes.
    problems_found: Option<((u64, u64, String), Vec<panels::ProblemItem>)>,
    /// The dock's size last frame, so side panels keep theirs when it changes.
    dock_size: Option<egui::Vec2>,
    /// Where the activity bar's icons are (by key), for dropping between them.
    activity_slots: Vec<(String, egui::Rect)>,
    /// The start page's list of recent projects, read from their folders.
    start: crate::pages::start_page::StartPage,
    /// A newer version on GitHub, when one was found.
    updater: crate::app::updater::Updater,
    /// Newer versions of the bundled block plugins, without an app update.
    block_updates: crate::plugins::updates::BlockUpdates,
    /// The Updates window (Help › Check for Updates) is open.
    updates_open: bool,
    /// A crash since the last start, offered for a report.
    crash_offer: Option<crate::app::crash::Crash>,
    bot: BotUi,
    stored: crate::pages::stored::StoredView,
    watcher: crate::plugins::watch::PluginWatcher,
    palette: Option<palette::Palette>,
    /// Use the operating system's title bar instead of ours.
    plugin_host: crate::plugins::host::PluginHost,
    /// The app's own plugins folder.
    bundled_plugins: Option<PathBuf>,
    /// The MCP server's link for AI agents, while a plugin offers them tools.
    agent_bridge: Option<crate::mcp::bridge::Bridge>,
    /// When starting it failed last.
    agent_bridge_failed: Option<std::time::Instant>,
    /// Agents' screenshots waiting for the canvas to settle or for the picture.
    agent_shots: Vec<plugins::AgentShot>,
    /// The Plugins tab's unsaved changes, while it is open.
    settings: crate::pages::settings::SettingsView,
    restore: crate::pages::restore::RestoreView,
    /// When unsaved changes were last copied for crash recovery.
    last_recovery: std::time::Instant,
    /// When changes were last looked for.
    last_check: std::time::Instant,
    /// Shows the window once it has drawn (no white flash at startup).
    reveal: crate::app::reveal::Reveal,
    /// With XC_FRAME_STATS set: frames drawn since the last report, and when it was.
    frame_stats: Option<(u32, std::time::Instant)>,
    /// `--fps`: the frame rate measurement that drives the app.
    fps: Option<fps::Run>,
    /// The First Start page, while it is open.
    setup: Option<crate::pages::setup::Setup>,
    /// The guided tour, while it runs, and the layout to put back afterwards.
    tour: Option<crate::pages::tour::Tour>,
    dock_before_tour: Option<Dock<Tab>>,
    /// The panels and tabs while no project is open (only the start page
    /// shows then); back when a project opens.
    dock_before_start: Option<Dock<Tab>>,
}

impl CircuitApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let (plugin_dirs, bundled_root) = find_plugin_dirs();
        let theme_id: Option<String> = cc.storage.and_then(|s| eframe::get_value(s, KEY_THEME));
        let last_project: Option<PathBuf> = cc.storage.and_then(|s| eframe::get_value(s, KEY_PROJECT));
        let saved_dock: Option<Dock<Tab>> = cc.storage.and_then(|s| {
            eframe::get_value(s, KEY_LAYOUT).or_else(|| eframe::get_value::<crate::ui::dock::legacy::DockState<Tab>>(s, KEY_DOCK).map(|d| d.convert()))
        });
        let prefs: crate::editor::prefs::Prefs = cc.storage.and_then(|s| eframe::get_value(s, KEY_PREFS)).unwrap_or_else(|| crate::editor::prefs::Prefs {
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
        if let Err(e) = crate::ui::fonts::install_with(&cc.egui_ctx, ed.prefs.console_font.as_deref()) {
            ed.log(Level::Warn, format!("The Console's font wasn't loaded: {e}"));
        }
        ed.apply_language();
        ed.prefs.apply_node();
        ed.restyle();
        ed.theme.apply(&cc.egui_ctx);
        // What draws the app, and whether it had to fall back.
        let (graphics, software) = crate::app::graphics::describe(cc);
        if let Some(why) = crate::app::graphics::remembered() {
            ed.log(Level::Info, why);
        }
        if let Ok(why) = std::env::var(crate::app::graphics::REASON_VAR) {
            ed.log(Level::Warn, format!("{why}; drawing with {graphics} instead"));
        }
        ed.log(Level::Info, format!("Graphics: {graphics}"));
        eprintln!("graphics: {graphics}");
        if software {
            ed.log(Level::Warn, "No graphics card driver could be used, so the app is drawn in software, which is slower.");
        }

        let examples_dir = bundled_root.as_ref().map(|r| r.join("examples"));
        ed.examples_dir = examples_dir.clone();
        let bundled_plugins = bundled_root.map(|r| r.join("plugins"));
        if let Some(dir) = &bundled_plugins {
            // The ones the app needs; optional ones can be turned off and removed.
            ed.bundled_plugins = ed.reg.plugins.iter().filter(|p| p.dir.starts_with(dir) && !p.manifest.optional).map(|p| p.manifest.id.clone()).collect();
            ed.optional_plugins = ed.reg.plugins.iter().filter(|p| p.dir.starts_with(dir) && p.manifest.optional).map(|p| (p.manifest.id.clone(), p.manifest.name.clone())).collect();
        }
        // Plugins turned off or removed: loaded again without them.
        if !ed.prefs.disabled_plugins.is_empty() || !ed.prefs.removed_plugins.is_empty() {
            ed.reload_plugins();
        }
        // The last project; the example on the very first start; nothing (the
        // start page) when the last one was closed.
        let closed = last_project.as_ref().is_some_and(|p| p.as_os_str().is_empty());
        // A project folder (or its circuit.json) given on the command line, or
        // in XC_CIRCUIT_PROJECT, opens instead.
        let asked = std::env::var_os("XC_CIRCUIT_PROJECT")
            .map(PathBuf::from)
            .or_else(|| std::env::args_os().nth(1).map(PathBuf::from))
            .map(|p| if p.ends_with(circuit_core::project::PROJECT_FILE) { p.parent().map(Path::to_path_buf).unwrap_or(p) } else { p })
            .filter(|p| p.join(circuit_core::project::PROJECT_FILE).is_file());
        let project = if let Some(p) = asked {
            Some(p)
        } else if closed {
            None
        } else {
            last_project
                .filter(|p| ed.prefs.reopen_last_project && p.join(circuit_core::project::PROJECT_FILE).is_file())
                .or_else(|| examples_dir.as_deref().and_then(writable_example))
        };
        if let Some(p) = project {
            ed.open_project(&p);
            if let Some(root) = ed.project.as_ref().map(|p| p.root.clone()) {
                ed.prefs.remember_project(&root);
            }
        }
        // The saved layout is used as-is; opening the project queued its first workspace.
        let dock = match saved_dock {
            Some(mut d) => {
                let keys: Vec<String> = ed.project.as_ref().map(|p| p.workspaces.keys().cloned().collect()).unwrap_or_default();
                if repair_layout(&mut d) {
                    ed.log(Level::Warn, t!("The panel layout was damaged, so it was put back together."));
                }
                // Workspaces that are gone, and the Plugins tab from before it was the side bar's view.
                close_tabs(&mut d, |t| matches!(t, Tab::Canvas(k) if !keys.contains(k)) || *t == Tab::Plugins);
                d
            }
            None => default_layout(),
        };

        let mut app = Self::with_editor(cc, ed, dock, bundled_plugins);
        // The server console tab of the last run, connected again where it
        // was (without its project's server, the tab closes as before).
        if app.dock.contains(&Tab::ServerConsole) || app.ed.prefs.activity.iter().any(|k| k == "server-console") {
            app.bot.hosting.restore_console(&app.ed);
        }
        app.frame_stats = std::env::var_os("XC_FRAME_STATS").map(|_| (0, std::time::Instant::now()));
        app.fps = std::env::var_os("XC_FPS_OUT").map(|out| fps::Run::new(PathBuf::from(out), graphics.clone()));
        // A crash the user hasn't been told about yet.
        app.crash_offer = crate::app::crash::newest_since(app.ed.prefs.last_crash_seen);
        if app.ed.prefs.check_updates {
            app.updater.check(false);
            app.block_updates.check(false);
        }
        app
    }

    /// The window around an editor that is ready (plugins loaded, language and
    /// theme applied): without looking for updates or crashes, so tests can
    /// build it too.
    fn with_editor(cc: &eframe::CreationContext<'_>, mut ed: Editor, dock: Dock<Tab>, bundled_plugins: Option<PathBuf>) -> Self {
        let mut plugin_host = crate::plugins::host::PluginHost::new(cc.storage.and_then(|s| eframe::get_value(s, KEY_PLUGIN_STORAGE)).unwrap_or_default());
        plugin_host.sync(&mut ed, bundled_plugins.as_deref());

        let repaint = cc.egui_ctx.clone();
        let mut watcher = crate::plugins::watch::PluginWatcher::new(move || repaint.request_repaint());
        watcher.watch(&ed.watch_dirs());
        let setup = (!ed.prefs.setup_done).then(crate::pages::setup::Setup::new);
        CircuitApp {
            ed,
            dock,
            title: String::new(),
            show_about: false,
            new_project: None,
            confirm_close: false,
            allow_close: false,
            problem_count: 0,
            problems_found: None,
            dock_size: None,
            activity_slots: Vec::new(),
            start: Default::default(),
            updater: Default::default(),
            block_updates: Default::default(),
            updates_open: false,
            crash_offer: None,
            bot: BotUi::new(),
            stored: Default::default(),
            watcher,
            palette: None,
            plugin_host,
            bundled_plugins,
            agent_bridge: None,
            agent_bridge_failed: None,
            agent_shots: Vec::new(),
            setup,
            tour: None,
            dock_before_tour: None,
            dock_before_start: None,
            settings: Default::default(),
            restore: Default::default(),
            last_recovery: std::time::Instant::now(),
            last_check: std::time::Instant::now(),
            reveal: crate::app::reveal::Reveal::new(cc),
            frame_stats: None,
            fps: None,
        }
    }

}

impl eframe::App for CircuitApp {
    /// Whether the window has the focus, from Windows itself: without it egui
    /// draws no text cursor, and the focus event it waits for can be missed
    /// while the window is still hidden at startup.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if let Some(front) = self.reveal.is_foreground() {
            raw_input.focused = front;
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(mut run) = self.fps.take()
            && run.frame(self, &ctx, frame.info().cpu_usage)
        {
            self.fps = Some(run);
        }
        self.reveal.frame(&ctx);
        self.poll_updates(&ctx);
        // Before anything is drawn: no frame shows the panels without a project.
        self.swap_start_page();
        // XC_FRAME_STATS=1 prints how many frames a second are drawn, to find
        // what keeps the app busy.
        if let Some((frames, since)) = &mut self.frame_stats {
            *frames += 1;
            if since.elapsed() >= std::time::Duration::from_secs(1) {
                eprintln!("frames: {frames}/s");
                (*frames, *since) = (0, std::time::Instant::now());
            }
        }
        self.ed.ui_rects_before = std::mem::take(&mut self.ed.ui_rects);
        self.ed.tour_parts = self.tour.is_some();
        // The icon set, for kit pieces drawn where only a ui is at hand (plugins' notes).
        let icons_id = egui::Id::unique(crate::ui::kit::KIT_ICONS);
        if ctx.data(|d| d.get_temp::<std::collections::HashMap<String, std::path::PathBuf>>(icons_id).map(|m| m.len())) != Some(self.ed.reg.icons.len()) {
            ctx.data_mut(|d| d.insert_temp(icons_id, self.ed.reg.icons.clone()));
        }
        // The tour's practice project is never worth a "save changes?" question.
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.tour.is_none() && self.ed.unsaved_now() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.confirm_close = true;
        }
        // Shortcuts that work everywhere (also while typing), unless one is being recorded.
        if !self.ed.recording_shortcut(&ctx) {
            // Presses in a tab's window of its own count here too.
            let passed = self.dock.take_keys();
            if !passed.is_empty() {
                ctx.input_mut(|i| i.events.extend(passed.into_iter().filter(keymap::passed_on)));
            }
            let keys = self.ed.prefs.shortcuts.clone();
            let typing = ctx.text_edit_focused();
            let hit = |action| keymap::consume_while(&ctx, &keys, action, typing);
            if hit(Hotkey::Save) {
                self.ed.save();
                self.bot.restart_if_running(&mut self.ed, &ctx);
            }
            if hit(Hotkey::Run) && self.ed.project.is_some() {
                self.bot.run_pressed(&mut self.ed, &ctx);
            }
            if hit(Hotkey::Stop) {
                self.bot.stop_pressed(&mut self.ed);
            }
            if hit(Hotkey::Start) && self.ed.project.is_some() {
                self.bot.start_pressed(&mut self.ed, &ctx);
            }
            if hit(Hotkey::Restart) && self.ed.project.is_some() {
                self.bot.restart_pressed(&mut self.ed, &ctx);
            }
            let has_project = self.ed.project.is_some();
            if hit(Hotkey::BotSettings) && has_project {
                self.bot.open_settings(&mut self.ed);
            }
            if hit(Hotkey::SlashCommands) && has_project && self.ed.has_slash_commands() {
                crate::builders::slash::open(&mut self.ed);
            }
            if hit(Hotkey::ExportBot) && has_project {
                self.bot.open_export(&self.ed);
            }
            if hit(Hotkey::Upload) && has_project {
                self.bot.hosting.upload(&mut self.ed);
            }
            if hit(Hotkey::ServerConsole) && has_project {
                self.bot.hosting.open_console(&mut self.ed);
            }
            if hit(Hotkey::ServerSettings) && has_project {
                self.bot.hosting.open(&self.ed);
            }
            if hit(Hotkey::ToggleRemote) && has_project {
                if self.bot.hosting.can_control(&self.ed) {
                    let server = !self.bot.hosting.remote;
                    self.pick_remote(server);
                    let to = if server { t!("the server") } else { t!("this computer") };
                    self.ed.log(Level::Info, tf!("Run, Restart and Stop now act on {place}.", place = to));
                } else {
                    self.ed.log(Level::Info, t!("Run can switch to the server once a Calagopus server is set up (Bot › Server…)."));
                }
            }
            if hit(Hotkey::Settings) {
                self.open_settings();
            }
            if hit(Hotkey::CommandPalette) {
                self.palette = if self.palette.is_some() { None } else { Some(Default::default()) };
                self.ed.picker = None;
            }
            if hit(Hotkey::NewProject) {
                self.new_project_dialog();
            }
            if hit(Hotkey::OpenProject) {
                self.open_project_dialog();
            }
        }
        self.bot.poll(&mut self.ed, &ctx);
        self.bot.hosting.poll(&mut self.ed, &ctx);
        if let Some(Err(e)) = self.ed.bot_profile.poll() {
            self.ed.log(crate::editor::Level::Warn, tf!("Couldn't load the bot's name and picture: {error}", error = e));
        }
        if self.ed.bot_profile.busy() || self.ed.servers.busy() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
        // The servers' channels and roles, for id fields' pick buttons.
        if crate::ui::widgets::servers_wanted_again(ui.ctx())
            && let Some(p) = &self.ed.project
        {
            self.ed.servers.fetch(&p.root, &p.meta.mode);
            let s = &self.ed.servers;
            crate::ui::widgets::share_servers(ui.ctx(), crate::ui::widgets::ServersView { list: s.shown.clone(), busy: s.busy(), no_token: s.no_token, error: s.error.clone() });
        }
        if self.ed.servers.poll() || !crate::ui::widgets::servers_shared(ui.ctx()) {
            let s = &self.ed.servers;
            crate::ui::widgets::share_servers(ui.ctx(), crate::ui::widgets::ServersView { list: s.shown.clone(), busy: s.busy(), no_token: s.no_token, error: s.error.clone() });
        }
        self.run_plugins(&ctx);
        if self.watcher.poll() {
            self.ed.log(Level::Info, t!("Plugin files changed, reloading"));
            self.reload_plugins(&ctx);
        } else if self.watcher.waiting() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.edit_shortcuts(&ctx);
        // A plugin's .zip dropped on the window is installed.
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        for path in dropped.into_iter().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"))) {
            self.install_plugin_zip(&path, &ctx);
        }
        // A block that just ran glows and fades out, so repaint while one does;
        // its run time shows for a while, then fades. Failed blocks stay
        // marked, but their mark doesn't move: no repaints.
        self.ed.activity.retain(|_, a| a.error.is_some() || a.live());
        let glowing = self.ed.activity.values().any(|a| a.glow() > 0.0);
        let next_fade = self.ed.activity.values().filter_map(|a| a.ms).map(|(_, at)| crate::bot::TIME_SHOWN.saturating_sub(at.elapsed())).min();
        match next_fade {
            _ if glowing => ctx.request_repaint_after(std::time::Duration::from_millis(33)),
            // Fading out over the last second.
            Some(left) if left <= std::time::Duration::from_secs(1) => ctx.request_repaint_after(std::time::Duration::from_millis(33)),
            Some(left) => ctx.request_repaint_after(left - std::time::Duration::from_secs(1)),
            None => {}
        }

        let title = match &self.ed.project {
            Some(p) => format!("{}{} — XC™ Circuit", if self.ed.unsaved() { "• " } else { "" }, p.meta.name),
            None => "XC™ Circuit".into(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        let theme = self.ed.theme.clone();
        let (margin, height) = if self.ed.prefs.system_titlebar { (Margin::symmetric(8, 4), None) } else { (Margin { left: 8, right: 0, top: 0, bottom: 0 }, Some(crate::app::titlebar::HEIGHT)) };
        let mut top = egui::Panel::top("menu").frame(Frame::new().fill(theme.frame).inner_margin(margin));
        if let Some(h) = height {
            top = top.exact_size(h);
        }
        top.show(ui, |ui| {
            crate::ui::theme::compact(ui.style_mut());
            self.menu_bar(ui)
        });
        egui::Panel::bottom("status")
            .exact_size(24.0)
            .frame(Frame::new().fill(theme.status_bar).inner_margin(Margin::symmetric(4, 0)))
            .show(ui, |ui| self.status_bar(ui));
        let key = (self.ed.history.revision(), self.ed.reloads, self.ed.prefs.language.clone());
        if self.problems_found.as_ref().is_none_or(|(k, _)| *k != key) {
            self.problems_found = Some((key, panels::collect_problems(&mut self.ed)));
            // The custom ids events offer change with the project, too.
            let ids = crate::editor::custom_ids::collect_for(&mut self.ed).suggestions();
            crate::ui::widgets::share_suggestions(ui.ctx(), ids);
        }
        let mut problems = self.problems_found.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();
        problems.extend(self.plugin_host.problems());
        self.problem_count = problems.len();
        let bars = self.setup.is_none() && self.tour.is_none() && self.ed.project.is_some();
        if self.ed.prefs.activity_bar && bars {
            egui::Panel::left("activity")
                .exact_size(ACTIVITY_W)
                .resizable(false)
                .frame(Frame::new().fill(theme.frame).inner_margin(Margin::symmetric(0, 6)))
                .show(ui, |ui| self.activity(ui));
        }
        // What the activity bar's icon picked, beside it (and a second one under it).
        activity::tidy_side(&mut self.ed.prefs);
        let side = self.ed.prefs.side_shown.clone().filter(|k| self.ed.prefs.activity.contains(k)).and_then(|k| self.activity_tab(&k).map(|t| (k, t)));
        if bars && let Some((key, tab)) = side {
            egui::Panel::left("side-bar")
                .resizable(true)
                .default_size(270.0)
                .size_range(200.0..=560.0)
                .frame(Frame::new().fill(theme.panel))
                .show(ui, |ui| self.side_bar(ui, &key, tab, &problems));
        }
        // Filled, so the dock's rounded panel corners show the frame color and not
        // whatever is behind the window's content.
        if let Some(mut setup) = self.setup.take() {
            let done = egui::CentralPanel::default().frame(Frame::new().fill(theme.frame)).show(ui, |ui| crate::pages::setup::show(ui, &mut self.ed, &mut setup)).inner;
            match done {
                Some(crate::pages::setup::Done::Finished { tour }) => {
                    self.ed.prefs.setup_done = true;
                    if tour {
                        self.start_tour();
                    }
                }
                Some(crate::pages::setup::Done::Closed) => {}
                None => self.setup = Some(setup),
            }
        } else {
        egui::CentralPanel::default().frame(Frame::new().fill(theme.frame)).show(ui, |ui| {
            let running = self.bot.state != BotState::Stopped;
            self.sync_dialog_tabs(ui.ctx());
            let has_example = self.ed.examples_dir.as_ref().is_some_and(|e| e.join("DemoBot").is_dir());
            let mut viewer = Viewer {
                bot: &mut self.bot,
                ed: &mut self.ed,
                problem_count: self.problem_count,
                problems,
                running,
                plugin_host: &mut self.plugin_host,
                settings: &mut self.settings,
                restore: &mut self.restore,
                asks: Vec::new(),
                start: &mut self.start,
                has_example,
                start_pick: None,
                stored: &mut self.stored,
                side_ask: None,
                to_side: None,
                closed_side: Vec::new(),
                canvases: self.dock.tabs().filter(|t| matches!(t, Tab::Canvas(_))).count(),
                no_workspace: None,
            };
            let size = ui.available_size();
            if let Some(before) = self.dock_size
                && before != size
                && before.min_elem() > 100.0
                && size.min_elem() > 100.0
                && let Some(main) = &mut self.dock.main
            {
                keep_panel_sizes(main, size - before);
            }
            self.dock_size = Some(size);
            self.dock.show(ui, &theme, &mut viewer);
            let asks = std::mem::take(&mut viewer.asks);
            let start_pick = viewer.start_pick.take();
            let side_ask = viewer.side_ask.take();
            let to_side = viewer.to_side.take();
            let closed_side = std::mem::take(&mut viewer.closed_side);
            if let Some(ask) = viewer.no_workspace.take() {
                self.no_workspace_ask(ask);
            }
            for view in closed_side {
                self.back_to_bar(view);
            }
            if let Some(ask) = side_ask {
                self.plugins_side_ask(ask, ui.ctx());
            }
            if let Some((tab, pos)) = to_side {
                self.move_to_side(tab, pos);
            }
            if repair_layout(&mut self.dock) {
                self.ed.log(Level::Warn, t!("The panel layout was damaged, so it was put back together."));
            }
            self.settings_asks(asks, ui.ctx());
            if let Some(pick) = start_pick {
                self.start_page_pick(pick);
            }
        });
        }

        self.activity_drag(&ctx);
        self.keep_side_items();
        if self.setup.is_none()
            && let Some(mut tour) = self.tour.take()
        {
            let running = self.bot.state != BotState::Stopped;
            let ask = crate::pages::tour::show(&ctx, &mut self.ed, &mut tour, running);
            self.tour = Some(tour);
            match ask {
                Some(crate::pages::tour::Ask::Run) => self.bot.start(&mut self.ed, &ctx),
                Some(crate::pages::tour::Ask::SetUpNode) => self.setup = Some(crate::pages::setup::Setup::node_only()),
                Some(crate::pages::tour::Ask::End(exit)) => self.end_tour(exit),
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
            crate::app::titlebar::frame_edges(&ctx, &theme);
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
        // A downloaded update: installed now, and it starts the new version.
        if let Some(file) = self.updater.ready.take()
            && let Err(e) = crate::app::updater::Install::current().install(&file)
        {
            eprintln!("Couldn't install the update ({}): {e}", file.display());
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Mid-tour, keep the user's own layout and project.
        let dock = self.dock_before_tour.as_ref().filter(|_| self.tour.is_some()).or(self.dock_before_start.as_ref()).unwrap_or(&self.dock);
        eframe::set_value(storage, KEY_LAYOUT, dock);
        eframe::set_value(storage, KEY_THEME, &self.ed.theme.id);
        eframe::set_value(storage, KEY_PREFS, &self.ed.prefs);
        eframe::set_value(storage, KEY_PLUGIN_STORAGE, &self.plugin_host.storage);
        let project = match &self.tour {
            Some(t) => t.before.clone(),
            None => self.ed.project.as_ref().map(|p| p.root.clone()),
        };
        // An empty path: closed on purpose, so the start page shows next time.
        eframe::set_value(storage, KEY_PROJECT, &project.unwrap_or_default());
    }
}

const ACTIVITY_W: f32 = 44.0;
/// How often unsaved changes are copied for crash recovery.
const RECOVERY_EVERY: std::time::Duration = std::time::Duration::from_secs(30);
/// How often changes are looked for without an edit or input.
const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

/// Who makes XC Circuit, and where.
const CREATOR: &str = "XCraftTM";
const CREATOR_URL: &str = "https://github.com/XCraftTM";

/// A status bar entry: highlights on hover, can be clicked, and its text
/// can't be selected.
fn status_item(ui: &mut egui::Ui, theme: &crate::ui::theme::Theme, dot: Option<egui::Color32>, text: &str) -> egui::Response {
    let font = egui::FontId::proportional(11.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, theme.status_text);
    let dot_w = if dot.is_some() { 14.0 } else { 0.0 };
    let size = egui::vec2(galley.size().x + dot_w + 16.0, ui.available_height());
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    crate::ui::kit::describe(&resp, egui::Role::Button, text);
    if resp.hovered() {
        ui.painter().rect_filled(rect.shrink2(egui::vec2(0.0, 2.0)), 4.0, crate::ui::theme::mix(theme.status_bar, theme.status_text, 0.14));
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
