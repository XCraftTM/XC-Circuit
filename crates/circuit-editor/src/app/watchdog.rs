//! The freeze detector: notices when the app stops answering (Windows then
//! says "Not Responding") and says where it was stuck, in the Console and in
//! `freezes.log` in the app's data folder.
//!
//! The frame marks what it's doing as it goes ([`mark`]); after the frame,
//! the mark says so. A thread looks every 200 ms: when no frame came for a
//! second while the window has the focus, it asks for one (an idle app draws
//! none). A healthy app answers within milliseconds; one that hasn't after
//! three seconds is stuck, in the step marked last, if Windows can't reach
//! the window either (what makes it say "Not Responding"). A window that
//! still takes Windows' messages but draws nothing (held while moved,
//! minimized, covered) isn't stuck.
//!
//! The thread itself never waits on the app: it asks for frames from a
//! helper thread, so a freeze inside egui (its lock held) is still found,
//! and said.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
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
/// The main window (Windows: its handle), to ask whether it takes messages.
static WINDOW: AtomicIsize = AtomicIsize::new(0);
/// A frame is being asked for and egui hasn't taken the request yet: its
/// lock is held, so the app is stuck inside egui.
static ASKING: AtomicBool = AtomicBool::new(false);
/// Said after the phase when the app was stuck inside egui.
const INSIDE_EGUI: &str = "(inside egui, which was busy: laying out a long text, say, or the window's accessibility tree)";
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

/// The main window, so the watching can ask Windows whether it answers.
pub fn set_window(hwnd: Option<isize>) {
    WINDOW.store(hwnd.unwrap_or(0), Ordering::Relaxed);
}

/// Whether the window takes Windows' messages within a second (`None`: can't
/// tell, elsewhere than Windows or without the window). A stuck app doesn't,
/// which is when Windows says "Not Responding"; one held while its window is
/// moved, or minimized, still does.
fn window_answers() -> Option<bool> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SMTO_ABORTIFHUNG, SMTO_BLOCK, SendMessageTimeoutW, WM_NULL};
        let hwnd = WINDOW.load(Ordering::Relaxed);
        if hwnd == 0 {
            return None;
        }
        let mut result = 0usize;
        // SAFETY: WM_NULL does nothing; a window that's gone just fails.
        let sent = unsafe { SendMessageTimeoutW(hwnd as _, WM_NULL, 0, 0, SMTO_ABORTIFHUNG | SMTO_BLOCK, 1000, &mut result) };
        Some(sent != 0)
    }
    #[cfg(not(windows))]
    None
}

