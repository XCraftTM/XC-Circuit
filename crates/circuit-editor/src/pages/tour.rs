//! The guided tour: a practice project, a reduced window and step-by-step
//! cards. Whatever the next click is on glows: the Add block button, the
//! picker's search box and its Math entry, the two pins to wire (with a hand
//! that shows the drag), the Inspector's fields, Run. The card says what to
//! do right now, notices mistakes (a wire to the wrong pin, a wrong number)
//! and offers to do each step for you. It can be skipped at any time.

use std::path::{Path, PathBuf};

use circuit_core::{Link, PinRef, Project, Workspace};
use eframe::egui::{self, Align, Color32, Layout, Order, Pos2, Rect, RichText, Stroke, StrokeKind, pos2, vec2};
use serde_json::{Value, json};

use crate::ui::kit;
use crate::editor::{Editor, NO_MODE};
use crate::i18n::n;

/// The practice project's workspace.
pub const WORKSPACE: &str = "hello";
const START_NODE: &str = "1";
const LOG_NODE: &str = "2";

/// What a step is about, kept in view (the rest is dimmed).
#[derive(Clone, Copy)]
enum Area {
    /// Nothing: the card sits in the middle.
    Center,
    /// A part of the window, by its name in `Editor::ui_rects`.
    Ui(&'static str),
    /// The blocks the tour started with.
    StartBlocks,
}

/// Something the user does to finish a step.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Wait {
    AddMath,
    Wire,
    Values,
    Run,
}

struct StepDef {
    title: &'static str,
    body: &'static str,
    area: Area,
    wait: Option<Wait>,
}

const STEPS: [StepDef; 9] = [
    StepDef {
        title: n("Let's build a tiny bot"),
        body: n("It takes about two minutes. Whatever you need to click next glows, and if you get stuck, Do it for me does the step. This is a practice project: nothing of yours changes."),
        area: Area::Center,
        wait: None,
    },
    StepDef {
        title: n("Bots are made of blocks"),
        body: n("Bot Started runs once when the bot starts. The green wire then runs Log, which writes a line in the Console. Drag empty space to look around; the mouse wheel zooms."),
        area: Area::StartBlocks,
        wait: None,
    },
    StepDef {
        title: n("Add a Math block"),
        body: n("Math adds two numbers. Your bot will say the answer."),
        area: Area::Ui("canvas"),
        wait: Some(Wait::AddMath),
    },
    StepDef {
        title: n("Wire Math to Log"),
        body: n("Coloured wires carry values from one block to another."),
        area: Area::Ui("canvas"),
        wait: Some(Wait::Wire),
    },
    StepDef {
        title: n("Fill in the numbers"),
        body: n("A block's settings are in the Inspector on the right, and on the block itself."),
        area: Area::Ui("canvas"),
        wait: Some(Wait::Values),
    },
    StepDef {
        title: n("Run your bot"),
        body: n("XC Circuit starts the bot. The blocks light up as they run."),
        area: Area::Ui("run"),
        wait: Some(Wait::Run),
    },
    StepDef {
        title: n("It worked!"),
        body: n("Your bot wrote 5, the answer from Math, in the Console. Everything a bot says shows up here, errors too, with a link to the block that said it."),
        area: Area::Ui("console"),
        wait: None,
    },
    StepDef {
        title: n("Find anything"),
        body: n("Search here (or press Ctrl+P) for any command, workspace or block. Settings are behind the gear, or Ctrl+,."),
        area: Area::Ui("search"),
        wait: None,
    },
    StepDef {
        title: n("That's the basics!"),
        body: n("Discord and Fluxer bots work the same way, with events like Slash Command and actions like Send Message. Where to next?"),
        area: Area::Center,
        wait: None,
    },
];

/// Where to go when the tour ends.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Exit {
    /// Back to what was open before.
    Return,
    DemoBot,
    NewProject,
}

pub enum Ask {
    Run,
    SetUpNode,
    End(Exit),
}

pub struct Tour {
    pub step: usize,
    /// The project that was open before, opened again afterwards.
    pub before: Option<PathBuf>,
}

