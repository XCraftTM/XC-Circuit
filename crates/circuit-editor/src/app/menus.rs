//! The menu bar: File, Edit, View, Bot, Plugins and Help.

use super::*;

impl CircuitApp {
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.ed.theme.clone();
        let custom = !self.ed.prefs.system_titlebar;
        let bar = ui.max_rect();
        if custom {
            crate::app::titlebar::drag_region(ui, bar);
        }
        // The search box sits in the middle, where the window title would be.
        let search = egui::Rect::from_center_size(bar.center(), egui::vec2(crate::app::titlebar::SEARCH_W, 24.0));
        let label = match &self.ed.project {
            Some(p) => tf!("Search {project}", project = p.meta.name),
            None => t!("Search").into(),
        };
        self.ed.ui_rects.insert("search".into(), search);
        if crate::app::titlebar::search_box(ui, search, &theme, &label, &self.key_label(Hotkey::CommandPalette)) {
            self.palette = Some(Default::default());
        }
        let row = |ui: &mut egui::Ui, layout| ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(layout));
        let mut left = row(ui, egui::Layout::left_to_right(egui::Align::Center));
        if self.tour.is_some() {
            left.add(egui::Image::new(egui::include_image!("../../assets/icon.svg")).fit_to_exact_size(egui::vec2(18.0, 18.0)));
            left.add_space(6.0);
            left.label(RichText::new(t!("Tour")).strong().color(theme.accent));
        } else {
        // The menus fill the bar's height, so they're easy to hit. (Set before
        // the menu bar: it sizes its row from this.)
        left.spacing_mut().interact_size.y = bar.height();
        egui::MenuBar::new().ui(&mut left, |ui| {
            ui.spacing_mut().button_padding.x = 10.0;
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add(egui::Image::new(egui::include_image!("../../assets/icon.svg")).fit_to_exact_size(egui::vec2(18.0, 18.0)));
            ui.add_space(4.0);
            ui.menu_button(t!("File"), |ui| {
                if ui.add(egui::Button::new(t!("New Project…")).shortcut_text(self.key_label(Hotkey::NewProject))).clicked() {
                    self.new_project_dialog();
                }
                if ui.add(egui::Button::new(t!("Open Project…")).shortcut_text(self.key_label(Hotkey::OpenProject))).clicked() {
                    self.open_project_dialog();
                }
                let recent = self.ed.prefs.recent_projects.clone();
                ui.add_enabled_ui(!recent.is_empty(), |ui| {
                    ui.menu_button(t!("Open Recent"), |ui| {
                        for path in &recent {
                            let name = Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
                            if ui.button(name).on_hover_text(path).clicked() {
                                self.open_recent(Path::new(path));
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.button(t!("Clear the list")).clicked() {
                            self.ed.prefs.recent_projects.clear();
                            ui.close();
                        }
                    });
                });
                if ui.add_enabled(self.ed.project.is_some(), egui::Button::new(t!("Close Project"))).clicked() {
                    self.close_project();
                }
                ui.separator();
                if ui.add_enabled(self.ed.project.is_some(), egui::Button::new(t!("Save")).shortcut_text(self.key_label(Hotkey::Save))).clicked() {
                    self.ed.save();
                    self.bot.restart_if_running(&mut self.ed, ui.ctx());
                }
                if ui.add_enabled(self.ed.project.is_some(), egui::Button::new(t!("Import workspaces…"))).clicked() {
                    crate::editor::exchange::pick_import(&mut self.ed);
                }
                if ui.add_enabled(self.ed.project.as_ref().is_some_and(|p| !p.workspaces.is_empty()), egui::Button::new(t!("Export workspaces…"))).clicked() {
                    crate::editor::exchange::open_export(&mut self.ed, crate::editor::exchange::Preselect::All);
                }
                ui.separator();
                if ui.add_enabled(self.ed.project.is_some(), egui::Button::new(t!("Restore from backup…"))).clicked() {
                    open_center(&mut self.dock, Tab::Backups);
                }
                if ui.add(egui::Button::new(t!("Settings…")).shortcut_text(self.key_label(Hotkey::Settings))).clicked() {
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
                if ui.add_enabled(self.ed.history.can_undo(&key) || self.ed.edited, egui::Button::new(t!("Undo")).shortcut_text(self.key_label(Hotkey::Undo))).clicked() {
                    self.undo();
                }
                if ui.add_enabled(self.ed.history.can_redo(&key), egui::Button::new(t!("Redo")).shortcut_text(self.key_label(Hotkey::Redo))).clicked() {
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
                if ui.add_enabled(has_sel, egui::Button::new(t!("Duplicate")).shortcut_text(self.key_label(Hotkey::Duplicate))).clicked() {
                    self.ed.duplicate();
                }
                if ui.add_enabled(has_sel, egui::Button::new(t!("Delete")).shortcut_text(self.key_label(Hotkey::Delete))).clicked() {
                    self.ed.delete_selection();
                }
                ui.separator();
                if ui.add(egui::Button::new(t!("Select all")).shortcut_text(self.key_label(Hotkey::SelectAll))).clicked() {
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
                if ui.add(egui::Button::new(t!("Command Palette…")).shortcut_text(self.key_label(Hotkey::CommandPalette))).clicked() {
                    self.palette = Some(Default::default());
                }
                if ui.button(t!("Reset layout")).clicked() {
                    self.reset_layout();
                }
            });
            let has_project = self.ed.project.is_some();
            ui.menu_button(t!("Bot"), |ui| {
                let running = self.bot.state != BotState::Stopped;
                if ui.add_enabled(has_project, egui::Button::new(if running { t!("Restart") } else { t!("Run") }).shortcut_text(self.key_label(Hotkey::Run))).clicked() {
                    self.bot.start(&mut self.ed, ui.ctx());
                }
                if ui.add_enabled(running, egui::Button::new(t!("Stop")).shortcut_text(self.key_label(Hotkey::Stop))).clicked() {
                    self.bot.stop(&mut self.ed);
                }
                ui.separator();
                if self.ed.has_slash_commands() && ui.add_enabled(has_project, egui::Button::new(t!("Slash Commands…"))).clicked() {
                    crate::builders::slash::open(&mut self.ed);
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
                    ui.label(RichText::new(t!("No plugin adds commands or panels")).color(theme.text_muted));
                    ui.separator();
                }
                if ui.button(t!("Install from Zip…")).clicked() {
                    self.pick_plugin_zip(ui.ctx());
                }
                if ui.button(t!("Manage Plugins…")).clicked() {
                    self.open_plugins();
                }
            });
            ui.menu_button(t!("Help"), |ui| {
                if self.ed.examples_dir.is_some()
                    && ui.button(t!("Open Example: DemoBot")).clicked()
                    && let Some(example) = self.ed.examples_dir.as_deref().and_then(writable_example)
                {
                    self.switch_project(&example);
                }
                if ui.button(t!("Report a Bug…")).on_hover_text(t!("Opens a bug report on GitHub with your version filled in")).clicked() {
                    let _ = open::that(crate::app::crash::report_url(crate::app::crash::Report::Bug));
                }
                if ui.button(t!("Suggest a Feature…")).clicked() {
                    let _ = open::that(crate::app::crash::report_url(crate::app::crash::Report::Feature));
                }
                if ui.button(t!("Open Crash Logs")).on_hover_text(t!("The folder where XC Circuit writes what happened when it crashed")).clicked() {
                    let dir = crate::app::crash::logs_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = open::that(dir);
                }
                ui.separator();
                if ui.add_enabled(!self.updater.busy(), egui::Button::new(t!("Check for Updates…"))).on_hover_text(t!("For the app, and for the bundled blocks")).clicked() {
                    self.updater.check(true);
                    self.block_updates.check(true);
                }
                if ui
                    .add_enabled(crate::plugins::updates::any_installed() && !self.block_updates.installing(), egui::Button::new(t!("Undo Block Updates")))
                    .on_hover_text(t!("Back to the blocks this app came with"))
                    .clicked()
                {
                    match crate::plugins::updates::undo() {
                        Ok(()) => {
                            self.reload_plugins(ui.ctx());
                            self.ed.log(Level::Success, t!("Block updates removed: the blocks this app came with are back"));
                        }
                        Err(e) => self.ed.log(Level::Error, tf!("Couldn't remove the block updates: {error}", error = e)),
                    }
                    ui.close();
                }
                if ui.button(t!("Welcome and setup…")).clicked() {
                    self.setup = Some(crate::pages::setup::Setup::new());
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
            crate::app::titlebar::window_buttons(ui, &theme);
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
        let run = ui.add_enabled_ui(self.ed.project.is_some(), |ui| crate::ui::dialog::styled_button(ui, &theme, run, theme.action, egui::Stroke::NONE)).inner;
        self.ed.ui_rects.insert("run".into(), run.rect);
        if run.on_hover_text(t!("Run the bot (F5)")).clicked() {
            self.bot.start(&mut self.ed, ui.ctx());
        }
    }
}
