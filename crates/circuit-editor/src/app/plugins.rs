//! Installing plugins from zips, reloading them and running their scripts.

use super::*;

impl CircuitApp {
    /// Shows the Plugins view (in the side bar, or its tab).
    pub(super) fn open_plugins(&mut self) {
        self.show_panel(Tab::Side(SideView::Plugins));
    }

    /// Removes a plugin: its folder is deleted. Never one that comes with the
    /// app, nor one outside the plugin folders.
    pub(super) fn remove_plugin(&mut self, id: &str, ctx: &egui::Context) {
        if self.ed.bundled_plugins.iter().any(|b| b == id) {
            return;
        }
        // One that comes with the app: its files come back with every update,
        // so it is left out instead, until added back.
        if let Some((_, name)) = self.ed.optional_plugins.iter().find(|(p, _)| p == id).cloned() {
            self.ed.prefs.disabled_plugins.retain(|p| p != id);
            if !self.ed.prefs.removed_plugins.iter().any(|p| p == id) {
                self.ed.prefs.removed_plugins.push(id.to_string());
            }
            self.ed.log(Level::Success, tf!("Removed {name}. Plugins › Add back brings it back.", name = name));
            self.reload_plugins(ctx);
            return;
        }
        let Some(lp) = self.ed.reg.plugins.iter().chain(&self.ed.reg.disabled).find(|p| p.manifest.id == id).cloned() else { return };
        let mut folders = self.ed.plugin_dirs.clone();
        if let Some(p) = &self.ed.project {
            folders.push(p.root.join("plugins"));
        }
        if !folders.iter().any(|f| lp.dir.parent() == Some(f.as_path())) {
            self.ed.log(Level::Warn, format!("{} isn't in a plugin folder, so it wasn't removed.", lp.manifest.name));
            return;
        }
        match std::fs::remove_dir_all(&lp.dir) {
            Ok(()) => {
                self.ed.prefs.disabled_plugins.retain(|p| p != id);
                self.ed.log(Level::Success, tf!("Removed {name}", name = lp.manifest.name));
                self.reload_plugins(ctx);
            }
            Err(e) => self.ed.log(Level::Error, format!("Couldn't remove {}: {e}", lp.manifest.name)),
        }
    }

    /// Plugins › Install from Zip…: asks for the file.
    pub(super) fn pick_plugin_zip(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new().set_title(t!("Install a plugin")).add_filter(t!("Plugin (.zip)"), &["zip"]).pick_file() {
            self.install_plugin_zip(&path, ctx);
        }
    }

    /// Installs the plugin in a .zip into the user's plugins folder (asking
    /// before replacing one) and loads it.
    pub(super) fn install_plugin_zip(&mut self, path: &Path, ctx: &egui::Context) {
        use crate::plugins::install::{InstallError, install_zip};
        let dir = crate::bot::app_data_dir().join("plugins");
        let result = match install_zip(path, &dir, false) {
            Err(InstallError::Exists { name, .. }) => {
                let replace = rfd::MessageDialog::new()
                    .set_title(t!("Install a plugin"))
                    .set_description(tf!("{name} is installed already. Replace it with the version in this file?", name = name))
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show();
                if replace != rfd::MessageDialogResult::Yes {
                    return;
                }
                install_zip(path, &dir, true)
            }
            other => other,
        };
        match result {
            Ok(done) => {
                self.ed.log(Level::Success, tf!("Installed {name} into {folder}", name = done.name, folder = done.dir.display()));
                self.reload_plugins(ctx);
            }
            Err(e) => self.ed.log(Level::Error, tf!("Couldn't install {file}: {error}", file = path.display(), error = e)),
        }
    }

    pub(super) fn reload_plugins(&mut self, ctx: &egui::Context) {
        self.ed.reload_plugins();
        self.ed.refresh_define_ctx();
        self.ed.theme.apply(ctx);
        self.plugin_host.sync(&mut self.ed, self.bundled_plugins.as_deref());
    }

