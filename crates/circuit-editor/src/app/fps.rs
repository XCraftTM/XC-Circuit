//! `xc-circuit --fps <results.json>`: real frame rates. The whole app runs in
//! its window with a made-up project (in a temporary folder, with its own
//! settings, so nothing of yours is touched), shows workspaces of 25 to 1,600
//! blocks and pans and zooms them by itself; nothing moves the mouse. Vsync is
//! off, so the frame rate is what the computer can draw, not the screen's
//! refresh rate. When it's done, it writes the results and closes.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui;
use serde_json::{Value, json};

use super::CircuitApp;
use crate::editor::Action;

/// The workspaces, by key: name and number of blocks.
const WORKSPACES: [(&str, usize); 4] = [("w25", 25), ("w200", 200), ("w800", 800), ("w1600", 1600)];
/// Frames left out at the start of each scene (layout settling, caches).
const WARM_UP: usize = 20;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Scene {
    Still,
    Panning,
    Zooming,
}

impl Scene {
    const ALL: [Scene; 3] = [Scene::Still, Scene::Panning, Scene::Zooming];
    fn length(self) -> Duration {
        Duration::from_secs(match self {
            Scene::Still => 2,
            _ => 3,
        })
    }
    fn name(self) -> &'static str {
        match self {
            Scene::Still => "still",
            Scene::Panning => "panning",
            Scene::Zooming => "zooming",
        }
    }
}

pub struct Run {
    out: PathBuf,
    graphics: String,
    /// Which workspace and scene; None before the first.
    at: Option<(usize, usize)>,
    scene_start: Instant,
    last_frame: Option<Instant>,
    /// Frame intervals (ms) and processor time per frame (ms) in this scene.
    intervals: Vec<f64>,
    cpu: Vec<f64>,
    frames: usize,
    /// When a workspace was asked to open, until its first frame is done.
    opening: Option<Instant>,
    opened_ms: f64,
    results: Vec<Value>,
    /// When it started: it gives up after `GIVE_UP`, so it never hangs.
    began: Instant,
}

const GIVE_UP: Duration = Duration::from_secs(240);

