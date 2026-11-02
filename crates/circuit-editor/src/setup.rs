//! The First Start page: welcome, Node.js (find one or install it), theme,
//! and the offer of a tour. Shown the first time XC Circuit starts; its
//! Node.js step also opens from Settings.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::thread;

use circuit_runner::node::MIN_NODE_MAJOR;
use circuit_runner::node_setup::{self, NodeInstall};
use eframe::egui::{self, Align, Align2, Color32, CursorIcon, FontId, Layout, Rect, RichText, ScrollArea, Sense, Stroke, Ui, pos2, vec2};

use crate::editor::{Editor, file_uri};
use crate::theme::{Theme, mix, with_alpha};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Welcome,
    Node,
    Look,
    Tour,
}

impl Step {
    const ALL: [(Step, &'static str); 4] = [(Step::Welcome, "Welcome"), (Step::Node, "Node.js"), (Step::Look, "Look"), (Step::Tour, "Tour")];

    fn index(self) -> usize {
        Self::ALL.iter().position(|(s, _)| *s == self).unwrap_or(0)
    }
}

/// What the page asks the app to do.
pub enum Done {
    /// Setup is over; start the tour or not.
    Finished { tour: bool },
    /// Closed (the Node.js step opened from Settings).
    Closed,
}

enum JobMsg {
    Log(String),
    Done(Result<PathBuf, String>),
}

/// One way of installing Node.js, given a place to send progress lines.
type Installer = fn(&mut dyn FnMut(String)) -> anyhow::Result<PathBuf>;

/// An install running in the background.
struct Job {
    rx: Receiver<JobMsg>,
    log: Vec<String>,
}

pub struct Setup {
    pub step: Step,
    /// Only the Node.js step, opened from Settings.
    node_only: bool,
    installs: Option<Vec<NodeInstall>>,
    scan: Option<Receiver<Vec<NodeInstall>>>,
    winget: Option<bool>,
    job: Option<Job>,
    /// The last install's outcome.
    outcome: Option<Result<PathBuf, String>>,
}

impl Setup {
    pub fn new() -> Setup {
        Setup { step: Step::Welcome, node_only: false, installs: None, scan: None, winget: None, job: None, outcome: None }
    }

    /// Just the Node.js step, from Settings.
    pub fn node_only() -> Setup {
        Setup { step: Step::Node, node_only: true, ..Setup::new() }
    }

    fn rescan(&mut self) {
        let (tx, rx) = channel();
        let root = private_root();
        thread::spawn(move || {
            let _ = tx.send(node_setup::find_installs(&root));
        });
        self.scan = Some(rx);
        self.installs = None;
    }
}

/// Where XC Circuit keeps the Node.js it downloads itself.
pub fn private_root() -> PathBuf {
    crate::bot::app_data_dir().join("node")
}

pub fn show(ui: &mut Ui, ed: &mut Editor, setup: &mut Setup) -> Option<Done> {
    let theme = ed.theme.clone();
    let ctx = ui.ctx().clone();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, theme.canvas);
    let mut done = None;

    // Background work.
    if setup.step == Step::Node && setup.installs.is_none() && setup.scan.is_none() {
        setup.rescan();
    }
    if let Some(rx) = &setup.scan {
        if let Ok(found) = rx.try_recv() {
            choose_default(ed, &found);
            setup.installs = Some(found);
            setup.scan = None;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    if setup.winget.is_none() && setup.step == Step::Node {
        setup.winget = Some(node_setup::winget_available());
    }
    let mut finished_job = None;
    if let Some(job) = &mut setup.job {
        while let Ok(msg) = job.rx.try_recv() {
            match msg {
                JobMsg::Log(l) => job.log.push(l),
                JobMsg::Done(r) => finished_job = Some(r),
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
    if let Some(result) = finished_job {
        setup.job = None;
        if let Ok(path) = &result {
            ed.prefs.node_path = Some(path.display().to_string());
            ed.prefs.apply_node();
        }
        setup.outcome = Some(result);
        setup.rescan();
    }

    // Layout: steps on top, the step's content, then the buttons.
    let column = Rect::from_center_size(full.center(), vec2(full.width().min(760.0) - 32.0, full.height() - 48.0));
    let mut col = ui.new_child(egui::UiBuilder::new().max_rect(column).layout(Layout::top_down(Align::Min)));
    if !setup.node_only {
        steps_bar(&mut col, &theme, setup.step);
        col.add_space(18.0);
    }
    let footer_h = 56.0;
    let body = Rect::from_min_max(col.cursor().min, pos2(column.right(), column.bottom() - footer_h));
    let mut body_ui = col.new_child(egui::UiBuilder::new().max_rect(body).layout(Layout::top_down(Align::Min)));
    ScrollArea::vertical().auto_shrink([false, false]).show(&mut body_ui, |ui| match setup.step {
        Step::Welcome => welcome(ui, ed, &theme),
        Step::Node => node_step(ui, ed, &theme, setup),
        Step::Look => look(ui, ed, &theme),
        Step::Tour => {
            if let Some(tour) = tour_choice(ui, ed, &theme) {
                done = Some(Done::Finished { tour });
            }
        }
    });

    // Footer.
    let footer = Rect::from_min_max(pos2(column.left(), column.bottom() - footer_h + 12.0), column.max);
    let mut f = col.new_child(egui::UiBuilder::new().max_rect(footer).layout(Layout::left_to_right(Align::Center)));
    if setup.node_only {
        f.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if crate::dialog::primary_button(ui, &theme, "Save and close").clicked() && setup.job.is_none() {
                done = Some(Done::Closed);
            }
        });
        return done;
    }
    if crate::dialog::secondary_button(&mut f, &theme, "Skip setup").on_hover_text("You can run it again from Help").clicked() {
        done = Some(Done::Finished { tour: false });
    }
    f.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let i = setup.step.index();
        if setup.step != Step::Tour {
            let label = if setup.step == Step::Welcome { "Get started" } else { "Next" };
            if crate::dialog::primary_button(ui, &theme, label).clicked() && setup.job.is_none() {
                setup.step = Step::ALL[i + 1].0;
            }
        }
        if i > 0 && crate::dialog::secondary_button(ui, &theme, "Back").clicked() && setup.job.is_none() {
            setup.step = Step::ALL[i - 1].0;
        }
    });
    done
}

/// Uses the PATH's Node.js if it is new enough, otherwise the newest usable one.
fn choose_default(ed: &mut Editor, found: &[NodeInstall]) {
    let chosen_ok = ed.prefs.node_path.as_ref().is_some_and(|p| found.iter().any(|n| n.path == std::path::Path::new(p) && n.usable()));
    if chosen_ok {
        return;
    }
    let path_ok = ed.prefs.node_path.is_none() && found.iter().any(|n| n.source == "PATH" && n.usable());
    if path_ok {
        return;
    }
    if let Some(best) = found.iter().find(|n| n.usable()) {
        ed.prefs.node_path = Some(best.path.display().to_string());
        ed.prefs.apply_node();
    }
}

fn steps_bar(ui: &mut Ui, theme: &Theme, current: Step) {
    const GAP: f32 = 8.0;
    const LINE: f32 = 36.0;
    // Measured first, so the whole stepper can be centered.
    let labels: f32 = Step::ALL.iter().map(|(_, name)| ui.painter().layout_no_wrap(name.to_string(), FontId::proportional(13.0), theme.text).size().x).sum();
    let n = Step::ALL.len() as f32;
    let width = n * (22.0 + GAP) + labels + n * GAP + (n - 1.0) * (LINE + GAP);
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        ui.spacing_mut().item_spacing.x = GAP;
        for (i, (step, name)) in Step::ALL.iter().enumerate() {
            let done = i < current.index();
            let active = *step == current;
            let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
            let fill = if active || done { theme.accent } else { theme.field };
            ui.painter().circle_filled(r.center(), 11.0, fill);
            let ink = if active || done { Color32::from_rgb(0, 24, 28) } else { theme.text_muted };
            if done {
                check_mark(ui, r.center(), 9.0, ink);
            } else {
                ui.painter().text(r.center(), Align2::CENTER_CENTER, (i + 1).to_string(), FontId::proportional(12.0), ink);
            }
            ui.label(RichText::new(*name).size(13.0).color(if active { theme.text } else { theme.text_muted }));
            if i + 1 < Step::ALL.len() {
                let (line, _) = ui.allocate_exact_size(vec2(LINE, 22.0), Sense::hover());
                ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, if done { theme.accent } else { theme.line }));
            }
        }
    });
}

