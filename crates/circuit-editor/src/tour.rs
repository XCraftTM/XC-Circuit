//! The guided tour: a practice project, a reduced window and step-by-step
//! cards that point at each part of the app. Some steps wait for you to do
//! something (add a block, wire it, run the bot) and offer to do it for you.
//! It can be skipped at any time.

use std::path::{Path, PathBuf};

use circuit_core::{Link, PinRef, Project, Workspace};
use eframe::egui::{self, Align, Color32, Layout, Order, Pos2, Rect, RichText, Stroke, StrokeKind, pos2, vec2};
use serde_json::{Value, json};

use crate::editor::{Editor, NO_MODE};

/// The practice project's workspace.
pub const WORKSPACE: &str = "hello";
const LOG_NODE: &str = "2";

/// What a step points at.
#[derive(Clone, Copy)]
enum Target {
    /// Nothing: the card sits in the middle.
    Center,
    /// A part of the window, by its name in `Editor::ui_rects`.
    Ui(&'static str),
    /// The blocks the tour started with.
    StartBlocks,
    /// The Math block (once added) and the Log block.
    MathAndLog,
}

/// Something the user does to finish a step.
#[derive(Clone, Copy, PartialEq)]
enum Wait {
    AddMath,
    Wire,
    Values,
    Run,
}

struct StepDef {
    title: &'static str,
    body: &'static str,
    target: Target,
    wait: Option<Wait>,
    /// Shown while waiting.
    hint: &'static str,
}

const STEPS: [StepDef; 10] = [
    StepDef {
        title: "Welcome to the tour",
        body: "In a few steps you'll add a block, connect it and run a small bot. This is a practice project, so nothing of yours changes. You can skip the tour any time.",
        target: Target::Center,
        wait: None,
        hint: "",
    },
    StepDef {
        title: "The canvas",
        body: "Bots are built here from blocks. Drag empty space to move around and use the mouse wheel to zoom.",
        target: Target::Ui("canvas"),
        wait: None,
        hint: "",
    },
    StepDef {
        title: "Blocks and wires",
        body: "Bot Started runs once when the bot starts. The green wire hands over to Log, which writes a line to the Console. Green wires decide the order things happen in.",
        target: Target::StartBlocks,
        wait: None,
        hint: "",
    },
    StepDef {
        title: "Add a block",
        body: "Click Add block in the canvas toolbar (or right-click empty canvas, or press Space), type Math and click it.",
        target: Target::Ui("canvas"),
        wait: Some(Wait::AddMath),
        hint: "Waiting for a Math block …",
    },
    StepDef {
        title: "Connect them",
        body: "Drag from the Result pin on the right of Math to the Message pin on the left of Log. Colored wires carry values.",
        target: Target::MathAndLog,
        wait: Some(Wait::Wire),
        hint: "Waiting for the wire …",
    },
    StepDef {
        title: "Change its settings",
        body: "Click the Math block: its settings show in the Inspector. Set A to 2 and B to 3. You can also type right on the block.",
        target: Target::Ui("inspector"),
        wait: Some(Wait::Values),
        hint: "Waiting for A = 2 and B = 3 …",
    },
    StepDef {
        title: "Run it",
        body: "Press Run (or F5). XC Circuit starts the bot, Bot Started fires and the blocks light up as they run.",
        target: Target::Ui("run"),
        wait: Some(Wait::Run),
        hint: "Waiting for the bot to say something …",
    },
    StepDef {
        title: "The Console",
        body: "Your bot wrote 5, the answer from Math. Everything a bot says shows up here, errors too, with a link to the block that said it.",
        target: Target::Ui("console"),
        wait: None,
        hint: "",
    },
    StepDef {
        title: "Find anything",
        body: "Search here (or press Ctrl+P) for any command, workspace or block. Settings are behind the gear in the activity bar, or Ctrl+,.",
        target: Target::Ui("search"),
        wait: None,
        hint: "",
    },
    StepDef {
        title: "That's the basics!",
        body: "Discord bots work the same way, with events like Slash Command and actions like Send Message. Where to next?",
        target: Target::Center,
        wait: None,
        hint: "",
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
            v("a") == Some(2.0) && v("b") == Some(3.0)
        }),
        // The bot writes "5": the Log line arrives in the Console.
        Wait::Run => ed.log.iter().any(|l| l.text.trim() == "5"),
    }
}

/// Does the step's task for the user. Returns an ask when the app must help.
fn do_it(ed: &mut Editor, wait: Wait) -> Option<Ask> {
    match wait {
        Wait::AddMath => {
            ed.add_node(WORKSPACE, "core.math", pos2(240.0, 300.0));
        }
        Wait::Wire => {
            if math_node(ed).is_none() {
                ed.add_node(WORKSPACE, "core.math", pos2(240.0, 300.0));
            }
            let m = math_node(ed)?;
            let ws = ed.project.as_mut()?.workspaces.get_mut(WORKSPACE)?;
            ws.connect(Link { from: PinRef::new(&m, "result"), to: PinRef::new(LOG_NODE, "message") }, true);
            ed.dirty = true;
        }
        Wait::Values => {
            let m = math_node(ed)?;
            let node = ed.project.as_mut()?.workspaces.get_mut(WORKSPACE)?.nodes.get_mut(&m)?;
            node.values.insert("a".into(), json!(2));
            node.values.insert("b".into(), json!(3));
            node.values.insert("op".into(), json!("add"));
            ed.select_only(WORKSPACE, &m);
            ed.dirty = true;
        }
        Wait::Run => return Some(Ask::Run),
    }
    None
}

