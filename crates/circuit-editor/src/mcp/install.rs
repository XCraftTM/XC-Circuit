//! Adding XC Circuit's MCP server to AI clients: Claude Desktop, Cursor and
//! VS Code by their JSON settings files, Claude Code through its `claude`
//! command. Each entry starts this program with `--mcp`.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::SERVER_NAME;

/// The clients, in the order they are shown.
const CLIENTS: [(&str, &str); 4] = [("claude-desktop", "Claude Desktop"), ("claude-code", "Claude Code"), ("cursor", "Cursor"), ("vscode", "VS Code")];

/// This program, as clients should start it: the AppImage itself rather
/// than the folder it unpacks to.
pub fn program() -> Result<PathBuf, String> {
    if let Some(image) = std::env::var_os("APPIMAGE").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(image));
    }
    let exe = std::env::current_exe().map_err(|e| format!("Couldn't find this program: {e}"))?;
    let exe = exe.canonicalize().unwrap_or(exe);
    // Without Windows' \\?\ prefix, which some clients can't start.
    Ok(match exe.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(plain) => PathBuf::from(plain),
        None => exe,
    })
}

/// The server's entry for clients' `mcpServers`.
fn entry(exe: &Path, vscode: bool) -> Value {
    let mut e = json!({ "command": exe.display().to_string(), "args": ["--mcp"] });
    if vscode {
        e["type"] = json!("stdio");
    }
    e
}

/// The entry to paste into another client's settings.
pub fn snippet() -> String {
    let exe = program().unwrap_or_else(|_| PathBuf::from("xc-circuit"));
    serde_json::to_string_pretty(&json!({ "mcpServers": { SERVER_NAME: entry(&exe, false) } })).unwrap_or_default()
}

/// A client's JSON settings files that hold MCP servers (more than one when
/// it may read either), the key they go under, and whether the client is there.
fn json_files(client: &str) -> Option<(Vec<PathBuf>, &'static str, bool)> {
    let config = dirs::config_dir()?;
    let home = dirs::home_dir()?;
    Some(match client {
        "claude-desktop" => {
            let usual = config.join("Claude");
            let mut dirs = vec![usual.clone()];
            // The Microsoft Store version keeps its settings in its own folder.
            if cfg!(windows)
                && let Some(packages) = dirs::data_local_dir().map(|d| d.join("Packages"))
                && let Ok(list) = std::fs::read_dir(packages)
            {
                for p in list.flatten() {
                    if p.file_name().to_string_lossy().starts_with("Claude_") {
                        let dir = p.path().join("LocalCache").join("Roaming").join("Claude");
                        if dir.is_dir() {
                            dirs.push(dir);
                        }
                    }
                }
            }
            let present = dirs.iter().any(|d| d.is_dir());
            // Every place it might read, that is there; the usual one when none is.
            let mut files: Vec<PathBuf> = dirs.iter().filter(|d| d.is_dir()).map(|d| d.join("claude_desktop_config.json")).collect();
            if files.is_empty() {
                files.push(usual.join("claude_desktop_config.json"));
            }
            (files, "mcpServers", present)
        }
        "cursor" => (vec![home.join(".cursor").join("mcp.json")], "mcpServers", home.join(".cursor").is_dir()),
        "vscode" => {
            let user = config.join("Code").join("User");
            (vec![user.join("mcp.json")], "servers", user.is_dir())
        }
        _ => return None,
    })
}

/// Whether a settings file has the server: "added" (this program),
/// "outdated" (another copy of it) or "missing".
pub fn state_in(text: Option<&str>, key: &str, exe: &Path) -> &'static str {
    let found = text.and_then(|t| serde_json::from_str::<Value>(t).ok()).and_then(|v| v.get(key)?.get(SERVER_NAME).cloned());
    match found {
        None => "missing",
        Some(e) if e["command"].as_str() == Some(&exe.display().to_string()) => "added",
        Some(_) => "outdated",
    }
}