/// Writes a fresh practice project into `dir`.
pub fn create_project(dir: &Path) -> anyhow::Result<()> {
    let _ = std::fs::remove_dir_all(dir);
    let mut project = Project::create(dir, "Tour Bot", NO_MODE)?;
    project.remove_workspace("main");
    let ws: Workspace = serde_json::from_value(json!({
        "name": "Hello",
        "nodes": {
            "1": { "block": "core.on_start@1", "at": [60, 120] },
            "2": { "block": "core.log@1", "at": [440, 120], "values": { "message": "Hello!", "level": "success" }, "pins": ["message"] }
        },
        "links": ["1.next > 2.in"]
    }))?;
    project.workspaces.insert(WORKSPACE.into(), ws);
    project.save()?;
    Ok(())
}

/// The Math block the user added, if any.
fn math_node(ed: &Editor) -> Option<String> {
    let ws = ed.workspace(WORKSPACE)?;
    ws.nodes.iter().find(|(_, n)| n.block.starts_with("core.math@")).map(|(id, _)| id.clone())
}

fn is_done(ed: &Editor, wait: Wait) -> bool {
    let Some(ws) = ed.workspace(WORKSPACE) else { return false };
    let math = math_node(ed);
    match wait {
        Wait::AddMath => math.is_some(),
        Wait::Wire => math.is_some_and(|m| ws.links.iter().any(|l| l.from == PinRef::new(&m, "result") && l.to == PinRef::new(LOG_NODE, "message"))),
        Wait::Values => math.is_some_and(|m| {
            let v = |k: &str| ws.nodes[&m].values.get(k).and_then(Value::as_f64);
            v("a") == Some(2.0) && v("b") == Some(3.0) && ws.nodes[&m].values.get("op").and_then(Value::as_str).is_none_or(|op| op == "add")
        }),
        // The bot writes "5": the Log line arrives in the Console.
        Wait::Run => ed.log.iter().any(|l| l.text.trim() == "5"),
    }
}

/// Does the step's task for the user. Returns an ask when the app must help.
fn do_it(ed: &mut Editor, wait: Wait) -> Option<Ask> {
    match wait {
        Wait::AddMath => {
            ed.add_node(WORKSPACE, "core.math", pos2(300.0, 270.0));
        }
        Wait::Wire => {
            if math_node(ed).is_none() {
                ed.add_node(WORKSPACE, "core.math", pos2(300.0, 270.0));
            }
            let m = math_node(ed)?;
            let ws = ed.project.as_mut()?.workspaces.get_mut(WORKSPACE)?;
            ws.connect(Link { from: PinRef::new(&m, "result"), to: PinRef::new(LOG_NODE, "message") }, true);
            ed.edited = true;
        }
        Wait::Values => {
            let m = math_node(ed)?;
            let node = ed.project.as_mut()?.workspaces.get_mut(WORKSPACE)?.nodes.get_mut(&m)?;
            node.values.insert("a".into(), json!(2));
            node.values.insert("b".into(), json!(3));
            node.values.insert("op".into(), json!("add"));
            ed.select_only(WORKSPACE, &m);
            ed.edited = true;
        }
        Wait::Run => return Some(Ask::Run),
    }
    None
}

/// Something to make glow.
#[derive(Clone, Copy, Debug)]
enum Glow {
    Rect(Rect),
    Pin(Pos2),
}

/// What the card says and what glows, for a step as things are right now.
#[derive(Default)]
struct Guide {
    /// What to do now.
    todo: String,
    /// A mistake to point out.
    oops: Option<String>,
    /// How far along it is: each part, and whether it's right ("A = 2", "B = 0, should be 3").
    check: Vec<(bool, String)>,
    glow: Vec<Glow>,
    /// A hand showing a drag, from and to.
    drag: Option<(Pos2, Pos2)>,
    /// What the card mustn't cover (the blocks being wired).
    keep_clear: Vec<Rect>,
}

/// Where `key` was drawn: this frame, or the one before for what's drawn
/// after the tour (the picker).
fn rect(ed: &Editor, key: &str) -> Option<Rect> {
    ed.ui_rects.get(key).or_else(|| ed.ui_rects_before.get(key)).copied()
}

