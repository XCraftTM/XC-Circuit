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
//!
//! A freeze shows in the Console once the app answers again. One it never
//! comes back from (closed by hand, or ended by Windows) is said at the next
//! start, from the log. After ten seconds stuck, the log says so again and,
//! on Windows, a dump of the app (every thread's stack: exactly where it's
//! stuck) is kept in `freezes/` next to the log, the last three.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// After the frame: what's left is drawing on the graphics card, and Windows.
pub const AFTER_FRAME: &str = "after its frame: drawing on the graphics card, or Windows and tools reading the window (accessibility, input methods)";

/// How long without a frame (with one asked for) counts as stuck.
const STUCK_MS: u64 = 3000;
/// How long without a frame before one is asked for.
const ASK_AFTER_MS: u64 = 1000;
/// How long stuck before the log says so again, with a dump on Windows.
const DUMP_AFTER_MS: u64 = 10_000;
/// Dumps kept, the newest.
const DUMPS_KEPT: usize = 3;

static CLOCK: OnceLock<Instant> = OnceLock::new();
static BEAT: AtomicU64 = AtomicU64::new(0);
static FOCUSED: AtomicBool = AtomicBool::new(false);
/// The app is closing: it no longer draws (it may wait for a save to be
/// written, or the bot to stop), which isn't a freeze.
static CLOSING: AtomicBool = AtomicBool::new(false);
/// The main window (Windows: its handle), to ask whether it takes messages.
static WINDOW: AtomicIsize = AtomicIsize::new(0);
/// A frame is being asked for and egui hasn't taken the request yet: its
/// lock is held, so the app is stuck inside egui.
static ASKING: AtomicBool = AtomicBool::new(false);
/// Said after the phase when the app was stuck inside egui.
const INSIDE_EGUI: &str = "(inside egui, which was busy: laying out a long text, say, or the window's accessibility tree)";
static PHASE: Mutex<String> = Mutex::new(String::new());
static FOUND: Mutex<Vec<Freeze>> = Mutex::new(Vec::new());
/// What this start runs on, said in the log before its first freeze.
static ABOUT: Mutex<String> = Mutex::new(String::new());
/// Where the run before this one was stuck when it ended, for the Console.
static ENDED_STUCK: Mutex<Option<String>> = Mutex::new(None);

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

/// What draws the app (`graphics`, and whether in software), for the log:
/// with the build (x86_64 or aarch64) and, on Windows, the processor, an
/// x86_64 build that Windows on ARM runs translated shows.
pub fn about(graphics: &str, software: bool) {
    if let Ok(mut a) = ABOUT.lock() {
        *a = describe_start(graphics, software, std::env::consts::ARCH, std::env::var("PROCESSOR_IDENTIFIER").ok().as_deref());
    }
}

fn describe_start(graphics: &str, software: bool, build: &str, processor: Option<&str>) -> String {
    let mut text = format!("{build} build, graphics: {graphics}");
    if software {
        text.push_str(" (in software)");
    }
    if let Some(p) = processor.map(str::trim).filter(|p| !p.is_empty()) {
        text.push_str(&format!(", processor: {p}"));
    }
    text
}

/// Whether the window has the focus: only then is a missing frame a freeze
/// (a minimized or hidden window may get none).
pub fn focused(on: bool) {
    FOCUSED.store(on && !CLOSING.load(Ordering::Relaxed), Ordering::Relaxed);
}