/// Makes the project and the settings folder, and sets the variables that
/// make the app use them. Returns where the results go.
pub fn prepare(out: &Path) -> std::io::Result<()> {
    let dir = std::env::temp_dir().join(format!("xc-fps-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("data"))?;
    let mut project = circuit_core::Project::create(dir.join("FPS Bench"), "FPS Bench", "discord").map_err(std::io::Error::other)?;
    project.workspaces.clear();
    for (key, n) in WORKSPACES {
        project.workspaces.insert(key.into(), crate::bench_data::workspace(&format!("{n} blocks"), n, 20));
    }
    project.save().map_err(std::io::Error::other)?;
    // SAFETY: set before the app or any thread starts.
    unsafe {
        std::env::set_var("XC_CIRCUIT_DATA", dir.join("data"));
        std::env::set_var("XC_CIRCUIT_PROJECT", &project.root);
        std::env::set_var("XC_FPS_OUT", out);
    }
    Ok(())
}

impl Run {
    pub fn new(out: PathBuf, graphics: String) -> Run {
        Run {
            out,
            graphics,
            at: None,
            scene_start: Instant::now(),
            last_frame: None,
            intervals: Vec::new(),
            cpu: Vec::new(),
            frames: 0,
            opening: None,
            opened_ms: 0.0,
            results: Vec::new(),
            began: Instant::now(),
        }
    }

    /// Called at the start of every frame. Returns false when it's done.
    pub fn frame(&mut self, app: &mut CircuitApp, ctx: &egui::Context, cpu_seconds: Option<f32>) -> bool {
        // Nothing but the canvas: no first-start page, tour or offers.
        app.setup = None;
        app.tour = None;
        app.crash_offer = None;
        ctx.request_repaint();
        let now = Instant::now();
        let interval = self.last_frame.map(|t| (now - t).as_secs_f64() * 1000.0);
        self.last_frame = Some(now);
        if let Some(asked) = self.opening.take() {
            self.opened_ms = (now - asked).as_secs_f64() * 1000.0;
        }

        if self.began.elapsed() > GIVE_UP {
            eprintln!("fps: gave up after {} s", GIVE_UP.as_secs());
            self.write(ctx);
            app.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return false;
        }
        let Some((w, s)) = self.at else {
            // Give the window a moment, then start.
            if app.ed.project.is_some() && self.scene_start.elapsed() > Duration::from_secs(1) {
                self.start(app, 0, 0);
            }
            return true;
        };
        self.frames += 1;
        if self.frames > WARM_UP {
            if let Some(i) = interval {
                self.intervals.push(i);
            }
            if let Some(c) = cpu_seconds {
                self.cpu.push(c as f64 * 1000.0);
            }
        }
        let scene = Scene::ALL[s];
        let key = WORKSPACES[w].0;
        let t = self.scene_start.elapsed().as_secs_f32();
        if let Some(view) = app.ed.views.get_mut(key) {
            match scene {
                Scene::Still => {}
                // Across and back, 600 points a second.
                Scene::Panning => {
                    let x = (t * 600.0) % 2400.0;
                    let x = if x > 1200.0 { 2400.0 - x } else { x };
                    view.offset = egui::vec2(40.0 - x, 40.0 - x * 0.4);
                }
                // From 100% out to 25% and back, every 1.5 s.
                Scene::Zooming => {
                    let phase = (t / 1.5 * std::f32::consts::PI).cos() * 0.5 + 0.5;
                    view.zoom = 0.25 + 0.75 * phase;
                }
            }
        }
        if self.scene_start.elapsed() < scene.length() {
            return true;
        }
        self.finish_scene(ctx, w, s);
        match (s + 1 < Scene::ALL.len(), w + 1 < WORKSPACES.len()) {
            (true, _) => self.start(app, w, s + 1),
            (false, true) => self.start(app, w + 1, 0),
            (false, false) => {
                self.write(ctx);
                app.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return false;
            }
        }
        true
    }

    fn start(&mut self, app: &mut CircuitApp, w: usize, s: usize) {
        let key = WORKSPACES[w].0;
        if s == 0 {
            app.ed.actions.push(Action::OpenWorkspace(key.into()));
            self.opening = Some(Instant::now());
        }
        if let Some(view) = app.ed.views.get_mut(key) {
            view.zoom = 1.0;
            view.offset = egui::vec2(40.0, 40.0);
        }
        eprintln!("fps: {} blocks, {}", WORKSPACES[w].1, Scene::ALL[s].name());
        self.at = Some((w, s));
        self.scene_start = Instant::now();
        self.intervals.clear();
        self.cpu.clear();
        self.frames = 0;
    }

    fn finish_scene(&mut self, ctx: &egui::Context, w: usize, s: usize) {
        let pick = |v: &mut Vec<f64>, q: f64| {
            if v.is_empty() {
                return 0.0;
            }
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[((v.len() - 1) as f64 * q).round() as usize]
        };
        let count = self.intervals.len();
        let total: f64 = self.intervals.iter().sum();
        let fps = if total > 0.0 { count as f64 * 1000.0 / total } else { 0.0 };
        let over = |limit: f64| self.intervals.iter().filter(|i| **i > limit).count() as f64 * 100.0 / count.max(1) as f64;
        let (over_60, over_120) = (over(1000.0 / 60.0), over(1000.0 / 120.0));
        let mut intervals = std::mem::take(&mut self.intervals);
        let mut cpu = std::mem::take(&mut self.cpu);
        let row = json!({
            "blocks": WORKSPACES[w].1,
            "scene": Scene::ALL[s].name(),
            "fps": r(fps),
            "frame_ms_median": r(pick(&mut intervals, 0.5)),
            "frame_ms_p95": r(pick(&mut intervals, 0.95)),
            "frame_ms_max": r(pick(&mut intervals, 1.0)),
            "cpu_ms_median": r(pick(&mut cpu, 0.5)),
            "cpu_ms_p95": r(pick(&mut cpu, 0.95)),
            "frames_over_16_7ms_percent": r(over_60),
            "frames_over_8_3ms_percent": r(over_120),
            "open_workspace_ms": if s == 0 { Value::from(r(self.opened_ms)) } else { Value::Null },
            "frames": count,
        });
        eprintln!("fps:   {row}");
        let _ = ctx;
        self.results.push(row);
    }

    fn write(&self, ctx: &egui::Context) {
        let size = ctx.content_rect().size();
        let doc = json!({
            "graphics": self.graphics,
            "window_points": [size.x.round(), size.y.round()],
            "pixels_per_point": ctx.pixels_per_point(),
            "vsync": false,
            "version": env!("CARGO_PKG_VERSION"),
            "scenes": self.results,
        });
        if let Some(dir) = self.out.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match std::fs::write(&self.out, serde_json::to_string_pretty(&doc).unwrap_or_default()) {
            Ok(()) => eprintln!("fps: results in {}", self.out.display()),
            Err(e) => eprintln!("fps: couldn't write {}: {e}", self.out.display()),
        }
    }
}

fn r(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
