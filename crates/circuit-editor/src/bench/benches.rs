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

/// A canvas of `n` blocks in rows of `cols` (see `bench_data`).
fn fill(ed: &mut Editor, n: usize, cols: usize) {
    let ws = crate::bench::data::workspace("Bench", n, cols);
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
    crate::ui::fonts::install(&ctx);
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

/// Starting: loading every plugin (every block compiled in QuickJS), opening
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
        ("Load all plugins (every block, QuickJS)", load),
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

/// Everyday tasks: opening projects and workspaces, loading and reloading
/// blocks, searching in the block picker, copy, paste and undo, starting a
/// bot and exporting it.
#[test]
#[ignore]
fn bench_tasks() {
    let ms = |d: Duration| round(d.as_secs_f64() * 1000.0);
    // The best of a few tries, so a stray hiccup doesn't count.
    fn best(times: usize, mut f: impl FnMut() -> Duration) -> Duration {
        (0..times).map(|_| f()).min().unwrap()
    }
    let mut rows = Vec::new();
    let mut add = |task: String, d: Duration| {
        println!("| {task} | {:.1} |", ms(d));
        rows.push(json!({ "task": task, "ms": ms(d) }));
    };
    println!("\nTasks\n| task | ms |\n|---|---|");

    // Reading the block files on their own, and loading them (read + compile).
    let files: Vec<PathBuf> = walk(&root().join("plugins"));
    add(format!("Read the {} plugin files from disk", files.len()), best(3, || {
        let start = Instant::now();
        for f in &files {
            let _ = std::fs::read(f);
        }
        start.elapsed()
    }));
    let mut ed = editor();
    add(format!("Load all plugins (read, compile {} blocks)", ed.reg.blocks.len()), best(3, || {
        let start = Instant::now();
        let _ = Editor::new(vec![root().join("plugins")], None).unwrap();
        start.elapsed()
    }));
    add("Reload all plugins (after a file changed)".into(), best(3, || {
        let start = Instant::now();
        ed.reload_plugins();
        start.elapsed()
    }));

    // Projects of each size, saved to disk and opened.
    let base = std::env::temp_dir().join(format!("xc-bench-tasks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    for n in [200, 800, 1600] {
        let dir = base.join(format!("p{n}"));
        let mut p = circuit_core::Project::create(&dir, "Bench", "discord").unwrap();
        p.workspaces.insert("main".into(), crate::bench::data::workspace("Main", n, 20));
        add(format!("Save a project of {n} blocks"), best(3, || {
            let start = Instant::now();
            p.save().unwrap();
            start.elapsed()
        }));
        add(format!("Open a project of {n} blocks"), best(3, || {
            let start = Instant::now();
            ed.open_project(&dir);
            start.elapsed()
        }));
    }

    // Opening a workspace: its first frame (the text and shapes of its
    // blocks are laid out for the first time), with the app already warm.
    ed.open_project(&root().join("examples/DemoBot"));
    let ctx = context(&ed);
    fill(&mut ed, 50, 20);
    look(&mut ed, 1.0, pos2(-40.0, -40.0));
    for _ in 0..5 {
        frame(&ctx, &mut ed, &LAPTOP, vec![]);
    }
    for n in [25, 200, 800, 1600] {
        fill(&mut ed, n, 20);
        ed.views.remove(WS);
        ed.shapes.clear();
        look(&mut ed, 1.0, pos2(-40.0, -40.0));
        let start = Instant::now();
        frame(&ctx, &mut ed, &LAPTOP, vec![]);
        add(format!("Open a workspace of {n} blocks (first frame)"), start.elapsed());
    }

    // The block picker: open, and typing a search a letter at a time.
    fill(&mut ed, 200, 20);
    let picker_frame = |ed: &mut Editor| {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, LAPTOP.size)), ..Default::default() };
        let start = Instant::now();
        let out = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| crate::canvas::show(ui, ed, WS));
            crate::canvas::picker::show(ui.ctx(), ed);
        });
        let _ = ctx.tessellate(out.shapes, out.pixels_per_point);
        start.elapsed()
    };
    ed.picker = Some(crate::canvas::picker::Picker::new(WS, pos2(500.0, 300.0), pos2(500.0, 300.0), None));
    add("Open the block picker (first frame)".into(), picker_frame(&mut ed));
    let mut typing = Vec::new();
    for q in ["s", "se", "sen", "send", "send ", "send m", "send me", "send mes"] {
        if let Some(p) = ed.picker.as_mut() {
            p.query = q.into();
        }
        typing.push(picker_frame(&mut ed));
    }
    typing.sort();
    add("Type a letter in the block picker (frame, worst)".into(), *typing.last().unwrap());
    ed.picker = None;

    // Copy, paste and undo 100 blocks.
    let ids: Vec<String> = (0..100).map(|i| format!("b{i}")).collect();
    let clip = ed.project.as_ref().unwrap().workspaces[WS].extract_with_notes(&ids, &[]);
    let text = serde_json::to_string_pretty(&clip).unwrap();
    ed.active_workspace = Some(WS.into());
    let start = Instant::now();
    let pasted = ed.paste(&text, Some(pos2(0.0, 8000.0)));
    add("Paste 100 blocks".into(), start.elapsed());
    assert!(pasted, "pasting worked");
    let start = Instant::now();
    ed.undo();
    add("Undo that paste".into(), start.elapsed());

    // Starting a bot: build it, start Node and the runtime, until its first block ran.
    let dir = base.join("run");
    let mut p = circuit_core::Project::create(&dir, "Run", "none").unwrap();
    let mut ws = crate::bench::data::workspace("Main", 0, 1);
    ws.nodes.insert("go".into(), circuit_core::Node { block: "core.on_start@1".into(), ..Default::default() });
    ws.nodes.insert("say".into(), circuit_core::Node { block: "core.log@1".into(), values: json!({ "message": "ready" }).as_object().cloned().unwrap(), ..Default::default() });
    ws.links.push("go.next > say.in".parse().unwrap());
    p.workspaces.insert("main".into(), ws);
    p.save().unwrap();
    add("Start a bot (build, Node, first block ran)".into(), best(3, || {
        let start = Instant::now();
        let bundle = circuit_runner::build_bundle(&p, &ed.reg, None).unwrap();
        let session = circuit_runner::Session::start(
            circuit_runner::SessionConfig { bundle: bundle.value, dependencies: bundle.dependencies, packages_dir: dir.join("packages"), run_dir: dir.join(".circuit/run"), data_dir: dir.join(".circuit/data"), env: vec![] },
            || {},
        );
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            let said = session.poll().into_iter().any(|m| matches!(m, circuit_runner::Msg::Bot(circuit_runner::BotEvent::Log { ref text, .. }) if text == "ready"));
            if said {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let took = start.elapsed();
        drop(session);
        took
    }));

    // Exporting an 800-block bot (without npm install).
    ed.open_project(&base.join("p800"));
    let bundle = circuit_runner::build_bundle(ed.project.as_ref().unwrap(), &ed.reg, ed.mode()).unwrap();
    let out = base.join("export");
    add("Export a bot of 800 blocks (without npm install)".into(), best(3, || {
        let _ = std::fs::remove_dir_all(&out);
        let opts = circuit_runner::ExportOptions { name: "Bench".into(), token: None, token_env: "DISCORD_TOKEN".into(), secrets: Vec::new(), install: false };
        let start = Instant::now();
        circuit_runner::export(&out, &bundle, &opts, &mut |_| {}).unwrap();
        start.elapsed()
    }));
    let _ = std::fs::remove_dir_all(&base);
    save("tasks", Value::Array(rows));
}

/// Every file under a folder.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}
