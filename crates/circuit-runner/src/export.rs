//! Exports a project as a folder that runs with `node index.js`.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::json;

use crate::RUNTIME_SOURCE;
use crate::bundle::Bundle;
use crate::node::ensure_packages;

pub struct ExportOptions {
    pub name: String,
    /// Written to `.env` as `DISCORD_TOKEN` when given.
    pub token: Option<String>,
    /// Run `npm install` in the export folder.
    pub install: bool,
}

/// Writes `index.js` (runtime + project + blocks), `package.json`, `.env`,
/// `.gitignore` and `README.txt`.
pub fn export(dir: &Path, bundle: &Bundle, opts: &ExportOptions, log: &mut dyn FnMut(String)) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

    let index = format!(
        "#!/usr/bin/env node\n// {name}: exported from XC Circuit. Start it with: node index.js\n\nconst __XC_BUNDLE__ = {bundle};\n\n{runtime}",
        name = opts.name,
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
    });
    fs::write(dir.join("package.json"), serde_json::to_string_pretty(&package)? + "\n")?;

    if let Some(token) = &opts.token {
        fs::write(dir.join(".env"), format!("# Keep this file private: it holds your bot token.\nDISCORD_TOKEN={token}\n"))?;
    } else if !dir.join(".env").exists() {
        fs::write(dir.join(".env"), "# Put your bot token here.\nDISCORD_TOKEN=\n")?;
    }
    fs::write(dir.join(".gitignore"), ".env\nnode_modules/\ndata/\n")?;
    fs::write(
        dir.join("README.txt"),
        format!(
            "{name}\n\nMade with XC Circuit.\n\n1. Install Node.js 22 or newer: https://nodejs.org\n2. In this folder, run: npm install   (once)\n3. Start the bot:      node index.js\n\nThe bot token is read from .env (DISCORD_TOKEN=...). Never share that file.\nSaved data is kept in the data/ folder.\n",
            name = opts.name
        ),
    )?;

    if opts.install {
        let name = package["name"].as_str().unwrap_or("bot");
        ensure_packages(dir, name, &bundle.dependencies, log)?;
        // ensure_packages wrote a minimal package.json; put the full one back.
        fs::write(dir.join("package.json"), serde_json::to_string_pretty(&package)? + "\n")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use indexmap::IndexMap;

    use super::*;

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
        let opts = ExportOptions { name: "Export Test".into(), token: Some("abc".into()), install: false };
        export(&dir, &bundle, &opts, &mut |_| {}).unwrap();

        assert!(fs::read_to_string(dir.join(".env")).unwrap().contains("DISCORD_TOKEN=abc"));
        let pkg: serde_json::Value = serde_json::from_str(&fs::read_to_string(dir.join("package.json")).unwrap()).unwrap();
        assert_eq!(pkg["name"], "export-test");

        // Start it, give it a moment, and look for the log line.
        let mut child = Command::new("node")
            .arg("index.js")
            .current_dir(&dir)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let _ = child.kill();
        let out = child.wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("exported and running"), "output was:\n{text}");
    }
}
