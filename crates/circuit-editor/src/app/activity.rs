//! The activity bar on the far left and the side bar beside it. Their icons
//! work like the dock's tabs: drag one to put the icons in another order, or
//! out onto the main area, where it becomes a tab; drag a tab (Workspaces, the
//! Inspector, a plugin's panel …) onto the bar and it becomes an icon there.

use super::*;

/// How tall a side view's heading is.
const SIDE_HEAD: f32 = 34.0;

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

/// The side bar's views put right after a change: the lower one only beside
/// another one that shows (it moves up when the upper one goes), and only
/// while its icon is in the activity bar.
pub(super) fn tidy_side(prefs: &mut crate::editor::prefs::Prefs) {
    let in_bar = |k: &Option<String>| k.as_ref().is_some_and(|k| prefs.activity.contains(k));
    if !in_bar(&prefs.side_lower) || prefs.side_lower == prefs.side_shown {
        prefs.side_lower = None;
    }
    if !in_bar(&prefs.side_shown) {
        prefs.side_shown = prefs.side_lower.take();
    }
    prefs.side_split = prefs.side_split.clamp(0.15, 0.85);
}

/// Shows `key` in the side bar: in its upper half (the one shown), or with
/// `lower` in its lower half, under the one shown. The two swap when it's
/// the other one already.
pub(super) fn show_in_side(prefs: &mut crate::editor::prefs::Prefs, key: &str, lower: bool) {
    let key = Some(key.to_string());
    match (lower, prefs.side_shown.is_some()) {
        // Nothing above it yet: it's the only one.
        (true, false) => prefs.side_shown = key,
        (true, true) if prefs.side_shown == key => std::mem::swap(&mut prefs.side_shown, &mut prefs.side_lower),
        (true, true) => prefs.side_lower = key,
        (false, _) if prefs.side_lower == key => std::mem::swap(&mut prefs.side_shown, &mut prefs.side_lower),
        (false, _) => prefs.side_shown = key,
    }
    tidy_side(prefs);
}

/// A click on an activity icon: shows its view, or hides it when it shows
/// (in either half; the other one then takes the whole side bar).
pub(super) fn click_in_side(prefs: &mut crate::editor::prefs::Prefs, key: &str) {
    let key = Some(key.to_string());
    if prefs.side_shown == key {
        prefs.side_shown = prefs.side_lower.take();
    } else if prefs.side_lower == key {
        prefs.side_lower = None;
    } else {
        prefs.side_shown = key;
    }
    tidy_side(prefs);
}

/// Over the side bar (`bar`), a drop at `pos` goes in its lower half when
/// it's below the middle and something else shows above.
fn drops_lower(prefs: &crate::editor::prefs::Prefs, bar: egui::Rect, pos: egui::Pos2, key: &str) -> bool {
    pos.y > bar.center().y && prefs.side_shown.as_deref().is_some_and(|k| k != key || prefs.side_lower.is_some())
}

