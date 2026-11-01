//! Finding Node.js and installing npm packages.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

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

/// The installed Node.js version, checked against the minimum.
pub fn check_node() -> Result<String> {
    let out = quiet(Command::new("node").arg("--version"))
        .output()
        .context("Node.js was not found. Install Node.js 22 or newer from https://nodejs.org and restart XC Circuit.")?;
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let major: u32 = version.trim_start_matches('v').split('.').next().and_then(|m| m.parse().ok()).unwrap_or(0);
    if major < MIN_NODE_MAJOR {
        bail!("Node.js {version} is too old; bots need Node.js {MIN_NODE_MAJOR} or newer (https://nodejs.org).");
    }
    Ok(version)
}

fn npm() -> Command {
    // On Windows npm is a .cmd script, which Command can only start by its full name.
    Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" })
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
