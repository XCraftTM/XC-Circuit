//! The menu bar: File, Edit, View, Bot, Plugins and Help.

use crate::ui::kit;
use super::*;

/// As in Windows' menu bars: while one menu is open, pointing at another
/// menu's name opens that one (and closes the first).
fn switch_on_hover(ctx: &egui::Context, tops: &[egui::Response]) {
    let ids: Vec<egui::Id> = tops.iter().map(egui::Popup::default_response_id).collect();
    let Some(open) = ids.iter().position(|id| egui::Popup::is_id_open(ctx, *id)) else { return };
    let Some(pos) = ctx.pointer_hover_pos() else { return };
    if let Some(i) = tops.iter().position(|r| r.interact_rect.contains(pos))
        && i != open
    {
        egui::Popup::close_id(ctx, ids[open]);
        egui::Popup::open_id(ctx, ids[i]);
    }
}

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
            let mut tops: Vec<egui::Response> = Vec::new();
            ui.spacing_mut().button_padding.x = 10.0;
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add(egui::Image::new(egui::include_image!("../../assets/icon.svg")).fit_to_exact_size(egui::vec2(18.0, 18.0)));
            ui.add_space(4.0);
            tops.push(ui.menu_button(t!("File"), |ui| {
                if kit::menu_item(ui, t!("New Project…"), Some(self.key_label(Hotkey::NewProject)), true).clicked() {
                    self.new_project_dialog();
                }
                if kit::menu_item(ui, t!("Open Project…"), Some(self.key_label(Hotkey::OpenProject)), true).clicked() {
                    self.open_project_dialog();
                }
                let recent = self.ed.prefs.recent_projects.clone();
                ui.add_enabled_ui(!recent.is_empty(), |ui| {
                    ui.menu_button(t!("Open Recent"), |ui| {
                        for path in &recent {
                            let name = Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
                            if kit::menu_item(ui, name, None, true).on_hover_text(path).clicked() {
                                self.open_recent(Path::new(path));
                                ui.close();
                            }
                        }
                        ui.separator();
                        if kit::menu_item(ui, t!("Clear the list"), None, true).clicked() {
                            self.ed.prefs.recent_projects.clear();
                            ui.close();
                        }
                    });
                });
                if kit::menu_item(ui, t!("Close Project"), None, self.ed.project.is_some()).clicked() {
                    self.close_project();
                }
                ui.separator();
                if kit::menu_item(ui, t!("Save"), Some(self.key_label(Hotkey::Save)), self.ed.project.is_some()).clicked() {
                    self.ed.save();
                    self.bot.restart_if_running(&mut self.ed, ui.ctx());
                }
                if kit::menu_item(ui, t!("Import workspaces…"), None, self.ed.project.is_some()).clicked() {
                    crate::editor::exchange::pick_import(&mut self.ed);
                }
                if kit::menu_item(ui, t!("Export workspaces…"), None, self.ed.project.as_ref().is_some_and(|p| !p.workspaces.is_empty())).clicked() {
                    crate::editor::exchange::open_export(&mut self.ed, crate::editor::exchange::Preselect::All);
                }
                ui.separator();
                if kit::menu_item(ui, t!("Get from Git…"), None, true).clicked() {
                    self.get_from_git();
                }
                if kit::menu_item(ui, "Git…", None, self.ed.project.is_some()).clicked() {
                    self.bot.git.open(&self.ed);
                }
                if kit::menu_item(ui, "Host…", None, true).clicked() {
                    self.bot.host.open();
                }
                if kit::menu_item(ui, t!("Live session…"), None, true).clicked() {
                    self.bot.live.open();
                }
                if kit::menu_item(ui, t!("Restore from backup…"), None, self.ed.project.is_some()).clicked() {
                    open_center(&mut self.dock, Tab::Backups);
                }
                if kit::menu_item(ui, t!("Settings…"), Some(self.key_label(Hotkey::Settings)), true).clicked() {
                    self.open_settings();
                }
                if kit::menu_item(ui, t!("Reload Plugins"), None, true).clicked() {
                    self.reload_plugins(ui.ctx());
                }
                ui.separator();
                if kit::menu_item(ui, t!("Exit"), None, true).clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
            }).response);
            tops.push(ui.menu_button(t!("Edit"), |ui| {
                let key = self.ed.undo_key();
                if kit::menu_item(ui, t!("Undo"), Some(self.key_label(Hotkey::Undo)), self.ed.history.can_undo(&key) || self.ed.edited).clicked() {
                    self.undo();
                }
                if kit::menu_item(ui, t!("Redo"), Some(self.key_label(Hotkey::Redo)), self.ed.history.can_redo(&key)).clicked() {
                    self.redo();
                }
                ui.separator();
                let has_sel = !self.ed.selection(&key).is_empty();
                if kit::menu_item(ui, t!("Cut"), Some("Ctrl+X".into()), has_sel).clicked() && self.ed.copy(ui.ctx()) {
                    self.ed.delete_selection();
                }
                if kit::menu_item(ui, t!("Copy"), Some("Ctrl+C".into()), has_sel).clicked() {
                    self.ed.copy(ui.ctx());
                }
                if kit::menu_item(ui, t!("Paste"), Some("Ctrl+V".into()), true).clicked()
                    && let Ok(text) = arboard::Clipboard::new().and_then(|mut c| c.get_text())
                {
                    self.ed.paste(&text, None);
                }
                if kit::menu_item(ui, t!("Duplicate"), Some(self.key_label(Hotkey::Duplicate)), has_sel).clicked() {
                    self.ed.duplicate();
                }
                if kit::menu_item(ui, t!("Delete"), Some(self.key_label(Hotkey::Delete)), has_sel).clicked() {
                    self.ed.delete_selection();
                }
                ui.separator();
                if kit::menu_item(ui, t!("Select all"), Some(self.key_label(Hotkey::SelectAll)), true).clicked() {
                    self.ed.select_all();
                }
            }).response);
            tops.push(ui.menu_button(t!("View"), |ui| {
                ui.label(RichText::new(t!("SIDE BAR")).size(10.5).color(theme.text_muted));
                for view in SideView::ALL {
                    let tab = Tab::Side(view);
                    let mut open = self.panel_present(&tab);
                    if kit::checkbox(ui, &theme, &mut open, view.name()).clicked() {
                        self.set_panel_open(tab, open);
                    }
                }
                ui.separator();
                ui.label(RichText::new(t!("PANELS")).size(10.5).color(theme.text_muted));
                for (tab, name) in Tab::PANELS {
                    let name = crate::i18n::t(name);
                    let mut open = self.panel_present(&tab);
                    if kit::checkbox(ui, &theme, &mut open, name).clicked() {
                        self.set_panel_open(tab, open);
                    }
                }
                ui.separator();
                ui.label(RichText::new(t!("THEME")).size(10.5).color(theme.text_muted));
                let themes: Vec<(String, String)> = self.ed.reg.themes.iter().map(|t| (t.id.clone(), t.name.clone())).collect();
                for (id, name) in themes {
                    if kit::radio(ui, &theme, self.ed.theme.id == id, name).clicked() {
                        self.ed.set_theme(&id);
                        self.ed.theme.apply(ui.ctx());
                    }
                }
                ui.separator();
                kit::checkbox(ui, &theme, &mut self.ed.prefs.activity_bar, t!("Show activity bar"));
                if kit::checkbox(ui, &theme, &mut self.ed.prefs.system_titlebar, t!("Use system title bar")).clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Decorations(self.ed.prefs.system_titlebar));
                }
                if kit::menu_item(ui, t!("Command Palette…"), Some(self.key_label(Hotkey::CommandPalette)), true).clicked() {
                    self.palette = Some(Default::default());
                }
                if kit::menu_item(ui, t!("Reset layout"), None, true).clicked() {
                    self.reset_layout();
                }
            }).response);
            let has_project = self.ed.project.is_some();
            tops.push(ui.menu_button(t!("Bot"), |ui| {
                let running = self.bot.shown_state() != BotState::Stopped;
                if kit::menu_item(ui, if running { t!("Restart") } else { t!("Run") }, Some(self.key_label(Hotkey::Run)), has_project).clicked() {
                    self.bot.run_pressed(&mut self.ed, ui.ctx());
                }
                if kit::menu_item(ui, t!("Stop"), Some(self.key_label(Hotkey::Stop)), running).clicked() {
                    self.bot.stop_pressed(&mut self.ed);
                }
                ui.separator();
                if self.ed.has_slash_commands() && kit::menu_item(ui, t!("Slash Commands…"), self.shortcut_shown(Hotkey::SlashCommands), has_project).clicked() {
                    crate::builders::slash::open(&mut self.ed);
                }
                if kit::menu_item(ui, t!("Settings…"), self.shortcut_shown(Hotkey::BotSettings), has_project).clicked() {
                    self.bot.open_settings(&mut self.ed);
                }
                if kit::menu_item(ui, t!("Export…"), self.shortcut_shown(Hotkey::ExportBot), has_project).clicked() {
                    self.bot.open_export(&self.ed);
                }
                ui.separator();
                if kit::menu_item(ui, t!("Remote…"), self.shortcut_shown(Hotkey::ServerSettings), has_project).clicked() {
                    self.bot.hosting.open(&self.ed);
                }
                let configured = has_project && crate::bot::hosting::HostingUi::configured(&self.ed);
                if kit::menu_item(ui, t!("Upload to the remote"), self.shortcut_shown(Hotkey::Upload), configured && self.bot.hosting.busy().is_none()).clicked() {
                    self.bot.hosting.upload(&mut self.ed);
                }
                let calagopus = has_project && crate::bot::hosting::settings_of(&self.ed).kind == circuit_runner::hosting::Kind::Calagopus;
                if calagopus && kit::menu_item(ui, t!("Remote console"), self.shortcut_shown(Hotkey::ServerConsole), true).clicked() {
                    self.bot.hosting.open_console(&mut self.ed);
                }
            }).response);
            tops.push(ui.menu_button(t!("Plugins"), |ui| {
                let mut any = false;
                for i in 0..self.plugin_host.plugins.len() {
                    let p = &self.plugin_host.plugins[i];
                    if p.commands().is_empty() && p.panels().is_empty() {
                        continue;
                    }
                    any = true;
                    ui.label(RichText::new(p.name.to_uppercase()).size(10.5).color(theme.text_muted));
                    let (id, commands, panels) = (p.id.clone(), p.commands().to_vec(), p.panels().to_vec());
                    for c in commands {
                        let mut b = kit::menu_item(ui, &c.title, None, true);
                        if !c.description.is_empty() {
                            b = b.on_hover_text(&c.description);
                        }
                        if b.clicked() {
                            self.plugin_host.queued.push(c.id.clone());
                        }
                    }
                    for panel in panels {
                        let tab = Tab::Plugin(id.clone(), panel.id.clone());
                        let mut open = self.panel_present(&tab);
                        if kit::checkbox(ui, &theme, &mut open, format!("{} panel", panel.title)).clicked() {
                            self.set_panel_open(tab, open);
                        }
                    }
                    ui.separator();
                }
                if !any {
                    ui.label(RichText::new(t!("No plugin adds commands or panels")).color(theme.text_muted));
                    ui.separator();
                }
                // The project's own plugins that wait for a yes: the question again.
                if !self.ed.plugins_waiting.is_empty() && kit::menu_item(ui, t!("Load This Project's Plugins…"), None, true).clicked() {
                    self.ed.plugin_ask = true;
                }
                if kit::menu_item(ui, t!("Install from Zip…"), None, true).clicked() {
                    self.pick_plugin_zip(ui.ctx());
                }
                if kit::menu_item(ui, t!("Manage Plugins…"), None, true).clicked() {
                    self.open_plugins();
                }
            }).response);
            tops.push(ui.menu_button(t!("Help"), |ui| {
                if self.ed.examples_dir.is_some()
                    && kit::menu_item(ui, t!("Open Example: DemoBot"), None, true).clicked()
                    && let Some(example) = self.ed.examples_dir.as_deref().and_then(writable_example)
                {
                    self.switch_project(&example);
                }
                if kit::menu_item(ui, t!("Report a Bug…"), None, true).on_hover_text(t!("Opens a bug report on GitHub with your version filled in")).clicked() {
                    let _ = open::that(crate::app::crash::report_url(crate::app::crash::Report::Bug));
                }
                if kit::menu_item(ui, t!("Suggest a Feature…"), None, true).clicked() {
                    let _ = open::that(crate::app::crash::report_url(crate::app::crash::Report::Feature));
                }
                if kit::menu_item(ui, t!("Open Crash Logs"), None, true).on_hover_text(t!("The folder where XC Circuit writes what happened when it crashed")).clicked() {
                    let dir = crate::app::crash::logs_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = open::that(dir);
                }
                ui.separator();
                if kit::menu_item(ui, t!("Check for Updates…"), None, true).on_hover_text(t!("For the app, and for the bundled blocks")).clicked() {
                    self.check_for_updates();
                }
                if kit::menu_item(ui, t!("Undo Block Updates"), None, crate::plugins::updates::any_installed() && !self.block_updates.installing())
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
                if kit::menu_item(ui, t!("Welcome and setup…"), None, true).clicked() {
                    self.setup = Some(crate::pages::setup::Setup::new());
                }
                if kit::menu_item(ui, t!("Take the tour"), None, self.tour.is_none()).clicked() {
                    self.start_tour();
                }
                if kit::menu_item(ui, t!("About XC™ Circuit"), None, true).clicked() {
                    self.show_about = true;
                }
            }).response);
            switch_on_hover(ui.ctx(), &tops);
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
        let state = self.bot.shown_state();
        let remote = self.bot.hosting.remote;
        let running = state != BotState::Stopped;
        let stop_tip = if remote { t!("Stop the bot on its remote (Shift+F5)") } else { t!("Stop the bot (Shift+F5)") };
        if running && kit::menu_item(ui, t!("Stop"), None, true).on_hover_text(stop_tip).clicked() {
            self.bot.stop_pressed(&mut self.ed);
        }
        let stopping = remote && self.bot.hosting.watch.as_ref().is_some_and(|w| w.state() == "stopping");
        let label = match state {
            BotState::Stopped => t!("Run"),
            BotState::Starting if stopping => t!("Stopping…"),
            BotState::Starting => t!("Starting…"),
            BotState::Running => t!("Restart"),
        };
        let run = egui::Button::new(RichText::new(label).strong().color(egui::Color32::from_rgb(4, 33, 13)));
        let run = ui.add_enabled_ui(self.ed.project.is_some(), |ui| crate::ui::kit::styled_button(ui, &theme, run, theme.action, egui::Stroke::NONE)).inner;
        self.ed.ui_rects.insert("run".into(), run.rect);
        let run_tip = if remote { t!("Upload what changed and start the bot on its remote (F5)") } else { t!("Run the bot (F5)") };
        if run.on_hover_text(run_tip).clicked() {
            self.bot.run_pressed(&mut self.ed, ui.ctx());
        }
        // Local | Server, when the project has a server that can be run from here.
        if self.ed.project.is_some() && self.bot.hosting.can_control(&self.ed) {
            ui.add_space(6.0);
            if let Some(server) = crate::bot::remote::switch(ui, &theme, remote) {
                self.pick_remote(server);
            }
        }
    }

    /// Local or Server beside Run: which bot Run, Restart and Stop (and the
    /// status bar) are about, kept per project. Picking Server stops the bot
    /// running here, so the two don't both answer.
    pub(super) fn pick_remote(&mut self, server: bool) {
        let Some(root) = self.ed.project.as_ref().map(|p| p.root.display().to_string()) else { return };
        self.ed.prefs.remote_bots.retain(|r| *r != root);
        if server {
            self.ed.prefs.remote_bots.push(root);
            if self.bot.state != BotState::Stopped {
                self.bot.stop(&mut self.ed);
                self.ed.log(Level::Info, t!("Stopped the bot here: Run, Restart and Stop now act on its remote."));
            }
        }
    }
}