    /// AI agents' requests through the MCP server, answered with plugins'
    /// tools (and the app's own `screenshot`). The server listens only while a plugin has tools (the AI
    /// agents plugin is on), and never in tests.
    fn serve_agents(&mut self, ctx: &egui::Context, running: bool) {
        if !self.plugin_host.has_tools() || cfg!(test) {
            self.agent_bridge = None;
        } else if self.agent_bridge.is_none() && self.agent_bridge_failed.is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(30)) {
            match crate::mcp::bridge::Bridge::start(ctx.clone()) {
                Ok(b) => self.agent_bridge = Some(b),
                Err(e) => {
                    // Tried again in a while, not every frame.
                    self.ed.log(Level::Warn, tf!("AI agents can't connect: {error}", error = e));
                    self.agent_bridge_failed = Some(std::time::Instant::now());
                }
            }
        }
        let Some(bridge) = &self.agent_bridge else {
            self.ed.agent_link = serde_json::json!({ "listening": false, "agents": 0 });
            self.agent_shots.clear();
            return;
        };
        let (requests, agents, port) = (bridge.poll(), bridge.agents(), bridge.port);
        for pending in requests {
            let answer = match pending.request["op"].as_str() {
                Some("list") => {
                    let mut list: Vec<serde_json::Value> = self.plugin_host.tools().iter().map(|t| serde_json::json!({ "name": t.name, "description": t.description, "inputSchema": t.input })).collect();
                    list.push(screenshot_tool());
                    serde_json::json!({ "tools": list })
                }
                Some("call") if pending.request["name"] == SCREENSHOT => {
                    let args = pending.request.get("args").cloned().unwrap_or_default();
                    let prepared = self.prepare_shot(&args);
                    self.log_agent(SCREENSHOT, &args, prepared.as_ref().err());
                    match prepared {
                        Ok(about) => {
                            ctx.request_repaint();
                            // About a quarter of a second: what it opened has drawn and windows have faded in.
                            self.agent_shots.push(AgentShot { pending, about, wait: 15, image: None });
                            continue;
                        }
                        Err(e) => serde_json::json!({ "result": crate::mcp::tool_result(Err(e)) }),
                    }
                }
                Some("call") => {
                    let name = pending.request["name"].as_str().unwrap_or("");
                    let args = pending.request.get("args").cloned().unwrap_or_else(|| serde_json::json!({}));
                    let result = self.plugin_host.call_tool(&mut self.ed, name, &args, running).unwrap_or_else(|| Err(format!("No tool `{name}`")));
                    self.log_agent(name, &args, result.as_ref().err());
                    serde_json::json!({ "result": crate::mcp::tool_result(result) })
                }
                _ => serde_json::json!({ "error": "unknown request" }),
            };
            pending.answer(answer);
        }
        self.ed.agent_link = serde_json::json!({ "listening": true, "agents": agents, "port": port });
        self.take_shots(ctx);
    }

    /// Writes an agent's tool call to the Console: the tool and what it was
    /// given, as a warning with the reason when it failed. A call about a
    /// block links to it.
    fn log_agent(&mut self, tool: &str, args: &serde_json::Value, failed: Option<&String>) {
        let given = agent_args(args);
        let text = match failed {
            None => tf!("AI agent: {tool} {given}", tool = tool, given = given),
            Some(e) => tf!("AI agent: {tool} {given} failed: {error}", tool = tool, given = given, error = e.lines().next().unwrap_or_default()),
        };
        let mut line = crate::editor::LogLine::new(Level::Mcp, text.trim().replace("  ", " "));
        if let (Some(ws), Some(block)) = (args["workspace"].as_str(), args["block"].as_str())
            && self.ed.workspace(ws).is_some_and(|w| w.nodes.contains_key(block))
        {
            line.target = Some((ws.to_string(), block.to_string()));
        }
        self.ed.push_log(line);
    }

    /// Opens what an agent's screenshot is of: a workspace, fitted to show
    /// everything, or centred on a block. Says what it shows.
    fn prepare_shot(&mut self, args: &serde_json::Value) -> Result<String, String> {
        let project = self.ed.project.as_ref().ok_or("No project is open in XC Circuit.")?;
        let wanted = args["workspace"].as_str().map(str::trim).filter(|w| !w.is_empty());
        let key = match wanted {
            Some(w) => project.workspaces.iter().find(|(k, ws)| k.as_str() == w || ws.name.eq_ignore_ascii_case(w)).map(|(k, _)| k.clone()).ok_or_else(|| format!("No workspace \"{w}\"."))?,
            None => self.ed.active_workspace.clone().filter(|k| project.workspaces.contains_key(k)).or_else(|| project.workspaces.keys().next().cloned()).ok_or("The project has no workspaces.")?,
        };
        let name = project.workspaces[&key].name.clone();
        let zoom = shot_zoom(&args["zoom"])?;
        let at = zoom.map(|z| format!(" at {}%", (z * 100.0).round())).unwrap_or_default();
        let about = match args["block"].as_str().map(str::trim).filter(|b| !b.is_empty()) {
            Some(block) => {
                if !project.workspaces[&key].nodes.contains_key(block) {
                    return Err(format!("Workspace {key} has no block \"{block}\"."));
                }
                self.ed.reveal(&key, block);
                format!("Workspace \"{name}\" ({key}), centred on block {block}{at}.")
            }
            None => {
                self.ed.actions.push(crate::editor::Action::OpenWorkspace(key.clone()));
                // Fitted (when it's drawn next) unless told not to, or given a zoom.
                if args["fit"].as_bool().unwrap_or(zoom.is_none()) {
                    if let Some(view) = self.ed.views.get_mut(&key) {
                        view.initialized = false;
                    }
                    format!("Workspace \"{name}\" ({key}), fitted to show every block{}.", if at.is_empty() { String::new() } else { format!(", then zoomed to{at}") })
                } else {
                    format!("Workspace \"{name}\" ({key}){}.", if at.is_empty() { String::from(", as it was") } else { at.clone() })
                }
            }
        };
        if let Some(z) = zoom {
            self.ed.set_zoom(&key, z);
        }
        Ok(about)
    }

    /// Asks for the window's picture once the canvas has settled, and answers
    /// with the canvas part of it.
    fn take_shots(&mut self, ctx: &egui::Context) {
        if self.agent_shots.is_empty() {
            return;
        }
        for shot in self.agent_shots.iter_mut().filter(|s| s.image.is_none()) {
            if shot.wait > 0 {
                shot.wait -= 1;
            } else {
                let got: Shared = Default::default();
                let (slot, wake) = (got.clone(), ctx.clone());
                ctx.request_screenshot(move |image| {
                    if let Ok(mut s) = slot.lock() {
                        *s = Some(image);
                    }
                    wake.request_repaint();
                });
                shot.image = Some(got);
            }
        }
        ctx.request_repaint();
        // Where the canvas was drawn: this frame's parts aren't drawn yet, so the last frame's.
        let canvas = self.ed.ui_rects.get("canvas").or_else(|| self.ed.ui_rects_before.get("canvas")).copied();
        let ppp = ctx.pixels_per_point();
        for shot in std::mem::take(&mut self.agent_shots) {
            let image = shot.image.as_ref().and_then(|s| s.lock().ok().and_then(|g| g.clone()));
            let Some(image) = image else {
                self.agent_shots.push(shot);
                continue;
            };
            let answer = match canvas.filter(|r| r.width() > 20.0 && r.height() > 20.0) {
                Some(rect) => match shot_png(&image.region(&rect, Some(ppp))) {
                    Ok(png) => serde_json::json!({ "content": [
                        { "type": "image", "data": png, "mimeType": "image/png" },
                        { "type": "text", "text": shot.about },
                    ] }),
                    Err(e) => crate::mcp::tool_result(Err(format!("Couldn't make the picture: {e}"))),
                },
                None => crate::mcp::tool_result(Err("No workspace is on screen in XC Circuit (is the window minimised, or the canvas tab closed?).".into())),
            };
            shot.pending.answer(serde_json::json!({ "result": answer }));
        }
    }

    /// Bot events, checks and queued commands for plugins' scripts.
    pub(super) fn run_plugins(&mut self, ctx: &egui::Context) {
        let running = self.bot.state != BotState::Stopped;
        for ev in std::mem::take(&mut self.bot.events) {
            self.plugin_host.bot_event(&mut self.ed, &ev, running);
        }
        self.plugin_host.tick(&mut self.ed, running);
        self.serve_agents(ctx, running);
        if let Some(text) = self.ed.copy_out.take() {
            ctx.copy_text(text);
        }
        for id in std::mem::take(&mut self.plugin_host.queued) {
            match id.as_str() {
                "bot.run" => self.run_command(Cmd::Run, ctx),
                "bot.stop" => self.run_command(Cmd::Stop, ctx),
                "file.save" => self.run_command(Cmd::Save, ctx),
                // The bot's server (Bot › Remote…).
                // (remote.… and, from before it was called that, server.…)
                "server.upload" | "remote.upload" => self.bot.hosting.upload(&mut self.ed),
                "server.start" | "server.restart" | "server.stop" | "server.kill" | "remote.start" | "remote.restart" | "remote.stop" | "remote.kill" => {
                    let action = id.trim_start_matches("server.").trim_start_matches("remote.").to_string();
                    self.bot.hosting.remote_action(&mut self.ed, &action);
                }
                // The project's repository (Share › Git…): git.store may
                // carry the words for the version after a colon.
                "git.fetch" => self.bot.git.fetch_now(&mut self.ed),
                store if store == "git.store" || store.starts_with("git.store:") => {
                    let message = store.strip_prefix("git.store:").unwrap_or_default().to_string();
                    self.bot.git.store_with(&mut self.ed, &message);
                }
                "edit.undo" => self.undo(),
                "edit.redo" => self.redo(),
                "run.local" => self.pick_remote(false),
                "run.server" | "run.remote" => {
                    if self.bot.hosting.can_control(&self.ed) {
                        self.pick_remote(true);
                    } else {
                        self.ed.log(Level::Info, t!("Run can switch to the remote once a Calagopus server is set up (Bot › Remote…)."));
                    }
                }
                _ => {
                    if !self.plugin_host.run_command(&mut self.ed, &id, running) {
                        self.ed.log(Level::Warn, format!("No command `{id}`"));
                    }
                }
            }
        }
        if self.plugin_host.plugins.iter().any(|p| p.status == crate::plugins::host::Status::Loaded) {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        // Plugins' timers, and what they float over the editor (a desk pet …).
        if let Some(wake) = self.plugin_host.frame(ctx, &mut self.ed, running) {
            ctx.request_repaint_after(wake);
        }
    }
}