/// Asks the app for a frame without waiting for it: from a helper thread,
/// one at a time, so the watching goes on if egui is stuck.
fn ask_for_frame(ctx: &egui::Context) {
    if ASKING.swap(true, Ordering::Relaxed) {
        return;
    }
    let ctx = ctx.clone();
    let spawned = std::thread::Builder::new().name("freeze-detector-ask".into()).spawn(move || {
        ctx.request_repaint();
        ASKING.store(false, Ordering::Relaxed);
    });
    if spawned.is_err() {
        ASKING.store(false, Ordering::Relaxed);
    }
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
    /// `answers` says whether the window still takes Windows' messages; it's
    /// asked only when no frame came for three seconds.
    fn look(&mut self, now: u64, beat: u64, focused: bool, answers: impl FnOnce() -> Option<bool>) -> Step {
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
                if answers() == Some(true) {
                    // Not drawing, but not stuck: ask again later.
                    self.asked = None;
                    return Step::Nothing;
                }
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
                match watch.look(now, BEAT.load(Ordering::Relaxed), FOCUSED.load(Ordering::Relaxed), window_answers) {
                    Step::Nothing => {}
                    Step::Ask => ask_for_frame(&ctx),
                    Step::Stuck { since } => {
                        phase_then = PHASE.lock().map(|p| p.clone()).unwrap_or_default();
                        if ASKING.load(Ordering::Relaxed) {
                            phase_then = format!("{phase_then} {INSIDE_EGUI}");
                        }
                        let ms = now.saturating_sub(since);
                        write_line(&log, &format!("not answering for {:.1} s, stuck in: {phase_then}", ms as f64 / 1000.0));
                    }
                    Step::Back { ms } => {
                        write_line(&log, &format!("answered again after {:.1} s, stuck in: {phase_then}", ms as f64 / 1000.0));
                        if let Ok(mut f) = FOUND.lock() {
                            f.push(Freeze { phase: std::mem::take(&mut phase_then), seconds: ms as f64 / 1000.0 });
                        }
                        ask_for_frame(&ctx);
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
        let hung = || Some(false);
        let mut w = Watch::default();
        assert_eq!(w.look(500, 0, true, hung), Step::Nothing, "a frame half a second ago");
        assert_eq!(w.look(1000, 0, true, hung), Step::Ask, "idle: asked for one");
        assert_eq!(w.look(1100, 1050, true, hung), Step::Nothing, "it came");
        assert_eq!(w.look(2100, 1050, true, hung), Step::Ask, "idle again");
        assert_eq!(w.look(3000, 1050, true, hung), Step::Nothing);
        assert_eq!(w.look(4100, 1050, true, hung), Step::Stuck { since: 1050 }, "none for three seconds");
        assert_eq!(w.look(5000, 1050, true, hung), Step::Nothing, "said once");
        assert_eq!(w.look(7300, 7250, true, hung), Step::Back { ms: 6200 });
        // Minimized or in the background: not asked, never stuck.
        let mut w = Watch::default();
        assert_eq!(w.look(9000, 0, false, hung), Step::Nothing);
        assert_eq!(w.look(20000, 0, false, hung), Step::Nothing);
    }

    /// No frame, but the window still takes Windows' messages (held while
    /// it's moved, say): not stuck, and asked again later. Where Windows
    /// can't be asked, a missing frame still counts.
    #[test]
    fn a_window_that_answers_windows_is_not_stuck() {
        let mut w = Watch::default();
        assert_eq!(w.look(1000, 0, true, || Some(true)), Step::Ask);
        assert_eq!(w.look(4000, 0, true, || Some(true)), Step::Nothing, "it answers Windows");
        assert_eq!(w.look(4200, 0, true, || Some(true)), Step::Ask, "asked again");
        assert_eq!(w.look(7200, 0, true, || Some(false)), Step::Stuck { since: 0 }, "now it doesn't");
        let mut w = Watch::default();
        assert_eq!(w.look(1000, 0, true, || None), Step::Ask);
        assert_eq!(w.look(4000, 0, true, || None), Step::Stuck { since: 0 }, "can't tell: counts");
        // Windows is asked only when it's about to count as stuck.
        let mut w = Watch::default();
        assert_eq!(w.look(1000, 0, true, || unreachable!()), Step::Ask);
        assert_eq!(w.look(2000, 0, true, || unreachable!()), Step::Nothing);
    }

    /// Asking for a frame doesn't wait for egui: with its lock held by a
    /// stuck frame, the watching goes on, and knows it's stuck inside egui.
    #[test]
    fn asking_for_a_frame_never_waits() {
        let ctx = egui::Context::default();
        let (tx, rx) = std::sync::mpsc::channel();
        let (held_tx, held_rx) = std::sync::mpsc::channel::<()>();
        let holder = {
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                // A frame stuck inside egui (its lock held).
                ctx.input(|_| {
                    held_tx.send(()).unwrap();
                    rx.recv().ok();
                });
            })
        };
        held_rx.recv().unwrap();
        let start = Instant::now();
        ask_for_frame(&ctx);
        assert!(start.elapsed() < Duration::from_millis(500), "it didn't wait");
        std::thread::sleep(Duration::from_millis(100));
        assert!(ASKING.load(Ordering::Relaxed), "egui hasn't taken it: stuck inside egui");
        tx.send(()).unwrap();
        holder.join().unwrap();
        let start = Instant::now();
        while ASKING.load(Ordering::Relaxed) && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!ASKING.load(Ordering::Relaxed), "taken once egui was free");
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