/// A block's name in the practice workspace.
fn block_name(ed: &Editor, node: &str) -> String {
    ed.workspace(WORKSPACE).and_then(|ws| ws.nodes.get(node)).and_then(|n| ed.reg.block(&n.block)).map(|d| d.name.clone()).unwrap_or_else(|| node.to_string())
}

/// A number for the card: 2, not 2.0.
fn number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{v}") }
}

/// What to do for `wait` right now, what glows, and what went wrong.
fn guide(ed: &Editor, wait: Wait) -> Guide {
    let mut g = Guide::default();
    let Some(ws) = ed.workspace(WORKSPACE) else { return g };
    let math = math_node(ed);
    match wait {
        Wait::AddMath => {
            if ed.picker.is_none() {
                g.todo = t!("Click the glowing Add block button at the bottom of the canvas.").into();
                g.glow.extend(rect(ed, "add-block").map(Glow::Rect));
            } else if let Some(r) = rect(ed, "picker:core.math") {
                g.todo = t!("Click Math in the list.").into();
                g.glow.push(Glow::Rect(r));
            } else {
                g.todo = t!("Type Math into the glowing search box.").into();
                g.glow.extend(rect(ed, "picker-search").map(Glow::Rect));
            }
        }
        Wait::Wire => {
            let Some(m) = math else {
                g.todo = t!("The Math block is gone: go Back a step and add it again.").into();
                return g;
            };
            let from = rect(ed, &format!("pin:{m}:out:result"));
            let to = rect(ed, &format!("pin:{LOG_NODE}:in:message"));
            match (from, to) {
                (Some(a), Some(b)) => {
                    g.todo = t!("Press on the glowing Result pin of Math, keep the mouse button down, and let go on the glowing Message pin of Log.").into();
                    g.glow.extend([Glow::Pin(a.center()), Glow::Pin(b.center())]);
                    g.drag = Some((a.center(), b.center()));
                    g.keep_clear.extend([format!("node:{m}"), format!("node:{LOG_NODE}")].iter().filter_map(|k| rect(ed, k)));
                }
                _ => g.todo = t!("Math or Log is out of view: drag empty canvas until you see both.").into(),
            }
            let result = PinRef::new(&m, "result");
            let message = PinRef::new(LOG_NODE, "message");
            if let Some(l) = ws.links.iter().find(|l| l.from == result && l.to != message) {
                g.oops = Some(tf!("That wire ends at {pin} on {block}. Drag from Result again, and let go on the glowing Message pin of Log.", pin = l.to.pin, block = block_name(ed, &l.to.node)));
            } else if let Some(l) = ws.links.iter().find(|l| l.to == message && l.from != result) {
                g.oops = Some(tf!("Message gets {pin} of {block}. It should get Result of Math: drag from Math's Result pin.", pin = l.from.pin, block = block_name(ed, &l.from.node)));
            }
        }
        Wait::Values => {
            let Some(m) = math else {
                g.todo = t!("The Math block is gone: go Back two steps and add it again.").into();
                return g;
            };
            let node = &ws.nodes[&m];
            let chosen = ed.selected.as_ref().is_some_and(|(w, id)| w == WORKSPACE && *id == m);
            if !chosen {
                g.todo = t!("Click the glowing Math block to choose it.").into();
                g.glow.extend(rect(ed, &format!("node:{m}")).map(Glow::Rect));
                return g;
            }
            g.todo = t!("Set A to 2 and B to 3 in the glowing boxes: click a number and type it (or drag it left and right).").into();
            for (key, label, want) in [("a", "A", 2.0), ("b", "B", 3.0)] {
                let got = node.values.get(key).and_then(Value::as_f64).unwrap_or(0.0);
                if got == want {
                    g.check.push((true, format!("{label} = {}", number(got))));
                } else {
                    g.check.push((false, tf!("{label} = {got}, should be {want}", label = label, got = number(got), want = number(want))));
                    g.glow.extend(rect(ed, &format!("field:{key}")).map(Glow::Rect));
                    g.glow.extend(rect(ed, &format!("row:{m}:{key}")).map(Glow::Rect));
                }
            }
            if node.values.get("op").and_then(Value::as_str).is_some_and(|op| op != "add") {
                g.oops = Some(t!("Operation isn't Add any more: set it back to Add.").into());
                g.glow.extend(rect(ed, "field:op").map(Glow::Rect));
            }
        }
        Wait::Run => {
            g.todo = t!("Click the glowing Run button at the top (or press F5).").into();
            g.glow.extend(rect(ed, "run").map(Glow::Rect));
            let said = ed.log.iter().rev().find(|l| l.target.as_ref().is_some_and(|(w, id)| w == WORKSPACE && id == LOG_NODE));
            if let Some(l) = said.filter(|l| l.text.trim() != "5") {
                g.oops = Some(tf!("Your bot wrote {text}, not 5. Go Back to check Math's numbers and the wire, then run it again.", text = l.text.trim()));
            } else if ed.log.last().is_some_and(|l| l.text.starts_with("Bot process ended") && !l.text.contains("code 0")) {
                g.oops = Some(t!("The bot stopped with an error. The Console below says why; Run tries again.").into());
            }
        }
    }
    g
}

