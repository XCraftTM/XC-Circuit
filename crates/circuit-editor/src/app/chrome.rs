//! The status bar, the tour and settings.

use super::*;

impl CircuitApp {
    /// What the Plugins tab's buttons asked for.
    pub(super) fn plugins_side_ask(&mut self, ask: crate::plugins::manager::SideAsk, ctx: &egui::Context) {
        match ask {
            crate::plugins::manager::SideAsk::Enable(id, on) => {
                self.ed.prefs.disabled_plugins.retain(|p| *p != id);
                if !on {
                    self.ed.prefs.disabled_plugins.push(id);
                }
                self.reload_plugins(ctx);
            }
            crate::plugins::manager::SideAsk::Remove(id) => self.remove_plugin(&id, ctx),
            crate::plugins::manager::SideAsk::AddBack(id) => {
                self.ed.prefs.removed_plugins.retain(|p| *p != id);
                self.ed.prefs.disabled_plugins.retain(|p| *p != id);
                self.reload_plugins(ctx);
            }
            crate::plugins::manager::SideAsk::Reload => self.reload_plugins(ctx),
            crate::plugins::manager::SideAsk::Install => self.pick_plugin_zip(ctx),
            crate::plugins::manager::SideAsk::OpenFolder => {
                let dir = crate::bot::app_data_dir().join("plugins");
                let _ = std::fs::create_dir_all(&dir);
                let _ = open::that(dir);
            }
        }
    }

    /// Starts the guided tour on a fresh practice project, with a reduced layout.
    pub(super) fn start_tour(&mut self) {
        if self.tour.is_some() {
            return;
        }
        let dir = crate::bot::app_data_dir().join("tour");
        if let Err(e) = crate::pages::tour::create_project(&dir) {
            self.ed.log(Level::Error, format!("Couldn't prepare the tour: {e:#}"));
            return;
        }
        self.bot.stop(&mut self.ed);
        let before = self.ed.project.as_ref().map(|p| p.root.clone());
        self.dock_before_tour = Some(self.dock.clone());
        self.load_project(&dir);
        self.dock = tour_layout();
        self.ed.log.clear();
        self.tour = Some(crate::pages::tour::Tour { step: 0, before });
    }