/// The app is closing: from now on a missing frame isn't a freeze, so the
/// next start doesn't say the run before ended stuck.
pub fn closing() {
    CLOSING.store(true, Ordering::Relaxed);
    FOCUSED.store(false, Ordering::Relaxed);
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

/// Where the run before this one was stuck when it ended (it was closed, or
/// ended, while not answering), once; for the Console.
pub fn take_ended_stuck() -> Option<String> {
    ENDED_STUCK.lock().ok()?.take()
}

/// The place a log's last freeze was stuck in, when it never answered again:
/// the log ends with its "not answering" line (and maybe "still not answering").
fn ended_stuck(log: &str) -> Option<String> {
    let last = log.lines().rev().map(str::trim).filter(|l| !l.is_empty()).find(|l| !l.contains(" · still not answering after "))?;
    let (_, rest) = last.split_once(" · not answering for ")?;
    Some(rest.split_once("stuck in: ").map(|(_, p)| p.trim().to_string()).unwrap_or_default())
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
    if let Some(phase) = note_ended_stuck(&log)
        && let Ok(mut e) = ENDED_STUCK.lock()
    {
        *e = Some(phase);
    }
    std::thread::Builder::new()
        .name("freeze-detector".into())
        .spawn(move || {
            let mut watch = Watch::default();
            let mut phase_then = String::new();
            let mut said_about = false;
            // This freeze was said again after ten seconds (and dumped).
            let mut said_long = false;
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
                        // Once a start: what it runs on, which a freeze often depends on.
                        if !std::mem::replace(&mut said_about, true) {
                            let about = ABOUT.lock().map(|a| a.clone()).unwrap_or_default();
                            if !about.is_empty() {
                                write_line(&log, &format!("this start: {about}"));
                            }
                        }
                        write_line(&log, &format!("not answering for {:.1} s, stuck in: {phase_then}", ms as f64 / 1000.0));
                    }
                    Step::Back { ms } => {
                        write_line(&log, &format!("answered again after {:.1} s, stuck in: {phase_then}", ms as f64 / 1000.0));
                        if let Ok(mut f) = FOUND.lock() {
                            f.push(Freeze { phase: std::mem::take(&mut phase_then), seconds: ms as f64 / 1000.0 });
                        }
                        said_long = false;
                        ask_for_frame(&ctx);
                    }
                }
                // Stuck for long: said again, with where exactly (a dump on Windows).
                if let Some(since) = watch.stuck
                    && !said_long
                    && now.saturating_sub(since) >= DUMP_AFTER_MS
                {
                    said_long = true;
                    let seconds = now.saturating_sub(since) / 1000;
                    let text = match log.parent().map(|d| keep_dump(&d.join("freezes"))) {
                        Some(Ok(path)) => format!("still not answering after {seconds} s; a dump of where it's stuck: {}", path.display()),
                        Some(Err(e)) if cfg!(windows) => format!("still not answering after {seconds} s; no dump of where it's stuck: {e}"),
                        _ => format!("still not answering after {seconds} s"),
                    };
                    write_line(&log, &text);
                }
            }
        })
        .ok();
}

/// Where the run before was stuck, if it ended while not answering (from
/// the log at `log`); noted in the log, so the next start doesn't say it again.
fn note_ended_stuck(log: &Path) -> Option<String> {
    let phase = ended_stuck(&std::fs::read_to_string(log).ok()?)?;
    write_line(log, "the run before ended while it wasn't answering (closed by hand, or ended by Windows)");
    Some(phase)
}