/// A drawn check mark (the font has no ✓).
fn check_mark(ui: &Ui, center: egui::Pos2, size: f32, color: Color32) {
    let s = size / 2.0;
    ui.painter().line(vec![center + vec2(-s, 0.0), center + vec2(-s * 0.3, s * 0.65), center + vec2(s, -s * 0.6)], Stroke::new(size / 6.0 + 0.6, color));
}

fn title(ui: &mut Ui, theme: &Theme, text: &str, sub: &str) {
    ui.label(RichText::new(text).size(26.0).strong().color(theme.text));
    ui.add_space(4.0);
    ui.label(RichText::new(sub).size(14.0).color(theme.text_muted));
    ui.add_space(18.0);
}

fn icon(ui: &Ui, ed: &Editor, name: &str, center: egui::Pos2, size: f32, tint: Color32) {
    if let Some(path) = ed.reg.icons.get(name) {
        egui::Image::from_uri(file_uri(path)).tint(tint).paint_at(ui, Rect::from_center_size(center, vec2(size, size)));
    }
}

fn card(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new().fill(theme.panel).stroke(Stroke::new(1.0, theme.line)).corner_radius(10).inner_margin(egui::Margin::symmetric(18, 14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
    ui.add_space(10.0);
}

fn welcome(ui: &mut Ui, ed: &Editor, theme: &Theme) {
    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        ui.add(egui::Image::new(egui::include_image!("../assets/icon.svg")).fit_to_exact_size(vec2(96.0, 96.0)));
        ui.add_space(14.0);
        ui.label(RichText::new("Welcome to XC™ Circuit").size(30.0).strong().color(theme.text));
        ui.add_space(6.0);
        ui.label(RichText::new("Build Discord bots by connecting blocks.").size(15.0).color(theme.text_muted));
    });
    ui.add_space(30.0);
    for (ic, head, text) in [
        ("workspace", "Connect blocks", "Every block does one thing. Wire them together on a canvas, no code needed."),
        ("bolt", "Run it with one click", "Start your bot from the editor and watch blocks light up as they run."),
        ("package", "Take it anywhere", "Export a folder that runs with node index.js on any server."),
    ] {
        card(ui, theme, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
                ui.painter().rect_filled(r, 10.0, with_alpha(theme.accent, 36));
                icon(ui, ed, ic, r.center(), 22.0, theme.accent);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(head).size(15.0).strong().color(theme.text));
                    ui.label(RichText::new(text).size(13.0).color(theme.text_muted));
                });
            });
        });
    }
    ui.add_space(8.0);
    ui.label(RichText::new("Setup takes a minute: Node.js, a look you like, and an optional tour.").size(12.5).color(theme.text_muted));
}

