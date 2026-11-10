//! Notifications: what happened that is worth a look, shown for a while in
//! the window's bottom right corner and kept in a list behind the bell at
//! the end of the status bar. Errors and warnings from the Console come
//! here (a block's error with a button to show the block), how storing in
//! Git and uploading went, and what is under way right now (saving,
//! storing, uploading, downloading an update) with a spinner or a bar.

use std::time::{Duration, Instant};

use super::*;

/// How many are kept; the oldest go.
const MOST: usize = 100;
/// How long one shows by itself before it is only in the list.
const SHOWN: Duration = Duration::from_secs(8);
const SHOWN_ERROR: Duration = Duration::from_secs(14);
const WIDTH: f32 = 380.0;

/// What clicking a notification's button does.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Go {
    /// Show the block it is about: workspace key, block id.
    Block(String, String),
    Console,
    Git,
    Remote,
}

#[derive(Clone, Debug)]
pub(super) struct Notice {
    id: u64,
    pub(super) level: Level,
    pub(super) text: String,
    pub(super) go: Go,
    /// The time of day it came, as the Console writes it.
    time: String,
    /// How many times it came (the same words again count up).
    pub(super) times: u32,
    /// Shown in the corner until then.
    until: Option<Instant>,
    /// Looked at in the list.
    seen: bool,
}

/// Something under way: its words, and how far it is when that is known.
struct Running {
    text: String,
    part: Option<f32>,
}

#[derive(Default)]
pub(super) struct Notices {
    pub(super) list: Vec<Notice>,
    next: u64,
    /// The Console's lines looked at so far.
    log_count: u64,
    /// The list behind the bell is open.
    pub(super) open: bool,
    /// Do not disturb: nothing shows by itself, the list still fills.
    pub(super) quiet: bool,
    /// The first frame is over: what the app said while starting isn't news.
    started: bool,
    /// Git and the upload were busy last frame: their outcome is told when they end.
    git_busy: bool,
    upload_busy: bool,
}

impl Notices {
    /// Adds one, or counts up the same words when they are still unread.
    pub(super) fn add(&mut self, level: Level, text: String, go: Go, time: String) {
        let long = if level == Level::Error { SHOWN_ERROR } else { SHOWN };
        let until = (!self.quiet).then(|| Instant::now() + long);
        if let Some(same) = self.list.iter_mut().find(|n| !n.seen && n.text == text && n.go == go) {
            same.times += 1;
            same.time = time;
            same.until = until.or(same.until);
            return;
        }
        self.next += 1;
        self.list.push(Notice { id: self.next, level, text, go, time, times: 1, until, seen: false });
        if self.list.len() > MOST {
            let cut = self.list.len() - MOST;
            self.list.drain(..cut);
        }
    }

    /// How many haven't been looked at.
    pub(super) fn unread(&self) -> usize {
        self.list.iter().filter(|n| !n.seen).count()
    }
}

impl CircuitApp {
    /// Takes in what happened since the last frame: new errors and warnings
    /// in the Console, and how storing and uploading ended.
    pub(super) fn collect_notices(&mut self) {
        let new = self.ed.log_count.saturating_sub(self.notices.log_count) as usize;
        self.notices.log_count = self.ed.log_count;
        // The tour's practice bot, and what the app said while it started
        // (what loaded), aren't news.
        let started = std::mem::replace(&mut self.notices.started, true);
        if self.tour.is_none() && started {
            let from = self.ed.log.len().saturating_sub(new);
            let lines: Vec<LogLine> = self.ed.log[from..].iter().filter(|l| matches!(l.level, Level::Error | Level::Warn)).cloned().collect();
            for line in lines {
                // What it says, without the lines of where in the code it happened.
                let words = first_words(&line.text);
                let go = match line.target {
                    Some((ws, id)) => Go::Block(ws, id),
                    // A block that failed says so without its place: the
                    // block marked as failed with these words is the one.
                    None => self.ed.activity.iter().find(|(_, a)| a.error.as_deref().is_some_and(|e| !e.is_empty() && words.contains(first_words(e).as_str()))).map_or(Go::Console, |((ws, id), _)| Go::Block(ws.clone(), id.clone())),
                };
                self.notices.add(line.level, words, go, line.time);
            }
        }
        let now = crate::editor::clock();
        let git = self.bot.git.busy_words().is_some();
        if self.notices.git_busy && !git && let Some((ok, text)) = self.bot.git.last_said().cloned() {
            self.notices.add(if ok { Level::Success } else { Level::Warn }, text, Go::Git, now.clone());
        }
        self.notices.git_busy = git;
        let upload = self.bot.hosting.busy().is_some();
        if self.notices.upload_busy && !upload && let Some(o) = &self.bot.hosting.outcome {
            self.notices.add(if o.ok { Level::Success } else { Level::Error }, o.text.clone(), Go::Remote, now);
        }
        self.notices.upload_busy = upload;
    }

