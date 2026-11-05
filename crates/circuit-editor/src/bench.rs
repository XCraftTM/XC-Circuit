//! Benchmarks: how long the editor needs per frame on canvases of every size,
//! while idle, hovering, panning, zooming and dragging, and how long it takes
//! to start. They time the CPU side of a frame: building the interface and
//! turning it into triangles (the GPU then only draws those). Run in release:
//!
//! ```text
//! cargo test --release -p circuit-editor bench_ -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Each prints a table and writes `target/bench/<name>.json`. The results are
//! on the website's Performance page.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use circuit_core::{Link, Node, Workspace};
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, Rect, Vec2, pos2, vec2};
use serde_json::{Value, json};

use crate::editor::Editor;

const WS: &str = "bench";
/// Frame budgets: 60 Hz and 120 Hz screens.
const BUDGET_60: f64 = 1000.0 / 60.0;
const BUDGET_120: f64 = 1000.0 / 120.0;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The editor with every bundled plugin and DemoBot open.
fn editor() -> Editor {
    let mut ed = Editor::new(vec![root().join("plugins")], None).unwrap();
    ed.open_project(&root().join("examples/DemoBot"));
    ed
}

/// A canvas of `n` blocks in rows of `cols`, as in a real bot: Send Message,
/// Math, If and Log in turn, each joined to the next by a green wire, and a
/// value wire from every Math into the block after it.
fn fill(ed: &mut Editor, n: usize, cols: usize) {
    let mut ws = Workspace { name: "Bench".into(), ..Default::default() };
    for i in 0..n {
        let (block, values, pins): (&str, Value, Vec<String>) = match i % 4 {
            0 => ("discord.send_message@3", json!({ "text": format!("Hello ${{user.username}}, this is message {i}") }), vec![]),
            1 => ("core.math@1", json!({ "a": i, "op": "multiply", "b": 2 }), vec![]),
            2 => ("core.if@1", json!({}), vec![]),
            _ => ("core.log@1", json!({ "message": format!("Step {i} done") }), vec!["message".into()]),
        };
        let values = values.as_object().cloned().unwrap_or_default();
        let at = [(i % cols) as f32 * 380.0, (i / cols) as f32 * 340.0];
        ws.nodes.insert(format!("b{i}"), Node { block: block.into(), at, values, pins, ..Default::default() });
    }
    let link = |s: String| s.parse::<Link>().unwrap();
    for i in 1..n {
        // Flow wires (Math is a value block, so the flow skips over it).
        let from = if i % 4 == 2 { i - 2 } else { i - 1 };
        let (from_pin, to_ok) = match (from % 4, i % 4) {
            (_, 1) => continue,
            (2, _) => ("true", true),
            _ => ("next", true),
        };
        if to_ok {
            ws.links.push(link(format!("b{from}.{from_pin} > b{i}.in")));
        }
        if i % 4 == 2 {
            ws.links.push(link(format!("b{}.result > b{i}.condition", i - 1)));
        }
        if i % 4 == 3 && i >= 2 {
            ws.links.push(link(format!("b{}.result > b{i}.message", i - 2)));
        }
    }
    ed.project.as_mut().unwrap().workspaces.insert(WS.into(), ws);
}

/// Sets the view: zoom, and the scene point in the top-left corner.
fn look(ed: &mut Editor, zoom: f32, top_left: Pos2) {
    let v = ed.views.entry(WS.into()).or_default();
    v.zoom = zoom;
    v.offset = -top_left.to_vec2() * zoom;
    v.initialized = true;
}

struct Screen {
    /// Window size in points.
    size: Vec2,
    pixels_per_point: f32,
}

const LAPTOP: Screen = Screen { size: vec2(1400.0, 900.0), pixels_per_point: 1.0 };
const UHD: Screen = Screen { size: vec2(2560.0, 1440.0), pixels_per_point: 1.5 };

/// One frame: the canvas built and tessellated. Returns its time and the
/// number of triangles the GPU gets.
fn frame(ctx: &egui::Context, ed: &mut Editor, screen: &Screen, events: Vec<Event>) -> (f64, usize) {
    let raw = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen.size)),
        events,
        ..Default::default()
    };
    ctx.set_pixels_per_point(screen.pixels_per_point);
    let start = Instant::now();
    let out = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| crate::canvas::show(ui, ed, WS));
    });
    let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    let triangles = prims
        .iter()
        .map(|p| match &p.primitive {
            egui::epaint::Primitive::Mesh(m) => m.indices.len() / 3,
            _ => 0,
        })
        .sum();
    (ms, triangles)
}