fn node_step(ui: &mut Ui, ed: &mut Editor, theme: &Theme, setup: &mut Setup) {
    title(ui, theme, "Node.js", &format!("Bots run on Node.js {MIN_NODE_MAJOR} or newer. XC Circuit can use one you already have, or install it for you."));

    // What is chosen now.
    let chosen = ed.prefs.node_path.clone();
    let installs = setup.installs.clone().unwrap_or_default();
    let current = match &chosen {
        Some(p) => installs.iter().find(|n| n.path == std::path::Path::new(p)),
        None => installs.iter().find(|n| n.source == "PATH"),
    };
    card(ui, theme, |ui| {
        ui.horizontal(|ui| {
            let ok = current.is_some_and(NodeInstall::usable);
            let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 16.0, with_alpha(if ok { theme.action } else { theme.warning }, 40));
            if ok {
                check_mark(ui, r.center(), 14.0, theme.action);
            } else {
                ui.painter().text(r.center(), Align2::CENTER_CENTER, "!", FontId::proportional(17.0), theme.warning);
            }
            ui.add_space(6.0);
            ui.vertical(|ui| {
                match (current, &setup.installs) {
                    (_, None) => {
                        ui.label(RichText::new("Looking for Node.js …").size(15.0).color(theme.text));
                    }
                    (Some(n), _) if n.usable() => {
                        ui.label(RichText::new(format!("Ready: Node.js {}", n.version.clone().unwrap_or_default())).size(15.0).strong().color(theme.text));
                        ui.label(RichText::new(n.path.display().to_string()).size(12.0).monospace().color(theme.text_muted));
                    }
                    (Some(n), _) => {
                        ui.label(RichText::new(format!("Node.js {} is too old", n.version.clone().unwrap_or_else(|| "?".into()))).size(15.0).strong().color(theme.text));
                        ui.label(RichText::new(format!("Choose a newer one below, or install Node.js {MIN_NODE_MAJOR}+.")).size(12.5).color(theme.text_muted));
                    }
                    (None, _) => {
                        ui.label(RichText::new("No Node.js found yet").size(15.0).strong().color(theme.text));
                        ui.label(RichText::new("Install it below, or browse to a node executable.").size(12.5).color(theme.text_muted));
                    }
                }
            });
        });
    });

    // Found on this computer.
    ui.horizontal(|ui| {
        ui.label(RichText::new("ON THIS COMPUTER").size(11.0).strong().color(theme.text_muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add_enabled(setup.job.is_none(), egui::Button::new("Browse…")).clicked()
                && let Some(path) = {
                    let dialog = rfd::FileDialog::new().set_title("Choose the node executable");
                    if cfg!(windows) { dialog.add_filter("Node.js", &["exe"]) } else { dialog }
                }
                .pick_file()
            {
                match circuit_runner::node::version_of(&path) {
                    Ok(_) => {
                        ed.prefs.node_path = Some(path.display().to_string());
                        ed.prefs.apply_node();
                        setup.rescan();
                    }
                    Err(e) => setup.outcome = Some(Err(format!("{e:#}"))),
                }
            }
            if ui.add_enabled(setup.scan.is_none() && setup.job.is_none(), egui::Button::new("Search again")).clicked() {
                setup.rescan();
            }
        });
    });
    ui.add_space(6.0);
    match &setup.installs {
        None => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("Searching the usual places …").color(theme.text_muted));
            });
        }
        Some(list) if list.is_empty() => {
            ui.label(RichText::new("None found.").color(theme.text_muted));
        }
        Some(list) => {
            let list = list.clone();
            for n in &list {
                let selected = match &chosen {
                    Some(p) => n.path == std::path::Path::new(p),
                    None => n.source == "PATH",
                };
                if install_row(ui, theme, n, selected) && n.usable() {
                    ed.prefs.node_path = Some(n.path.display().to_string());
                    ed.prefs.apply_node();
                }
            }
        }
    }
    // Something chosen by hand that the search didn't find.
    if let Some(p) = &chosen
        && setup.installs.as_ref().is_some_and(|l| !l.iter().any(|n| n.path == std::path::Path::new(p)))
    {
        ui.label(RichText::new(format!("Chosen: {p}")).size(12.0).monospace().color(theme.text_muted));
    }

    // Installing.
    ui.add_space(14.0);
    ui.label(RichText::new("INSTALL NODE.JS").size(11.0).strong().color(theme.text_muted));
    ui.add_space(6.0);
    let busy = setup.job.is_some();
    let mut start: Option<Installer> = None;
    if cfg!(windows) {
        if setup.winget == Some(true) {
            card(ui, theme, |ui| {
                if install_option(ui, theme, "Install with winget", "Recommended. Windows' own package manager installs the current LTS version. Windows asks for permission.", busy) {
                    start = Some(|log| node_setup::install_with_winget(log));
                }
            });
        }
        card(ui, theme, |ui| {
            if install_option(ui, theme, "Download the official installer", "From nodejs.org, checked against its published checksum, then installed without questions. Windows asks for permission.", busy) {
                start = Some(|log| node_setup::install_official_windows(&crate::bot::app_data_dir().join("downloads"), log));
            }
        });
    } else {
        card(ui, theme, |ui| {
            if install_option(ui, theme, "Download Node.js for XC Circuit", "Into XC Circuit's own folder, no password needed. From nodejs.org, checked against its published checksum.", busy) {
                start = Some(|log| node_setup::install_private(&private_root(), log));
            }
        });
        ui.label(RichText::new("Or install it with your package manager. Distribution packages are often older than Node.js 22, so check the version.").size(12.0).color(theme.text_muted));
    }
    if let Some(run) = start {
        let (tx, rx) = channel();
        let ctx = ui.ctx().clone();
        thread::spawn(move || {
            let tx2 = tx.clone();
            let ctx2 = ctx.clone();
            let result = run(&mut |line| {
                let _ = tx2.send(JobMsg::Log(line));
                ctx2.request_repaint();
            });
            let _ = tx.send(JobMsg::Done(result.map_err(|e| format!("{e:#}"))));
            ctx.request_repaint();
        });
        setup.job = Some(Job { rx, log: Vec::new() });
        setup.outcome = None;
    }

    // Progress and outcome.
    if let Some(job) = &setup.job {
        egui::Frame::new().fill(theme.field).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("Installing Node.js …").color(theme.text));
            });
            for line in job.log.iter().rev().take(6).rev() {
                ui.label(RichText::new(line).monospace().size(11.5).color(theme.text_muted));
            }
        });
    }
    match &setup.outcome {
        Some(Ok(path)) => {
            ui.label(RichText::new(format!("Node.js is installed and chosen: {}", path.display())).color(theme.action));
        }
        Some(Err(e)) => {
            ui.label(RichText::new(e).color(theme.error));
        }
        None => {}
    }
}

