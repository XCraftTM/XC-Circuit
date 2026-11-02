//! Finding Node.js and installing npm packages.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::RwLock;

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

fn npm() -> Command {
    // npm sits next to node. On Windows it is a .cmd script, which Command can
    // only start by its full name.
    let file = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let path = node_path().and_then(|p| p.parent().map(|d| d.join(file))).filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from(file));
    let mut cmd = Command::new(path);
    with_node_on_path(&mut cmd);
    cmd
}

/// Makes sure `dir/node_modules` has the packages, running `npm install` when the
/// list changed or a package is missing. Output lines go to `log`.
pub fn ensure_packages(dir: &Path, name: &str, deps: &IndexMap<String, String>, log: &mut dyn FnMut(String)) -> Result<()> {
    fs::create_dir_all(dir)?;
    let manifest = json!({ "name": name, "private": true, "dependencies": deps });
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
    let stderr = child.stderr.take();
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(|l| l.ok()).filter(|l| !l.trim().is_empty()) {
            log(line);
        }
    }
    let errors: Vec<String> = stderr
        .map(|e| BufReader::new(e).lines().map_while(|l| l.ok()).filter(|l| !l.trim().is_empty()).collect())
        .unwrap_or_default();
    let status = child.wait()?;
    if !status.success() {
        let _ = fs::remove_file(&manifest_path);
        bail!("npm install failed:\n{}", errors.join("\n"));
    }
    log("Packages installed".into());
    Ok(())
}