/// A settings file's text with the server added (or replaced), or taken out
/// (`entry` None). Everything else in it stays.
pub fn edited(text: Option<&str>, key: &str, entry: Option<Value>) -> Result<String, String> {
    let mut root: Value = match text.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => serde_json::from_str(t).map_err(|e| format!("it isn't plain JSON ({e}); comments, perhaps"))?,
        None => json!({}),
    };
    let obj = root.as_object_mut().ok_or("it doesn't hold a JSON object")?;
    let servers = obj.entry(key).or_insert_with(|| Value::Object(Map::new()));
    let servers = servers.as_object_mut().ok_or_else(|| format!("its `{key}` isn't an object"))?;
    match entry {
        Some(e) => {
            servers.insert(SERVER_NAME.into(), e);
        }
        None => {
            servers.remove(SERVER_NAME);
        }
    }
    serde_json::to_string_pretty(&root).map(|t| t + "\n").map_err(|e| e.to_string())
}

/// Writes a settings file, keeping its first version beside it.
fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let backup = path.with_extension("json.before-xc-circuit");
    if path.is_file() && !backup.exists() {
        let _ = std::fs::copy(path, &backup);
    }
    let tmp = path.with_extension("json.xc-circuit-tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Claude Code's `claude` command, when installed.
fn claude_cli() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["claude.exe", "claude.cmd"] } else { &["claude"] };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".claude").join("local"));
    }
    dirs.iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
}

/// Runs `claude` with `args`, without a console window. Its output when it worked.
fn run_claude(cli: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = if cli.extension().is_some_and(|e| e.eq_ignore_ascii_case("cmd")) {
        let mut c = std::process::Command::new("cmd");
        c.arg("/C").arg(cli);
        c
    } else {
        std::process::Command::new(cli)
    };
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)).trim().to_string();
    if out.status.success() { Ok(text) } else { Err(text) }
}

/// The command that adds the server to Claude Code, for pasting into a terminal.
fn claude_code_command(exe: &Path) -> String {
    format!("claude mcp add --scope user {SERVER_NAME} -- \"{}\" --mcp", exe.display())
}

/// Every client: { id, name, state, detail }. State: "added", "outdated",
/// "missing", or "absent" (the client isn't installed).
pub fn clients() -> Value {
    let exe = program().unwrap_or_default();
    Value::Array(
        CLIENTS
            .iter()
            .map(|(id, name)| {
                let (state, detail) = match *id {
                    "claude-code" => {
                        // Its user settings: ~/.claude.json, `mcpServers` at the top.
                        let file = dirs::home_dir().map(|h| h.join(".claude.json"));
                        let text = file.as_ref().and_then(|f| std::fs::read_to_string(f).ok());
                        let present = claude_cli().is_some() || text.is_some();
                        let state = if present { state_in(text.as_deref(), "mcpServers", &exe) } else { "absent" };
                        (state, claude_cli().map(|c| c.display().to_string()).unwrap_or_else(|| claude_code_command(&exe)))
                    }
                    _ => match json_files(id) {
                        Some((files, key, present)) => {
                            let states: Vec<&str> = files.iter().map(|f| state_in(std::fs::read_to_string(f).ok().as_deref(), key, &exe)).collect();
                            let state = if !present {
                                "absent"
                            } else if states.iter().all(|s| *s == "added") {
                                "added"
                            } else if states.contains(&"outdated") {
                                "outdated"
                            } else {
                                "missing"
                            };
                            (state, files.first().map(|f| f.display().to_string()).unwrap_or_default())
                        }
                        None => ("absent", String::new()),
                    },
                };
                json!({ "id": id, "name": name, "state": state, "detail": detail })
            })
            .collect(),
    )
}

fn name_of(client: &str) -> Result<&'static str, String> {
    CLIENTS.iter().find(|(id, _)| *id == client).map(|(_, n)| *n).ok_or_else(|| format!("No AI client `{client}`"))
}

