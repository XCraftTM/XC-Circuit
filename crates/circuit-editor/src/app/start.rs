//! The start window, while no project is open: a small window with what
//! there is to choose on the left (Projects, Live session, Host, Plugins,
//! Learn; Settings at the bottom) and that page on the right. Opening a
//! project turns it into the app's window, as big and where that was;
//! closing the project brings the start window back.

use super::*;

/// The start window's pages.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub(super) enum Page {
    #[default]
    Projects,
    Live,
    Host,
    Plugins,
    Learn,
    Settings,
}

impl Page {
    const LIST: [Page; 5] = [Page::Projects, Page::Live, Page::Host, Page::Plugins, Page::Learn];

    fn name(self) -> &'static str {
        match self {
            Page::Projects => t!("Projects"),
            Page::Live => t!("Live session"),
            Page::Host => "Host",
            Page::Plugins => t!("Plugins"),
            Page::Learn => t!("Learn"),
            Page::Settings => t!("Settings"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Page::Projects => "workspace",
            Page::Live => "user-plus",
            Page::Host => "server",
            Page::Plugins => "plug",
            Page::Learn => "info",
            Page::Settings => "settings",
        }
    }
}

/// How big the start window is.
pub(super) const SIZE: egui::Vec2 = egui::vec2(880.0, 640.0);
/// The smallest the app's window gets with a project open.
const MAIN_MIN: egui::Vec2 = egui::vec2(900.0, 600.0);
const MAIN_SIZE: egui::Vec2 = egui::vec2(1440.0, 900.0);
const NAV_W: f32 = 230.0;

/// Where the start window goes: over the middle of where the app's window
/// is (`outer`, on the screen), or left to the system without it.
pub(super) fn start_place(outer: Option<egui::Rect>) -> Option<egui::Pos2> {
    outer.map(|r| (r.center() - SIZE / 2.0).max(egui::pos2(0.0, 0.0)))
}

impl CircuitApp {
    /// Whether the start window shows: no project, and neither the first
    /// start page nor the tour.
    pub(super) fn start_shown(&self) -> bool {
        self.ed.project.is_none() && self.setup.is_none() && self.tour.is_none() && self.fps.is_none()
    }