    /// What is under way right now.
    fn running(&self) -> Vec<Running> {
        let mut list = Vec::new();
        if self.ed.saver.busy() {
            list.push(Running { text: t!("Saving…").to_string(), part: None });
        }
        if let Some(words) = self.bot.git.busy_words() {
            list.push(Running { text: words, part: None });
        }
        if let Some(what) = self.bot.hosting.busy() {
            list.push(Running { text: tf!("{what} …", what = what), part: None });
        }
        if let Some(part) = self.updater.progress() {
            list.push(Running { text: t!("Downloading the update").to_string(), part: Some(part) });
        }
        if self.block_updates.installing() {
            list.push(Running { text: t!("Installing block updates").to_string(), part: None });
        }
        list
    }

    fn go(&mut self, go: Go) {
        match go {
            Go::Block(ws, id) => self.ed.reveal(&ws, &id),
            Go::Console => self.show_panel(Tab::Console),
            Go::Git => self.show_git(),
            Go::Remote => self.bot.hosting.open(&self.ed),
        }
    }

    /// The bell at the end of the status bar, with how many are unread.
    /// Laid out right to left.
    pub(super) fn bell(&mut self, ui: &mut egui::Ui) {
        let t = self.ed.theme.clone();
        let unread = self.notices.unread();
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(if unread > 0 { 40.0 } else { 26.0 }, ui.available_height()), egui::Sense::click());
        let name = if unread > 0 { tf!("Notifications ({count} new)", count = unread) } else { t!("Notifications").to_string() };
        crate::ui::kit::describe_selected(&resp, egui::Role::Button, self.notices.open, &name);
        if resp.hovered() || self.notices.open {
            ui.painter().rect_filled(rect.shrink2(egui::vec2(0.0, 2.0)), 4.0, crate::ui::theme::mix(t.status_bar, t.status_text, 0.14));
        }
        let icon = egui::Rect::from_center_size(egui::pos2(rect.left() + 13.0, rect.center().y), egui::vec2(13.0, 13.0));
        let ink = if self.notices.quiet { crate::ui::theme::with_alpha(t.status_text, 110) } else { t.status_text };
        if let Some(path) = self.ed.reg.icons.get("bell") {
            egui::Image::from_uri(crate::editor::file_uri(path)).tint(ink).paint_at(ui, icon);
        }
        if unread > 0 {
            let text = if unread > 99 { "99+".to_string() } else { unread.to_string() };
            ui.painter().text(egui::pos2(icon.right() + 4.0, rect.center().y), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(11.0), t.accent);
        }
        self.ed.ui_rects.insert("bell".into(), rect);
        if resp.on_hover_text(name).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            self.notices.open = !self.notices.open;
        }
    }

    /// The corner's cards and, while it is open, the list. Called after
    /// everything else is drawn.
    pub(super) fn notices_ui(&mut self, ctx: &egui::Context) {
        let t = self.ed.theme.clone();
        let icons = self.ed.reg.icons.clone();
        let running = self.running();
        let now = Instant::now();
        let mut go = None;
        let mut close: Vec<u64> = Vec::new();
        let foot = if self.ed.prefs.status_bar_hidden { 12.0 } else { 34.0 };
        if self.notices.open {
            let mut clear = false;
            let area = egui::Area::new(egui::Id::unique("notices-list")).order(egui::Order::Foreground).anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -foot)).show(ctx, |ui| {
                card(&t).show(ui, |ui| {
                    ui.set_width(WIDTH);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t!("Notifications")).size(12.5).strong().color(t.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            if crate::ui::side::action(ui, &t, &icons, "x", t!("Close")).clicked() {
                                self.notices.open = false;
                            }
                            if crate::ui::side::action(ui, &t, &icons, "trash", t!("Clear all")).clicked() {
                                clear = true;
                            }
                            let (icon, tip) = if self.notices.quiet { ("eye-off", t!("Do not disturb is on: nothing shows by itself. Click to turn it off.")) } else { ("eye", t!("Do not disturb: keep them in this list only")) };
                            if crate::ui::side::action(ui, &t, &icons, icon, tip).clicked() {
                                self.notices.quiet = !self.notices.quiet;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    for r in &running {
                        running_row(ui, &t, r);
                    }
                    if self.notices.list.is_empty() && running.is_empty() {
                        ui.add_space(4.0);
                        ui.label(RichText::new(t!("Nothing new.")).size(12.0).color(t.text_muted));
                        ui.add_space(4.0);
                    }
                    egui::ScrollArea::vertical().id_salt("notices-scroll").max_height(420.0).auto_shrink([false, true]).show(ui, |ui| {
                        // The newest on top.
                        for n in self.notices.list.iter().rev() {
                            match notice_row(ui, &t, &icons, n, false) {
                                Some(Asked::Go) => go = Some(n.go.clone()),
                                Some(Asked::Close) => close.push(n.id),
                                None => {}
                            }
                        }
                    });
                });
            });
            // Looked at now; and what showed in the corner is in the list.
            for n in &mut self.notices.list {
                n.seen = true;
                n.until = None;
            }
            if clear {
                self.notices.list.clear();
            }
            // A click elsewhere (not on the bell, which toggles it) closes it.
            let bell = self.ed.ui_rects.get("bell").copied();
            let outside = ctx.input(|i| i.pointer.any_pressed().then(|| i.pointer.interact_pos()).flatten()).is_some_and(|p| !area.response.rect.contains(p) && bell.is_none_or(|b| !b.contains(p)));
            if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.notices.open = false;
            }
        } else {
            // The corner: what is under way, and what came in lately.
            let shown: Vec<Notice> = self.notices.list.iter().filter(|n| n.until.is_some_and(|u| u > now)).rev().take(4).cloned().collect();
            if !shown.is_empty() || !running.is_empty() {
                let mut hovered = false;
                egui::Area::new(egui::Id::unique("notices-corner")).order(egui::Order::Foreground).anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -foot)).show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    for n in shown.iter().rev() {
                        let r = card(&t).show(ui, |ui| {
                            ui.set_width(WIDTH);
                            notice_row(ui, &t, &icons, n, true)
                        });
                        hovered |= ui.rect_contains_pointer(r.response.rect);
                        match r.inner {
                            Some(Asked::Go) => {
                                go = Some(n.go.clone());
                                close.push(n.id);
                            }
                            Some(Asked::Close) => close.push(n.id),
                            None => {}
                        }
                    }
                    if !running.is_empty() {
                        card(&t).show(ui, |ui| {
                            ui.set_width(WIDTH);
                            for r in &running {
                                running_row(ui, &t, r);
                            }
                        });
                    }
                });
                // Under the pointer they stay; otherwise the next one to go says when.
                if hovered {
                    for n in &mut self.notices.list {
                        if n.until.is_some_and(|u| u > now) {
                            n.until = Some(now + SHOWN);
                        }
                    }
                }
                let next = self.notices.list.iter().filter_map(|n| n.until).filter(|u| *u > now).min();
                match next {
                    Some(at) => ctx.request_repaint_after(at - now + Duration::from_millis(30)),
                    None if !running.is_empty() => ctx.request_repaint_after(Duration::from_millis(120)),
                    None => {}
                }
            }
            // In the corner, × only takes it out of the corner.
            for id in close.drain(..) {
                if let Some(n) = self.notices.list.iter_mut().find(|n| n.id == id) {
                    n.until = None;
                    n.seen |= go.is_some();
                }
            }
        }
        self.notices.list.retain(|n| !close.contains(&n.id));
        if let Some(go) = go {
            self.notices.open = false;
            self.go(go);
        }
    }
}

