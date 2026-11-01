//! The window: menu bar, dockable panels, status bar.

use std::env;
use std::path::{Path, PathBuf};

use circuit_core::Project;
use eframe::egui::{self, Frame, Id, Margin, RichText, ViewportCommand, WidgetText};
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};
use serde::{Deserialize, Serialize};

use crate::editor::{Action, Editor, Level};
use crate::{canvas, panels, picker};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tab {
    Workspaces,
    Canvas(String),
    Inspector,
    Console,
    Problems,
}

impl Tab {
    const PANELS: [(Tab, &'static str); 4] =
        [(Tab::Workspaces, "Workspaces"), (Tab::Inspector, "Inspector"), (Tab::Console, "Console"), (Tab::Problems, "Problems")];
}

const KEY_DOCK: &str = "dock";
const KEY_THEME: &str = "theme";
const KEY_PROJECT: &str = "project";

pub struct CircuitApp {
    ed: Editor,
    dock: DockState<Tab>,
    examples_dir: Option<PathBuf>,
    title: String,
    show_about: bool,
    confirm_close: bool,
    allow_close: bool,
    problem_count: usize,
}

impl CircuitApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let (plugin_dirs, bundled_root) = find_plugin_dirs();
        let theme_id: Option<String> = cc.storage.and_then(|s| eframe::get_value(s, KEY_THEME));
        let last_project: Option<PathBuf> = cc.storage.and_then(|s| eframe::get_value(s, KEY_PROJECT));
        let saved_dock: Option<DockState<Tab>> = cc.storage.and_then(|s| eframe::get_value(s, KEY_DOCK));

        let mut ed = match Editor::new(plugin_dirs, theme_id.as_deref()) {
            Ok(ed) => ed,
            Err(e) => panic!("could not start the block sandbox: {e:#}"),
        };
        ed.theme.apply(&cc.egui_ctx);

        let examples_dir = bundled_root.map(|r| r.join("examples"));
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

        CircuitApp { ed, dock, examples_dir, title: String::new(), show_about: false, confirm_close: false, allow_close: false, problem_count: 0 }
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
        let surface = self.dock.main_surface_mut();
        match tab {
            Tab::Workspaces => {
                surface.split_left(NodeIndex::root(), 0.17, vec![tab]);
            }
            Tab::Inspector => {
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
        self.dock.retain_tabs(|t| !matches!(t, Tab::Canvas(_)));
        self.ed.open_project(dir);
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.ed.theme.clone();
        egui::MenuBar::new().ui(ui, |ui| {
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
                }
                if ui.button("Reload Plugins").clicked() {
                    self.ed.reload_plugins();
                    self.ed.theme.apply(ui.ctx());
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
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
                if ui.button("Reset layout").clicked() {
                    let canvases: Vec<Tab> = self.dock.iter_all_tabs().map(|(_, t)| t.clone()).filter(|t| matches!(t, Tab::Canvas(_))).collect();
                    self.dock = default_layout();
                    for t in canvases {
                        if let Tab::Canvas(k) = t {
                            open_canvas(&mut self.dock, k);
                        }
                    }
                }
            });
            ui.menu_button("Bot", |ui| {
                ui.add_enabled(false, egui::Button::new("Run  (next milestone)"));
                ui.add_enabled(false, egui::Button::new("Export…  (next milestone)"));
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About XC™ Circuit").clicked() {
                    self.show_about = true;
                }
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let t = self.ed.theme.clone();
        let text = |s: String| RichText::new(s).size(11.5).color(t.status_text);
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            match &self.ed.project {
                Some(p) => {
                    ui.label(text(p.meta.name.clone()));
                    let mode = self.ed.reg.mode(&p.meta.mode).map(|m| m.name.clone()).unwrap_or(p.meta.mode.clone());
                    ui.label(text(mode));
                }
                None => {
                    ui.label(text("No project".into()));
                }
            }
            ui.label(text(format!("{} plugins · {} blocks", self.ed.reg.plugins.len(), self.ed.reg.blocks.len())));
            if self.problem_count > 0 {
                ui.label(text(format!("⚠ {} problems", self.problem_count)));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(text(format!("Theme: {}", t.name)));
                ui.label(text(if self.ed.dirty { "Unsaved changes".into() } else { "Saved".into() }));
            });
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.show_about {
            egui::Window::new("About XC™ Circuit").collapsible(false).resizable(false).open(&mut self.show_about).show(ctx, |ui| {
                ui.add(egui::Image::new(egui::include_image!("../assets/icon.svg")).fit_to_exact_size(egui::vec2(56.0, 56.0)));
                ui.heading("XC™ Circuit");
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.label("Build bots by connecting blocks.");
            });
        }
        if self.confirm_close {
            egui::Modal::new(Id::new("confirm-close")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.heading("Save changes?");
                ui.label("This project has changes that are not saved yet.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save and close").clicked() {
                        self.ed.save();
                        self.allow_close = true;
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                    if ui.button("Close without saving").clicked() {
                        self.allow_close = true;
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm_close = false;
                    }
                });
            });
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
        egui::Panel::top("menu")
            .frame(Frame::new().fill(theme.frame).inner_margin(Margin::symmetric(8, 4)))
            .show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status")
            .exact_size(24.0)
            .frame(Frame::new().fill(theme.status_bar).inner_margin(Margin::symmetric(12, 0)))
            .show(ui, |ui| self.status_bar(ui));
        let problems = panels::collect_problems(&self.ed);
        self.problem_count = problems.len();
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let style = theme.dock_style(ui.style());
            let mut viewer = Viewer { ed: &mut self.ed, problem_count: self.problem_count, problems };
            DockArea::new(&mut self.dock)
                .style(style)
                .show_add_buttons(false)
                .show_leaf_close_all_buttons(false)
                .show_leaf_collapse_buttons(false)
                .show_inside(ui, &mut viewer);
        });

        picker::show(&ctx, &mut self.ed);
        self.apply_actions();
        self.dialogs(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, KEY_DOCK, &self.dock);
        eframe::set_value(storage, KEY_THEME, &self.ed.theme.id);
        if let Some(p) = &self.ed.project {
            eframe::set_value(storage, KEY_PROJECT, &p.root);
        }
    }
}

struct Viewer<'a> {
    ed: &'a mut Editor,
    problem_count: usize,
    problems: Vec<panels::ProblemItem>,
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
            Tab::Canvas(key) => self.ed.workspace(key).map(|w| w.name.clone()).unwrap_or_else(|| key.clone()).into(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match tab {
            Tab::Workspaces => panels::workspaces(ui, self.ed),
            Tab::Inspector => panels::inspector(ui, self.ed),
            Tab::Console => panels::console(ui, self.ed),
            Tab::Problems => panels::problems(ui, self.ed, std::mem::take(&mut self.problems)),
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
    let mut dock = DockState::new(vec![Tab::Console, Tab::Problems]);
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
        Some(np) if matches!(dock.find_tab_from(|t| matches!(t, Tab::Canvas(_))), Some(_)) => {
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