fn target_rect(ed: &Editor, target: Target) -> Option<Rect> {
    let r = |k: &str| ed.ui_rects.get(k).copied();
    match target {
        Target::Center => None,
        Target::Ui(name) => r(name),
        Target::StartBlocks => Some(r("node:1")?.union(r("node:2")?)),
        Target::MathAndLog => {
            let log = r(&format!("node:{LOG_NODE}"))?;
            Some(match math_node(ed).and_then(|m| r(&format!("node:{m}"))) {
                Some(m) => m.union(log),
                None => log,
            })
        }
    }
}

/// Draws the dimmed backdrop, the highlight and the step's card.
pub fn show(ctx: &egui::Context, ed: &mut Editor, tour: &mut Tour, bot_running: bool) -> Option<Ask> {
    let theme = ed.theme.clone();
    let step = &STEPS[tour.step.min(STEPS.len() - 1)];
    let screen = ctx.content_rect();
    let target = target_rect(ed, step.target).map(|t| t.expand(6.0).intersect(screen));
    // Steps that pick up the Math block: show it as well.
    let target = match (step.wait, target) {
        (Some(Wait::Values), Some(t)) => Some(math_node(ed).and_then(|m| ed.ui_rects.get(&format!("node:{m}")).copied()).map(|m| t.union(m.expand(6.0))).unwrap_or(t)),
        _ => target,
    };

    // Dim everything but the target. Paint only: clicks still reach the app.
    let painter = ctx.layer_painter(egui::LayerId::new(Order::Middle, egui::Id::new("tour-dim")));
    let dim = Color32::from_black_alpha(150);
    match target {
        Some(t) => {
            for r in [
                Rect::from_min_max(screen.min, pos2(screen.right(), t.top())),
                Rect::from_min_max(pos2(screen.left(), t.bottom()), screen.max),
                Rect::from_min_max(pos2(screen.left(), t.top()), pos2(t.left(), t.bottom())),
                Rect::from_min_max(pos2(t.right(), t.top()), pos2(screen.right(), t.bottom())),
            ] {
                painter.rect_filled(r, 0.0, dim);
            }
            let pulse = (ctx.input(|i| i.time) * 2.0).sin() as f32 * 0.5 + 0.5;
            painter.rect_stroke(t, 8.0, Stroke::new(2.0 + pulse, theme.accent), StrokeKind::Outside);
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        None => {
            painter.rect_filled(screen, 0.0, dim);
        }
    }

    // The card, next to the target where there is room.
    let card_w = 360.0;
    let card_h = 220.0;
    let pos = match target {
        None => screen.center() - vec2(card_w / 2.0, card_h / 2.0),
        Some(t) => place_card(screen, t, vec2(card_w, card_h)),
    };
    let waiting = step.wait.is_some_and(|w| !is_done(ed, w));
    let mut ask = None;
    // Arrow keys move through the steps (not while typing).
    if !ctx.text_edit_focused() {
        if !waiting && tour.step + 1 < STEPS.len() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)) {
            tour.step += 1;
        } else if tour.step > 0 && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)) {
            tour.step -= 1;
        }
    }
    egui::Area::new(egui::Id::new("tour-card")).order(Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::new()
            .fill(theme.panel)
            .stroke(Stroke::new(1.0, theme.accent))
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(18, 16))
            .shadow(ctx.global_style().visuals.popup_shadow)
            .show(ui, |ui| {
                ui.set_width(card_w - 36.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} of {}", tour.step + 1, STEPS.len())).size(11.5).color(theme.accent));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.link(RichText::new("Skip tour").size(12.0).color(theme.text_muted)).clicked() {
                            ask = Some(Ask::End(Exit::Return));
                        }
                    });
                });
                ui.add_space(2.0);
                ui.label(RichText::new(step.title).size(17.0).strong().color(theme.text));
                ui.add_space(4.0);
                ui.label(RichText::new(step.body).size(13.0).color(theme.text));
                if waiting {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new(step.hint).size(12.0).color(theme.text_muted));
                    });
                    if step.wait == Some(Wait::Run) && circuit_runner::node::check_node_cached().is_err() {
                        ui.add_space(4.0);
                        ui.label(RichText::new("Node.js isn't set up yet, so the bot can't start.").size(12.0).color(theme.warning));
                        if ui.button("Set up Node.js").clicked() {
                            ask = Some(Ask::SetUpNode);
                        }
                    } else if step.wait == Some(Wait::Run) && bot_running {
                        ui.label(RichText::new("The bot is starting …").size(12.0).color(theme.text_muted));
                    }
                } else if step.wait.is_some() {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Done!").size(12.5).strong().color(theme.action));
                }
                ui.add_space(12.0);
                if tour.step + 1 == STEPS.len() {
                    for (label, exit) in [("Open the DemoBot example", Exit::DemoBot), ("Start a new project", Exit::NewProject), ("Back to my project", Exit::Return)] {
                        let primary = exit == Exit::DemoBot;
                        let b = if primary { crate::dialog::primary_button(ui, &theme, label) } else { crate::dialog::secondary_button(ui, &theme, label) };
                        if b.clicked() {
                            ask = Some(Ask::End(exit));
                        }
                        ui.add_space(2.0);
                    }
                    return;
                }
                ui.horizontal(|ui| {
                    if tour.step > 0 && crate::dialog::secondary_button(ui, &theme, "Back").clicked() {
                        tour.step -= 1;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let next = ui.add_enabled_ui(!waiting, |ui| crate::dialog::primary_button(ui, &theme, "Next")).inner;
                        if next.clicked() {
                            tour.step += 1;
                        }
                        if waiting
                            && let Some(w) = step.wait
                            && crate::dialog::secondary_button(ui, &theme, "Do it for me").clicked()
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