/// One found Node.js. Returns true when clicked.
fn install_row(ui: &mut Ui, theme: &Theme, n: &NodeInstall, selected: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
    let usable = n.usable();
    let fill = if selected { with_alpha(theme.accent, 30) } else if resp.hovered() && usable { theme.panel_header } else { theme.panel };
    let stroke = if selected { Stroke::new(1.5, theme.accent) } else { Stroke::new(1.0, theme.line) };
    let p = ui.painter();
    p.rect(rect.shrink2(vec2(0.0, 3.0)), 8.0, fill, stroke, egui::StrokeKind::Inside);
    let c = pos2(rect.left() + 20.0, rect.center().y);
    p.circle_stroke(c, 7.0, Stroke::new(1.5, if selected { theme.accent } else { theme.text_muted }));
    if selected {
        p.circle_filled(c, 3.5, theme.accent);
    }
    let text = if usable { theme.text } else { mix(theme.text_muted, theme.panel, 0.3) };
    let version = n.version.clone().unwrap_or_else(|| "won't start".into());
    p.text(pos2(rect.left() + 38.0, rect.center().y - 8.0), Align2::LEFT_CENTER, &version, FontId::proportional(14.0), text);
    p.text(pos2(rect.left() + 38.0, rect.center().y + 9.0), Align2::LEFT_CENTER, n.path.display().to_string(), FontId::monospace(11.0), theme.text_muted);
    let note = if usable { n.source.clone() } else if n.version.is_some() { format!("{} · too old", n.source) } else { format!("{} · doesn't start", n.source) };
    p.text(pos2(rect.right() - 14.0, rect.center().y), Align2::RIGHT_CENTER, note, FontId::proportional(12.0), if usable { theme.text_muted } else { theme.warning });
    let resp = if usable { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp.on_hover_text(format!("Bots need Node.js {MIN_NODE_MAJOR} or newer")) };
    resp.clicked()
}

