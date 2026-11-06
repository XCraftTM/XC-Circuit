//! The activity bar, the side bar, the status bar, the tour and settings.

use super::*;

impl CircuitApp {
    /// The activity bar: an icon per side bar view. Clicking one shows that view
    /// in the side bar; clicking the one shown folds the side bar away.
    pub(super) fn activity(&mut self, ui: &mut egui::Ui) {
        let t = self.ed.theme.clone();
        let line = ui.max_rect();
        ui.painter().vline(line.right() - 0.5, line.y_range(), egui::Stroke::new(1.0, t.line));
        self.ed.ui_rects.insert("activity-bar".into(), line);
        ui.spacing_mut().item_spacing.y = 2.0;
        for view in SideView::ALL {
            let shown = self.ed.prefs.side_view == Some(view) && self.ed.prefs.side_tab.is_none();
            if self.activity_item(ui, view.icon(), view.name(), shown) {
                self.ed.prefs.side_tab = None;
                self.ed.prefs.side_view = if shown { None } else { Some(view) };
            }
        }
        // Panels moved into the side bar: an icon each, like the views above.
        let mut back: Option<String> = None;
        for key in self.ed.prefs.side_tabs.clone() {
            let Some(tab) = Tab::from_side_key(&key) else { continue };
            let shown = self.ed.prefs.side_tab.as_deref() == Some(key.as_str());
            let name = self.panel_name(&tab);
            let resp = self.activity_item_response(ui, tab.icon(), &name, shown);
            if resp.clicked() {
                self.ed.prefs.side_tab = if shown { None } else { Some(key.clone()) };
                if shown {
                    self.ed.prefs.side_view = None;
                }
            }
            resp.context_menu(|ui| {
                if crate::ui::kit::menu_item(ui, t!("Move back to the main area"), None, true).clicked() {
                    back = Some(key.clone());
                    ui.close();
                }
            });
        }
        if let Some(key) = back {
            self.move_to_main(&key);
        }
        // A panel's tab dragged over here: show that the bar takes it.
        if let Some(tab) = super::dock::dragging_tab(ui.ctx())
            && !self.ed.prefs.side_tabs.contains(&tab.side_key().unwrap_or_default())
        {
            let (slot, _) = ui.allocate_exact_size(egui::vec2(ACTIVITY_W, 40.0), egui::Sense::hover());
            let box_ = slot.shrink2(egui::vec2(8.0, 4.0));
            ui.painter().rect_stroke(box_, 6.0, egui::Stroke::new(1.5, t.accent), egui::StrokeKind::Inside);
            if let Some(path) = self.ed.reg.icons.get(tab.icon()) {
                egui::Image::from_uri(crate::editor::file_uri(path)).tint(t.accent).paint_at(ui, egui::Rect::from_center_size(slot.center(), egui::vec2(18.0, 18.0)));
            }
            ui.painter().rect_filled(line, 0.0, crate::ui::theme::with_alpha(t.accent, 18));
            ui.ctx().request_repaint();
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            if self.activity_item(ui, "settings", t!("Settings (Ctrl+,)"), self.dock.find_tab(&Tab::Settings).is_some()) {
                self.toggle_settings();
            }
        });
    }

    /// The side bar: the view picked in the activity bar (or a panel moved
    /// into it), under its name.
    pub(super) fn side_bar(&mut self, ui: &mut egui::Ui, view: SideView, tab: Option<Tab>, problems: &[panels::ProblemItem]) {
        let t = self.ed.theme.clone();
        let line = ui.max_rect();
        ui.painter().vline(line.right() - 0.5, line.y_range(), egui::Stroke::new(1.0, t.line));
        let heading = match &tab {
            Some(tab) => self.panel_name(tab),
            None => view.name().to_string(),
        };
        ui.horizontal(|ui| {
            ui.set_min_height(34.0);
            ui.add_space(14.0);
            ui.label(RichText::new(heading.to_uppercase()).size(11.0).strong().color(t.text_muted));
        });
        self.ed.ui_rects.insert("side-bar".into(), ui.max_rect());
        // The view draws in a box of the side bar's size: whatever it holds, the
        // side bar keeps the width it was given (or dragged to).
        let area = ui.available_rect_before_wrap();
        let mut view_ui = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::top_down(egui::Align::Min)));
        view_ui.set_clip_rect(area.intersect(ui.clip_rect()));
        ui.allocate_rect(area, egui::Sense::hover());
        let ui = &mut view_ui;
        if let Some(tab) = tab {
            let running = self.bot.state != BotState::Stopped;
            match tab {
                Tab::Inspector => panels::inspector(ui, &mut self.ed),
                Tab::Console => panels::console(ui, &mut self.ed),
                Tab::Problems => panels::problems(ui, &mut self.ed, problems.to_vec()),
                Tab::Plugin(plugin, panel) => self.native.panel_ui(ui, &mut self.ed, &plugin, &panel, running),
                _ => {}
            }
            return;
        }
        match view {
            SideView::Workspaces => {
                // The Problems panel's entries, counted per workspace.
                let mut counts = std::collections::HashMap::new();
                for (_, _, at) in self.problems_found.iter().flat_map(|(_, p)| p) {
                    if let Some((ws, _)) = at {
                        *counts.entry(ws.clone()).or_insert(0) += 1;
                    }
                }
                crate::pages::sidebar::panel(ui, &mut self.ed, &counts);
            }
            SideView::StoredData => {
                let running = self.bot.state != BotState::Stopped;
                crate::pages::stored::panel(ui, &mut self.ed, &mut self.stored, running);
            }
            SideView::Plugins => match crate::plugins::manager::side(ui, &t, &self.ed, &self.native) {
                Some(crate::plugins::manager::SideAsk::Manage) => self.open_plugins(),
                Some(crate::plugins::manager::SideAsk::Reload) => self.reload_plugins(ui.ctx()),
                Some(crate::plugins::manager::SideAsk::Install) => self.pick_plugin_zip(ui.ctx()),
                Some(crate::plugins::manager::SideAsk::OpenFolder) => {
                    let dir = crate::bot::app_data_dir().join("plugins");
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = open::that(dir);
                }
                None => {}
            },
        }
    }

    /// One activity bar icon. Returns true when clicked.
    pub(super) fn activity_item(&self, ui: &mut egui::Ui, icon: &str, tip: &str, active: bool) -> bool {
        self.activity_item_response(ui, icon, tip, active).clicked()
    }

    /// [`Self::activity_item`], with its response (for a context menu).
    pub(super) fn activity_item_response(&self, ui: &mut egui::Ui, icon: &str, tip: &str, active: bool) -> egui::Response {
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
        resp
    }

    /// A panel's name, as its tab shows it.
    pub(super) fn panel_name(&self, tab: &Tab) -> String {
        match tab {
            Tab::Plugin(plugin, panel) => self.native.panel_title(plugin, panel).unwrap_or_else(|| panel.clone()),
            other => Tab::PANELS.iter().find(|(t, _)| t == other).map(|(_, n)| crate::i18n::t(n).to_string()).unwrap_or_default(),
        }
    }

    /// Moves a panel out of the main area into the side bar, and shows it there.
    pub(super) fn move_to_side(&mut self, tab: Tab) {
        let Some(key) = tab.side_key() else { return };
        close_tabs(&mut self.dock, |t| *t == tab);
        if !self.ed.prefs.side_tabs.contains(&key) {
            self.ed.prefs.side_tabs.push(key.clone());
        }
        self.ed.prefs.side_tab = Some(key);
    }

    /// Puts a panel from the side bar back into the main area.
    pub(super) fn move_to_main(&mut self, key: &str) {
        self.ed.prefs.side_tabs.retain(|k| k != key);
        if self.ed.prefs.side_tab.as_deref() == Some(key) {
            self.ed.prefs.side_tab = None;
        }
        if let Some(tab) = Tab::from_side_key(key)
            && self.dock.find_tab(&tab).is_none()
        {
            place_panel(&mut self.dock, tab);
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
        let focused = self.dock.find_active_focused().is_some_and(|(_, t)| *t == Tab::Settings);
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
                Ask::AllowNative(on) => {
                    self.native.allow = on;
                    self.native.sync(&mut self.ed, self.bundled_plugins.as_deref());
                }
                Ask::ResetLayout => self.reset_layout(),
                Ask::NodeSetup => self.setup = Some(crate::pages::setup::Setup::node_only()),
                Ask::OpenBackups => open_center(&mut self.dock, Tab::Backups),
            }
        }
    }

    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
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
        if let Some(r) = &self.updater.available
            && status_item(&mut right, &t, Some(t.accent), &tf!("Update {version} available", version = r.version)).on_hover_text(t!("What's new, and the download")).clicked()
        {
            self.updater.show = true;
        }
        let saved = if self.ed.unsaved() { t!("Unsaved changes") } else { t!("Saved") };
        if status_item(&mut right, &t, None, saved).on_hover_text(t!("Save (Ctrl+S)")).clicked() && has_project {
            self.ed.save();
            self.bot.restart_if_running(&mut self.ed, &ctx);
        }
    }

    pub(super) fn reset_layout(&mut self) {
        let canvases: Vec<Tab> = self.dock.iter_all_tabs().map(|(_, t)| t.clone()).filter(|t| matches!(t, Tab::Canvas(_))).collect();
        self.dock = default_layout();
        for t in canvases {
            if let Tab::Canvas(k) = t {
                open_canvas(&mut self.dock, k);
            }
        }
    }

    pub(super) fn toggle_panel(&mut self, tab: Tab) {
        // A panel in the side bar is shown or hidden there.
        if let Some(key) = tab.side_key()
            && self.ed.prefs.side_tabs.contains(&key)
        {
            let shown = self.ed.prefs.side_tab.as_deref() == Some(key.as_str());
            self.ed.prefs.side_tab = if shown { None } else { Some(key) };
            return;
        }
        if self.dock.find_tab(&tab).is_some() {
            close_tabs(&mut self.dock, |t| *t == tab);
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
            (Tab::CanvasBuilder, self.ed.canvas_edit.is_some(), egui::vec2(1280.0, 820.0)),
            (Tab::SlashCommands, self.ed.slash_edit.is_some(), egui::vec2(1320.0, 720.0)),
            (Tab::BotSettings, self.bot.settings.is_some(), egui::vec2(820.0, 620.0)),
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

    pub(super) fn apply_actions(&mut self) {
        self.swap_start_page();
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
        self.drop_placeholder();
    }
}
