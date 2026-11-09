//! Finding Node.js and installing npm packages.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use serde_json::json;

pub const MIN_NODE_MAJOR: u32 = 22;

/// Hides the console window a child process would open on Windows.
pub fn quiet(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// The Node.js chosen in setup or Settings; `None` uses `node` from the PATH.
static NODE_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Chooses the Node.js that bots and npm run with.
pub fn set_node_path(path: Option<PathBuf>) {
    *NODE_PATH.write().unwrap_or_else(|e| e.into_inner()) = path;
}

pub fn node_path() -> Option<PathBuf> {
    NODE_PATH.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// A command running the chosen Node.js.
pub fn node_command() -> Command {
    let mut cmd = Command::new(node_path().unwrap_or_else(|| PathBuf::from("node")));
    with_node_on_path(&mut cmd);
    cmd
}

/// Puts the chosen Node.js's folder first on a child's PATH, so npm and the
/// packages it runs use the same Node.js.
fn with_node_on_path(cmd: &mut Command) {
    let Some(dir) = node_path().and_then(|p| p.parent().map(Path::to_path_buf)) else { return };
    let mut paths = vec![dir];
    if let Some(old) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&old));
    }
    if let Ok(joined) = std::env::join_paths(paths) {
        cmd.env("PATH", joined);
    }
}

/// The major version in `v22.4.1`.
pub fn major_version(version: &str) -> u32 {
    version.trim().trim_start_matches('v').split('.').next().and_then(|m| m.parse().ok()).unwrap_or(0)
}

/// The version a given `node` executable reports.
pub fn version_of(path: &Path) -> Result<String> {
    let out = quiet(Command::new(path).arg("--version")).output().with_context(|| format!("could not start {}", path.display()))?;
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || !version.starts_with('v') {
        bail!("{} doesn't look like Node.js", path.display());
    }
    Ok(version)
}

/// How often [`check_node_cached`] asks Node.js again.
const RECHECK_EVERY: Duration = Duration::from_secs(3);

/// What [`check_node_cached`] knows: the last answer for the Node.js chosen,
/// and whether a check is under way.
struct NodeCheck {
    path: Option<PathBuf>,
    /// The last answer, and when it came.
    last: Option<(Instant, std::result::Result<String, String>)>,
    running: bool,
    /// Raised when the answer being worked out no longer counts (another
    /// Node.js was chosen, or [`forget_node_check`]).
    generation: u64,
}

impl NodeCheck {
    const fn new() -> NodeCheck {
        NodeCheck { path: None, last: None, running: false, generation: 0 }
    }
}

static NODE_CHECK: Mutex<NodeCheck> = Mutex::new(NodeCheck::new());

/// `check_node`'s last answer, without waiting for it: safe to call while
/// drawing. The check runs on a thread of its own, again every few seconds
/// (or at once when the chosen Node.js changes); `None` until its first
/// answer. Starting `node` can take long (the first start after installing
/// it, a virus scanner looking at it, a slow virtual machine), and the
/// window must not stop answering meanwhile.
pub fn check_node_cached() -> Option<std::result::Result<String, String>> {
    cached_check(&NODE_CHECK, node_path(), || check_node().map_err(|e| format!("{e:#}")))
}

/// Forgets the last answer, so [`check_node_cached`] asks again now (Check
/// again in Settings).
pub fn forget_node_check() {
    forget_check(&NODE_CHECK);
}

/// [`check_node_cached`]'s workings, with the state and the check given.
fn cached_check(
    state: &'static Mutex<NodeCheck>,
    path: Option<PathBuf>,
    check: impl FnOnce() -> std::result::Result<String, String> + Send + 'static,
) -> Option<std::result::Result<String, String>> {
    let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
    if s.path != path {
        s.path = path;
        s.last = None;
        s.generation += 1;
    }
    let stale = s.last.as_ref().is_none_or(|(at, _)| at.elapsed() >= RECHECK_EVERY);
    if stale && !s.running {
        s.running = true;
        let generation = s.generation;
        let spawned = std::thread::Builder::new().name("node-check".into()).spawn(move || {
            let result = check();
            let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
            s.running = false;
            if s.generation == generation {
                s.last = Some((Instant::now(), result));
            }
        });
        if spawned.is_err() {
            s.running = false;
        }
    }
    s.last.as_ref().map(|(_, result)| result.clone())
}