fn install_option(ui: &mut Ui, theme: &Theme, head: &str, text: &str, busy: bool) -> bool {
    let mut go = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() - 120.0).max(200.0));
            ui.label(RichText::new(head).size(14.5).strong().color(theme.text));
            ui.label(RichText::new(text).size(12.5).color(theme.text_muted));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            go = ui.add_enabled(!busy, egui::Button::new(RichText::new("Install").strong()).min_size(vec2(90.0, 30.0))).clicked();
        });
    });
    go
}

fn look(ui: &mut Ui, ed: &mut Editor, theme: &Theme) {
    title(ui, theme, "Make it yours", "Pick a theme and an accent color. You can change both any time in Settings.");
    crate::settings::theme_cards(ui, ed, theme);
    ui.add_space(18.0);
    ui.label(RichText::new("ACCENT COLOR").size(11.0).strong().color(theme.text_muted));
    ui.add_space(8.0);
    crate::settings::accent_swatches(ui, ed, theme, 0.0);
    ui.add_space(22.0);
    ui.horizontal(|ui| {
        let mut touch = ed.prefs.touch;
        if crate::widgets::toggle(ui, &mut touch, theme).changed() {
            ed.prefs.touch = touch;
            ed.restyle();
            ed.theme.apply(ui.ctx());
        }
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new("Touch mode").size(13.5).color(theme.text));
            ui.label(RichText::new("Bigger buttons and more room between them, for touch screens. Also in Settings.").size(12.0).color(theme.text_muted));
        });
    });
}