    /// Gives the window the shape of what it shows: the start window's, or
    /// the app's as it was last. Called every frame; it acts when the two
    /// differ.
    pub(super) fn shape_window(&mut self, ctx: &egui::Context) {
        let start = self.start_shown();
        if self.window_shaped.is_none() {
            // The smallest size is the start window's until a project opens.
            ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(if start { SIZE } else { MAIN_MIN }));
        }
        let was = self.window_shaped.replace(start);
        if self.ed.prefs.start_window == start {
            if was.is_none() && start {
                // Maximized last time by another way than ours: not the start window.
                crate::app::reveal::take_maximize();
                ctx.send_viewport_cmd(ViewportCommand::Maximized(false));
            }
            return;
        }
        self.ed.prefs.start_window = start;
        let (outer, inner, maximized) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().inner_rect, i.viewport().maximized.unwrap_or(false)));
        if start {
            // The app's window as it is, for when a project opens.
            let maximized = crate::app::reveal::take_maximize() || maximized;
            self.ed.prefs.main_maximized = maximized;
            if !maximized && let (Some(o), Some(i)) = (outer, inner) {
                self.ed.prefs.main_window = Some([o.min.x, o.min.y, i.width(), i.height()]);
            }
            ctx.send_viewport_cmd(ViewportCommand::Maximized(false));
            ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(SIZE));
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(SIZE));
            if let Some(pos) = start_place(outer) {
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
            }
        } else {
            ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(MAIN_MIN));
            match self.ed.prefs.main_window {
                Some([x, y, w, h]) => {
                    ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(w, h).max(MAIN_MIN)));
                    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(egui::pos2(x, y)));
                }
                None => {
                    ctx.send_viewport_cmd(ViewportCommand::InnerSize(MAIN_SIZE));
                    // Around where the start window was.
                    if let Some(o) = outer {
                        ctx.send_viewport_cmd(ViewportCommand::OuterPosition((o.center() - MAIN_SIZE / 2.0).max(egui::pos2(0.0, 0.0))));
                    }
                }
            }
            if self.ed.prefs.main_maximized || self.ed.prefs.main_window.is_none() {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(true));
            }
            // What the start window's pages had open goes with it.
            self.bot.host.open = false;
            self.bot.live.open = false;
            self.start_page = Page::Projects;
        }
    }

    /// The start window: its title bar, the pages' names on the left and
    /// the picked page on the right.
    pub(super) fn start_window(&mut self, ui: &mut egui::Ui) {
        let theme = self.ed.theme.clone();
        let all = ui.max_rect();
        ui.painter().rect_filled(all, 0.0, theme.panel);
        let mut rest = all;
        if !self.ed.prefs.system_titlebar {
            let bar = egui::Rect::from_min_size(all.min, egui::vec2(all.width(), crate::app::titlebar::HEIGHT));
            rest.min.y = bar.bottom();
            ui.painter().rect_filled(bar, 0.0, theme.frame);
            let mut row = ui.new_child(egui::UiBuilder::new().id_salt("start-bar").max_rect(bar).layout(egui::Layout::left_to_right(egui::Align::Center)));
            crate::app::titlebar::drag_region(&mut row, bar);
            row.add_space(12.0);
            row.add(egui::Image::new(egui::include_image!("../../assets/icon.svg")).fit_to_exact_size(egui::vec2(16.0, 16.0)));
            row.add_space(4.0);
            row.add(egui::Label::new(RichText::new(t!("Welcome to XC Circuit")).size(12.5).color(theme.text_muted)).selectable(false));
            let mut right = ui.new_child(egui::UiBuilder::new().id_salt("start-buttons").max_rect(bar).layout(egui::Layout::right_to_left(egui::Align::Center)));
            right.spacing_mut().item_spacing.x = 0.0;
            crate::app::titlebar::window_buttons(&mut right, &theme);
        }
        let nav = egui::Rect::from_min_max(rest.min, egui::pos2(rest.left() + NAV_W, rest.bottom()));
        let page = egui::Rect::from_min_max(egui::pos2(nav.right(), rest.top()), rest.max);
        ui.painter().rect_filled(nav, 0.0, theme.frame);
        ui.painter().vline(nav.right() - 0.5, nav.y_range(), egui::Stroke::new(1.0, theme.line));
        self.ed.ui_rects.insert("start-nav".into(), nav);

        let mut side = ui.new_child(egui::UiBuilder::new().id_salt("start-nav").max_rect(nav).layout(egui::Layout::top_down(egui::Align::Min)));
        side.set_clip_rect(nav);
        side.spacing_mut().item_spacing.y = 2.0;
        side.add_space(16.0);
        side.horizontal(|ui| {
            ui.add_space(16.0);
            ui.add(egui::Image::new(egui::include_image!("../../assets/icon.svg")).fit_to_exact_size(egui::vec2(34.0, 34.0)));
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(RichText::new("XC™ Circuit").size(14.5).strong().color(theme.text));
                ui.label(RichText::new(env!("CARGO_PKG_VERSION")).size(11.5).color(theme.text_muted));
            });
        });
        side.add_space(16.0);
        for p in Page::LIST {
            // Plugins says how many there are, as a number at the row's end.
            let count = (p == Page::Plugins).then(|| self.ed.reg.plugins.len().to_string());
            if self.start_nav_row(&mut side, &theme, p, count.as_deref()) {
                self.pick_start_page(p);
            }
        }
        let foot = egui::Rect::from_min_max(egui::pos2(nav.left(), nav.bottom() - 46.0), nav.max);
        let mut foot_ui = ui.new_child(egui::UiBuilder::new().id_salt("start-foot").max_rect(foot).layout(egui::Layout::top_down(egui::Align::Min)));
        if self.start_nav_row(&mut foot_ui, &theme, Page::Settings, None) {
            self.pick_start_page(Page::Settings);
        }

        let body = page.shrink2(egui::vec2(20.0, 16.0));
        let mut pane = ui.new_child(egui::UiBuilder::new().id_salt(("start-page", self.start_page)).max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
        pane.set_clip_rect(page);
        match self.start_page {
            Page::Projects => {
                let recent = self.ed.prefs.recent_projects.clone();
                if let Some(pick) = crate::pages::start_page::show(&mut pane, &theme, &mut self.start, &recent, &self.ed.reg) {
                    self.start_page_pick(pick);
                }
            }
            // These draw to their edges themselves.
            Page::Live | Page::Host | Page::Plugins | Page::Settings => {
                let mut whole = ui.new_child(egui::UiBuilder::new().id_salt(("start-whole", self.start_page)).max_rect(page).layout(egui::Layout::top_down(egui::Align::Min)));
                whole.set_clip_rect(page);
                match self.start_page {
                    Page::Live => self.bot.live.join_page(&mut whole, &self.ed),
                    Page::Host => {
                        // Its Close button has nothing to close here.
                        self.bot.host.open = true;
                        self.bot.host.tab(&mut whole, &mut self.ed);
                    }
                    Page::Plugins => {
                        if let Some(ask) = crate::plugins::manager::side(&mut whole, &theme, &self.ed, &self.plugin_host) {
                            self.plugins_side_ask(ask, ui.ctx());
                        }
                    }
                    _ => {
                        let mut asks = Vec::new();
                        crate::pages::settings::ui(&mut whole, &mut self.ed, &mut self.settings, &mut asks);
                        self.settings_asks(asks, ui.ctx());
                    }
                }
            }
            Page::Learn => self.start_learn(&mut pane, &theme),
        }
        if !self.ed.prefs.system_titlebar {
            crate::app::titlebar::frame_edges(ui.ctx(), &theme);
        }
    }

    /// Shows one of the start window's pages; Live session and Host get
    /// ready as they do when opened from the menu.
    pub(super) fn pick_start_page(&mut self, page: Page) {
        self.start_page = page;
        match page {
            Page::Live => self.bot.live.ready_to_join(),
            Page::Host => self.bot.host.open(),
            _ => {}
        }
    }

    /// One name in the start window's list. True when it was clicked.
    fn start_nav_row(&self, ui: &mut egui::Ui, theme: &crate::ui::theme::Theme, page: Page, count: Option<&str>) -> bool {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), egui::Sense::click());
        let picked = self.start_page == page;
        crate::ui::kit::describe_selected(&resp, egui::Role::Button, picked, page.name());
        let row = rect.shrink2(egui::vec2(8.0, 0.0));
        if picked {
            ui.painter().rect_filled(row, 6.0, crate::ui::theme::with_alpha(theme.accent, 70));
        } else if resp.hovered() {
            ui.painter().rect_filled(row, 6.0, crate::ui::theme::mix(theme.frame, theme.text, 0.08));
        }
        let color = if picked || resp.hovered() { theme.text } else { theme.text_muted };
        if let Some(path) = self.ed.reg.icons.get(page.icon()) {
            egui::Image::from_uri(crate::editor::file_uri(path)).tint(color).paint_at(ui, egui::Rect::from_center_size(egui::pos2(row.left() + 18.0, row.center().y), egui::vec2(16.0, 16.0)));
        }
        ui.painter().text(egui::pos2(row.left() + 36.0, row.center().y), egui::Align2::LEFT_CENTER, page.name(), egui::FontId::proportional(13.0), color);
        if let Some(count) = count {
            ui.painter().text(egui::pos2(row.right() - 10.0, row.center().y), egui::Align2::RIGHT_CENTER, count, egui::FontId::proportional(11.5), theme.text_muted);
        }
        resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
    }

    /// Learn: the tour, the example bot, the docs and the first start page.
    fn start_learn(&mut self, ui: &mut egui::Ui, theme: &crate::ui::theme::Theme) {
        enum Ask {
            Tour,
            Example,
            Docs,
            Setup,
        }
        let has_example = self.ed.examples_dir.as_ref().is_some_and(|e| e.join("DemoBot").is_dir());
        let mut ask = None;
        ui.label(RichText::new(t!("Learn")).size(17.0).strong().color(theme.text));
        ui.add_space(2.0);
        ui.label(RichText::new(t!("New here? These get you going.")).size(12.5).color(theme.text_muted));
        ui.add_space(14.0);
        let mut row = |ui: &mut egui::Ui, title: &str, about: &str, button: &str, what: Ask| {
            egui::Frame::new().fill(theme.panel_header).corner_radius(8).inner_margin(egui::Margin::symmetric(14, 12)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - 190.0).max(160.0));
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new(title).size(13.5).strong().color(theme.text));
                        ui.label(RichText::new(about).size(12.0).color(theme.text_muted));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::ui::kit::secondary_button(ui, theme, button).clicked() {
                            ask = Some(what);
                        }
                    });
                });
            });
            ui.add_space(8.0);
        };
        row(ui, t!("Take the tour"), t!("A few minutes on a practice bot: blocks, wires, Run."), t!("Start the tour"), Ask::Tour);
        if has_example {
            row(ui, t!("The example bot"), t!("DemoBot: slash commands, embeds, buttons and stored data to look at and change."), t!("Open DemoBot"), Ask::Example);
        }
        row(ui, t!("The docs"), t!("How things work, every block, and how to write plugins."), t!("Open the docs"), Ask::Docs);
        row(ui, t!("Welcome and setup"), t!("The first start page again: language, Node.js, what the app needs."), t!("Open setup"), Ask::Setup);
        match ask {
            Some(Ask::Tour) => self.start_tour(),
            Some(Ask::Example) => {
                if let Some(example) = self.ed.examples_dir.as_deref().and_then(writable_example) {
                    self.switch_project(&example);
                }
            }
            Some(Ask::Docs) => {
                let _ = open::that("https://xcrafttm.github.io/XC-Circuit/");
            }
            Some(Ask::Setup) => self.setup = Some(crate::pages::setup::Setup::new()),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_window_sits_over_where_the_app_was() {
        let app = egui::Rect::from_min_size(egui::pos2(200.0, 100.0), egui::vec2(1440.0, 900.0));
        assert_eq!(start_place(Some(app)), Some(egui::pos2(480.0, 230.0)));
        assert_eq!(start_place(None), None);
        // At a screen's corner it stays on the screen.
        let small = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 300.0));
        assert_eq!(start_place(Some(small)), Some(egui::pos2(0.0, 0.0)));
        assert_eq!(Page::default(), Page::Projects);
        assert!(!Page::LIST.contains(&Page::Settings), "Settings is at the bottom, on its own");
    }
}