    pub(super) fn end_tour(&mut self, exit: crate::pages::tour::Exit) {
        use crate::pages::tour::Exit;
        let Some(tour) = self.tour.take() else { return };
        self.bot.stop(&mut self.ed);
        // The practice project isn't kept.
        self.ed.forget_changes();
        let target = match exit {
            Exit::DemoBot => self.ed.examples_dir.as_deref().and_then(writable_example),
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

    pub(super) fn open_settings(&mut self) {
        open_center(&mut self.dock, Tab::Settings);
    }

    /// The activity bar's gear: opens Settings, or closes it when it is the tab in view.
    pub(super) fn toggle_settings(&mut self) {
        let focused = self.dock.focused() == Some(&Tab::Settings);
        if focused {
            close_tabs(&mut self.dock, |t| *t == Tab::Settings);
        } else {
            self.open_settings();
        }
    }

    pub(super) fn settings_asks(&mut self, asks: Vec<crate::pages::settings::Ask>, ctx: &egui::Context) {
        use crate::pages::settings::Ask;
        for ask in asks {
            match ask {
                Ask::BotSettings => self.bot.open_settings(&mut self.ed),
                Ask::SlashCommands => crate::builders::slash::open(&mut self.ed),
                Ask::Export => self.bot.open_export(&self.ed),
                Ask::ManagePlugins => self.open_plugins(),
                Ask::ReloadPlugins => self.reload_plugins(ctx),
                Ask::ResetLayout => self.reset_layout(),
                Ask::NodeSetup => self.setup = Some(crate::pages::setup::Setup::node_only()),
                Ask::OpenBackups => open_center(&mut self.dock, Tab::Backups),
                Ask::ConsoleFont => {
                    if let Err(e) = crate::ui::fonts::install_with(ctx, self.ed.prefs.console_font.as_deref()) {
                        self.ed.log(Level::Warn, format!("The Console's font wasn't loaded: {e}"));
                    }
                }
            }
        }
    }

    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        let t = self.ed.theme.clone();
        let ctx = ui.ctx().clone();
        ui.spacing_mut().item_spacing.x = 2.0;
        let bar = ui.max_rect();
        let shown = self.bot.shown_state();
        // With Server picked beside Run: the server's state (red when it can't be reached).
        let remote = self.bot.hosting.watch.as_ref().filter(|_| self.bot.hosting.remote).map(|w| (w.label(), w.tip(), w.problem.is_some() && w.last.is_none()));
        let dot = match (shown, &remote) {
            (_, Some((_, _, true))) => t.error,
            (BotState::Stopped, _) => t.status_text.gamma_multiply(0.45),
            (BotState::Starting, _) => egui::Color32::from_rgb(0xFF, 0xB4, 0x54),
            (BotState::Running, _) => t.action,
        };
        let state = match (shown, &remote) {
            (_, Some((label, _, _))) => *label,
            (BotState::Stopped, None) => t!("Bot stopped"),
            (BotState::Starting, None) => t!("Bot starting"),
            (BotState::Running, None) => t!("Bot running"),
        };
        let has_project = self.ed.project.is_some();
        let mut left = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let running = shown != BotState::Stopped;
        let tip = match (&remote, running) {
            (Some((_, tip, _)), true) => format!("{tip}\n{}", t!("Click to stop it (Shift+F5)")),
            (Some((_, tip, _)), false) => format!("{tip}\n{}", t!("Click to upload and start it (F5)")),
            (None, true) => t!("Stop the bot (Shift+F5)").to_string(),
            (None, false) => t!("Run the bot (F5)").to_string(),
        };
        if status_item(&mut left, &t, Some(dot), state).on_hover_text(tip).clicked() && has_project {
            if running { self.bot.stop_pressed(&mut self.ed) } else { self.bot.run_pressed(&mut self.ed, &ctx) }
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
                self.show_panel(Tab::Problems);
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
        if let Some(r) = &self.updater.available
            && status_item(&mut right, &t, Some(t.accent), &tf!("Update {version} available", version = r.version)).on_hover_text(t!("What's new, and the download")).clicked()
        {
            self.updater.show = true;
        }
        // Uploading to the bot's server: under way, or how the last one went.
        if has_project && crate::bot::hosting::HostingUi::configured(&self.ed) {
            let (dot, text, tip) = match (self.bot.hosting.busy(), &self.bot.hosting.outcome) {
                (Some(what), _) => (Some(t.accent), tf!("{what} …", what = what), t!("Bot › Remote…").to_string()),
                (None, Some(o)) if o.ok => (None, tf!("Uploaded {ago}", ago = crate::bot::ago(o.at.elapsed())), o.text.clone()),
                (None, Some(o)) => (Some(t.error), t!("Upload failed").to_string(), o.text.clone()),
                (None, None) => (None, t!("Upload to the remote").to_string(), t!("Bot › Remote…").to_string()),
            };
            if status_item(&mut right, &t, dot, &text).on_hover_text(tip).clicked() {
                self.bot.hosting.open(&self.ed);
            }
        }
        // The project's repository: what waits to be stored or fetched.
        if has_project && let Some((dot, text)) = self.bot.git.status() {
            let dot = dot.then_some(t.accent);
            if status_item(&mut right, &t, dot, &text).on_hover_text(t!("File › Git…")).clicked() {
                self.bot.git.open(&self.ed);
            }
        }
        let saved = if self.ed.unsaved() {
            t!("Unsaved changes")
        } else if self.ed.saver.busy() {
            t!("Saving…")
        } else {
            t!("Saved")
        };
        if status_item(&mut right, &t, None, saved).on_hover_text(t!("Save (Ctrl+S)")).clicked() && has_project {
            self.ed.save();
            self.bot.restart_if_running(&mut self.ed, &ctx);
        }
    }

    pub(super) fn reset_layout(&mut self) {
        let fresh = crate::editor::prefs::Prefs::default();
        self.ed.prefs.activity = fresh.activity;
        self.ed.prefs.side_shown = fresh.side_shown;
        let canvases: Vec<Tab> = self.dock.tabs().filter(|t| matches!(t, Tab::Canvas(_))).cloned().collect();
        self.dock = default_layout();
        for t in canvases {
            if let Tab::Canvas(k) = t {
                open_canvas(&mut self.dock, k);
            }
        }
    }

    pub(super) fn toggle_panel(&mut self, tab: Tab) {
        // One in the activity bar is shown or hidden in the side bar.
        if let Some(key) = tab.side_key()
            && self.ed.prefs.activity.contains(&key)
        {
            let shown = self.ed.prefs.side_shown.as_deref() == Some(key.as_str());
            self.ed.prefs.side_shown = (!shown).then_some(key);
            return;
        }
        if self.dock.contains(&tab) {
            close_tabs(&mut self.dock, |t| *t == tab);
            if let Tab::Side(view) = tab {
                self.back_to_bar(view);
            }
            return;
        }
        place_panel(&mut self.dock, tab);
    }

    /// The Bot Settings tab shows the settings as they are after undo or redo.
    pub(super) fn reopen_bot_settings(&mut self) {
        if self.ed.undo_project && self.bot.settings.is_some() {
            self.bot.open_settings(&mut self.ed);
        }
    }

    /// The editors that used to be dialogs are tabs: one is shown while its
    /// draft exists, and closed once it is saved or cancelled.
    pub(super) fn sync_dialog_tabs(&mut self, ctx: &egui::Context) {
        let wanted = [
            (Tab::EmbedBuilder, self.ed.embed_edit.is_some(), egui::vec2(1060.0, 760.0)),
            (Tab::LayoutBuilder, self.ed.layout_edit.is_some(), egui::vec2(1240.0, 800.0)),
            (Tab::ModalBuilder, self.ed.modal_edit.is_some(), egui::vec2(1200.0, 800.0)),
            (Tab::CanvasBuilder, self.ed.canvas_edit.is_some(), egui::vec2(1280.0, 820.0)),
            (Tab::SlashCommands, self.ed.slash_edit.is_some(), egui::vec2(1320.0, 720.0)),
            (Tab::BotSettings, self.bot.settings.is_some(), egui::vec2(820.0, 620.0)),
            (Tab::Export, self.bot.export.is_some(), egui::vec2(560.0, 620.0)),
            (Tab::Hosting, self.bot.hosting.draft.is_some(), egui::vec2(680.0, 760.0)),
            (Tab::Git, self.bot.git.open, egui::vec2(680.0, 760.0)),
            (Tab::Host, self.bot.host.open, egui::vec2(680.0, 720.0)),
            (Tab::ServerConsole, self.bot.hosting.console.is_some(), egui::vec2(900.0, 560.0)),
            (Tab::WorkspaceExport, self.ed.ws_export.is_some(), egui::vec2(660.0, 680.0)),
            (Tab::WorkspaceImport, self.ed.ws_import.is_some(), egui::vec2(660.0, 680.0)),
        ];
        for (tab, open, size) in wanted {
            // As big as it was last time (when it was a window of its own).
            let key = window_key(&tab);
            match (open, self.dock.contains(&tab)) {
                (true, false) => match self.ed.prefs.window_places.get(&key) {
                    // Where it was last, and as big (kept inside the window).
                    Some(r) => open_floating_at(&mut self.dock, ctx, tab, egui::Rect::from_min_size(egui::pos2(r[0], r[1]), egui::vec2(r[2], r[3]))),
                    None => open_floating(&mut self.dock, ctx, tab, size),
                },
                (false, true) => {
                    self.dock.remove(&tab);
                }
                (true, true) => remember_place(&self.dock, &mut self.ed.prefs.window_places, &tab, key),
                _ => {}
            }
        }
    }

    pub(super) fn apply_actions(&mut self) {
        self.swap_start_page();
        for action in std::mem::take(&mut self.ed.actions) {
            match action {
                Action::OpenWorkspace(key) => {
                    self.ed.active_workspace = Some(key.clone());
                    open_canvas(&mut self.dock, key);
                }
                Action::CloseWorkspaceTabs(key) => {
                    let tab = Tab::Canvas(key.clone());
                    let others = self.dock.tabs().any(|t| matches!(t, Tab::Canvas(k) if *k != key));
                    if !others && self.ed.project.is_some() && !self.dock.contains(&Tab::Start) {
                        self.dock.replace(&tab, Tab::Start);
                    }
                    close_tabs(&mut self.dock, |t| *t == tab);
                    self.ed.views.remove(&key);
                    if self.ed.selected.as_ref().is_some_and(|(w, _)| *w == key) {
                        self.ed.selected = None;
                    }
                }
            }
        }
        self.drop_placeholder();
    }
}

/// The name a window's size is kept under: its tab's kind.
pub(super) fn window_key(tab: &Tab) -> String {
    format!("{tab:?}")
}

/// Keeps where `tab`'s window inside the app (one with only it) is and how
/// big, for the next time it opens.
pub(super) fn remember_place(dock: &crate::ui::dock::Dock<Tab>, places: &mut std::collections::BTreeMap<String, [f32; 4]>, tab: &Tab, key: String) {
    let Some(w) = dock.windows.iter().find(|w| w.os.is_none() && w.node.leaves().len() == 1 && w.node.contains(tab)) else { return };
    let place = [w.pos.x.round(), w.pos.y.round(), w.size.x.round(), w.size.y.round()];
    if places.get(&key) != Some(&place) {
        places.insert(key, place);
    }
}

