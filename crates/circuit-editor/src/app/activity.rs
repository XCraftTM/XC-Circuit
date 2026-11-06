//! The activity bar on the far left and the side bar beside it. Their icons
//! work like the dock's tabs: drag one to put the icons in another order, or
//! out onto the main area, where it becomes a tab; drag a tab (Workspaces, the
//! Inspector, a plugin's panel …) onto the bar and it becomes an icon there.

use super::*;

/// An activity bar icon being dragged (its key).
#[derive(Clone)]
pub(super) struct ActivityDrag(pub(super) String);

/// Puts `key` into the activity bar's list before `before` (at the end
/// without), taking it from where it was.
pub(super) fn put_in_activity(list: &mut Vec<String>, key: &str, before: Option<&str>) {
    if before == Some(key) && list.iter().any(|k| k == key) {
        return;
    }
    list.retain(|k| k != key);
    let at = before.and_then(|b| list.iter().position(|k| k == b)).unwrap_or(list.len());
    list.insert(at, key.to_string());
}

/// The icon something dropped at height `y` goes before: the first one whose
/// middle is below it (`None`: after the last).
pub(super) fn icon_after(slots: &[(String, egui::Rect)], y: f32) -> Option<&str> {
    slots.iter().find(|(_, r)| r.center().y > y).map(|(k, _)| k.as_str())
}

impl CircuitApp {
    /// The tab an activity bar key stands for, while it can be shown (a
    /// plugin's panel only while the plugin has it).
    pub(super) fn activity_tab(&self, key: &str) -> Option<Tab> {
        let tab = Tab::from_side_key(key)?;
        match &tab {
            Tab::Plugin(plugin, panel) => self.native.panel_title(plugin, panel).map(|_| tab),
            _ => Some(tab),
        }
    }

    /// A tab's name, as its tab shows it.
    pub(super) fn panel_name(&self, tab: &Tab) -> String {
        match tab {
            Tab::Side(view) => view.name().to_string(),
            Tab::Plugin(plugin, panel) => self.native.panel_title(plugin, panel).unwrap_or_else(|| panel.clone()),
            other => Tab::PANELS.iter().find(|(t, _)| t == other).map(|(_, n)| crate::i18n::t(n).to_string()).unwrap_or_default(),
        }
    }