/// Adds the server to a client. Says what was done.
pub fn install(client: &str) -> Result<String, String> {
    let name = name_of(client)?;
    let exe = program()?;
    if client == "claude-code" {
        let cli = claude_cli().ok_or_else(|| format!("Claude Code's `claude` command wasn't found. Run this in a terminal: {}", claude_code_command(&exe)))?;
        // Replaced, when it's there from another copy of the app.
        let _ = run_claude(&cli, &["mcp", "remove", "--scope", "user", SERVER_NAME]);
        let exe_text = exe.display().to_string();
        run_claude(&cli, &["mcp", "add", "--scope", "user", SERVER_NAME, "--", &exe_text, "--mcp"]).map_err(|e| format!("Claude Code couldn't add it: {e}"))?;
        return Ok(format!("Added XC Circuit to {name}. Start a new Claude Code session to use it."));
    }
    let (files, key, _) = json_files(client).ok_or("No settings folder found")?;
    for f in &files {
        let text = std::fs::read_to_string(f).ok();
        let new = edited(text.as_deref(), key, Some(entry(&exe, client == "vscode"))).map_err(|e| format!("Couldn't change {}: {e}. Add it by hand with Copy settings.", f.display()))?;
        write(f, &new).map_err(|e| format!("Couldn't write {}: {e}", f.display()))?;
    }
    let restart = match client {
        "claude-desktop" => " Quit and reopen Claude Desktop to use it.",
        "vscode" => " It shows under MCP servers in VS Code's Chat view.",
        _ => " Restart it to use it.",
    };
    Ok(format!("Added XC Circuit to {name}.{restart}"))
}

/// Takes the server out of a client. Says what was done.
pub fn remove(client: &str) -> Result<String, String> {
    let name = name_of(client)?;
    if client == "claude-code" {
        let cli = claude_cli().ok_or_else(|| format!("Claude Code's `claude` command wasn't found. Run this in a terminal: claude mcp remove --scope user {SERVER_NAME}"))?;
        run_claude(&cli, &["mcp", "remove", "--scope", "user", SERVER_NAME]).map_err(|e| format!("Claude Code couldn't remove it: {e}"))?;
        return Ok(format!("Removed XC Circuit from {name}."));
    }
    let (files, key, _) = json_files(client).ok_or("No settings folder found")?;
    for f in files.iter().filter(|f| f.is_file()) {
        let text = std::fs::read_to_string(f).map_err(|e| e.to_string())?;
        let new = edited(Some(&text), key, None).map_err(|e| format!("Couldn't change {}: {e}", f.display()))?;
        write(f, &new).map_err(|e| format!("Couldn't write {}: {e}", f.display()))?;
    }
    Ok(format!("Removed XC Circuit from {name}."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_settings_files_and_keeps_the_rest() {
        let exe = PathBuf::from(r"C:\Apps\XC Circuit\xc-circuit.exe");
        let before = r#"{ "theme": "dark", "mcpServers": { "other": { "command": "node", "args": ["x.js"] } } }"#;
        assert_eq!(state_in(Some(before), "mcpServers", &exe), "missing");
        let added = edited(Some(before), "mcpServers", Some(entry(&exe, false))).unwrap();
        let v: Value = serde_json::from_str(&added).unwrap();
        assert_eq!(v["theme"], "dark", "the rest stays");
        assert_eq!(v["mcpServers"]["other"]["command"], "node");
        assert_eq!(v["mcpServers"][SERVER_NAME]["args"], json!(["--mcp"]));
        assert_eq!(state_in(Some(&added), "mcpServers", &exe), "added");
        assert_eq!(state_in(Some(&added), "mcpServers", Path::new("/other/xc-circuit")), "outdated");
        let removed = edited(Some(&added), "mcpServers", None).unwrap();
        assert_eq!(state_in(Some(&removed), "mcpServers", &exe), "missing");
        assert!(serde_json::from_str::<Value>(&removed).unwrap()["mcpServers"]["other"].is_object());

        // A new file; VS Code's own key and shape.
        let vs = edited(None, "servers", Some(entry(&exe, true))).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&vs).unwrap()["servers"][SERVER_NAME]["type"], "stdio");
        // Files it can't read safely are left alone.
        assert!(edited(Some("{ // a comment\n }"), "servers", Some(entry(&exe, true))).is_err());
        assert!(edited(Some("[1]"), "servers", None).is_err());
    }
}