/// Draws a soft, pulsing glow around `glow`.
fn paint_glow(painter: &egui::Painter, glow: Glow, color: Color32, pulse: f32) {
    use crate::ui::theme::with_alpha;
    let strength = 0.55 + 0.45 * pulse;
    for i in 1..=5 {
        let spread = i as f32 * 2.6;
        let alpha = ((80 - i * 14) as f32 * strength) as u8;
        let stroke = Stroke::new(2.6, with_alpha(color, alpha));
        match glow {
            Glow::Rect(r) => painter.rect_stroke(r.expand(spread), 6.0 + spread, stroke, StrokeKind::Outside),
            Glow::Pin(c) => painter.circle_stroke(c, 7.0 + spread, stroke),
        };
    }
    let solid = Stroke::new(2.0, with_alpha(color, (170.0 + 85.0 * pulse) as u8));
    match glow {
        Glow::Rect(r) => painter.rect_stroke(r.expand(1.0), 6.0, solid, StrokeKind::Outside),
        Glow::Pin(c) => painter.circle_stroke(c, 7.5, solid),
    };
}

/// A hand dragging from `a` to `b`, along the curve a wire takes, again and again.
fn paint_drag(painter: &egui::Painter, a: Pos2, b: Pos2, color: Color32, time: f64) {
    use crate::ui::theme::with_alpha;
    let cycle = 2.2;
    let t = ((time % cycle) / 1.6).min(1.0) as f32;
    let ease = t * t * (3.0 - 2.0 * t);
    let dx = ((b.x - a.x).abs() * 0.5).max(40.0);
    let (c1, c2) = (a + vec2(dx, 0.0), b - vec2(dx, 0.0));
    let at = |t: f32| {
        let u = 1.0 - t;
        (a.to_vec2() * u * u * u + c1.to_vec2() * 3.0 * u * u * t + c2.to_vec2() * 3.0 * u * t * t + b.to_vec2() * t * t * t).to_pos2()
    };
    let points: Vec<Pos2> = (0..=24).map(|i| at(ease * i as f32 / 24.0)).collect();
    painter.add(egui::Shape::line(points, Stroke::new(3.0, with_alpha(color, 150))));
    let hand = at(ease);
    painter.circle_filled(hand, 9.0, with_alpha(color, 210));
    painter.circle_stroke(hand, 9.0, Stroke::new(2.0, Color32::WHITE));
    painter.circle_filled(hand, 3.0, Color32::WHITE);
}

/// The step's place on screen: what stays undimmed.
fn area_rect(ed: &Editor, area: Area) -> Option<Rect> {
    match area {
        Area::Center => None,
        Area::Ui(name) => rect(ed, name),
        Area::StartBlocks => Some(rect(ed, &format!("node:{START_NODE}"))?.union(rect(ed, &format!("node:{LOG_NODE}"))?)),
    }
}