fn context(ed: &Editor) -> egui::Context {
    let ctx = egui::Context::default();
    crate::fonts::install(&ctx);
    egui_extras::install_image_loaders(&ctx);
    ed.theme.apply(&ctx);
    ctx
}

/// The events for each measured frame.
type Input = Box<dyn FnMut(usize) -> Vec<Event>>;

#[derive(Clone, Copy)]
struct Stats {
    median: f64,
    p95: f64,
    max: f64,
    triangles: usize,
}

fn stats(mut ms: Vec<f64>, triangles: usize) -> Stats {
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| ms[((ms.len() - 1) as f64 * q).round() as usize];
    Stats { median: at(0.5), p95: at(0.95), max: *ms.last().unwrap(), triangles }
}

/// Runs `frames` frames after a warm-up, with the events `input` gives for
/// each, and measures them.
fn measure(ctx: &egui::Context, ed: &mut Editor, screen: &Screen, frames: usize, mut input: impl FnMut(usize) -> Vec<Event>) -> Stats {
    for _ in 0..8 {
        frame(ctx, ed, screen, vec![]);
    }
    let mut times = Vec::with_capacity(frames);
    let mut triangles = 0;
    for i in 0..frames {
        let (ms, t) = frame(ctx, ed, screen, input(i));
        times.push(ms);
        triangles = triangles.max(t);
    }
    stats(times, triangles)
}

fn row(label: &str, s: Stats) -> Value {
    let verdict = if s.p95 <= BUDGET_120 {
        "120 Hz"
    } else if s.p95 <= BUDGET_60 {
        "60 Hz"
    } else {
        "drops frames"
    };
    println!("| {label:<44} | {:>7.2} | {:>7.2} | {:>7.2} | {:>9} | {verdict} |", s.median, s.p95, s.max, s.triangles);
    json!({ "case": label, "median_ms": round(s.median), "p95_ms": round(s.p95), "max_ms": round(s.max), "triangles": s.triangles, "smooth_at": verdict })
}

