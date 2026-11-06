//! Exports a project as a folder that runs with `node index.js`.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::RUNTIME_SOURCE;
use crate::bundle::Bundle;
use crate::node::ensure_packages;

pub struct ExportOptions {
    pub name: String,
    /// Written to `.env` (as `token_env`) when given.
    pub token: Option<String>,
    /// The variable the bot reads its token from: the mode's, such as `DISCORD_TOKEN`.
    pub token_env: String,
    /// The build mode's other secrets (music logins …), each a line in `.env`.
    pub secrets: Vec<ExportSecret>,
    /// Run `npm install` in the export folder.
    pub install: bool,
}

/// A secret setting in the exported `.env`.
pub struct ExportSecret {
    /// The variable the bot reads it from, such as `XC_SECRET_YOUTUBECOOKIE`.
    pub env: String,
    /// What it is, as a comment above it.
    pub label: String,
    /// Its value, when it is exported.
    pub value: Option<String>,
}

/// In the first lines of an exported `index.js`, so an earlier export is
/// recognised and may be replaced.
const MARKER: &str = "exported from XC Circuit";

/// A folder name made from a bot's name: letters, digits, `-` and `_` only, so
/// a name can't point the export somewhere else (`..`, `/`, `C:`).
pub fn folder_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    format!("{}-bot", if s.is_empty() { "my" } else { s.as_str() })
}

/// Refuses folders holding other things: only an empty folder, or an earlier
/// export of a bot, is written into.
fn check_target(dir: &Path) -> Result<()> {
    let Ok(mut entries) = fs::read_dir(dir) else { return Ok(()) };
    if entries.next().is_none() {
        return Ok(());
    }
    let earlier = fs::read_to_string(dir.join("index.js")).is_ok_and(|t| t.lines().take(3).any(|l| l.contains(MARKER)));
    if earlier {
        return Ok(());
    }
    bail!("{} already has other files in it. Choose an empty folder, or one you exported a bot to before.", dir.display())
}

/// Writes `index.js` (runtime + project + blocks), `package.json`, `.env`,
/// `.gitignore` and `README.txt`.
pub fn export(dir: &Path, bundle: &Bundle, opts: &ExportOptions, log: &mut dyn FnMut(String)) -> Result<()> {
    check_target(dir)?;
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    // The name goes into a comment and a text file: one line, no control characters.
    let one_line: String = opts.name.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();

    let index = format!(
        "#!/usr/bin/env node\n// {name}: {MARKER}. Start it with: node index.js\n// The XC Circuit runtime in this file: Copyright (c) 2026 XCraftTM, all rights reserved.\n// You may run and share this bot as exported (XC Circuit licence, section 2).\n\nconst __XC_BUNDLE__ = {bundle};\n\n{runtime}",
        name = one_line,
        bundle = serde_json::to_string(&bundle.value)?,
        runtime = RUNTIME_SOURCE
    );
    fs::write(dir.join("index.js"), index)?;

    let package_name: String = opts
        .name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let package = json!({
        "name": if package_name.is_empty() { "bot".to_string() } else { package_name },
        "private": true,
        "main": "index.js",
        "scripts": { "start": "node index.js" },
        "engines": { "node": ">=22" },
        "dependencies": bundle.dependencies,
        "overrides": crate::node::npm_overrides(),
    });
    fs::write(dir.join("package.json"), serde_json::to_string_pretty(&package)? + "\n")?;

    let env = dir.join(".env");
    let existing = fs::read_to_string(&env).ok();
    fs::write(&env, env_file(existing.as_deref(), &opts.token_env, opts.token.as_deref(), &opts.secrets))?;
    private_file(&env)?;
    keep_ignored(&dir.join(".gitignore"), &[".env", "node_modules/", "data/"])?;
    fs::write(
        dir.join("README.txt"),
        format!(
            "{name}\n\nMade with XC Circuit.\n\n1. Install Node.js 22 or newer: https://nodejs.org\n2. In this folder, run: npm install   (once)\n3. Start the bot:      node index.js\n\nThe bot token is read from .env ({env}=...). Never share that file.\nSaved data is kept in the data/ folder.\n",
            name = one_line,
            env = opts.token_env
        ),
    )?;

    if opts.install {
        let name = package["name"].as_str().unwrap_or("bot");
        ensure_packages(dir, name, &bundle.dependencies, log, &|| false)?;
        // ensure_packages wrote a minimal package.json; put the full one back.
        fs::write(dir.join("package.json"), serde_json::to_string_pretty(&package)? + "\n")?;
    }
    Ok(())
}

