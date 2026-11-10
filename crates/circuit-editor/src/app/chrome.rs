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
            crate::plugins::manager::SideAsk::Open(id) => {
                // In the start window there is nowhere for it.
                if self.ed.project.is_some() {
                    open_center(&mut self.dock, Tab::PluginPage(id));
                }
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
        if !has_project && status_item(&mut left, &t, None, t!("No project")).on_hover_text(t!("Open a project")).clicked() {
            self.open_project_dialog();
        }
        // The project's repository: what waits to be stored or fetched.
        if has_project
            && let Some((waits, text)) = self.bot.git.status()
            && status_item(&mut left, &t, waits.then_some(t.accent), &text).on_hover_text(t!("Show the Git view")).clicked()
        {
            self.show_git();
        }
        // Problems: how many, or that there are none.
        if has_project {
            let label = match self.problem_count {
                0 => t!("No problems").to_string(),
                1 => t!("⚠ 1 problem").to_string(),
                count => tf!("⚠ {count} problems", count = count),
            };
            if status_item(&mut left, &t, None, &label).on_hover_text(t!("Show the Problems panel")).clicked() {
                self.show_panel(Tab::Problems);
            }
        }
        // A live session: who is in it.
        if let Some(text) = self.bot.live.status()
            && status_item(&mut left, &t, Some(t.action), &text).on_hover_text(t!("Share › Live session…")).clicked()
        {
            self.bot.live.open();
            self.show_shared(Tab::Live);
        }

        let mut right = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(egui::Layout::right_to_left(egui::Align::Center)));
        self.bell(&mut right);
        if let Some(r) = &self.updater.available
            && status_item(&mut right, &t, Some(t.accent), &tf!("Update {version} available", version = r.version)).on_hover_text(t!("What's new, and the download")).clicked()
        {
            self.updater.show = true;
        }
        // The kind of bot: its settings.
        if let Some(mode_id) = self.ed.project.as_ref().map(|p| p.meta.mode.clone()) {
            let mode = self.ed.reg.mode(&mode_id).map(|m| m.name.clone());
            let mode_name = match mode {
                Some(name) => name,
                None if mode_id == crate::editor::NO_MODE => t!("Core blocks").to_string(),
                None => mode_id,
            };
            if status_item(&mut right, &t, None, &mode_name).on_hover_text(t!("Bot settings")).clicked() {
                self.bot.open_settings(&mut self.ed);
            }
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

    /// Git, Host or Live session is opened from a menu or the status bar:
    /// shown where it is (the side bar, when it is an icon in the activity bar).
    pub(super) fn show_shared(&mut self, tab: Tab) {
        if let Some(key) = tab.side_key().filter(|k| self.ed.prefs.activity.contains(k)) {
            if self.ed.prefs.side_lower.as_deref() != Some(key.as_str()) {
                self.ed.prefs.side_shown = Some(key);
            }
        } else if self.dock.contains(&tab) {
            self.dock.focus(&tab);
        }
    }

    /// Shows the Git view where it is: the side bar or its tab; without
    /// either, Git's window opens.
    pub(super) fn show_git(&mut self) {
        if self.ed.prefs.activity.iter().any(|k| k == "git") {
            if self.ed.prefs.side_lower.as_deref() != Some("git") {
                self.ed.prefs.side_shown = Some("git".into());
            }
        } else if self.dock.contains(&Tab::Git) {
            self.dock.focus(&Tab::Git);
        } else {
            self.bot.git.open(&self.ed);
        }
    }

    /// Whether a part of the window shows: the side bar, the panels at the
    /// bottom, or the ones on the right.
    pub(super) fn part_shown(&self, part: WindowPart) -> bool {
        match part {
            WindowPart::SideBar => self.ed.prefs.side_shown.is_some(),
            WindowPart::Bottom => self.dock.main.as_ref().is_some_and(|m| m.leaves().iter().any(|l| l.tabs.iter().any(|t| t.side() == Side::Bottom))),
            WindowPart::Right => self.dock.main.as_ref().is_some_and(|m| m.leaves().iter().any(|l| l.tabs.iter().any(|t| t.side() == Side::Right))),
        }
    }

    /// Hides a part of the window, or shows it again with what it had.
    pub(super) fn toggle_part(&mut self, part: WindowPart) {
        let shown = self.part_shown(part);
        match part {
            WindowPart::SideBar if shown => self.side_before = self.ed.prefs.side_shown.take(),
            WindowPart::SideBar => {
                let back = self.side_before.take().filter(|k| self.ed.prefs.activity.contains(k)).or_else(|| self.ed.prefs.activity.first().cloned());
                self.ed.prefs.side_shown = back;
            }
            WindowPart::Bottom | WindowPart::Right => {
                let side = if part == WindowPart::Bottom { Side::Bottom } else { Side::Right };
                if shown {
                    let tabs: Vec<Tab> = self.dock.main.iter().flat_map(|m| m.leaves()).flat_map(|l| l.tabs.clone()).filter(|t| t.side() == side).collect();
                    close_tabs(&mut self.dock, |t| t.side() == side);
                    self.hidden_panels.retain(|t| t.side() != side);
                    self.hidden_panels.extend(tabs);
                } else {
                    let mut tabs: Vec<Tab> = self.hidden_panels.iter().filter(|t| t.side() == side).cloned().collect();
                    self.hidden_panels.retain(|t| t.side() != side);
                    // One that became an icon in the activity bar meanwhile stays there.
                    tabs.retain(|t| !t.side_key().is_some_and(|k| self.ed.prefs.activity.contains(&k)));
                    if tabs.is_empty() {
                        tabs = if side == Side::Bottom { vec![Tab::Console, Tab::Problems] } else { vec![Tab::Inspector] };
                        tabs.retain(|t| !t.side_key().is_some_and(|k| self.ed.prefs.activity.contains(&k)));
                    }
                    let first = tabs.first().cloned();
                    for tab in tabs {
                        place_panel(&mut self.dock, tab);
                    }
                    if let Some(first) = first {
                        self.dock.focus(&first);
                    }
                }
            }
        }
    }

    /// Lets the bottom panels fill the window (the workspaces keep a strip),
    /// or gives the workspaces the room they had.
    pub(super) fn fill_bottom(&mut self, fill: bool) {
        /// The split with the workspaces above and the bottom panels below.
        fn find(node: &mut Node<Tab>) -> Option<&mut f32> {
            let Node::Split(s) = node else { return None };
            let bottom = |n: &Node<Tab>| n.leaves().iter().any(|l| l.tabs.iter().any(|t| t.side() == Side::Bottom));
            if s.dir == Dir::Vertical && has_center(&s.first) && bottom(&s.second) && !has_center(&s.second) {
                return Some(&mut s.fraction);
            }
            if bottom(&s.first) { find(&mut s.first) } else { find(&mut s.second) }
        }
        let Some(fraction) = self.dock.main.as_mut().and_then(find) else {
            self.bottom_before = None;
            return;
        };
        match (fill, self.bottom_before) {
            (true, None) => {
                self.bottom_before = Some(*fraction);
                *fraction = 0.05;
            }
            (false, Some(before)) => {
                *fraction = before;
                self.bottom_before = None;
            }
            _ => {}
        }
    }

    /// Customize Layout, the fourth button in the title bar: every part of
    /// the window to tick, and the side the side bar is on.
    pub(super) fn layout_menu(&mut self, ui: &mut egui::Ui, theme: &crate::ui::theme::Theme) {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(28.0, ui.available_height().min(crate::app::titlebar::HEIGHT)), egui::Sense::click());
        crate::ui::kit::describe(&resp, egui::Role::Button, t!("Customize Layout"));
        let color = if resp.hovered() { theme.text } else { theme.text_muted };
        if resp.hovered() {
            ui.painter().rect_filled(rect.shrink2(egui::vec2(2.0, 6.0)), 4.0, crate::ui::theme::mix(theme.frame, theme.text, 0.1));
        }
        // The window in four pieces.
        let frame = egui::Rect::from_center_size(rect.center(), egui::vec2(15.0, 12.0));
        let ink = egui::Stroke::new(1.2, color);
        let p = ui.painter();
        p.rect_stroke(frame, 2.0, ink, egui::StrokeKind::Inside);
        p.vline(frame.left() + 5.5, frame.y_range(), ink);
        p.hline(egui::Rangef::new(frame.left() + 5.5, frame.right()), frame.center().y, ink);
        let resp = resp.on_hover_text(t!("Customize Layout"));
        egui::Popup::menu(&resp).width(250.0).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.label(RichText::new(t!("VISIBILITY")).size(10.5).color(theme.text_muted));
            crate::ui::kit::checkbox(ui, theme, &mut self.ed.prefs.activity_bar, t!("Activity bar"));
            for part in WindowPart::ALL {
                let mut on = self.part_shown(part);
                if crate::ui::kit::checkbox(ui, theme, &mut on, part.name()).clicked() {
                    self.toggle_part(part);
                }
            }
            let mut status = !self.ed.prefs.status_bar_hidden;
            if crate::ui::kit::checkbox(ui, theme, &mut status, t!("Status bar")).clicked() {
                self.ed.prefs.status_bar_hidden = !status;
            }
            ui.separator();
            ui.label(RichText::new(t!("SIDE BAR POSITION")).size(10.5).color(theme.text_muted));
            if crate::ui::kit::radio(ui, theme, !self.ed.prefs.side_right, t!("Left")).clicked() {
                self.ed.prefs.side_right = false;
            }
            if crate::ui::kit::radio(ui, theme, self.ed.prefs.side_right, t!("Right")).clicked() {
                self.ed.prefs.side_right = true;
            }
            ui.separator();
            if crate::ui::kit::menu_item(ui, t!("Reset layout"), None, true).clicked() {
                self.ed.prefs.side_right = false;
                self.ed.prefs.status_bar_hidden = false;
                self.ed.prefs.activity_bar = true;
                self.reset_layout();
                ui.close();
            }
        });
    }

    /// The title bar's three buttons that hide and show the side bar, the
    /// bottom panels and the right ones, drawn as the window with that part
    /// filled in. Laid out right to left.
    pub(super) fn layout_toggles(&mut self, ui: &mut egui::Ui, theme: &crate::ui::theme::Theme) {
        // In the order the parts are in the window.
        let order = if self.ed.prefs.side_right { [WindowPart::SideBar, WindowPart::Bottom, WindowPart::Right] } else { [WindowPart::Right, WindowPart::Bottom, WindowPart::SideBar] };
        for part in order {
            let on = self.part_shown(part);
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(28.0, ui.available_height().min(crate::app::titlebar::HEIGHT)), egui::Sense::click());
            crate::ui::kit::describe_selected(&resp, egui::Role::Button, on, part.name());
            let color = if resp.hovered() { theme.text } else { theme.text_muted };
            if resp.hovered() {
                ui.painter().rect_filled(rect.shrink2(egui::vec2(2.0, 6.0)), 4.0, crate::ui::theme::mix(theme.frame, theme.text, 0.1));
            }
            let frame = egui::Rect::from_center_size(rect.center(), egui::vec2(15.0, 12.0));
            let left = (part == WindowPart::SideBar) != self.ed.prefs.side_right;
            let piece = match part {
                WindowPart::SideBar | WindowPart::Right if left => frame.with_max_x(frame.left() + 5.0),
                WindowPart::SideBar | WindowPart::Right => frame.with_min_x(frame.right() - 5.0),
                WindowPart::Bottom => frame.with_min_y(frame.bottom() - 4.5),
            };
            let p = ui.painter();
            p.rect_stroke(frame, 2.0, egui::Stroke::new(1.2, color), egui::StrokeKind::Inside);
            if on {
                p.rect_filled(piece, 1.0, color);
            } else {
                p.rect_stroke(piece, 1.0, egui::Stroke::new(1.0, color), egui::StrokeKind::Inside);
            }
            let tip = if on { tf!("Hide: {part}", part = part.name()) } else { tf!("Show: {part}", part = part.name()) };
            if resp.on_hover_text(tip).clicked() {
                self.toggle_part(part);
            }
        }
    }

    pub(super) fn reset_layout(&mut self) {
        let fresh = crate::editor::prefs::Prefs::default();
        self.ed.prefs.activity = fresh.activity;
        self.ed.prefs.side_shown = fresh.side_shown;
        self.hidden_panels.clear();
        self.bottom_before = None;
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

    /// What is open besides the workspaces and panels, each where it
    /// belongs: the builders and the bot's settings are tabs in the middle,
    /// the remote's console a panel at the bottom, and what is set up once
    /// and closed again a window of its own. Returns those windows.
    pub(super) fn sync_dialog_tabs(&mut self) -> Vec<Tab> {
        let wanted = [
            (Tab::EmbedBuilder, self.ed.embed_edit.is_some()),
            (Tab::LayoutBuilder, self.ed.layout_edit.is_some()),
            (Tab::ModalBuilder, self.ed.modal_edit.is_some()),
            (Tab::CanvasBuilder, self.ed.canvas_edit.is_some()),
            (Tab::SlashCommands, self.ed.slash_edit.is_some()),
            (Tab::BotSettings, self.bot.settings.is_some()),
            (Tab::Export, self.bot.export.is_some()),
            (Tab::Hosting, self.bot.hosting.draft.is_some()),
            (Tab::Git, self.bot.git.open),
            (Tab::Host, self.bot.host.open),
            (Tab::Live, self.bot.live.open),
            (Tab::ServerConsole, self.bot.hosting.console.is_some()),
            (Tab::WorkspaceExport, self.ed.ws_export.is_some()),
            (Tab::WorkspaceImport, self.ed.ws_import.is_some()),
        ];
        let mut own = Vec::new();
        for (tab, open) in wanted {
            // Git's window is apart from its short view (a tab or in the side bar).
            if tab == Tab::Git {
                if open {
                    own.push(tab);
                }
                continue;
            }
            // One that is an icon in the activity bar shows in the side bar.
            let in_bar = tab.side_key().is_some_and(|k| self.ed.prefs.activity.contains(&k));
            match (open, self.dock.contains(&tab)) {
                (true, false) if in_bar || self.hidden_panels.contains(&tab) => {}
                (true, false) if tab.own_window().is_some() => own.push(tab),
                (true, false) => place_panel(&mut self.dock, tab),
                (false, true) => {
                    self.dock.remove(&tab);
                }
                _ => {}
            }
        }
        own
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

/// The name a window of its own is known by: its tab's kind.
pub(super) fn window_key(tab: &Tab) -> String {
    format!("{tab:?}")
}

/// The parts of the window that the title bar's buttons hide and show.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum WindowPart {
    SideBar,
    Bottom,
    Right,
}

impl WindowPart {
    pub(super) const ALL: [WindowPart; 3] = [WindowPart::SideBar, WindowPart::Bottom, WindowPart::Right];

    pub(super) fn name(self) -> &'static str {
        match self {
            WindowPart::SideBar => t!("Side bar"),
            WindowPart::Bottom => t!("Bottom panels"),
            WindowPart::Right => t!("Right panels"),
        }
    }
}