/// Draws the dimmed backdrop, the glows and the step's card.
pub fn show(ctx: &egui::Context, ed: &mut Editor, tour: &mut Tour, bot_running: bool) -> Option<Ask> {
    let theme = ed.theme.clone();
    let step = &STEPS[tour.step.min(STEPS.len() - 1)];
    let screen = ctx.content_rect();
    // A new block lands in the middle of the view, which is where Log is: Math goes below, so both pins show.
    if ed.picker.is_none()
        && let Some(m) = math_node(ed)
        && let Some(math) = rect(ed, &format!("node:{m}"))
        && [START_NODE, LOG_NODE].iter().filter_map(|id| rect(ed, &format!("node:{id}"))).any(|r| r.intersects(math))
        && let Some(node) = ed.project.as_mut().and_then(|p| p.workspaces.get_mut(WORKSPACE)).and_then(|ws| ws.nodes.get_mut(&m))
    {
        node.at = [300.0, 270.0];
        ed.edited = true;
        ctx.request_repaint();
    }
    let waiting = step.wait.is_some_and(|w| !is_done(ed, w));
    let guide = step.wait.filter(|_| waiting).map(|w| guide(ed, w)).unwrap_or_default();
    let glow_rect = |g: &Glow| match *g {
        Glow::Rect(r) => r,
        Glow::Pin(c) => Rect::from_center_size(c, vec2(16.0, 16.0)),
    };
    // The blocks the tour started with glow on their own step.
    let mut glows = guide.glow.clone();
    if matches!(step.area, Area::StartBlocks) {
        glows.extend([START_NODE, LOG_NODE].iter().filter_map(|id| rect(ed, &format!("node:{id}"))).map(Glow::Rect));
    }
    let picker = rect(ed, "picker").filter(|_| ed.picker.is_some());
    let focus = area_rect(ed, step.area).map(|r| r.expand(6.0).intersect(screen));
    let focus = glows.iter().map(glow_rect).chain(picker).fold(focus, |f, r| Some(f.map_or(r, |f| f.union(r))));

    // Dim everything else. Paint only: clicks still reach the app.
    let dim = Color32::from_black_alpha(120);
    let under = ctx.layer_painter(egui::LayerId::new(Order::Middle, egui::Id::new("tour-dim")));
    match focus {
        Some(t) => {
            let t = t.expand(4.0);
            for r in [
                Rect::from_min_max(screen.min, pos2(screen.right(), t.top())),
                Rect::from_min_max(pos2(screen.left(), t.bottom()), screen.max),
                Rect::from_min_max(pos2(screen.left(), t.top()), pos2(t.left(), t.bottom())),
                Rect::from_min_max(pos2(t.right(), t.top()), pos2(screen.right(), t.bottom())),
            ] {
                under.rect_filled(r, 0.0, dim);
            }
        }
        None => {
            under.rect_filled(screen, 0.0, dim);
        }
    }
    // Glows and the hand go over everything, the picker too.
    let time = ctx.input(|i| i.time);
    let pulse = (time * 3.0).sin() as f32 * 0.5 + 0.5;
    let over = ctx.layer_painter(egui::LayerId::new(Order::Tooltip, egui::Id::new("tour-glow")));
    for g in &glows {
        paint_glow(&over, *g, theme.accent, pulse);
    }
    if let Some((a, b)) = guide.drag {
        paint_drag(&over, a, b, theme.accent, time);
    }
    if !glows.is_empty() || guide.drag.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }

    // The card: next to the picker while it's open, else next to what glows, else the step's area.
    let card_id = egui::Id::new("tour-card");
    let card_w = 340.0;
    let size = ctx.memory(|m| m.area_rect(card_id)).map(|r| r.size()).unwrap_or(vec2(card_w, 240.0));
    let beside = picker.or_else(|| glows.iter().map(glow_rect).chain(guide.keep_clear.iter().copied()).reduce(|a, b| a.union(b))).or(focus);
    let pos = match beside {
        Some(t) if step.wait.is_some() || !matches!(step.area, Area::Center) => place_card(screen, t.expand(10.0), size),
        _ => screen.center() - size / 2.0,
    };
    let mut ask = None;
    // Arrow keys move through the steps (not while typing).
    if !ctx.text_edit_focused() {
        if !waiting && tour.step + 1 < STEPS.len() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)) {
            tour.step += 1;
        } else if tour.step > 0 && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)) {
            tour.step -= 1;
        }
    }
    egui::Area::new(card_id).order(Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::new()
            .fill(theme.panel)
            .stroke(Stroke::new(1.0, theme.accent))
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(18, 16))
            .shadow(ctx.global_style().visuals.popup_shadow)
            .show(ui, |ui| {
                ui.set_width(card_w - 36.0);
                // Progress: a dot for each step.
                crate::ui::kit::line(ui, |ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(STEPS.len() as f32 * 15.0, 14.0), egui::Sense::hover());
                    for i in 0..STEPS.len() {
                        let c = pos2(r.left() + 7.0 + i as f32 * 15.0, r.center().y);
                        if i < tour.step {
                            ui.painter().circle_filled(c, 3.5, theme.accent);
                        } else if i == tour.step {
                            ui.painter().circle_filled(c, 4.5, theme.accent);
                            ui.painter().circle_stroke(c, 6.0, Stroke::new(1.0, crate::ui::theme::with_alpha(theme.accent, 120)));
                        } else {
                            ui.painter().circle_stroke(c, 3.5, Stroke::new(1.0, theme.text_muted));
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if crate::ui::kit::button(ui, &theme, t!("Skip tour")).clicked() {
                            ask = Some(Ask::End(Exit::Return));
                        }
                    });
                });
                ui.add_space(4.0);
                ui.label(RichText::new(crate::i18n::t(step.title)).size(18.0).strong().color(theme.text));
                ui.add_space(4.0);
                ui.label(RichText::new(crate::i18n::t(step.body)).size(13.0).color(theme.text_muted));
                if waiting {
                    ui.add_space(10.0);
                    // What to do right now, in a box of its own.
                    egui::Frame::new()
                        .fill(crate::ui::theme::with_alpha(theme.accent, 28))
                        .stroke(Stroke::new(1.0, crate::ui::theme::with_alpha(theme.accent, 90)))
                        .corner_radius(8)
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(&guide.todo).size(14.0).strong().color(theme.text));
                            if !guide.check.is_empty() {
                                ui.add_space(4.0);
                                for (right, text) in &guide.check {
                                    if *right {
                                        crate::ui::kit::done_label(ui, RichText::new(text).size(12.5), theme.action);
                                    } else {
                                        ui.label(RichText::new(text).size(12.5).color(theme.warning));
                                    }
                                }
                            }
                            if let Some(oops) = &guide.oops {
                                ui.add_space(4.0);
                                ui.label(RichText::new(oops).size(12.5).color(theme.warning));
                            }
                            if step.wait == Some(Wait::Run) && circuit_runner::node::check_node_cached().is_err() {
                                ui.add_space(4.0);
                                ui.label(RichText::new(t!("Node.js isn't set up yet, so the bot can't start.")).size(12.5).color(theme.warning));
                                if kit::button(ui, &theme, t!("Set up Node.js")).clicked() {
                                    ask = Some(Ask::SetUpNode);
                                }
                            } else if step.wait == Some(Wait::Run) && bot_running {
                                ui.add_space(4.0);
                                crate::ui::kit::line(ui, |ui| {
                                    ui.spinner();
                                    ui.label(RichText::new(t!("The bot is starting …")).size(12.5).color(theme.text_muted));
                                });
                            }
                        });
                } else if step.wait.is_some() {
                    ui.add_space(8.0);
                    crate::ui::kit::done_label(ui, RichText::new(t!("Done! Click Next.")).size(14.0).strong(), theme.action);
                }
                ui.add_space(12.0);
                if tour.step + 1 == STEPS.len() {
                    for (label, exit) in [(t!("Open the DemoBot example"), Exit::DemoBot), (t!("Start a new project"), Exit::NewProject), (t!("Back to my project"), Exit::Return)] {
                        let primary = exit == Exit::DemoBot;
                        let b = if primary { crate::ui::kit::primary_button(ui, &theme, label) } else { crate::ui::kit::secondary_button(ui, &theme, label) };
                        if b.clicked() {
                            ask = Some(Ask::End(exit));
                        }
                        ui.add_space(2.0);
                    }
                    return;
                }
                crate::ui::kit::line(ui, |ui| {
                    if tour.step > 0 && crate::ui::kit::secondary_button(ui, &theme, t!("Back")).clicked() {
                        tour.step -= 1;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let label = if tour.step == 0 { t!("Let's go") } else { t!("Next") };
                        let next = ui.add_enabled_ui(!waiting, |ui| crate::ui::kit::primary_button(ui, &theme, label)).inner;
                        if next.clicked() {
                            tour.step += 1;
                        }
                        if waiting
                            && let Some(w) = step.wait
                            && crate::ui::kit::secondary_button(ui, &theme, t!("Do it for me")).clicked()
                        {
                            ask = do_it(ed, w);
                        }
                    });
                });
            });
    });
    ask
}