/// Writes a dump of the app into `dir` (as freeze-<time>.dmp), keeping the
/// newest [`DUMPS_KEPT`]: every thread's stack, so it shows exactly where
/// the app is stuck. Open it in Visual Studio or WinDbg. Windows only.
fn keep_dump(dir: &Path) -> Result<PathBuf, String> {
    if !cfg!(windows) {
        return Err("dumps are only kept on Windows".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut old: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("freeze-") && n.ends_with(".dmp")))
        .collect();
    // The names hold the time, so they sort oldest first.
    old.sort();
    for p in old.iter().take((old.len() + 1).saturating_sub(DUMPS_KEPT)) {
        let _ = std::fs::remove_file(p);
    }
    let path = dir.join(format!("freeze-{}.dmp", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    write_dump(&path)?;
    Ok(path)
}

/// `xc-circuit --dump-process <pid> <file>`: the helper that writes a dump.
pub const DUMP_ARG: &str = "--dump-process";

/// A dump of this process, written by a helper process ([`DUMP_ARG`]): as
/// Windows advises, since a process that's stuck can also get its own dump
/// stuck (holding a lock the dump needs), and the dump pauses every thread.
#[cfg(windows)]
fn write_dump(path: &Path) -> Result<(), String> {
    /// How long the helper may take before it's ended.
    const DUMP_WITHIN: Duration = Duration::from_secs(60);
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut helper = std::process::Command::new(exe).arg(DUMP_ARG).arg(std::process::id().to_string()).arg(path).spawn().map_err(|e| e.to_string())?;
    let started = Instant::now();
    loop {
        match helper.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("the dump helper ended with {status}")),
            Ok(None) if started.elapsed() > DUMP_WITHIN => {
                let _ = helper.kill();
                let _ = std::fs::remove_file(path);
                return Err("the dump took over a minute".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(not(windows))]
fn write_dump(_path: &Path) -> Result<(), String> {
    Err("dumps are only kept on Windows".into())
}

/// The helper ([`DUMP_ARG`]): `args` are the process id and the file. The
/// exit code says whether it worked; why not goes to stderr.
pub fn dump_process(args: &[String]) -> i32 {
    let (Some(pid), Some(path)) = (args.first().and_then(|p| p.parse::<u32>().ok()), args.get(1)) else {
        eprintln!("usage: xc-circuit {DUMP_ARG} <process id> <file>");
        return 2;
    };
    match dump_of(pid, Path::new(path)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// Writes a dump of process `pid` (its threads' stacks and loaded modules) to `path`.
#[cfg(windows)]
fn dump_of(pid: u32, path: &Path) -> Result<(), String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::Debug::{MiniDumpWithThreadInfo, MiniDumpWithUnloadedModules, MiniDumpWriteDump};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
    // SAFETY: plain arguments; a null handle means it couldn't be opened.
    let process = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid) };
    if process.is_null() {
        return Err(format!("couldn't open process {pid}: {}", std::io::Error::last_os_error()));
    }
    let result = std::fs::File::create(path).map_err(|e| e.to_string()).and_then(|file| {
        // SAFETY: an open process handle and a file open for writing; no
        // exception, extra streams or callback (null is allowed for each).
        let written = unsafe { MiniDumpWriteDump(process, pid, file.as_raw_handle(), MiniDumpWithThreadInfo | MiniDumpWithUnloadedModules, std::ptr::null(), std::ptr::null(), std::ptr::null()) };
        if written == 0 { Err(std::io::Error::last_os_error().to_string()) } else { Ok(()) }
    });
    // SAFETY: the handle opened above, closed once.
    unsafe { CloseHandle(process) };
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}

#[cfg(not(windows))]
fn dump_of(_pid: u32, _path: &Path) -> Result<(), String> {
    Err("dumps are only made on Windows".into())
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

    /// The log says once a start what it runs on: an x86_64 build drawing in
    /// software on an ARM processor (Windows on ARM translating it) shows.
    #[test]
    fn a_start_says_what_it_runs_on() {
        assert_eq!(
            describe_start("DirectX 12 · Microsoft Basic Render Driver", true, "x86_64", Some("ARMv8 (64-bit) Family 8 ")),
            "x86_64 build, graphics: DirectX 12 · Microsoft Basic Render Driver (in software), processor: ARMv8 (64-bit) Family 8"
        );
        assert_eq!(describe_start("Vulkan · AMD Radeon", false, "x86_64", None), "x86_64 build, graphics: Vulkan · AMD Radeon");
    }

    /// A log ending with "not answering" (and maybe "still not answering")
    /// is from a run that ended stuck, which the next start says. One that
    /// answered again, or was said already, isn't.
    #[test]
    fn a_run_that_ended_stuck_is_said_at_the_next_start() {
        let stuck = "2026-11-08 23:14:20 · XC Circuit 0.2.5 · not answering for 3.2 s, stuck in: after its frame: drawing\n";
        assert_eq!(ended_stuck(stuck).as_deref(), Some("after its frame: drawing"));
        let long = format!("{stuck}2026-11-08 23:14:27 · XC Circuit 0.2.5 · still not answering after 10 s; a dump of where it's stuck: C:\\freeze.dmp\n\n");
        assert_eq!(ended_stuck(&long).as_deref(), Some("after its frame: drawing"));
        let back = format!("{stuck}2026-11-08 23:14:30 · XC Circuit 0.2.5 · answered again after 10.0 s, stuck in: after its frame\n");
        assert_eq!(ended_stuck(&back), None);
        assert_eq!(ended_stuck(""), None);

        // Said once: the start notes it in the log.
        let dir = std::env::temp_dir().join(format!("xc-ended-stuck-{}", std::process::id()));
        let log = dir.join("freezes.log");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&log, &long).unwrap();
        assert_eq!(note_ended_stuck(&log).as_deref(), Some("after its frame: drawing"));
        assert_eq!(note_ended_stuck(&log), None, "not again at the next start");
        assert!(std::fs::read_to_string(&log).unwrap().contains("still not answering"), "the log is kept");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Dumps are only kept on Windows; elsewhere the log just says the app is still stuck.
    #[test]
    #[cfg(not(windows))]
    fn no_dumps_elsewhere() {
        let dir = std::env::temp_dir().join(format!("xc-no-dumps-{}", std::process::id()));
        assert!(keep_dump(&dir).is_err());
        assert!(!dir.exists(), "nothing made");
        // The helper: its arguments are checked, and it says it can't here.
        assert_eq!(dump_process(&[]), 2, "no process id or file");
        assert_eq!(dump_process(&["not a number".into(), "x.dmp".into()]), 2);
        assert_eq!(dump_process(&[std::process::id().to_string(), dir.join("x.dmp").display().to_string()]), 1);
    }

    /// Closing, the app no longer draws (waiting for a save, or the bot):
    /// not a freeze, whatever the window said last.
    #[test]
    fn closing_is_not_a_freeze() {
        focused(true);
        closing();
        focused(true);
        assert!(!FOCUSED.load(Ordering::Relaxed));
        let mut w = Watch::default();
        assert_eq!(w.look(9000, 0, FOCUSED.load(Ordering::Relaxed), || Some(false)), Step::Nothing);
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