/// The `.env` file: the token and every secret. A file from an earlier export
/// is kept as it is (with what the user filled in) when no token is written,
/// and only gets the secrets it doesn't list yet.
fn env_file(existing: Option<&str>, token_env: &str, token: Option<&str>, secrets: &[ExportSecret]) -> String {
    let clean = |v: &str| v.trim().replace(['\r', '\n'], "");
    let mut text = match (token, existing) {
        (Some(token), _) => format!("# Keep this file private: it holds your bot token.\n{token_env}={}\n", clean(token)),
        (None, Some(old)) => old.to_string(),
        (None, None) => format!("# Put your bot token here.\n{token_env}=\n"),
    };
    let keep_values = token.is_none() && existing.is_some();
    for s in secrets {
        let listed = text.lines().any(|l| l.trim_start().strip_prefix(&s.env).is_some_and(|rest| rest.trim_start().starts_with('=')));
        if keep_values && listed {
            continue;
        }
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&format!("# {}\n{}={}\n", s.label, s.env, s.value.as_deref().map(clean).unwrap_or_default()));
    }
    text
}

/// Makes sure `.gitignore` lists `lines`, keeping what the user added to it.
fn keep_ignored(path: &Path, lines: &[&str]) -> Result<()> {
    let mut text = fs::read_to_string(path).unwrap_or_default();
    let missing: Vec<&str> = lines.iter().copied().filter(|l| !text.lines().any(|t| t.trim() == *l)).collect();
    if missing.is_empty() {
        return Ok(());
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    for l in missing {
        text.push_str(l);
        text.push('\n');
    }
    fs::write(path, text)?;
    Ok(())
}

/// Only the owner may read a file holding a secret (Linux; on Windows files in
/// the user's folders are already private to them).
fn private_file(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).with_context(|| format!("protecting {}", path.display()))?;
    }
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use indexmap::IndexMap;

    use super::*;

    #[test]
    fn env_lists_every_secret() {
        let secrets = |v: Option<&str>| vec![ExportSecret { env: "XC_SECRET_YOUTUBECOOKIE".into(), label: "YouTube cookie".into(), value: v.map(String::from) }];
        assert_eq!(env_file(None, "DISCORD_TOKEN", None, &secrets(None)), "# Put your bot token here.\nDISCORD_TOKEN=\n# YouTube cookie\nXC_SECRET_YOUTUBECOOKIE=\n");
        assert_eq!(
            env_file(None, "DISCORD_TOKEN", Some("t"), &secrets(Some("c\n"))),
            "# Keep this file private: it holds your bot token.\nDISCORD_TOKEN=t\n# YouTube cookie\nXC_SECRET_YOUTUBECOOKIE=c\n"
        );
        // An earlier .env keeps what was filled in, and gets the missing lines.
        assert_eq!(env_file(Some("DISCORD_TOKEN=x\nXC_SECRET_YOUTUBECOOKIE = mine\n"), "DISCORD_TOKEN", None, &secrets(None)), "DISCORD_TOKEN=x\nXC_SECRET_YOUTUBECOOKIE = mine\n");
        assert_eq!(env_file(Some("DISCORD_TOKEN=x"), "DISCORD_TOKEN", None, &secrets(None)), "DISCORD_TOKEN=x\n# YouTube cookie\nXC_SECRET_YOUTUBECOOKIE=\n");
    }

    #[test]
    fn gitignore_keeps_added_lines() {
        let dir = std::env::temp_dir().join(format!("xc-ignore-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join(".gitignore");
        let _ = fs::remove_file(&file);
        keep_ignored(&file, &[".env", "data/"]).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), ".env\ndata/\n");
        fs::write(&file, "secrets.txt\n.env").unwrap();
        keep_ignored(&file, &[".env", "data/"]).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "secrets.txt\n.env\ndata/\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_folders_are_safe() {
        assert_eq!(folder_name("Demo Bot"), "Demo-Bot-bot");
        assert_eq!(folder_name("../../Windows/System32"), "Windows-System32-bot");
        assert_eq!(folder_name(r"C:\evil"), "C-evil-bot");
        assert_eq!(folder_name("..."), "my-bot");

        let dir = std::env::temp_dir().join(format!("xc-export-target-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("package.json"), "{\"name\": \"something-else\"}").unwrap();
        let bundle = Bundle { value: serde_json::json!({}), dependencies: IndexMap::new() };
        let opts = ExportOptions { name: "Name\nrequire('x')".into(), token: None, token_env: "DISCORD_TOKEN".into(), secrets: Vec::new(), install: false };
        assert!(export(&dir, &bundle, &opts, &mut |_| {}).is_err(), "a folder with other files is refused");
        assert_eq!(fs::read_to_string(dir.join("package.json")).unwrap(), "{\"name\": \"something-else\"}");

        fs::remove_dir_all(&dir).unwrap();
        export(&dir, &bundle, &opts, &mut |_| {}).unwrap();
        export(&dir, &bundle, &opts, &mut |_| {}).expect("an earlier export may be replaced");
        let index = fs::read_to_string(dir.join("index.js")).unwrap();
        assert!(index.lines().nth(1).unwrap().starts_with("// Name require('x'): "), "the name stays on its comment line");
        assert!(!index.contains("\nrequire('x')"));
    }

    /// The exported index.js runs on its own and reads its token from .env.
    #[test]
    fn exported_bot_runs_with_node() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/xc.core/blocks");
        let block = |rel: &str| json!({ "source": fs::read_to_string(root.join(rel)).unwrap(), "file": rel });
        let value = json!({
            "project": { "name": "Export Test", "mode": "none", "settings": {} },
            "blocks": { "core.on_start": block("events/on_start.js"), "core.log": block("debug/log.js") },
            "libs": [],
            "workspaces": { "main": {
                "name": "Main",
                "nodes": {
                    "a": { "block": "core.on_start@1", "at": [0, 0] },
                    "l": { "block": "core.log@1", "at": [0, 0], "values": { "message": "exported and running", "level": "success" } }
                },
                "links": ["a.next > l.in"]
            } }
        });
        let bundle = Bundle { value, dependencies: IndexMap::new() };
        let dir = std::env::temp_dir().join(format!("xc-export-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let opts = ExportOptions { name: "Export Test".into(), token: Some("abc".into()), token_env: "DISCORD_TOKEN".into(), secrets: Vec::new(), install: false };
        export(&dir, &bundle, &opts, &mut |_| {}).unwrap();

        assert!(fs::read_to_string(dir.join(".env")).unwrap().contains("DISCORD_TOKEN=abc"));
        let pkg: serde_json::Value = serde_json::from_str(&fs::read_to_string(dir.join("package.json")).unwrap()).unwrap();
        assert_eq!(pkg["name"], "export-test");

        // Start it and wait for the log line (slow machines take a while).
        let mut child = Command::new("node")
            .arg("index.js")
            .current_dir(&dir)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::BufRead;
            for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut text = String::new();
        while !text.contains("exported and running") {
            match rx.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())) {
                Ok(line) => {
                    text.push_str(&line);
                    text.push('\n');
                }
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(text.contains("exported and running"), "output was:\n{text}");
    }
}