    /// The activity bar: an icon each for what's in it (Workspaces, Stored
    /// Data and Plugins to start with). Clicking one shows it in the side bar;
    /// clicking the one shown folds the side bar away. Settings at the bottom.
    pub(super) fn activity(&mut self, ui: &mut egui::Ui) {
        let t = self.ed.theme.clone();
        let bar = ui.max_rect();
        ui.painter().vline(bar.right() - 0.5, bar.y_range(), egui::Stroke::new(1.0, t.line));
        self.ed.ui_rects.insert("activity-bar".into(), bar);
        ui.spacing_mut().item_spacing.y = 2.0;
        let ctx = ui.ctx().clone();
        let icon_drag = egui::DragAndDrop::payload::<ActivityDrag>(&ctx).map(|d| d.0.clone());
        // A tab from the main area that can come here, being dragged.
        let tab_drag = self.dock.dragged().filter(|t| t.side_key().is_some_and(|k| !self.ed.prefs.activity.contains(&k))).cloned();
        let mut slots = Vec::new();
        let mut out = None;
        for key in self.ed.prefs.activity.clone() {
            let Some(tab) = self.activity_tab(&key) else { continue };
            let shown = self.ed.prefs.side_shown.as_deref() == Some(key.as_str());
            let name = self.panel_name(&tab);
            let lifted = icon_drag.as_deref() == Some(key.as_str());
            let resp = self.activity_button(ui, tab.icon(), &name, shown, lifted, egui::Sense::click_and_drag());
            slots.push((key.clone(), resp.rect));
            resp.dnd_set_drag_payload(ActivityDrag(key.clone()));
            if resp.clicked() {
                self.ed.prefs.side_shown = (!shown).then(|| key.clone());
            }
            resp.context_menu(|ui| {
                if crate::ui::kit::menu_item(ui, t!("Move to the main area"), None, true).clicked() {
                    out = Some(key.clone());
                    ui.close();
                }
            });
        }
        if let Some(key) = out {
            self.move_to_main(&key, None);
        }
        // Something that can come here being dragged: the bar lights up (for a
        // tab), and over it a line shows where it would go.
        if icon_drag.is_some() || tab_drag.is_some() {
            if tab_drag.is_some() {
                ui.painter().rect_filled(bar, 0.0, crate::ui::theme::with_alpha(t.accent, 18));
            }
            if let Some(pos) = ctx.pointer_latest_pos().filter(|p| bar.contains(*p)) {
                let y = match icon_after(&slots, pos.y) {
                    Some(k) => slots.iter().find(|(s, _)| s == k).map(|(_, r)| r.top() - 1.0),
                    None => slots.last().map(|(_, r)| r.bottom() + 1.0),
                };
                let y = y.unwrap_or(bar.top() + 4.0);
                ui.painter().hline(egui::Rangef::new(bar.left() + 8.0, bar.right() - 8.0), y, egui::Stroke::new(2.0, t.accent));
            }
            ctx.request_repaint();
        }
        self.activity_slots = slots;
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            if self.activity_item(ui, "settings", t!("Settings (Ctrl+,)"), self.dock.contains(&Tab::Settings)) {
                self.toggle_settings();
            }
        });
    }

    /// One activity bar icon that's only clicked. Returns true when it is.
    pub(super) fn activity_item(&self, ui: &mut egui::Ui, icon: &str, tip: &str, active: bool) -> bool {
        self.activity_button(ui, icon, tip, active, false, egui::Sense::click()).clicked()
    }

    /// One activity bar icon: marked when `active`, faint while `lifted` (dragged).
    fn activity_button(&self, ui: &mut egui::Ui, icon: &str, tip: &str, active: bool, lifted: bool, sense: egui::Sense) -> egui::Response {
        let t = &self.ed.theme;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ACTIVITY_W, 40.0), sense);
        let resp = if lifted { resp } else { resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand) };
        if active {
            ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(2.0, rect.height())), 0.0, t.accent);
        }
        let color = if active || resp.hovered() { t.text } else { t.text_muted };
        let color = if lifted { crate::ui::theme::with_alpha(color, 70) } else { color };
        if let Some(path) = self.ed.reg.icons.get(icon) {
            egui::Image::from_uri(crate::editor::file_uri(path)).tint(color).paint_at(ui, egui::Rect::from_center_size(rect.center(), egui::vec2(20.0, 20.0)));
        }
        resp
    }

    /// An activity bar icon being dragged: it follows the pointer like a tab,
    /// the place it would take in the main area lights up, and letting go
    /// puts it there (or, over the bar, in its new place in the bar).
    /// Called after the dock is drawn, for where its areas are now.
    pub(super) fn activity_drag(&mut self, ctx: &egui::Context) {
        let Some(key) = egui::DragAndDrop::payload::<ActivityDrag>(ctx).map(|d| d.0.clone()) else { return };
        let Some(pos) = ctx.pointer_latest_pos() else { return };
        let over = |name: &str| self.ed.ui_rects.get(name).is_some_and(|r| r.contains(pos));
        let over_bar = over("activity-bar");
        let target = if over_bar || over("side-bar") { None } else { self.dock.target_at(pos, None, ctx.layer_id_at(pos)) };
        if ctx.input(|i| i.pointer.any_released()) {
            egui::DragAndDrop::clear_payload(ctx);
            if over_bar {
                let before = icon_after(&self.activity_slots, pos.y).map(str::to_string);
                put_in_activity(&mut self.ed.prefs.activity, &key, before.as_deref());
            } else if let Some(drop) = target {
                self.move_to_main(&key, Some(drop.target));
            }
            return;
        }
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        let t = self.ed.theme.clone();
        if let Some(drop) = &target {
            crate::ui::dock::paint_drop(ctx, &t, drop);
        }
        // The icon and its name next to the pointer, as a dragged tab looks.
        if let Some(tab) = self.activity_tab(&key) {
            crate::ui::dock::paint_ghost(ctx, &t, self.ed.reg.icons.get(tab.icon()).map(|p| p.as_path()), &self.panel_name(&tab), pos);
        }
        ctx.request_repaint();
    }

    /// Takes an icon out of the activity bar and makes it a tab in the main
    /// area: where it was dropped, or (without) where its kind opens.
    pub(super) fn move_to_main(&mut self, key: &str, at: Option<crate::ui::dock::Target<Tab>>) {
        self.ed.prefs.activity.retain(|k| k != key);
        if self.ed.prefs.side_shown.as_deref() == Some(key) {
            self.ed.prefs.side_shown = None;
        }
        if let Some(tab) = Tab::from_side_key(key)
            && !self.dock.contains(&tab)
        {
            match at {
                Some(target) => self.dock.put(tab, &target),
                None => place_panel(&mut self.dock, tab),
            }
        }
    }

    /// Makes a tab dropped on the activity bar (or the side bar) an icon
    /// there, where it was dropped, and shows it in the side bar.
    pub(super) fn move_to_side(&mut self, tab: Tab, pos: Option<egui::Pos2>) {
        let Some(key) = tab.side_key() else { return };
        close_tabs(&mut self.dock, |t| *t == tab);
        let before = pos.filter(|p| self.ed.ui_rects.get("activity-bar").is_some_and(|r| r.contains(*p))).and_then(|p| icon_after(&self.activity_slots, p.y)).map(str::to_string);
        put_in_activity(&mut self.ed.prefs.activity, &key, before.as_deref());
        self.ed.prefs.side_shown = Some(key);
    }

    /// Each of Workspaces, Stored Data, Plugins and the panels is an icon in
    /// the activity bar or a tab, never both (a layout saved while it was a
    /// tab may still have it); and the three views are always one or the
    /// other: closing their tab puts them back in the bar.
    pub(super) fn keep_side_items(&mut self) {
        if self.setup.is_some() || self.tour.is_some() || self.dock_before_tour.is_some() || self.dock_before_start.is_some() || self.ed.project.is_none() {
            return;
        }
        for key in &self.ed.prefs.activity {
            if let Some(tab) = Tab::from_side_key(key)
                && self.dock.contains(&tab)
            {
                close_tabs(&mut self.dock, |t| *t == tab);
            }
        }
        for view in SideView::ALL {
            if !self.ed.prefs.activity.iter().any(|k| k == view.key()) && !self.dock.contains(&Tab::Side(view)) {
                self.ed.prefs.activity.push(view.key().into());
            }
        }
    }

    /// Whether a panel (or Workspaces …) can be seen: a tab, or what the side bar shows.
    pub(super) fn panel_open(&self, tab: &Tab) -> bool {
        match tab.side_key() {
            Some(key) if self.ed.prefs.activity.contains(&key) => self.ed.prefs.side_shown.as_deref() == Some(key.as_str()),
            _ => self.dock.contains(tab),
        }
    }

    /// Shows a panel: in the side bar when it's in the activity bar, else its tab.
    pub(super) fn show_panel(&mut self, tab: Tab) {
        if let Some(key) = tab.side_key()
            && self.ed.prefs.activity.contains(&key)
        {
            self.ed.prefs.side_shown = Some(key);
            return;
        }
        match self.dock.contains(&tab) {
            true => self.dock.focus(&tab),
            false => place_panel(&mut self.dock, tab),
        }
    }

    /// The side bar: what the activity bar's icon picked, under its name.
    pub(super) fn side_bar(&mut self, ui: &mut egui::Ui, key: &str, tab: Tab, problems: &[panels::ProblemItem]) {
        let t = self.ed.theme.clone();
        let line = ui.max_rect();
        ui.painter().vline(line.right() - 0.5, line.y_range(), egui::Stroke::new(1.0, t.line));
        self.ed.ui_rects.insert("side-bar".into(), line);
        let heading = self.panel_name(&tab);
        let head = ui
            .horizontal(|ui| {
                ui.set_min_height(34.0);
                ui.add_space(14.0);
                ui.label(RichText::new(heading.to_uppercase()).size(11.0).strong().color(t.text_muted));
            })
            .response;
        let mut out = false;
        head.interact(egui::Sense::click()).context_menu(|ui| {
            if crate::ui::kit::menu_item(ui, t!("Move to the main area"), None, true).clicked() {
                out = true;
                ui.close();
            }
        });
        // It draws in a box of the side bar's size: whatever it holds, the
        // side bar keeps the width it was given (or dragged to).
        let area = ui.available_rect_before_wrap();
        let mut view_ui = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::top_down(egui::Align::Min)));
        view_ui.set_clip_rect(area.intersect(ui.clip_rect()));
        ui.allocate_rect(area, egui::Sense::hover());
        let ui = &mut view_ui;
        let running = self.bot.state != BotState::Stopped;
        match tab {
            Tab::Side(view) => {
                if let Some(ask) = side_view_ui(ui, &mut self.ed, &mut self.stored, &mut self.native, running, problems, view) {
                    self.plugins_side_ask(ask, ui.ctx());
                }
            }
            Tab::Inspector => {
                self.ed.ui_rects.insert("inspector".into(), ui.max_rect());
                panels::inspector(ui, &mut self.ed);
            }
            Tab::Console => {
                self.ed.ui_rects.insert("console".into(), ui.max_rect());
                panels::console(ui, &mut self.ed);
            }
            Tab::Problems => panels::problems(ui, &mut self.ed, problems.to_vec()),
            Tab::Plugin(plugin, panel) => self.native.panel_ui(ui, &mut self.ed, &plugin, &panel, running),
            _ => {}
        }
        if out {
            self.move_to_main(key, None);
        }
    }
}
