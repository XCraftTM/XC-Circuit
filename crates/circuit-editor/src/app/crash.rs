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
    /// A native plugin is running on this thread: its panics are caught and
    /// reported in the Console, so they aren't crashes of the app.
    static IN_PLUGIN: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f` (a call into a native plugin), marking its panics as caught ones.
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
        let text = format!(
            "XC Circuit {}\nThread: {thread}\nPanic: {message}\nAt: {location}\n\nBacktrace:\n{}\n",
            version_line(),
            std::backtrace::Backtrace::force_capture()
        );
        let _ = write_log(&logs_dir(), &text);
    }));
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
}

pub fn newest_since(seen: u64) -> Option<Crash> {
    newest_in(&logs_dir(), seen)
}

fn newest_in(dir: &Path, seen: u64) -> Option<Crash> {
    let (at, path) = list(dir).into_iter().rfind(|(at, _)| *at > seen)?;
    let text = std::fs::read_to_string(&path).ok()?;
    let line = |p: &str| text.lines().find_map(|l| l.strip_prefix(p)).unwrap_or("").to_string();
    let panic = format!("{} (at {})", line("Panic: "), line("At: "));
    Some(Crash { at, path, panic })
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

    #[test]
    fn panics_in_plugins_are_not_crashes() {
        assert!(!IN_PLUGIN.with(Cell::get));
        plugin_call(|| assert!(IN_PLUGIN.with(Cell::get)));
        assert!(!IN_PLUGIN.with(Cell::get));
    }
}