impl CircuitApp {
    /// While an icon or a tab is dragged over the side bar: the half it
    /// would show in lights up, with what letting go does.
    fn paint_side_drop(&self, ctx: &egui::Context, key: &str, pos: egui::Pos2) {
        let Some(bar) = self.ed.ui_rects.get("side-bar").copied().filter(|r| r.contains(pos)) else { return };
        let t = &self.ed.theme;
        let lower = drops_lower(&self.ed.prefs, bar, pos, key);
        let half = if !lower && self.ed.prefs.side_shown.is_none() {
            bar
        } else if lower {
            egui::Rect::from_min_max(egui::pos2(bar.left(), bar.center().y), bar.max)
        } else {
            egui::Rect::from_min_max(bar.min, egui::pos2(bar.right(), bar.center().y))
        };
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("side-drop")));
        painter.rect_filled(half.shrink(3.0), 6.0, crate::ui::theme::with_alpha(t.accent, 40));
        painter.rect_stroke(half.shrink(3.0), 6.0, egui::Stroke::new(1.5, t.accent), egui::StrokeKind::Inside);
        let text = if lower { t!("Show below") } else { t!("Show above") };
        painter.text(half.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(13.0), t.accent);
    }

    /// The tab an activity bar key stands for, while it can be shown (a
    /// plugin's panel only while the plugin has it).
    pub(super) fn activity_tab(&self, key: &str) -> Option<Tab> {
        let tab = Tab::from_side_key(key)?;
        match &tab {
            Tab::Plugin(plugin, panel) => self.plugin_host.panel_title(plugin, panel).map(|_| tab),
            _ => Some(tab),
        }
    }

    /// A tab's name, as its tab shows it.
    pub(super) fn panel_name(&self, tab: &Tab) -> String {
        match tab {
            Tab::Side(view) => view.name().to_string(),
            Tab::Plugin(plugin, panel) => self.plugin_host.panel_title(plugin, panel).unwrap_or_else(|| panel.clone()),
            Tab::ServerConsole => match &self.bot.hosting.console {
                Some(c) => tf!("Console of {server}", server = c.name),
                None => t!("Remote console").into(),
            },
            Tab::Git => "Git".into(),
            Tab::Host => "Host".into(),
            Tab::Live => t!("Live session").into(),
            other => Tab::PANELS.iter().find(|(t, _)| t == other).map(|(_, n)| crate::i18n::t(n).to_string()).unwrap_or_default(),
        }
    }

    /// The activity bar: an icon each for what's in it (Workspaces, Stored
    /// Data and Plugins to start with). Clicking one shows it in the side bar;
    /// clicking the one shown folds the side bar away. Settings at the bottom.
    pub(super) fn activity(&mut self, ui: &mut egui::Ui) {
        let t = self.ed.theme.clone();
        let bar = ui.max_rect();
        // The line towards the rest of the window.
        let edge = if self.ed.prefs.side_right { bar.left() + 0.5 } else { bar.right() - 0.5 };
        ui.painter().vline(edge, bar.y_range(), egui::Stroke::new(1.0, t.line));
        self.ed.ui_rects.insert("activity-bar".into(), bar);
        ui.spacing_mut().item_spacing.y = 2.0;
        let ctx = ui.ctx().clone();
        let icon_drag = egui::DragAndDrop::payload::<ActivityDrag>(&ctx).map(|d| d.0.clone());
        // A tab from the main area that can come here, being dragged.
        let tab_drag = self.dock.dragged().filter(|t| t.side_key().is_some_and(|k| !self.ed.prefs.activity.contains(&k))).cloned();
        let mut slots = Vec::new();
        let mut out = None;
        let mut hide = None;
        for key in self.ed.prefs.activity.clone() {
            let Some(tab) = self.activity_tab(&key) else { continue };
            let shown = self.ed.prefs.side_shown.as_deref() == Some(key.as_str()) || self.ed.prefs.side_lower.as_deref() == Some(key.as_str());
            let name = self.panel_name(&tab);
            let lifted = icon_drag.as_deref() == Some(key.as_str());
            let resp = self.activity_button(ui, tab.icon(), &name, shown, lifted, egui::Sense::click_and_drag());
            // How much waits there: changes to store or fetch, problems.
            let count = match &tab {
                Tab::Git => self.bot.git.waiting_count(),
                Tab::Problems => self.problem_count,
                _ => 0,
            };
            if count > 0 && !lifted {
                let text = if count > 99 { "99+".to_string() } else { count.to_string() };
                let galley = ui.painter().layout_no_wrap(text, egui::FontId::proportional(9.5), egui::Color32::WHITE);
                let size = egui::vec2((galley.size().x + 8.0).max(16.0), 15.0);
                let badge = egui::Rect::from_center_size(resp.rect.center() + egui::vec2(9.0, 9.0), size);
                ui.painter().rect_filled(badge, 8.0, if tab == Tab::Problems { t.warning } else { t.accent });
                let ink = if tab == Tab::Problems { egui::Color32::from_rgb(40, 26, 0) } else { egui::Color32::from_rgb(4, 24, 30) };
                ui.painter().galley_with_override_text_color(badge.center() - galley.size() / 2.0, galley, ink);
            }
            slots.push((key.clone(), resp.rect));
            resp.dnd_set_drag_payload(ActivityDrag(key.clone()));
            if resp.clicked() {
                click_in_side(&mut self.ed.prefs, &key);
            }
            let mut below = false;
            resp.context_menu(|ui| {
                let other_above = self.ed.prefs.side_shown.as_deref().is_some_and(|k| k != key);
                if other_above && self.ed.prefs.side_lower.as_deref() != Some(key.as_str()) && crate::ui::kit::menu_item(ui, t!("Show in the lower half"), None, true).clicked() {
                    below = true;
                    ui.close();
                }
                if crate::ui::kit::menu_item(ui, t!("Move to the main area"), None, true).clicked() {
                    out = Some(key.clone());
                    ui.close();
                }
                if crate::ui::kit::menu_item(ui, tf!("Hide '{name}'", name = name), None, true).clicked() {
                    hide = Some(tab.clone());
                    ui.close();
                }
                ui.separator();
                self.activity_menu(ui);
            });
            if below {
                show_in_side(&mut self.ed.prefs, &key, true);
            }
        }
        if let Some(key) = out {
            self.move_to_main(&key, None);
        }
        if let Some(tab) = hide {
            self.set_panel_open(tab, false);
        }
        // The rest of the bar: its menu on a right-click.
        let rest = ui.available_rect_before_wrap();
        ui.interact(rest, egui::Id::unique("activity-rest"), egui::Sense::click()).context_menu(|ui| self.activity_menu(ui));
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
            let gear = self.activity_button(ui, "settings", t!("Manage"), self.dock.contains(&Tab::Settings), false, egui::Sense::click());
            self.gear_menu(&gear);
        });
    }

    /// The activity bar's own menu: everything that can be an icon in it,
    /// ticked while it is one (or open as a tab), and the bar itself.
    fn activity_menu(&mut self, ui: &mut egui::Ui) {
        let theme = self.ed.theme.clone();
        let mut list: Vec<(Tab, String)> = SideView::ALL.iter().map(|v| (Tab::Side(*v), v.name().to_string())).collect();
        for tab in [Tab::Git, Tab::Live, Tab::Host, Tab::Inspector, Tab::Console, Tab::Problems] {
            let name = self.panel_name(&tab);
            list.push((tab, name));
        }
        for p in &self.plugin_host.plugins {
            for panel in p.panels() {
                list.push((Tab::Plugin(p.id.clone(), panel.id.clone()), panel.title.clone()));
            }
        }
        for (tab, name) in list {
            let mut on = self.panel_present(&tab);
            if crate::ui::kit::checkbox(ui, &theme, &mut on, name).clicked() {
                // Ticked here, it becomes an icon in the bar.
                match (on, tab.side_key()) {
                    (true, Some(key)) if !self.dock.contains(&tab) => {
                        put_in_activity(&mut self.ed.prefs.activity, &key, None);
                        show_in_side(&mut self.ed.prefs, &key, false);
                    }
                    _ => self.set_panel_open(tab, on),
                }
            }
        }
        ui.separator();
        if crate::ui::kit::menu_item(ui, t!("Hide the activity bar"), None, true).on_hover_text(t!("View › Show activity bar brings it back")).clicked() {
            self.ed.prefs.activity_bar = false;
            ui.close();
        }
    }

    /// The gear's menu at the bottom of the activity bar: the search, the
    /// settings pages, plugins, themes and updates.
    fn gear_menu(&mut self, gear: &egui::Response) {
        let theme = self.ed.theme.clone();
        let has_project = self.ed.project.is_some();
        egui::Popup::menu(gear).width(240.0).show(|ui| {
            use crate::ui::kit::menu_item;
            if menu_item(ui, t!("Command Palette…"), Some(self.key_label(Hotkey::CommandPalette)), true).clicked() {
                self.palette = Some(Default::default());
            }
            ui.separator();
            if menu_item(ui, t!("Settings"), Some(self.key_label(Hotkey::Settings)), true).clicked() {
                self.toggle_settings();
            }
            if menu_item(ui, t!("Bot settings"), self.shortcut_shown(Hotkey::BotSettings), has_project).clicked() {
                self.bot.open_settings(&mut self.ed);
            }
            if menu_item(ui, t!("Plugins"), None, true).clicked() {
                self.open_plugins();
            }
            ui.menu_button(t!("Themes"), |ui| {
                let themes: Vec<(String, String)> = self.ed.reg.themes.iter().map(|t| (t.id.clone(), t.name.clone())).collect();
                for (id, name) in themes {
                    if crate::ui::kit::radio(ui, &theme, self.ed.theme.id == id, name).clicked() {
                        self.ed.set_theme(&id);
                        self.ed.theme.apply(ui.ctx());
                    }
                }
            });
            ui.separator();
            if menu_item(ui, t!("Check for Updates…"), None, true).clicked() {
                self.check_for_updates();
            }
        });
    }

    /// One activity bar icon: marked when `active`, faint while `lifted` (dragged).
    fn activity_button(&self, ui: &mut egui::Ui, icon: &str, tip: &str, active: bool, lifted: bool, sense: egui::Sense) -> egui::Response {
        let t = &self.ed.theme;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ACTIVITY_W, 40.0), sense);
        // Named by its tip, also while dragged; "pressed" while its view shows.
        crate::ui::kit::describe_selected(&resp, egui::Role::Button, active, tip);
        let resp = if lifted { resp } else { resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand) };
        if active {
            // The mark is on the bar's outer edge.
            let x = if self.ed.prefs.side_right { rect.right() - 2.0 } else { rect.left() };
            ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(2.0, rect.height())), 0.0, t.accent);
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
        // A tab from the main area held over the side bar: the half it would show in.
        if let (Some(key), Some(pos)) = (self.dock.dragged().and_then(Tab::side_key), ctx.pointer_latest_pos()) {
            self.paint_side_drop(ctx, &key, pos);
        }
        let Some(key) = egui::DragAndDrop::payload::<ActivityDrag>(ctx).map(|d| d.0.clone()) else { return };
        let Some(pos) = ctx.pointer_latest_pos() else { return };
        let over = |name: &str| self.ed.ui_rects.get(name).is_some_and(|r| r.contains(pos));
        let over_bar = over("activity-bar");
        let over_side = over("side-bar");
        let target = if over_bar || over_side { None } else { self.dock.target_at(pos, None, ctx.layer_id_at(pos)) };
        if ctx.input(|i| i.pointer.any_released()) {
            egui::DragAndDrop::clear_payload(ctx);
            if over_bar {
                let before = icon_after(&self.activity_slots, pos.y).map(str::to_string);
                put_in_activity(&mut self.ed.prefs.activity, &key, before.as_deref());
            } else if over_side {
                // Into the side bar's upper or lower half.
                let bar = self.ed.ui_rects["side-bar"];
                let lower = drops_lower(&self.ed.prefs, bar, pos, &key);
                show_in_side(&mut self.ed.prefs, &key, lower);
            } else if let Some(drop) = target {
                self.move_to_main(&key, Some(drop.target));
            }
            return;
        }
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        self.paint_side_drop(ctx, &key, pos);
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
        tidy_side(&mut self.ed.prefs);
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
        // Dropped on the side bar's lower half: shown there, under the other one.
        let lower = match (pos, self.ed.ui_rects.get("side-bar")) {
            (Some(p), Some(bar)) if bar.contains(p) => drops_lower(&self.ed.prefs, *bar, p, &key),
            _ => false,
        };
        show_in_side(&mut self.ed.prefs, &key, lower);
    }

    /// Each of Workspaces, Stored Data, Plugins and the panels is an icon in
    /// the activity bar or a tab, never both (a layout saved while it was a
    /// tab may still have it).
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
    }

    /// Puts Workspaces, Stored Data or Plugins back in the activity bar (at
    /// the end, not shown): where its tab goes when closed.
    pub(super) fn back_to_bar(&mut self, view: SideView) {
        if !self.ed.prefs.activity.iter().any(|k| k == view.key()) {
            self.ed.prefs.activity.push(view.key().into());
        }
    }

    /// Whether a panel (or Workspaces …) is open anywhere: an icon in the
    /// activity bar, or a tab. What View › Side bar and Panels tick.
    pub(super) fn panel_present(&self, tab: &Tab) -> bool {
        tab.side_key().is_some_and(|k| self.ed.prefs.activity.contains(&k)) || self.dock.contains(tab)
    }

    /// Opens or takes away a panel (or Workspaces …), wherever it is. Opened,
    /// Workspaces, Stored Data and Plugins come back in the activity bar,
    /// panels as a tab; either is shown.
    pub(super) fn set_panel_open(&mut self, tab: Tab, open: bool) {
        let key = tab.side_key();
        if !open {
            if let Some(key) = &key {
                self.ed.prefs.activity.retain(|k| k != key);
                if self.ed.prefs.side_shown.as_ref() == Some(key) {
                    self.ed.prefs.side_shown = None;
                }
                tidy_side(&mut self.ed.prefs);
            }
            close_tabs(&mut self.dock, |t| *t == tab);
            return;
        }
        match (&tab, key) {
            (Tab::Side(view), Some(key)) if !self.dock.contains(&tab) => {
                self.back_to_bar(*view);
                self.ed.prefs.side_shown = Some(key);
            }
            _ => self.show_panel(tab),
        }
    }

    /// Whether a panel (or Workspaces …) can be seen: a tab, or what the side bar shows.
    pub(super) fn panel_open(&self, tab: &Tab) -> bool {
        match tab.side_key() {
            Some(key) if self.ed.prefs.activity.contains(&key) => [&self.ed.prefs.side_shown, &self.ed.prefs.side_lower].iter().any(|k| k.as_deref() == Some(key.as_str())),
            _ => self.dock.contains(tab),
        }
    }

    /// Shows a panel: in the side bar when it's in the activity bar, else its tab.
    pub(super) fn show_panel(&mut self, tab: Tab) {
        if let Some(key) = tab.side_key()
            && self.ed.prefs.activity.contains(&key)
        {
            // Already in the lower half: it shows.
            if self.ed.prefs.side_lower.as_deref() != Some(key.as_str()) {
                self.ed.prefs.side_shown = Some(key);
                tidy_side(&mut self.ed.prefs);
            }
            return;
        }
        match self.dock.contains(&tab) {
            true => self.dock.focus(&tab),
            false => place_panel(&mut self.dock, tab),
        }
    }

    /// The side bar: what the activity bar's icon picked, under its name, and
    /// with a second one picked for its lower half, that one under it, with a
    /// line between them to drag.
    pub(super) fn side_bar(&mut self, ui: &mut egui::Ui, key: &str, tab: Tab, problems: &[panels::ProblemItem]) {
        let t = self.ed.theme.clone();
        let line = ui.max_rect();
        let edge = if self.ed.prefs.side_right { line.left() + 0.5 } else { line.right() - 0.5 };
        ui.painter().vline(edge, line.y_range(), egui::Stroke::new(1.0, t.line));
        self.ed.ui_rects.insert("side-bar".into(), line);
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, egui::Sense::hover());
        let lower = self.ed.prefs.side_lower.clone().filter(|k| k != key).and_then(|k| self.activity_tab(&k).map(|t| (k, t)));
        let Some((lower_key, lower_tab)) = lower else {
            self.side_folded = None;
            self.side_section(ui, full, key, tab, problems, false);
            return;
        };
        // One of the two folded away to its heading: the other has the rest.
        let split = match self.side_folded {
            Some(false) => full.top() + SIDE_HEAD + 3.0,
            Some(true) => full.bottom() - SIDE_HEAD - 3.0,
            None => full.top() + full.height() * self.ed.prefs.side_split,
        };
        let top = egui::Rect::from_min_max(full.min, egui::pos2(full.right(), split - 3.0));
        let bottom = egui::Rect::from_min_max(egui::pos2(full.left(), split + 3.0), full.max);
        self.side_section(ui, top, key, tab, problems, false);
        // The line between them: drag it to share the height differently.
        // A wider strip than the line it shows, so it is easy to take hold of.
        let divider = egui::Rect::from_min_max(egui::pos2(full.left(), split - 5.0), egui::pos2(full.right() - 1.0, split + 5.0));
        let drag = ui.interact(divider, egui::Id::unique("side-split"), egui::Sense::click_and_drag()).on_hover_cursor(egui::CursorIcon::ResizeVertical).on_hover_text(t!("Drag to share the height differently; double-click for half each"));
        let held = drag.hovered() || drag.dragged();
        ui.painter().hline(divider.x_range(), split, egui::Stroke::new(if held { 3.0 } else { 1.0 }, if held { t.accent } else { t.line }));
        if drag.dragged()
            && let Some(p) = ui.ctx().pointer_interact_pos()
        {
            // Dragging it brings a folded view back.
            self.side_folded = None;
            self.ed.prefs.side_split = ((p.y - full.top()) / full.height().max(1.0)).clamp(0.15, 0.85);
        }
        if drag.double_clicked() {
            self.side_folded = None;
            self.ed.prefs.side_split = 0.5;
        }
        self.side_section(ui, bottom, &lower_key, lower_tab, problems, true);
    }

    /// One view in the side bar, in `rect`: its name (with a menu), then it.
    #[allow(clippy::too_many_arguments)]
    fn side_section(&mut self, ui: &mut egui::Ui, rect: egui::Rect, key: &str, tab: Tab, problems: &[panels::ProblemItem], lower: bool) {
        let t = self.ed.theme.clone();
        let split = self.ed.prefs.side_lower.is_some();
        let heading = self.panel_name(&tab);
        let head_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), SIDE_HEAD));
        // Dragged by its name, a view goes to the other half, to another
        // place in the activity bar, or out into the main area as a tab.
        let head = ui.interact(head_rect, egui::Id::unique(("side-head", key)), egui::Sense::click_and_drag());
        head.dnd_set_drag_payload(ActivityDrag(key.to_string()));
        let folded = split && self.side_folded == Some(lower);
        let lifted = egui::DragAndDrop::payload::<ActivityDrag>(ui.ctx()).is_some_and(|d| d.0 == key);
        let ink = if lifted { crate::ui::theme::with_alpha(t.text_muted, 90) } else if head.hovered() { t.text } else { t.text_muted };
        let mut x = head_rect.left() + 14.0;
        if split {
            // With two views, an arrow: a click on the name folds this one away.
            let c = egui::pos2(x + 3.0, head_rect.center().y);
            let line = egui::Stroke::new(1.4, ink);
            if folded {
                ui.painter().line_segment([c + egui::vec2(-1.5, -3.5), c + egui::vec2(2.0, 0.0)], line);
                ui.painter().line_segment([c + egui::vec2(2.0, 0.0), c + egui::vec2(-1.5, 3.5)], line);
            } else {
                ui.painter().line_segment([c + egui::vec2(-3.5, -1.5), c + egui::vec2(0.0, 2.0)], line);
                ui.painter().line_segment([c + egui::vec2(0.0, 2.0), c + egui::vec2(3.5, -1.5)], line);
            }
            x += 14.0;
        }
        ui.painter().text(egui::pos2(x, head_rect.center().y), egui::Align2::LEFT_CENTER, heading.to_uppercase(), egui::FontId::proportional(11.0), ink);
        if head.clicked() && split {
            self.side_folded = if folded { None } else { Some(lower) };
        }
        let head = head.on_hover_cursor(egui::CursorIcon::Grab).on_hover_text(if split { t!("Drag to move this view; click to fold it away") } else { t!("Drag to move this view: onto the lower half, the activity bar, or out as a tab") });
        let (mut out, mut close, mut swap, mut connect) = (false, false, false, false);
        let mut view_menu = |ui: &mut egui::Ui| {
            if split {
                if crate::ui::kit::menu_item(ui, if lower { t!("Move to the upper half") } else { t!("Move to the lower half") }, None, true).clicked() {
                    swap = true;
                    ui.close();
                }
                if crate::ui::kit::menu_item(ui, t!("Close this half"), None, true).clicked() {
                    close = true;
                    ui.close();
                }
            }
            if crate::ui::kit::menu_item(ui, t!("Move to the main area"), None, true).clicked() {
                out = true;
                ui.close();
            }
        };
        head.context_menu(&mut view_menu);
        // At the heading's right end: the menu, and before it the view's own buttons.
        let icons = self.ed.reg.icons.clone();
        let mut bar = ui.new_child(egui::UiBuilder::new().id_salt(("side-head-bar", key)).max_rect(head_rect.shrink2(egui::vec2(8.0, 0.0))).layout(egui::Layout::right_to_left(egui::Align::Center)));
        bar.spacing_mut().item_spacing.x = 2.0;
        let more = crate::ui::side::action(&mut bar, &t, &icons, "more", t!("More…"));
        egui::Popup::menu(&more).id(egui::Id::unique(("side-head-menu", key))).show(&mut view_menu);
        let mut side_ask = None;
        match &tab {
            Tab::Side(SideView::Workspaces) if self.ed.project.is_some() => {
                if crate::ui::side::action(&mut bar, &t, &icons, "folder", t!("Open the project's folder")).clicked() {
                    self.open_project_folder();
                }
            }
            Tab::Side(SideView::Plugins) => {
                if crate::ui::side::action(&mut bar, &t, &icons, "folder", t!("Open the folder your own plugins go in")).clicked() {
                    side_ask = Some(crate::plugins::manager::SideAsk::OpenFolder);
                }
                if crate::ui::side::action(&mut bar, &t, &icons, "repeat", t!("Load plugin files again")).clicked() {
                    side_ask = Some(crate::plugins::manager::SideAsk::Reload);
                }
                if crate::ui::side::action(&mut bar, &t, &icons, "plus", t!("Install a plugin from a .zip file (or drop the file on the window)")).clicked() {
                    side_ask = Some(crate::plugins::manager::SideAsk::Install);
                }
            }
            Tab::Git => {
                if crate::ui::side::action(&mut bar, &t, &icons, "settings", t!("Git's window: where it is stored, signing in, keeping in step")).clicked() {
                    self.bot.git.open(&self.ed);
                }
                if self.bot.git.can_fetch() && crate::ui::side::action(&mut bar, &t, &icons, "repeat", t!("Fetch what is stored in the repository")).clicked() {
                    self.bot.git.fetch_now(&mut self.ed);
                }
            }
            Tab::Problems if self.problem_count > 0 => {
                crate::ui::side::badge(&mut bar, &t, self.problem_count);
            }
            _ => {}
        }
        if let Some(ask) = side_ask {
            self.plugins_side_ask(ask, ui.ctx());
        }
        // It draws in a box of its part's size: whatever it holds, the side
        // bar keeps the width it was given (or dragged to).
        let area = egui::Rect::from_min_max(egui::pos2(rect.left(), head_rect.bottom()), rect.max);
        let mut view_ui = ui.new_child(egui::UiBuilder::new().id_salt(("side-view", key)).max_rect(area).layout(egui::Layout::top_down(egui::Align::Min)));
        view_ui.set_clip_rect(area.intersect(ui.clip_rect()));
        let ui = &mut view_ui;
        let running = self.bot.state != BotState::Stopped;
        // Folded away, it is its heading only.
        if !folded {
        match tab {
            Tab::Side(view) => {
                if let Some(ask) = side_view_ui(ui, &mut self.ed, &mut self.stored, &mut self.plugin_host, running, problems, view) {
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
            Tab::Plugin(plugin, panel) => self.plugin_host.panel_ui(ui, &mut self.ed, &plugin, &panel, running),
            // The short view; Git's window has the rest.
            Tab::Git => self.bot.git.side(ui, &mut self.ed),
            // In the side bar they are always at hand: opened when shown.
            Tab::Host => {
                if !self.bot.host.open {
                    self.bot.host.open();
                }
                self.bot.host.tab(ui, &mut self.ed);
            }
            Tab::Live => {
                if !self.bot.live.open {
                    self.bot.live.open();
                }
                self.bot.live.tab(ui, &mut self.ed);
            }
            Tab::ServerConsole => match &mut self.bot.hosting.console {
                Some(c) => c.tab(ui, &self.ed),
                // Not connected (no server chosen yet, or another project).
                None => {
                    let t = &self.ed.theme;
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(t!("Not connected to a remote.")).size(12.5).color(t.text_muted));
                            ui.add_space(6.0);
                            if crate::ui::kit::secondary_button(ui, t, t!("Connect")).clicked() {
                                connect = true;
                            }
                        });
                    });
                }
            },
            _ => {}
        }
        }
        if connect {
            self.bot.hosting.open_console(&mut self.ed);
        }
        if swap {
            std::mem::swap(&mut self.ed.prefs.side_shown, &mut self.ed.prefs.side_lower);
        }
        if close {
            click_in_side(&mut self.ed.prefs, key);
        }
        if out {
            self.move_to_main(key, None);
        }
    }
}