/// The first of an error's lines: the rest says where in the code it
/// happened, which the Console keeps.
fn first_words(text: &str) -> String {
    text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default().to_string()
}

enum Asked {
    Go,
    Close,
}

fn card(t: &crate::ui::theme::Theme) -> Frame {
    Frame::new().fill(t.panel_header).stroke(egui::Stroke::new(1.0, t.node_border)).corner_radius(6).inner_margin(Margin::symmetric(10, 8)).shadow(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: egui::Color32::from_black_alpha(90) })
}

/// What is under way: a spinner or a bar, and its words.
fn running_row(ui: &mut egui::Ui, t: &crate::ui::theme::Theme, r: &Running) {
    ui.horizontal(|ui| {
        if r.part.is_none() {
            ui.add(egui::Spinner::new().size(13.0).color(t.accent));
        }
        ui.label(RichText::new(&r.text).size(12.5).color(t.text));
    });
    if let Some(part) = r.part {
        let (bar, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 4.0), egui::Sense::hover());
        ui.painter().rect_filled(bar, 2.0, t.field);
        ui.painter().rect_filled(bar.with_max_x(bar.left() + bar.width() * part.clamp(0.0, 1.0)), 2.0, t.accent);
    }
    ui.add_space(2.0);
}

/// One notification: its colour and icon, its words, and its buttons.
fn notice_row(ui: &mut egui::Ui, t: &crate::ui::theme::Theme, icons: &std::collections::HashMap<String, PathBuf>, n: &Notice, corner: bool) -> Option<Asked> {
    let mut asked = None;
    let (icon, color) = match n.level {
        Level::Error => ("alert", t.error),
        Level::Warn => ("alert", t.warning),
        Level::Success => ("check", t.action),
        _ => ("info", t.accent),
    };
    ui.horizontal_top(|ui| {
        let (place, _) = ui.allocate_exact_size(egui::vec2(16.0, 18.0), egui::Sense::hover());
        if let Some(path) = icons.get(icon) {
            egui::Image::from_uri(crate::editor::file_uri(path)).tint(color).paint_at(ui, egui::Rect::from_center_size(place.center(), egui::vec2(14.0, 14.0)));
        }
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.set_width(ui.available_width() - 26.0);
            // A long one (a stack of lines) is cut; the Console has it whole.
            let words: String = if n.text.chars().count() > 280 { n.text.chars().take(280).collect::<String>() + "…" } else { n.text.clone() };
            ui.add(egui::Label::new(RichText::new(words).size(12.5).color(t.text)).wrap());
            ui.horizontal(|ui| {
                let label = match &n.go {
                    Go::Block(..) => t!("Show block"),
                    Go::Console => t!("Show the Console"),
                    Go::Git => t!("Show Git"),
                    Go::Remote => t!("Show the remote"),
                };
                if crate::ui::kit::link(ui, t, label, egui::FontId::proportional(12.0)).clicked() {
                    asked = Some(Asked::Go);
                }
                let mut about = n.time.clone();
                if n.times > 1 {
                    about = tf!("{time} · {count} times", time = about, count = n.times);
                }
                ui.label(RichText::new(about).size(11.0).color(t.text_muted));
            });
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            let tip = if corner { t!("Hide (it stays in the list behind the bell)") } else { t!("Remove") };
            if crate::ui::side::action(ui, t, icons, "x", tip).clicked() {
                asked = Some(Asked::Close);
            }
        });
    });
    if !corner {
        ui.add_space(2.0);
        ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), egui::Stroke::new(1.0, t.line));
        ui.add_space(4.0);
    }
    asked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_words_count_up_and_old_ones_go() {
        let mut n = Notices::default();
        n.add(Level::Error, "Send Message failed".into(), Go::Block("main".into(), "4".into()), "12:00:00".into());
        n.add(Level::Error, "Send Message failed".into(), Go::Block("main".into(), "4".into()), "12:00:01".into());
        n.add(Level::Error, "Send Message failed".into(), Go::Block("main".into(), "5".into()), "12:00:02".into());
        assert_eq!(n.list.len(), 2, "the same block's error again counts up; another block's is its own");
        assert_eq!((n.list[0].times, n.unread()), (2, 2));
        // Read, the same words are news again.
        n.list[0].seen = true;
        n.add(Level::Error, "Send Message failed".into(), Go::Block("main".into(), "4".into()), "12:01:00".into());
        assert_eq!(n.list.len(), 3);
        // Do not disturb: kept, but not shown by itself.
        n.quiet = true;
        n.add(Level::Warn, "Quiet".into(), Go::Console, "12:02:00".into());
        assert!(n.list.last().unwrap().until.is_none());
        for i in 0..150 {
            n.add(Level::Warn, format!("line {i}"), Go::Console, "12:03:00".into());
        }
        assert_eq!(n.list.len(), MOST);
        assert_eq!(n.list.last().unwrap().text, "line 149");
        assert_eq!(first_words("Log (#4): Cannot read 'value'
    at fillTemplate (x.js:1:2)
    at run"), "Log (#4): Cannot read 'value'");
        assert_eq!(first_words("
  plain  "), "plain");
    }
}