fn forget_check(state: &Mutex<NodeCheck>) {
    let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
    s.last = None;
    s.generation += 1;
}

/// The chosen Node.js's version, checked against the minimum.
pub fn check_node() -> Result<String> {
    let out = quiet(node_command().arg("--version")).output().context(match node_path() {
        Some(p) => format!("The Node.js chosen in Settings ({}) couldn't be started. Choose another in Settings › Bot › Node.js.", p.display()),
        None => "Node.js was not found. Set it up in Settings › Bot › Node.js, or install Node.js 22 or newer from https://nodejs.org and restart XC Circuit.".to_string(),
    })?;
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if major_version(&version) < MIN_NODE_MAJOR {
        bail!("Node.js {version} is too old; bots need Node.js {MIN_NODE_MAJOR} or newer. Set up a newer one in Settings › Bot › Node.js.");
    }
    Ok(version)
}

/// Ends a process and the ones it started (npm runs node under a shell on Windows).
fn end_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        let ended = quiet(Command::new("taskkill").args(["/T", "/F", "/PID", &child.id().to_string()])).stdout(Stdio::null()).stderr(Stdio::null()).status();
        if ended.is_ok_and(|s| s.success()) {
            let _ = child.wait();
            return;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn npm() -> Command {
    // npm sits next to node. On Windows it is a .cmd script, which Command can
    // only start by its full name.
    let file = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let path = node_path().and_then(|p| p.parent().map(|d| d.join(file))).filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from(file));
    let mut cmd = Command::new(path);
    with_node_on_path(&mut cmd);
    cmd
}

/// Versions npm installs instead of the ones packages ask for (npm's
/// `overrides`), for packages bots use that pin a version with known holes:
/// - axios: @mithron/deezer-music-metadata (the Deezer music source) asks for
///   ^0.27, and every axios 0.x has SSRF and credential-leak advisories. It
///   only calls axios.get(url, options) and reads .data, which 1.x does alike.
pub const NPM_OVERRIDES: [(&str, &str); 1] = [("axios", "^1.21.0")];

/// The `overrides` of a package.json for [`NPM_OVERRIDES`].
pub fn npm_overrides() -> serde_json::Value {
    serde_json::Value::Object(NPM_OVERRIDES.iter().map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string()))).collect())
}

