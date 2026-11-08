//! The freeze detector: notices when the app stops answering (Windows then
//! says "Not Responding") and says where it was stuck, in the Console and in
//! `freezes.log` in the app's data folder.
//!
//! The frame marks what it's doing as it goes ([`mark`]); after the frame,
//! the mark says so. A thread looks every 200 ms: when no frame came for a
//! second while the window has the focus, it asks for one (an idle app draws
//! none). A healthy app answers within milliseconds; one that hasn't after
//! three seconds is stuck, in the step marked last.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// After the frame: what's left is drawing on the graphics card, and Windows.
pub const AFTER_FRAME: &str = "after its frame: drawing on the graphics card, or Windows and tools reading the window (accessibility, input methods)";

/// How long without a frame (with one asked for) counts as stuck.
const STUCK_MS: u64 = 3000;
/// How long without a frame before one is asked for.
const ASK_AFTER_MS: u64 = 1000;

static CLOCK: OnceLock<Instant> = OnceLock::new();
static BEAT: AtomicU64 = AtomicU64::new(0);
static FOCUSED: AtomicBool = AtomicBool::new(false);
static PHASE: Mutex<String> = Mutex::new(String::new());
static FOUND: Mutex<Vec<Freeze>> = Mutex::new(Vec::new());

fn now_ms() -> u64 {
    CLOCK.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// The frame is at `what` now (also: the app is alive).
pub fn mark(what: impl Into<String>) {
    let what = what.into();
    if let Ok(mut p) = PHASE.lock()
        && *p != what
    {
        *p = what;
    }
    BEAT.store(now_ms(), Ordering::Relaxed);
}

/// Whether the window has the focus: only then is a missing frame a freeze
/// (a minimized or hidden window may get none).
pub fn focused(on: bool) {
    FOCUSED.store(on, Ordering::Relaxed);
}

/// A freeze that ended: where it was stuck, and how long.
#[derive(Clone, Debug, PartialEq)]
pub struct Freeze {
    pub phase: String,
    pub seconds: f64,
}

/// Freezes found since last asked, for the Console.
pub fn take_found() -> Vec<Freeze> {
    FOUND.lock().map(|mut f| std::mem::take(&mut *f)).unwrap_or_default()
}

/// What the watching thread decides on one look.
#[derive(Debug, PartialEq)]
enum Step {
    Nothing,
    /// Ask the app for a frame.
    Ask,
    /// It's stuck since `since` (ms), in this phase.
    Stuck { since: u64 },
    /// It answered again after being stuck: for so many ms.
    Back { ms: u64 },
}

/// The watching, without a clock or a thread (so it can be tested).
#[derive(Default)]
struct Watch {
    /// When a frame was asked for, and the beat then.
    asked: Option<(u64, u64)>,
    /// Stuck since this beat.
    stuck: Option<u64>,
}

impl Watch {
    fn look(&mut self, now: u64, beat: u64, focused: bool) -> Step {
        if let Some(since) = self.stuck {
            if beat != since {
                self.stuck = None;
                self.asked = None;
                return Step::Back { ms: beat.saturating_sub(since) };
            }
            return Step::Nothing;
        }
        if let Some((_, at_beat)) = self.asked
            && beat != at_beat
        {
            // It answered.
            self.asked = None;
        }
        if !focused {
            self.asked = None;
            return Step::Nothing;
        }
        match self.asked {
            None if now.saturating_sub(beat) >= ASK_AFTER_MS => {
                self.asked = Some((now, beat));
                Step::Ask
            }
            Some((at, at_beat)) if now.saturating_sub(at) >= STUCK_MS.saturating_sub(ASK_AFTER_MS) && now.saturating_sub(at_beat) >= STUCK_MS => {
                self.stuck = Some(at_beat);
                Step::Stuck { since: at_beat }
            }
            _ => Step::Nothing,
        }
    }
}

/// Starts watching (once): `ctx` is asked for frames.
pub fn start(ctx: egui::Context, log: std::path::PathBuf) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::Relaxed) {
        return;
    }
    mark("while starting");
    std::thread::Builder::new()
        .name("freeze-detector".into())
        .spawn(move || {
            let mut watch = Watch::default();
            let mut phase_then = String::new();
            loop {
                std::thread::sleep(Duration::from_millis(200));
                let now = now_ms();
                match watch.look(now, BEAT.load(Ordering::Relaxed), FOCUSED.load(Ordering::Relaxed)) {
                    Step::Nothing => {}
                    Step::Ask => ctx.request_repaint(),
                    Step::Stuck { since } => {
                        phase_then = PHASE.lock().map(|p| p.clone()).unwrap_or_default();
                        let ms = now.saturating_sub(since);
                        write_line(&log, &format!("not answering for {:.1} s, stuck in: {phase_then}", ms as f64 / 1000.0));
                    }
                    Step::Back { ms } => {
                        write_line(&log, &format!("answered again after {:.1} s, stuck in: {phase_then}", ms as f64 / 1000.0));
                        if let Ok(mut f) = FOUND.lock() {
                            f.push(Freeze { phase: std::mem::take(&mut phase_then), seconds: ms as f64 / 1000.0 });
                        }
                        ctx.request_repaint();
                    }
                }
            }
        })
        .ok();
}

/// A line in the log, with the time and the app's version; kept short (the
/// last 200 lines).
fn write_line(path: &std::path::Path, text: &str) {
    let line = format!("{} · XC Circuit {} · {text}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), env!("CARGO_PKG_VERSION"));
    let old = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<&str> = old.lines().collect();
    let keep = lines.len().saturating_sub(199);
    lines.drain(..keep);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::File::create(path) {
        for l in lines {
            let _ = writeln!(f, "{l}");
        }
        let _ = f.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An idle app is asked for a frame and answers: no freeze. One that
    /// doesn't answer for three seconds is stuck, and says when it's back
    /// and how long it was gone. Without the focus, nothing is asked.
    #[test]
    fn a_missing_frame_is_asked_for_then_counted() {
        let mut w = Watch::default();
        assert_eq!(w.look(500, 0, true), Step::Nothing, "a frame half a second ago");
        assert_eq!(w.look(1000, 0, true), Step::Ask, "idle: asked for one");
        assert_eq!(w.look(1100, 1050, true), Step::Nothing, "it came");
        assert_eq!(w.look(2100, 1050, true), Step::Ask, "idle again");
        assert_eq!(w.look(3000, 1050, true), Step::Nothing);
        assert_eq!(w.look(4100, 1050, true), Step::Stuck { since: 1050 }, "none for three seconds");
        assert_eq!(w.look(5000, 1050, true), Step::Nothing, "said once");
        assert_eq!(w.look(7300, 7250, true), Step::Back { ms: 6200 });
        // Minimized or in the background: not asked, never stuck.
        let mut w = Watch::default();
        assert_eq!(w.look(9000, 0, false), Step::Nothing);
        assert_eq!(w.look(20000, 0, false), Step::Nothing);
    }

    #[test]
    fn the_log_keeps_its_last_lines() {
        let path = std::env::temp_dir().join(format!("xc-freezes-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        for i in 0..205 {
            write_line(&path, &format!("line {i}"));
        }
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 200);
        assert!(text.lines().last().unwrap().ends_with("line 204") && text.lines().next().unwrap().ends_with("line 5"));
        let _ = std::fs::remove_file(&path);
    }
}