/// A spot for the card beside `target`: right, left, below, above, or inside.
fn place_card(screen: Rect, target: Rect, size: egui::Vec2) -> Pos2 {
    let gap = 16.0;
    let clamp_y = |y: f32| y.clamp(screen.top() + 8.0, screen.bottom() - size.y - 8.0);
    let clamp_x = |x: f32| x.clamp(screen.left() + 8.0, screen.right() - size.x - 8.0);
    if target.right() + gap + size.x < screen.right() {
        pos2(target.right() + gap, clamp_y(target.top()))
    } else if target.left() - gap - size.x > screen.left() {
        pos2(target.left() - gap - size.x, clamp_y(target.top()))
    } else if target.bottom() + gap + size.y < screen.bottom() {
        pos2(clamp_x(target.left()), target.bottom() + gap)
    } else if target.top() - gap - size.y > screen.top() {
        pos2(clamp_x(target.left()), target.top() - gap - size.y)
    } else {
        pos2(clamp_x(target.right() - size.x - 16.0), clamp_y(target.bottom() - size.y - 16.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_finish_when_done_for_the_user() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("xc-tour-{}", std::process::id()));
        create_project(&dir).unwrap();
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&dir);
        assert!(ed.mode().is_none(), "the practice project needs no Discord or npm packages");
        for wait in [Wait::AddMath, Wait::Wire, Wait::Values] {
            assert!(!is_done(&ed, wait));
            assert!(do_it(&mut ed, wait).is_none());
            assert!(is_done(&ed, wait), "Do it for me finishes the step");
        }
        assert!(matches!(do_it(&mut ed, Wait::Run), Some(Ask::Run)));
    }

    #[test]
    fn the_practice_bot_prints_five() {
        // The finished practice project, run for real.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("xc-tour-run-{}", std::process::id()));
        create_project(&dir).unwrap();
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&dir);
        for wait in [Wait::AddMath, Wait::Wire, Wait::Values] {
            do_it(&mut ed, wait);
        }
        let bundle = circuit_runner::build_bundle(ed.project.as_ref().unwrap(), &ed.reg, ed.mode()).unwrap();
        assert!(bundle.dependencies.is_empty());
        // The pin type conversions go along, for the runtime to apply on wires.
        assert_eq!(bundle.value["types"]["discord.member"]["converts"][0]["via"], "v => v.user");
        let session = circuit_runner::Session::start(
            circuit_runner::SessionConfig { bundle: bundle.value, dependencies: bundle.dependencies, packages_dir: dir.join("packages"), run_dir: dir.join(".circuit/run"), data_dir: dir.join(".circuit/data"), env: vec![] },
            || {},
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut lines = Vec::new();
        while std::time::Instant::now() < deadline {
            for m in session.poll() {
                if let circuit_runner::Msg::Bot(circuit_runner::BotEvent::Log { text, .. }) = m {
                    if text == "5" {
                        return;
                    }
                    lines.push(text);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        panic!("the bot never said 5: {lines:?}");
    }

    #[test]
    fn cards_stay_on_screen() {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0));
        for target in [Rect::from_min_size(pos2(1200.0, 20.0), vec2(80.0, 30.0)), Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 900.0)), Rect::from_min_size(pos2(300.0, 600.0), vec2(900.0, 280.0))] {
            let p = place_card(screen, target, vec2(360.0, 220.0));
            assert!(screen.contains_rect(Rect::from_min_size(p, vec2(360.0, 220.0))), "{target:?} -> {p:?}");
        }
    }
}