fn round(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn header(title: &str) {
    println!("\n{title}\n| {:<44} | median  | p95     | max     | triangles | smooth at |\n|---|---|---|---|---|---|", "case");
}

fn save(name: &str, rows: Value) {
    let dir = root().join("target/bench");
    std::fs::create_dir_all(&dir).unwrap();
    let doc = json!({ "profile": if cfg!(debug_assertions) { "debug" } else { "release" }, "rows": rows });
    std::fs::write(dir.join(format!("{name}.json")), serde_json::to_string_pretty(&doc).unwrap()).unwrap();
}

/// The pointer moving across the canvas, as when just looking around.
fn hover(i: usize) -> Vec<Event> {
    vec![Event::PointerMoved(pos2(200.0 + (i % 40) as f32 * 20.0, 300.0 + (i % 7) as f32 * 15.0))]
}

/// How frame time grows with the number of blocks: all at 100% (a few in
/// view), and zoomed all the way out (the screen full of blocks).
#[test]
#[ignore]
fn bench_canvas_size() {
    let mut ed = editor();
    header("Canvas size (pointer moving, 1400×900 window)");
    let mut rows = Vec::new();
    for n in [25, 50, 100, 200, 400, 800, 1600, 3200] {
        fill(&mut ed, n, 10);
        for (zoom, what) in [(1.0, "100%"), (0.25, "25%, screen full")] {
            let ctx = context(&ed);
            look(&mut ed, zoom, pos2(-40.0, -40.0));
            let s = measure(&ctx, &mut ed, &LAPTOP, 60, hover);
            rows.push(row(&format!("{n} blocks, zoom {what}"), s));
        }
    }
    save("canvas_size", Value::Array(rows));
}

/// What the busiest moments cost: panning, zooming, dragging a block and
/// box-selecting, on a 400-block canvas zoomed to 50%, and on a big screen.
#[test]
#[ignore]
fn bench_interactions() {
    let mut ed = editor();
    header("Interactions (400 blocks, zoom 50%)");
    let mut rows = Vec::new();
    let press = |pos: Pos2, pressed: bool| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    let screens = [("1400×900", &LAPTOP), ("2560×1440 at 150% (4K)", &UHD)];
    for (screen_name, screen) in screens {
        let cases: Vec<(&str, Input)> = vec![
            ("idle", Box::new(|_| vec![])),
            ("pointer moving", Box::new(hover)),
            // Drag on empty canvas between blocks: pans.
            ("panning", Box::new(move |i| {
                let from = pos2(170.0, 160.0);
                match i {
                    0 => vec![Event::PointerMoved(from), press(from, true)],
                    _ => vec![Event::PointerMoved(from + vec2((i % 50) as f32 * 4.0, (i % 50) as f32 * 2.0))],
                }
            })),
            ("zooming (pinch or Ctrl+wheel)", Box::new(|i| vec![Event::PointerMoved(pos2(600.0, 400.0)), Event::Zoom(if (i / 10) % 2 == 0 { 1.03 } else { 1.0 / 1.03 })])),
            // Drag a block by its title (the first block's header at 50%).
            ("dragging a block", Box::new(move |i| {
                let from = pos2(40.0 + 60.0 * 0.5, 40.0 + 12.0 * 0.5);
                match i {
                    0 => vec![Event::PointerMoved(from), press(from, true)],
                    _ => vec![Event::PointerMoved(from + vec2((i % 50) as f32 * 6.0, (i % 30) as f32 * 3.0))],
                }
            })),
        ];
        for (name, mut input) in cases {
            let ctx = context(&ed);
            fill(&mut ed, 400, 20);
            look(&mut ed, 0.5, pos2(-80.0, -80.0));
            let s = measure(&ctx, &mut ed, screen, 90, &mut input);
            rows.push(row(&format!("{name}, {screen_name}"), s));
        }
    }
    save("interactions", Value::Array(rows));
}

/// Where frames start to drop: the screen filled edge to edge with blocks
/// at every zoom, on a laptop-sized and a 4K window.
#[test]
#[ignore]
fn bench_frame_drops() {
    let mut ed = editor();
    header("Blocks on screen (screen filled with blocks, pointer moving)");
    let mut rows = Vec::new();
    for (screen_name, screen) in [("1400×900", &LAPTOP), ("4K", &UHD)] {
        for zoom in [1.0, 0.75, 0.5, 0.35, 0.25] {
            let cols = (screen.size.x / zoom / 380.0).ceil() as usize;
            let lines = (screen.size.y / zoom / 340.0).ceil() as usize;
            let n = cols * lines;
            fill(&mut ed, n, cols);
            let ctx = context(&ed);
            look(&mut ed, zoom, pos2(-20.0, -20.0));
            let s = measure(&ctx, &mut ed, screen, 60, hover);
            rows.push(row(&format!("{n} on screen, {screen_name}, zoom {:.0}%", zoom * 100.0), s));
        }
    }
    save("frame_drops", Value::Array(rows));
}

/// Starting: loading every plugin (205 blocks compiled in QuickJS), opening
/// DemoBot, the first frames, building a bot to run and saving.
#[test]
#[ignore]
fn bench_startup() {
    let time = |f: &mut dyn FnMut()| {
        let start = Instant::now();
        f();
        start.elapsed()
    };
    let mut rows = Vec::new();
    let ms = |d: Duration| round(d.as_secs_f64() * 1000.0);
    let mut ed = None;
    let load = time(&mut || ed = Some(Editor::new(vec![root().join("plugins")], None).unwrap()));
    let mut ed = ed.unwrap();
    let open = time(&mut || ed.open_project(&root().join("examples/DemoBot")));
    fill(&mut ed, 400, 20);
    let ctx = context(&ed);
    look(&mut ed, 0.5, pos2(-40.0, -40.0));
    let first = time(&mut || {
        frame(&ctx, &mut ed, &LAPTOP, vec![]);
    });
    let mut later = Duration::ZERO;
    for _ in 0..5 {
        later = time(&mut || {
            frame(&ctx, &mut ed, &LAPTOP, vec![]);
        });
    }
    let bundle = time(&mut || {
        circuit_runner::build_bundle(ed.project.as_ref().unwrap(), &ed.reg, ed.mode()).unwrap();
    });
    let dir = std::env::temp_dir().join(format!("xc-bench-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut copy = ed.project.clone().unwrap();
    copy.root = dir.clone();
    let save_time = time(&mut || copy.save().unwrap());
    let _ = std::fs::remove_dir_all(&dir);

    println!("\nStartup and project work\n| step | ms |\n|---|---|");
    for (step, d) in [
        ("Load all plugins (205 blocks, QuickJS)", load),
        ("Open DemoBot", open),
        ("First frame of a 400-block canvas", first),
        ("Next frames (fonts and shapes cached)", later),
        ("Build the bot to run (400 + DemoBot blocks)", bundle),
        ("Save the project", save_time),
    ] {
        println!("| {step} | {:.1} |", ms(d));
        rows.push(json!({ "step": step, "ms": ms(d) }));
    }
    save("startup", Value::Array(rows));
}