/// Makes sure `dir/node_modules` has the packages, running `npm install` when the
/// list changed or a package is missing. Output lines go to `log`. When
/// `cancel` turns true, npm is ended and this returns an error.
pub fn ensure_packages(dir: &Path, name: &str, deps: &IndexMap<String, String>, log: &mut dyn FnMut(String), cancel: &dyn Fn() -> bool) -> Result<()> {
    fs::create_dir_all(dir)?;
    let manifest = json!({ "name": name, "private": true, "dependencies": deps, "overrides": npm_overrides() });
    let text = serde_json::to_string_pretty(&manifest)? + "\n";
    let manifest_path = dir.join("package.json");
    if deps.is_empty() {
        fs::write(&manifest_path, &text)?;
        return Ok(());
    }
    let unchanged = fs::read_to_string(&manifest_path).is_ok_and(|old| old == text);
    let all_present = deps.keys().all(|d| dir.join("node_modules").join(d).join("package.json").is_file());
    if unchanged && all_present {
        return Ok(());
    }
    fs::write(&manifest_path, &text)?;
    log(format!("Installing {} …", deps.keys().cloned().collect::<Vec<_>>().join(", ")));
    let mut child = quiet(npm().args(["install", "--no-audit", "--no-fund", "--loglevel=error"]).current_dir(dir))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("npm was not found. It comes with Node.js: reinstall Node.js from https://nodejs.org.")?;
    // Output is read on its own threads, so npm can be ended while it runs.
    let (tx, rx) = std::sync::mpsc::channel::<(bool, String)>();
    let out = child.stdout.take().map(|o| Box::new(o) as Box<dyn std::io::Read + Send>);
    let err = child.stderr.take().map(|e| Box::new(e) as Box<dyn std::io::Read + Send>);
    for (is_err, stream) in [(false, out), (true, err)] {
        let Some(stream) = stream else { continue };
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(|l| l.ok()).filter(|l| !l.trim().is_empty()) {
                let _ = tx.send((is_err, line));
            }
        });
    }
    drop(tx);
    let mut errors = Vec::new();
    let status = loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok((false, line)) => log(line),
            Ok((true, line)) => errors.push(line),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            // Both streams closed: npm is done.
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break child.wait()?,
        }
        if cancel() {
            end_tree(&mut child);
            let _ = fs::remove_file(&manifest_path);
            bail!("Installing packages was stopped");
        }
    };
    if !status.success() {
        let _ = fs::remove_file(&manifest_path);
        bail!("npm install failed:\n{}", errors.join("\n"));
    }
    log("Packages installed".into());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stopping while npm installs ends it right away.
    #[test]
    fn an_install_can_be_stopped() {
        if check_node().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("xc-npm-stop-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let local = dir.join("local-package");
        fs::create_dir_all(&local).unwrap();
        fs::write(local.join("package.json"), r#"{ "name": "local-package", "version": "1.0.0" }"#).unwrap();
        let deps: IndexMap<String, String> = [("local-package".to_string(), format!("file:{}", local.display()))].into();
        let started = Instant::now();
        let result = ensure_packages(&dir.join("install"), "stop-test", &deps, &mut |_| {}, &|| true);
        assert!(result.unwrap_err().to_string().contains("stopped"));
        assert!(started.elapsed() < Duration::from_secs(5), "it didn't wait for npm");
        assert!(!dir.join("install/package.json").exists(), "the next start installs again");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Waits until `state` has an answer for `path` (no new check is started meanwhile).
    fn answer(state: &'static Mutex<NodeCheck>, path: Option<PathBuf>) -> std::result::Result<String, String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Some(result) = cached_check(state, path.clone(), || panic!("checked again")) {
                return result;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("no answer");
    }

    /// Asking whether Node.js works never waits for it (the tour asks while
    /// drawing): a slow answer comes later, from a thread of its own. Another
    /// Node.js chosen meanwhile, or Check again, throws the old answer away.
    #[test]
    fn checking_node_never_waits() {
        static STATE: Mutex<NodeCheck> = Mutex::new(NodeCheck::new());
        let slow = |answer: std::result::Result<String, String>| {
            move || {
                std::thread::sleep(Duration::from_millis(300));
                answer
            }
        };
        let started = Instant::now();
        assert_eq!(cached_check(&STATE, None, slow(Ok("v22.1.0".into()))), None, "not known yet");
        assert!(started.elapsed() < Duration::from_millis(200), "it didn't wait");
        assert_eq!(cached_check(&STATE, None, || panic!("one check at a time")), None);
        assert_eq!(answer(&STATE, None), Ok("v22.1.0".into()));

        // Another Node.js: the old answer doesn't count for it.
        let other = Some(PathBuf::from("/somewhere/else/node"));
        assert_eq!(cached_check(&STATE, other.clone(), slow(Err("too old".into()))), None);
        assert_eq!(answer(&STATE, other.clone()), Err("too old".into()));

        // Check again: asked anew.
        forget_check(&STATE);
        assert_eq!(cached_check(&STATE, other.clone(), slow(Ok("v24.0.0".into()))), None);
        assert_eq!(answer(&STATE, other), Ok("v24.0.0".into()));
    }
}