/// The app's own tool for agents: a picture of the canvas.
const SCREENSHOT: &str = "screenshot";

fn screenshot_tool() -> serde_json::Value {
    serde_json::json!({
        "name": SCREENSHOT,
        "description": "A picture of XC Circuit's canvas as the user sees it: a workspace (the open one when not given), fitted to show every block, or centred on one block. Use it to check a layout after placing blocks (overlaps, wires, readability). It opens that workspace in the editor.",
        "inputSchema": { "type": "object", "properties": {
            "workspace": { "type": "string", "description": "The workspace's key or name." },
            "block": { "type": "string", "description": "A block id to centre on (instead of fitting everything)." },
            "fit": { "type": "boolean", "description": "Fit every block and note into view (the default without a block or zoom); false keeps the zoom." },
            "zoom": { "type": "number", "description": "The zoom in percent, as the canvas toolbar shows it (25 to 200, 100 is full size): to see how the canvas looks at a zoom, or to look closer. Around a block when one is given, else around the middle of the canvas." },
        } },
    })
}

/// A screenshot's `zoom` (percent) as a scale; none when not given.
fn shot_zoom(v: &serde_json::Value) -> Result<Option<f32>, String> {
    match v {
        serde_json::Value::Null => Ok(None),
        v => match v.as_f64() {
            Some(p) if p.is_finite() && p > 0.0 => {
                let (lo, hi) = crate::canvas::ZOOM_RANGE;
                Ok(Some((p as f32 / 100.0).clamp(lo, hi)))
            }
            _ => Err(format!("zoom is a percentage such as 85, not {v}.")),
        },
    }
}