/// Returns `Some(true)` to start the tour, `Some(false)` to skip it.
fn tour_choice(ui: &mut Ui, ed: &Editor, theme: &Theme) -> Option<bool> {
    title(ui, theme, "You're all set", "Want a quick tour? It takes about three minutes and uses a small example bot, so nothing of yours is changed.");
    let mut choice = None;
    ui.columns(2, |cols| {
        for (i, (ic, head, text, tour)) in [("bolt", "Take the tour", "Add a block, wire it up and run a bot, with each part of the app pointed out.", true), ("workspace", "Start building", "Jump straight in. The tour is in the Help menu whenever you want it.", false)].into_iter().enumerate() {
            let ui = &mut cols[i];
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 170.0), Sense::click());
            let primary = tour;
            let fill = if resp.hovered() { theme.panel_header } else { theme.panel };
            ui.painter().rect(rect, 12.0, fill, Stroke::new(if primary { 1.5 } else { 1.0 }, if primary { theme.accent } else { theme.line }), egui::StrokeKind::Inside);
            let ic_rect = Rect::from_min_size(rect.min + vec2(20.0, 20.0), vec2(44.0, 44.0));
            ui.painter().rect_filled(ic_rect, 12.0, with_alpha(theme.accent, 36));
            icon(ui, ed, ic, ic_rect.center(), 24.0, theme.accent);
            ui.painter().text(rect.min + vec2(20.0, 90.0), Align2::LEFT_CENTER, head, FontId::proportional(17.0), theme.text);
            let galley = ui.painter().layout(text.to_string(), FontId::proportional(12.5), theme.text_muted, rect.width() - 40.0);
            ui.painter().galley(rect.min + vec2(20.0, 106.0), galley, theme.text_muted);
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                choice = Some(tour);
            }
        }
    });
    choice
}
