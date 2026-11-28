//! Crash logs and bug reports. When XC Circuit panics, what happened goes
//! into a file in the app's data folder (`crash-logs/`); the next start
//! offers to report it on GitHub with the details filled in. Help › Report a
//! Bug opens the same form any time.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Shown next to the version while the app isn't finished.
pub const CHANNEL: &str = "Early Access";
pub const REPO: &str = "https://github.com/XCraftTM/XC-Circuit";
/// Crash logs kept; older ones are deleted.
const KEPT: usize = 20;

thread_local! {
    /// A plugin's script is being served on this thread: its panics are
    /// caught and reported in the Console, so they aren't crashes of the app.
    static IN_PLUGIN: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f` (a call into a plugin), marking its panics as caught ones.
pub fn plugin_call<R>(f: impl FnOnce() -> R) -> R {
    let before = IN_PLUGIN.with(|c| c.replace(true));
    let r = f();
    IN_PLUGIN.with(|c| c.set(before));
    r
}

pub fn logs_dir() -> PathBuf {
    crate::bot::app_data_dir().join("crash-logs")
}

/// "0.1.0 (Early Access) · windows x86_64"
pub fn version_line() -> String {
    format!("{} ({CHANNEL}) · {} {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH)
}

/// Writes a crash log for every panic that isn't caught, keeping the usual
/// message on the console too.
pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        if IN_PLUGIN.with(Cell::get) {
            return;
        }
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "a panic without a message".into());
        let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        // The graphics card's device gone (a driver reset, say): not a
        // mistake in the app, and said so at the next start.
        let lost = GRAPHICS_LOST.try_lock().ok().and_then(|g| g.clone());
        let graphics_failed = lost.is_some() || is_graphics_failure(&message);
        let (about, doing) = crate::app::watchdog::for_crash_log();
        let lost_line = if graphics_failed { format!("{LOST_LINE}{}\n", lost.as_deref().unwrap_or("(as the panic says)")) } else { String::new() };
        let text = format!(
            "XC Circuit {}\nThread: {thread}\nPanic: {message}\nAt: {location}\nRunning on: {about}\n{DOING_LINE}{doing}\n{lost_line}\nBacktrace:\n{}\n",
            version_line(),
            std::backtrace::Backtrace::force_capture()
        );
        let _ = write_log(&logs_dir(), &text);
        if graphics_failed {
            start_again();
        }
    }));
}

/// Set when wgpu says the graphics card's device is lost: why.
static GRAPHICS_LOST: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// The graphics device was lost (wgpu's device-lost callback): the crash
/// that follows (the next frame can't be drawn) is said to be that.
pub fn graphics_lost(why: String) {
    if let Ok(mut g) = GRAPHICS_LOST.lock() {
        g.get_or_insert(why);
    }
}

/// How egui-wgpu and wgpu stop when the graphics device is gone (staging
/// memory can't be had from a lost device either): the driver was reset (Windows does that to a driver that
/// hangs), updated, or the card changed (a laptop's, after sleep, Remote
/// Desktop). Also known when the device-lost callback came too late.
fn is_graphics_failure(panic: &str) -> bool {
    let p = panic.to_lowercase();
    ["failed to create staging buffer", "device lost", "device is lost", "devicelost", "parent device is lost"].iter().any(|s| p.contains(s))
}

const LOST_LINE: &str = "Graphics device lost: ";
const DOING_LINE: &str = "Doing: ";
/// How often the app started again by itself after losing its graphics in
/// a row: twice at most, so a card that keeps failing doesn't loop.
pub const RESTARTS_VAR: &str = "XC_GRAPHICS_RESTARTS";

/// After losing the graphics device: the app starts again by itself (its
/// unsaved changes are offered back, from crash recovery), unless it did so
/// twice in a row already or measures frame rates.
fn start_again() {
    if std::env::var_os("XC_FPS_OUT").is_some() {
        return;
    }
    let tries: u32 = std::env::var(RESTARTS_VAR).ok().and_then(|t| t.parse().ok()).unwrap_or(0);
    if tries >= 2 {
        return;
    }
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).args(std::env::args_os().skip(1)).env(RESTARTS_VAR, (tries + 1).to_string()).spawn();
    }
}

/// Whether this start is one the app made by itself after losing its graphics.
pub fn started_again() -> bool {
    std::env::var_os(RESTARTS_VAR).is_some()
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn write_log(dir: &Path, text: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("crash-{}.txt", now_secs()));
    std::fs::write(&path, text)?;
    // Only the newest are kept.
    let mut logs = list(dir);
    while logs.len() > KEPT {
        let (_, old) = logs.remove(0);
        let _ = std::fs::remove_file(old);
    }
    Ok(path)
}

/// Crash logs, oldest first, with when they were written (seconds).
fn list(dir: &Path) -> Vec<(u64, PathBuf)> {
    let mut out: Vec<(u64, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let secs = name.strip_prefix("crash-")?.strip_suffix(".txt")?.parse().ok()?;
            Some((secs, e.path()))
        })
        .collect();
    out.sort();
    out
}

/// A crash newer than `seen` (when the user last saw one): its time, file and
/// the panic's message.
#[derive(Clone, Debug)]
pub struct Crash {
    pub at: u64,
    pub path: PathBuf,
    pub panic: String,
    /// The graphics device was lost: why (wgpu's words, or the panic's).
    pub graphics: Option<String>,
}

pub fn newest_since(seen: u64) -> Option<Crash> {
    newest_in(&logs_dir(), seen)
}

fn newest_in(dir: &Path, seen: u64) -> Option<Crash> {
    let (at, path) = list(dir).into_iter().rfind(|(at, _)| *at > seen)?;
    let text = std::fs::read_to_string(&path).ok()?;
    let line = |p: &str| text.lines().find_map(|l| l.strip_prefix(p)).unwrap_or("").to_string();
    let panic = format!("{} (at {})", line("Panic: "), line("At: "));
    let graphics = text.lines().find_map(|l| l.strip_prefix(LOST_LINE)).map(str::to_string);
    Some(Crash { at, path, panic, graphics })
}

/// What a report is about.
pub enum Report<'a> {
    Bug,
    Feature,
    /// A crash: its message goes into the form.
    Crash(&'a Crash),
}

/// GitHub's form for a new issue, with what we know filled in.
pub fn report_url(kind: Report) -> String {
    let enc = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    let version = enc(&version_line());
    match kind {
        Report::Bug => format!("{REPO}/issues/new?template=bug_report.yml&version={version}"),
        Report::Feature => format!("{REPO}/issues/new?template=feature_request.yml"),
        Report::Crash(c) => {
            let title = enc(&format!("Crash: {}", c.panic.chars().take(80).collect::<String>()));
            let logs = enc(&format!("{}\n\n(The full crash log is in {}; please attach it.)", c.panic, c.path.display()));
            format!("{REPO}/issues/new?template=bug_report.yml&title={title}&version={version}&logs={logs}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_logs_are_written_found_and_reported() {
        let dir = std::env::temp_dir().join(format!("xc-crash-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(newest_in(&dir, 0).is_none());
        let text = "XC Circuit 0.1.0\nThread: main\nPanic: index out of bounds\nAt: src/canvas.rs:12\n\nBacktrace:\n…";
        write_log(&dir, text).unwrap();
        let c = newest_in(&dir, 0).unwrap();
        assert_eq!(c.panic, "index out of bounds (at src/canvas.rs:12)");
        assert!(newest_in(&dir, c.at).is_none(), "seen crashes aren't offered again");
        let url = report_url(Report::Crash(&c));
        assert!(url.starts_with("https://github.com/XCraftTM/XC-Circuit/issues/new?template=bug_report.yml&title=Crash%3A%20index"), "{url}");
        assert!(url.contains("&version=") && url.contains("&logs="));
        // Only the newest are kept.
        for i in 0..(KEPT + 5) {
            std::fs::write(dir.join(format!("crash-{}.txt", 10 + i)), "x").unwrap();
        }
        write_log(&dir, text).unwrap();
        assert_eq!(list(&dir).len(), KEPT);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// egui-wgpu's panic after the graphics device was lost (as a tester
    /// got it) is told apart from the app's own mistakes, and the next start
    /// reads it back from the log.
    #[test]
    fn a_lost_graphics_device_is_told_apart() {
        let lost = "Failed to create staging buffer for index data. Index count: 64398. Required index buffer size: 257592. Actual size 390636 and capacity: 390636 (bytes)";
        assert!(is_graphics_failure(lost));
        assert!(is_graphics_failure("Validation Error: Parent device is lost"));
        for own in ["index out of bounds: the len is 3 but the index is 7", "called `Option::unwrap()` on a `None` value", "a panic without a message"] {
            assert!(!is_graphics_failure(own), "{own}");
        }
        let dir = std::env::temp_dir().join(format!("xc-crash-graphics-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_log(&dir, &format!("XC Circuit 0.3.2\nThread: main\nPanic: {lost}\nAt: renderer.rs:1033\nRunning on: x86_64 build, graphics: Vulkan · Some Card\n{DOING_LINE}after its frame\n{LOST_LINE}Unknown: Device lost\n\nBacktrace:\n…")).unwrap();
        let c = newest_in(&dir, 0).unwrap();
        assert_eq!(c.graphics.as_deref(), Some("Unknown: Device lost"));
        let _ = std::fs::remove_dir_all(&dir);
        write_log(&dir, "XC Circuit 0.3.2\nThread: main\nPanic: index out of bounds\nAt: x.rs:1\n").unwrap();
        assert_eq!(newest_in(&dir, 0).unwrap().graphics, None, "the app's own crash is reported as before");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn panics_in_plugins_are_not_crashes() {
        assert!(!IN_PLUGIN.with(Cell::get));
        plugin_call(|| assert!(IN_PLUGIN.with(Cell::get)));
        assert!(!IN_PLUGIN.with(Cell::get));
    }
}