/// An agent's screenshot on its way.
pub(super) struct AgentShot {
    pending: crate::mcp::bridge::Pending,
    /// What it shows, said with the picture.
    about: String,
    /// Frames to wait for the canvas to show it.
    wait: u8,
    /// Where the picture arrives, once asked for.
    image: Option<Shared>,
}

type Shared = std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<egui::ColorImage>>>>;

/// A picture as base64 PNG, at most 1568 pixels on its longer side.
fn shot_png(image: &egui::ColorImage) -> Result<String, String> {
    use base64::Engine as _;
    let [w, h] = image.size;
    let raw: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
    let mut img = image::RgbaImage::from_raw(w as u32, h as u32, raw).ok_or("bad size")?;
    let longest = w.max(h) as f32;
    if longest > 1568.0 {
        let k = 1568.0 / longest;
        img = image::imageops::resize(&img, (w as f32 * k) as u32, (h as f32 * k) as u32, image::imageops::FilterType::Triangle);
    }
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(out.into_inner()))
}

/// What an agent gave a tool, short: `· workspace main, type "core.log", values {message, level}`.
fn agent_args(args: &serde_json::Value) -> String {
    use serde_json::Value;
    let short = |text: &str, max: usize| -> String {
        let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or_default().trim();
        let more = text.lines().filter(|l| !l.trim().is_empty()).count() > 1;
        match line.char_indices().nth(max) {
            Some((i, _)) => format!("{}…", &line[..i]),
            None if more => format!("{line} …"),
            None => line.to_string(),
        }
    };
    let Some(map) = args.as_object().filter(|m| !m.is_empty()) else { return String::new() };
    let parts: Vec<String> = map
        .iter()
        .map(|(k, v)| {
            let value = match v {
                // Keys of a workspace, block or note read as they are.
                Value::String(t) if matches!(k.as_str(), "workspace" | "block" | "note" | "from" | "to" | "after") => short(t, 40),
                Value::String(t) => format!("\"{}\"", short(t, if k == "code" { 60 } else { 40 })),
                Value::Array(list) if list.iter().all(Value::is_string) && list.len() <= 6 => list.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "),
                Value::Array(list) => format!("[{} items]", list.len()),
                Value::Object(o) => format!("{{{}}}", o.keys().take(6).cloned().collect::<Vec<_>>().join(", ")),
                other => other.to_string(),
            };
            format!("{k} {value}")
        })
        .collect();
    format!("· {}", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn agent_calls_read_short_in_the_console() {
        assert_eq!(super::agent_args(&json!({})), "");
        let line = super::agent_args(&json!({ "workspace": "main", "type": "core.log", "values": { "message": "hi", "level": "warn" }, "around": ["1", "2"] }));
        assert_eq!(line, "· workspace main, type \"core.log\", values {message, level}, around 1, 2");
        let code = super::agent_args(&json!({ "code": "const a = tools.add_block({ workspace: 'main', type: 'core.log' });\nreturn a;" }));
        assert!(code.starts_with("· code \"const a = tools.add_block") && code.ends_with("…\""), "{code}");
        assert_eq!(super::agent_args(&json!({ "blocks": [1, 2, 3] })), "· blocks [3 items]");
    }
}
