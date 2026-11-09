//! JavaScript editor plugins. A plugin's `editor.js` (or `editor.ts`) adds
//! commands, panels and checks through the global `xc` object:
//!
//! ```js
//! xc.command({ id: "hello.greet", title: "Say hello", run() { xc.log("Hello!") } });
//! xc.panel({ id: "hello", title: "Hello", render(ui) { ui.heading(xc.project()?.name ?? "No project") } });
//! xc.check(() => xc.blocks().filter((b) => b.type === "core.log" && !b.values.text).map((b) => ({ message: "An empty Log", workspace: b.workspace, block: b.id })));
//! ```
//!
//! Each plugin gets its own QuickJS sandbox with a time and memory limit. It
//! reaches the app only through `xc` (no files, network or processes), so
//! scripts need no approval. They run in the editor only; bot code never
//! runs here.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText, vec2};
use rquickjs::{Context, Ctx, Function, Object, Runtime};
use serde::Deserialize;
use serde_json::{Value, json};
use crate::plugins::api::{self, BotEvent, Command, Host, PanelInfo, PluginObject, Problem};

/// Commands and clicks may do more work than drawing a panel or a check,
/// which run often.
const QUICK: Duration = Duration::from_millis(100);
const SLOW: Duration = Duration::from_millis(1000);
/// How long an AI agent's tool may take.
const TOOL: Duration = Duration::from_secs(5);
const MEMORY_LIMIT: usize = 32 * 1024 * 1024;
/// How often an open panel is drawn again, at most, without a click.
const RENDER_EVERY: Duration = Duration::from_millis(250);
/// How often an overlay is drawn again without a click, timer or event.
const OVERLAY_EVERY: Duration = Duration::from_secs(1);
/// The shortest wait for a timer, and between a repeating timer's runs.
const TIMER_MIN: Duration = Duration::from_millis(16);
const REPEAT_MIN: Duration = Duration::from_millis(50);

/// What the script keeps in the editor between calls: its timers and where
/// its overlays are.
#[derive(Default)]
struct State {
    timers: Vec<Timer>,
    /// Overlays the user or the script moved, by id: their top-left corner.
    overlays: HashMap<String, egui::Pos2>,
    /// The editor window's size, for `xc.screen()`.
    screen: egui::Vec2,
}

struct Timer {
    id: u32,
    due: Instant,
    every: Option<Duration>,
}

/// The `xc` object and the calls the editor makes into the script.
const PRELUDE: &str = include_str!("script_prelude.js");

type HostPtr = Rc<Cell<Option<*mut (dyn Host + 'static)>>>;

/// Lends the editor to the script for one call, and takes it back even if
/// the call panics.
struct Lend<'a>(&'a HostPtr);

impl<'a> Lend<'a> {
    fn new(slot: &'a HostPtr, host: &mut dyn Host) -> Lend<'a> {
        let ptr: *mut (dyn Host + '_) = host;
        // SAFETY: only the lifetime is erased. The pointer is used by `__host`
        // calls the script makes during this call (scripts can't run between
        // calls: there are no timers and pending jobs aren't run), and `Drop`
        // clears it before the borrow ends.
        let ptr: *mut (dyn Host + 'static) = unsafe { std::mem::transmute(ptr) };
        slot.set(Some(ptr));
        Lend(slot)
    }
}

impl Drop for Lend<'_> {
    fn drop(&mut self) {
        self.0.set(None);
    }
}

/// What the script registered.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Registered {
    commands: Vec<CommandInfo>,
    panels: Vec<PanelEntry>,
    checks: usize,
    save: usize,
    #[serde(rename = "botEvent")]
    bot_event: usize,
    selection: usize,
    overlays: Vec<OverlayEntry>,
    tools: Vec<ToolEntry>,
}

/// A tool for AI agents (`xc.tool`).
#[derive(Clone, Deserialize)]
struct ToolEntry {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    input: Value,
}

/// Something floating over the editor (`xc.overlay`).
#[derive(Clone, Deserialize)]
struct OverlayEntry {
    id: String,
    /// Where it starts; negative: from the right or bottom edge.
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    /// Drawn on a panel-coloured card (otherwise only its items).
    #[serde(default)]
    frame: bool,
    #[serde(default = "yes")]
    draggable: bool,
    /// It has an onClick handler.
    #[serde(default)]
    click: bool,
}

#[derive(Deserialize)]
struct CommandInfo {
    id: String,
    title: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize)]
struct PanelEntry {
    id: String,
    title: String,
}

#[derive(Deserialize)]
struct Reply {
    ok: Option<Value>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct ProblemEntry {
    message: String,
    workspace: Option<String>,
    block: Option<String>,
}

/// A panel's last drawing.
#[derive(Default)]
struct View {
    items: Vec<Item>,
    error: Option<String>,
    drawn: Option<Instant>,
    /// Something was clicked or typed: draw again now.
    stale: bool,
}

/// What `render(ui)` built: `ui.heading("…")` adds a `heading`, and so on.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
enum Item {
    Heading {
        text: String,
    },
    Text {
        text: String,
        #[serde(default)]
        muted: bool,
        #[serde(default)]
        mono: bool,
        #[serde(default)]
        bold: bool,
        size: Option<f32>,
        color: Option<String>,
    },
    Stat {
        label: String,
        value: String,
    },
    Separator,
    Space {
        size: f32,
    },
    Button {
        text: String,
        on: Option<usize>,
        #[serde(default)]
        primary: bool,
        tooltip: Option<String>,
    },
    Link {
        text: String,
        on: Option<usize>,
        tooltip: Option<String>,
    },
    Checkbox {
        text: String,
        value: bool,
        on: Option<usize>,
    },
    Input {
        value: String,
        on: Option<usize>,
        #[serde(default)]
        hint: String,
        #[serde(default)]
        multiline: bool,
        key: Option<String>,
    },
    Progress {
        value: f32,
        #[serde(default)]
        text: String,
    },
    Image {
        file: String,
        width: Option<f32>,
        height: Option<f32>,
        tint: Option<String>,
    },
    Row {
        items: Vec<Item>,
    },
    /// A switch with its text.
    Toggle {
        text: String,
        value: bool,
        on: Option<usize>,
    },
    /// A dropdown: (value, what it shows).
    Select {
        value: String,
        choices: Vec<(String, String)>,
        on: Option<usize>,
        #[serde(default = "select_width")]
        width: f32,
    },
    /// A number to type or drag.
    Number {
        value: f64,
        on: Option<usize>,
        min: Option<f64>,
        max: Option<f64>,
        step: Option<f64>,
        #[serde(default)]
        suffix: String,
    },
    /// A slider with its number.
    Slider {
        value: f64,
        min: f64,
        max: f64,
        on: Option<usize>,
        step: Option<f64>,
        #[serde(default)]
        suffix: String,
    },
    /// A colour, as #RRGGBB.
    Color {
        value: String,
        on: Option<usize>,
    },
    /// A box of text: info, warning or success.
    Note {
        text: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        tone: String,
    },
    Section {
        text: String,
        items: Vec<Item>,
        #[serde(default = "yes")]
        open: bool,
    },
}

fn yes() -> bool {
    true
}

fn select_width() -> f32 {
    200.0
}

/// The icon set, as the app last published it (for a note's icon).
fn icon_set(ui: &egui::Ui) -> std::collections::HashMap<String, PathBuf> {
    ui.ctx().data(|d| d.get_temp(egui::Id::unique(crate::ui::kit::KIT_ICONS))).unwrap_or_default()
}

pub struct ScriptPlugin {
    name: String,
    runtime: Runtime,
    context: Context,
    deadline: Arc<Mutex<Option<Instant>>>,
    host: HostPtr,
    registered: Registered,
    commands: Vec<Command>,
    panels: Vec<PanelInfo>,
    views: HashMap<String, View>,
    /// The plugin's folder: files for images, sounds and `require`.
    root: PathBuf,
    state: Rc<RefCell<State>>,
    /// The selection last seen, for `xc.on("selection")`.
    last_selection: Option<(Option<String>, Vec<String>)>,
}

/// Loads a plugin's script. Also returns every file it read, to reload it
/// when one of them changes (even when loading failed).
pub fn load(name: &str, dir: &Path, script: &str) -> (Result<ScriptPlugin, String>, Vec<PathBuf>) {
    let read = Rc::new(std::cell::RefCell::new(Vec::new()));
    let result = ScriptPlugin::load(name, dir, script, read.clone());
    let files = read.borrow().clone();
    (result, if files.is_empty() { vec![dir.join(script)] } else { files })
}

impl ScriptPlugin {
    fn load(name: &str, dir: &Path, script: &str, read: Rc<std::cell::RefCell<Vec<PathBuf>>>) -> Result<ScriptPlugin, String> {
        let runtime = Runtime::new().map_err(|e| e.to_string())?;
        runtime.set_memory_limit(MEMORY_LIMIT);
        let deadline: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        let watch = deadline.clone();
        runtime.set_interrupt_handler(Some(Box::new(move || watch.lock().ok().and_then(|d| *d).is_some_and(|d| Instant::now() > d))));
        let context = Context::full(&runtime).map_err(|e| e.to_string())?;
        let host: HostPtr = Rc::new(Cell::new(None));
        let root = plain_path(dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf()));
        let state: Rc<RefCell<State>> = Rc::default();
        let slot = host.clone();
        let (own_state, own_root) = (state.clone(), root.clone());
        let plugin_root = root.clone();
        context
            .with(|ctx| -> rquickjs::Result<()> {
                let g = ctx.globals();
                g.set(
                    "__host",
                    Function::new(ctx.clone(), move |method: String, args: String| -> String {
                        let result = match slot.get() {
                            // SAFETY: see `Lend`: set only while the editor waits for this script.
                            Some(ptr) => host_call(unsafe { &mut *ptr }, &method, &args),
                            None => Err("the app can only be used from a command, panel, check or event handler, not while the script loads".into()),
                        };
                        reply(result)
                    })?,
                )?;
                g.set(
                    "__read",
                    Function::new(ctx.clone(), move |from: String, name: String| -> String {
                        let result = read_module(&root, &from, &name);
                        if let Ok((path, ..)) = &result {
                            read.borrow_mut().push(path.clone());
                        }
                        reply(result.map(|(_, rel, dir, source)| json!({ "path": rel, "dir": dir, "source": source })))
                    })?,
                )?;
                g.set(
                    "__compile",
                    Function::new(ctx.clone(), compile)?,
                )?;
                // What the script asks of its own part in the editor (timers,
                // overlays, sounds); also works while it loads.
                g.set(
                    "__self",
                    Function::new(ctx.clone(), move |method: String, args: String| -> String { reply(self_call(&own_state, &own_root, &method, &args)) })?,
                )?;
                g.set("__pluginName", name)?;
                ctx.eval::<(), _>(PRELUDE)
            })
            .map_err(|e| format!("the editor's script setup failed: {e}"))?;

        let mut plugin = ScriptPlugin {
            name: name.to_string(),
            runtime,
            context,
            deadline,
            host,
            registered: Registered::default(),
            commands: Vec::new(),
            panels: Vec::new(),
            views: HashMap::new(),
            root: plugin_root,
            state,
            last_selection: None,
        };
        let main = format!("./{}", script.trim_start_matches("./"));
        let registered: Registered = serde_json::from_value(plugin.run(None, SLOW, "load", vec![json!(main)])?).map_err(|e| e.to_string())?;
        plugin.commands = registered.commands.iter().map(|c| Command::new(&c.id, &c.title).description(&c.description)).collect();
        plugin.panels = registered.panels.iter().map(|p| PanelInfo::new(&p.id, &p.title)).collect();
        plugin.registered = registered;
        Ok(plugin)
    }

    /// Calls `__xc[method](...args)`, lending the editor when there is one.
    fn run(&self, host: Option<&mut dyn Host>, budget: Duration, method: &str, args: Vec<Value>) -> Result<Value, String> {
        let _lent = host.map(|h| Lend::new(&self.host, h));
        let end = Instant::now() + budget;
        self.set_deadline(Some(end));
        let text = self.context.with(|ctx| {
            let call = || -> rquickjs::Result<String> {
                let xc: Object = ctx.globals().get("__xc")?;
                let f: Function = xc.get(method)?;
                f.call((Value::Array(args).to_string(),))
            };
            call().map_err(|e| match e {
                rquickjs::Error::Exception => {
                    let caught = ctx.catch();
                    if Instant::now() > end {
                        format!("stopped after {} ms: it took too long (is there an endless loop?)", budget.as_millis())
                    } else {
                        caught.as_exception().and_then(|e| e.message()).unwrap_or_else(|| "an error the script didn't catch".into())
                    }
                }
                other => other.to_string(),
            })
        });
        self.set_deadline(None);
        if budget == SLOW {
            self.runtime.run_gc();
        }
        let reply: Reply = serde_json::from_str(&text?).map_err(|e| e.to_string())?;
        match (reply.ok, reply.error) {
            (_, Some(e)) => Err(e),
            (ok, None) => Ok(ok.unwrap_or(Value::Null)),
        }
    }

    fn set_deadline(&self, deadline: Option<Instant>) {
        if let Ok(mut d) = self.deadline.lock() {
            *d = deadline;
        }
    }

    fn fail(&self, host: &mut dyn Host, what: &str, e: &str) {
        host.log(api::Level::Error, &format!("{} ({what}): {e}", self.name));
    }
}

/// A module's source as a function of `module`, `exports` and `require`,
/// named after its file in error messages.
fn compile<'js>(ctx: Ctx<'js>, path: String, source: String) -> rquickjs::Result<rquickjs::Value<'js>> {
    let mut options = rquickjs::context::EvalOptions::default();
    options.global = true;
    options.strict = false;
    options.filename = Some(path);
    ctx.eval_with_options(format!("(function (module, exports, require, __filename) {{{source}\n}})"), options)
}

/// What `__self` does: the script's timers, overlays and sounds.
fn self_call(state: &RefCell<State>, root: &Path, method: &str, args: &str) -> Result<Value, String> {
    let a: Vec<Value> = serde_json::from_str(args).map_err(|e| e.to_string())?;
    let num = |i: usize| a.get(i).and_then(Value::as_f64).unwrap_or(0.0);
    let text = |i: usize| a.get(i).and_then(Value::as_str).unwrap_or_default().to_string();
    let mut st = state.borrow_mut();
    Ok(match method {
        "timer" => {
            let wait = Duration::from_millis(num(1).max(0.0) as u64);
            let every = a.get(2).and_then(Value::as_bool).unwrap_or(false).then(|| wait.max(REPEAT_MIN));
            st.timers.push(Timer { id: num(0) as u32, due: Instant::now() + wait.max(TIMER_MIN), every });
            Value::Null
        }
        "cancel" => {
            let id = num(0) as u32;
            st.timers.retain(|t| t.id != id);
            Value::Null
        }
        "screen" => json!({ "width": st.screen.x, "height": st.screen.y }),
        "overlayMove" => {
            st.overlays.insert(text(0), egui::pos2(num(1) as f32, num(2) as f32));
            Value::Null
        }
        "overlayPos" => st.overlays.get(&text(0)).map_or(Value::Null, |p| json!({ "x": p.x, "y": p.y })),
        "sound" => {
            let path = plugin_file(root, &text(0)).ok_or_else(|| format!("xc.playSound: no file `{}` in the plugin", text(0)))?;
            let bytes = std::fs::read(&path).map_err(|e| format!("xc.playSound: {e}"))?;
            crate::plugins::sound::play(bytes, num(1) as f32);
            Value::Null
        }
        other => return Err(format!("xc has no `{other}`")),
    })
}

/// A canonical path without Windows' `\\?\` prefix, which file URLs (for
/// images) don't understand.
fn plain_path(p: PathBuf) -> PathBuf {
    match p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => p,
    }
}

/// A file inside the plugin's folder, if it is one.
fn plugin_file(root: &Path, name: &str) -> Option<PathBuf> {
    inside(root, Path::new(name)).filter(|p| p.is_file())
}

fn reply(result: Result<Value, String>) -> String {
    match result {
        Ok(v) => json!({ "ok": v }).to_string(),
        Err(e) => json!({ "error": e }).to_string(),
    }
}

fn level(name: &str) -> api::Level {
    match name {
        "success" => api::Level::Success,
        "warn" => api::Level::Warn,
        "error" => api::Level::Error,
        _ => api::Level::Info,
    }
}

/// JSON text from the editor, parsed (null when missing or broken).
fn parsed(text: Option<String>) -> Value {
    text.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null)
}

/// What `xc.*` does in the editor. `args` is a JSON array.
fn host_call(h: &mut dyn Host, method: &str, args: &str) -> Result<Value, String> {
    let a: Vec<Value> = serde_json::from_str(args).map_err(|e| e.to_string())?;
    let s = |i: usize| match a.get(i) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(v) => v.to_string(),
    };
    let value = |i: usize| a.get(i).cloned().unwrap_or(Value::Null);
    Ok(match method {
        "project" => match h.project_name() {
            Some(name) => json!({ "name": name, "root": h.project_root().map(|p| p.display().to_string()) }),
            None => Value::Null,
        },
        "workspaces" => Value::Array(h.workspaces().into_iter().map(|w| json!({ "key": w.key, "name": w.name })).collect()),
        "workspace" => parsed(h.workspace_json(&s(0))),
        "setWorkspace" => {
            h.set_workspace_json(&s(0), &value(1).to_string())?;
            Value::Null
        }
        "activeWorkspace" => json!(h.active_workspace()),
        "selection" => json!(h.selection()),
        "reveal" => {
            // The zoom in percent, as the canvas toolbar shows it.
            let zoom = a.get(2).and_then(Value::as_f64).filter(|p| p.is_finite() && *p > 0.0).map(|p| p as f32 / 100.0);
            h.reveal(&s(0), &s(1), zoom);
            Value::Null
        }
        "blockName" => json!(h.block_name(&s(0))),
        "getData" => parsed(h.project_data(&s(0))),
        "setData" => {
            h.set_project_data(&s(0), &value(1).to_string())?;
            Value::Null
        }
        "log" => {
            h.log(level(&s(0)), &s(1));
            Value::Null
        }
        "run" => {
            h.run_command(&s(0));
            Value::Null
        }
        "botRunning" => json!(h.bot_running()),
        "storageGet" => parsed(h.storage_get(&s(0))),
        "storageSet" => {
            h.storage_set(&s(0), &value(1).to_string());
            Value::Null
        }
        "addBlock" => {
            let at = [a.get(2).and_then(Value::as_f64).unwrap_or(0.0) as f32, a.get(3).and_then(Value::as_f64).unwrap_or(0.0) as f32];
            json!(h.add_block(&s(0), &s(1), at, &value(4).to_string())?)
        }
        "select" => {
            let ids: Vec<String> = value(1).as_array().map(|l| l.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
            h.select(&s(0), &ids);
            Value::Null
        }
        "blockTypes" => h.block_types(&s(0)),
        "blockType" => h.block_type(&s(0)).unwrap_or(Value::Null),
        "blockPins" => h.block_pins(&s(0), &s(1))?,
        "connect" => {
            h.connect(&s(0), &s(1), &s(2))?;
            Value::Null
        }
        "addWorkspace" => {
            let folder = s(1);
            json!(h.add_workspace(&s(0), (!folder.is_empty()).then_some(folder.as_str()))?)
        }
        "problems" => h.problems(),
        "console" => h.console(a.first().and_then(Value::as_u64).unwrap_or(50).min(1000) as usize),
        "t" => json!(h.translate(&s(0))),
        "fieldValue" => h.field_value(&s(0), &s(1), &value(2))?,
        "blockRects" => h.block_rects(&s(0)),
        "openBuilder" => json!(h.open_builder(&s(0), &s(1), &s(2))?),
        "copyText" => {
            h.copy_text(&s(0));
            Value::Null
        }
        "agentClients" => h.agent_clients(),
        "agentInstall" => json!(h.agent_install(&s(0), a.get(1).and_then(Value::as_bool).unwrap_or(false))?),
        "agentSnippet" => json!(h.agent_snippet()),
        "agentLink" => h.agent_link(),
        "appSettings" => h.app_settings(),
        "botSettings" => h.bot_settings(),
        "setBotSettings" => h.set_bot_settings(&value(0))?,
        "server" => h.server(),
        "plugins" => h.plugins(),
        "blockSource" => h.block_source(&s(0))?,
        other => return Err(format!("xc has no `{other}`")),
    })
}

/// `path` relative to `root`, with `.` and `..` worked out; None when it
/// leaves `root`.
fn inside(root: &Path, path: &Path) -> Option<PathBuf> {
    let mut out = root.to_path_buf();
    let depth = root.components().count();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            // Never above the plugin folder.
            Component::ParentDir if out.components().count() > depth => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
            _ => return None,
        }
    }
    Some(out)
}

/// A file for `require(name)` from a module in the plugin folder `from`
/// (relative to the plugin). Only the plugin's own files can be loaded.
fn read_module(root: &Path, from: &str, name: &str) -> Result<(PathBuf, String, String, String), String> {
    if !(name.starts_with("./") || name.starts_with("../")) {
        return Err(format!("require(\"{name}\"): editor plugins can only load their own files (\"./file.js\")"));
    }
    let base = inside(root, &Path::new(from).join(name)).ok_or_else(|| format!("require(\"{name}\"): that's outside the plugin's folder"))?;
    let candidates = [base.clone(), base.with_extension("js"), base.with_extension("ts"), base.join("index.js")];
    let path = candidates.into_iter().find(|p| p.is_file()).ok_or_else(|| format!("require(\"{name}\"): no such file in the plugin"))?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
    let source = match path.extension().and_then(|e| e.to_str()) {
        Some("ts") => circuit_plugins::ts::to_javascript(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?,
        Some("json") => format!("module.exports = {text};"),
        _ => circuit_plugins::ts::imports_to_require(&text),
    };
    let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
    let dir = Path::new(&rel).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    Ok((path, rel, dir, source))
}

fn bot_event_json(event: &BotEvent) -> Value {
    match event {
        BotEvent::Started => json!({ "type": "started" }),
        BotEvent::Stopped => json!({ "type": "stopped" }),
        BotEvent::Log { level, text } => {
            let level = match level {
                api::Level::Info => "info",
                api::Level::Success => "success",
                api::Level::Warn => "warn",
                api::Level::Error => "error",
            };
            json!({ "type": "log", "level": level, "text": text })
        }
        BotEvent::Block { workspace, node, phase, ms, error } => json!({ "type": "block", "workspace": workspace, "block": node, "phase": phase, "ms": ms, "error": error }),
    }
}

impl PluginObject for ScriptPlugin {
    fn commands(&self) -> Result<Vec<Command>, String> {
        Ok(self.commands.clone())
    }

    fn run_command(&mut self, id: &str, host: &mut dyn Host) -> Result<(), String> {
        if let Err(e) = self.run(Some(&mut *host), SLOW, "command", vec![json!(id)]) {
            self.fail(host, id, &e);
        }
        Ok(())
    }

    fn panels(&self) -> Result<Vec<PanelInfo>, String> {
        Ok(self.panels.clone())
    }

    fn panel_ui(&mut self, id: &str, ui: &mut egui::Ui, host: &mut dyn Host) -> Result<(), String> {
        let since = self.views.get(id).and_then(|v| v.drawn).map(|t| t.elapsed());
        let due = self.views.get(id).is_none_or(|v| v.stale) || since.is_none_or(|s| s >= RENDER_EVERY);
        if due {
            let result = self.run(Some(&mut *host), QUICK, "render", vec![json!(id)]).and_then(|v| serde_json::from_value::<Vec<Item>>(v).map_err(|e| format!("render() built something the panel can't show: {e}")));
            let view = self.views.entry(id.to_string()).or_default();
            match result {
                Ok(items) => {
                    view.items = items;
                    view.error = None;
                }
                Err(e) => view.error = Some(e),
            }
            view.drawn = Some(Instant::now());
            view.stale = false;
        } else if let Some(s) = since {
            ui.ctx().request_repaint_after(RENDER_EVERY.saturating_sub(s));
        }

        let view = self.views.entry(id.to_string()).or_default();
        let mut events = Vec::new();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(12).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if let Some(e) = &view.error {
                    ui.label(RichText::new(tf!("{plugin} couldn't draw this panel", plugin = self.name)).strong().color(ui.visuals().error_fg_color));
                    ui.add(egui::Label::new(RichText::new(e).monospace().size(11.5)).wrap());
                    ui.label(RichText::new(t!("Fix the script and save it: the plugin reloads by itself.")).weak());
                    ui.add_space(10.0);
                }
                let theme = crate::ui::kit::theme_of(ui);
                draw(ui, &theme, &mut view.items, &self.root, &mut Vec::new(), &mut events);
            });
        });
        if !events.is_empty() {
            for (handler, value) in events {
                if let Err(e) = self.run(Some(&mut *host), SLOW, "event", vec![json!(id), json!(handler), value]) {
                    self.fail(host, id, &e);
                }
            }
            if let Some(v) = self.views.get_mut(id) {
                v.stale = true;
            }
            ui.ctx().request_repaint();
        }
        Ok(())
    }

    fn check(&mut self, host: &mut dyn Host) -> Result<Vec<Problem>, String> {
        if self.registered.checks == 0 {
            return Ok(Vec::new());
        }
        let found = self.run(Some(host), QUICK, "check", Vec::new()).and_then(|v| serde_json::from_value::<Vec<ProblemEntry>>(v).map_err(|e| e.to_string()));
        Ok(match found {
            Ok(list) => list
                .into_iter()
                .map(|p| match (p.workspace, p.block) {
                    (Some(ws), Some(node)) => Problem::new(p.message).at(ws, node),
                    _ => Problem::new(p.message),
                })
                .collect(),
            Err(e) => vec![Problem::new(format!("A check failed: {e}"))],
        })
    }

    fn on_bot_event(&mut self, event: &BotEvent, host: &mut dyn Host) -> Result<(), String> {
        if self.registered.bot_event > 0
            && let Err(e) = self.run(Some(&mut *host), QUICK, "botEvent", vec![bot_event_json(event)])
        {
            self.fail(host, "botEvent", &e);
        }
        Ok(())
    }

    fn on_save(&mut self, host: &mut dyn Host) -> Result<(), String> {
        if self.registered.save > 0
            && let Err(e) = self.run(Some(&mut *host), SLOW, "save", Vec::new())
        {
            self.fail(host, "save", &e);
        }
        Ok(())
    }

    fn tick(&mut self, host: &mut dyn Host) -> Result<Option<Duration>, String> {
        // Timers that are due; a repeating one is set for its next run.
        let now = Instant::now();
        let due: Vec<u32> = {
            let mut st = self.state.borrow_mut();
            let due = st.timers.iter().filter(|t| t.due <= now).map(|t| t.id).collect();
            for t in st.timers.iter_mut().filter(|t| t.due <= now) {
                if let Some(every) = t.every {
                    t.due = now + every;
                }
            }
            st.timers.retain(|t| t.due > now);
            due
        };
        for id in &due {
            if let Err(e) = self.run(Some(&mut *host), QUICK, "timer", vec![json!(id)]) {
                // A failing timer would fail again and again: it stops.
                self.state.borrow_mut().timers.retain(|t| t.id != *id);
                self.fail(host, t!("timer (stopped)"), &e);
            }
        }
        // xc.on("selection"): when other blocks are selected.
        if self.registered.selection > 0 {
            let now_selected = (host.active_workspace(), host.selection());
            if self.last_selection.as_ref() != Some(&now_selected) {
                let first = self.last_selection.is_none();
                self.last_selection = Some(now_selected.clone());
                if !first {
                    let event = json!({ "workspace": now_selected.0, "blocks": now_selected.1 });
                    if let Err(e) = self.run(Some(&mut *host), QUICK, "selection", vec![event]) {
                        self.fail(host, "selection", &e);
                    }
                }
                self.views.values_mut().for_each(|v| v.stale = true);
            }
        }
        if !due.is_empty() {
            self.views.values_mut().for_each(|v| v.stale = true);
        }
        let next = self.state.borrow().timers.iter().map(|t| t.due).min();
        Ok(next.map(|d| d.saturating_duration_since(Instant::now())))
    }

    fn tools(&self) -> Vec<api::Tool> {
        self.registered.tools.iter().map(|t| api::Tool { name: t.name.clone(), description: t.description.clone(), input: t.input.clone() }).collect()
    }

    fn call_tool(&mut self, name: &str, args: &Value, host: &mut dyn Host) -> Result<Value, String> {
        self.run(Some(host), TOOL, "tool", vec![json!(name), args.clone()])
    }

    fn overlay_ui(&mut self, ctx: &egui::Context, host: &mut dyn Host) -> Result<(), String> {
        let screen = ctx.content_rect();
        self.state.borrow_mut().screen = screen.size();
        for o in self.registered.overlays.clone() {
            let key = format!("overlay:{}", o.id);
            let since = self.views.get(&key).and_then(|v| v.drawn).map(|t| t.elapsed());
            if self.views.get(&key).is_none_or(|v| v.stale) || since.is_none_or(|s| s >= OVERLAY_EVERY) {
                let result = self.run(Some(&mut *host), QUICK, "renderOverlay", vec![json!(o.id)]).and_then(|v| serde_json::from_value::<Vec<Item>>(v).map_err(|e| e.to_string()));
                let view = self.views.entry(key.clone()).or_default();
                match result {
                    Ok(items) => (view.items, view.error) = (items, None),
                    Err(e) => view.error = Some(e),
                }
                (view.drawn, view.stale) = (Some(Instant::now()), false);
            }
            ctx.request_repaint_after(OVERLAY_EVERY);

            let size = vec2(o.width.max(8.0), o.height.max(8.0));
            let start = egui::pos2(
                if o.x < 0.0 { screen.right() + o.x - size.x } else { screen.left() + o.x },
                if o.y < 0.0 { screen.bottom() + o.y - size.y } else { screen.top() + o.y },
            );
            let at = self.state.borrow().overlays.get(&o.id).copied().unwrap_or(start);
            // Always fully on screen, also after the window got smaller.
            let at = egui::pos2(at.x.clamp(screen.left(), (screen.right() - size.x).max(screen.left())), at.y.clamp(screen.top(), (screen.bottom() - size.y).max(screen.top())));
            let root = self.root.clone();
            let name = self.name.clone();
            let view = self.views.entry(key.clone()).or_default();
            let mut events = Vec::new();
            let mut moved: Option<egui::Pos2> = None;
            let mut clicked = false;
            egui::Area::new(egui::Id::unique(("xc-overlay", &name, &o.id))).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
                let (rect, bg) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
                if o.frame {
                    let v = ui.visuals();
                    ui.painter().rect(rect, 10.0, v.window_fill, v.window_stroke, egui::StrokeKind::Inside);
                }
                let inner = if o.frame { rect.shrink(10.0) } else { rect };
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
                if let Some(e) = &view.error {
                    child.add(egui::Label::new(RichText::new(format!("{name}: {e}")).small().color(child.visuals().error_fg_color)).wrap());
                }
                let theme = crate::ui::kit::theme_of(&child);
                draw(&mut child, &theme, &mut view.items, &root, &mut Vec::new(), &mut events);
                if o.draggable && bg.hovered() {
                    ui.ctx().set_cursor_icon(if bg.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
                }
                if o.draggable && bg.dragged() {
                    moved = Some(at + bg.drag_delta());
                }
                clicked = o.click && bg.clicked();
            });
            if let Some(p) = moved {
                self.state.borrow_mut().overlays.insert(o.id.clone(), p);
            }
            if clicked {
                events.push((usize::MAX, Value::Null));
            }
            for (handler, value) in events {
                let result = if handler == usize::MAX {
                    self.run(Some(&mut *host), SLOW, "overlayClick", vec![json!(o.id)])
                } else {
                    self.run(Some(&mut *host), SLOW, "event", vec![json!(key), json!(handler), value])
                };
                if let Err(e) = result {
                    self.fail(host, &o.id, &e);
                }
                if let Some(v) = self.views.get_mut(&key) {
                    v.stale = true;
                }
                ctx.request_repaint();
            }
        }
        Ok(())
    }
}

/// A colour by name (`accent`, `success`, `warning`, `error`, `muted`) or `#rrggbb`.
fn color(ui: &egui::Ui, name: &str) -> Option<egui::Color32> {
    let v = ui.visuals();
    Some(match name {
        "accent" => v.hyperlink_color,
        "success" => egui::Color32::from_rgb(87, 186, 112),
        "warning" | "warn" => v.warn_fg_color,
        "error" => v.error_fg_color,
        "muted" => v.weak_text_color(),
        hex => egui::Color32::from_hex(hex).ok()?,
    })
}

fn stat(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(value).size(22.0).strong());
        ui.label(RichText::new(label).size(11.0).weak());
    });
}

/// Draws a panel's items. Clicks and edits land in `events` as (handler, value).
/// Drawn with the UI kit, so a plugin's panel looks like the rest of the app.
fn draw(ui: &mut egui::Ui, theme: &crate::ui::theme::Theme, items: &mut [Item], root: &Path, path: &mut Vec<usize>, events: &mut Vec<(usize, Value)>) {
    use crate::ui::kit;
    for (i, item) in items.iter_mut().enumerate() {
        path.push(i);
        let id = ui.scope_id().with(("xc-script-item", path.as_slice()));
        match item {
            Item::Heading { text } => {
                ui.label(RichText::new(text.as_str()).size(15.0).strong().color(theme.text));
            }
            Item::Text { text, muted, mono, bold, size, color: c } => {
                let mut rt = RichText::new(text.as_str()).size(size.unwrap_or(12.5)).color(if *muted { theme.text_muted } else { theme.text });
                if *mono {
                    rt = rt.monospace();
                }
                if *bold {
                    rt = rt.strong();
                }
                if let Some(c) = c.as_deref().and_then(|c| color(ui, c)) {
                    rt = rt.color(c);
                }
                ui.add(egui::Label::new(rt).wrap());
            }
            Item::Stat { label, value } => stat(ui, label, value),
            Item::Separator => {
                ui.separator();
            }
            Item::Space { size } => ui.add_space(size.clamp(0.0, 200.0)),
            Item::Button { text, on, primary, tooltip } => {
                let mut r = ui.add_enabled_ui(on.is_some(), |ui| if *primary { kit::primary_button(ui, theme, text.as_str()) } else { kit::button(ui, theme, text.as_str()) }).inner;
                if let Some(t) = tooltip {
                    r = r.on_hover_text(t.as_str());
                }
                if r.clicked()
                    && let Some(h) = on
                {
                    events.push((*h, Value::Null));
                }
            }
            Item::Link { text, on, tooltip } => {
                let mut r = ui.link(text.as_str());
                if let Some(t) = tooltip {
                    r = r.on_hover_text(t.as_str());
                }
                if r.clicked()
                    && let Some(h) = on
                {
                    events.push((*h, Value::Null));
                }
            }
            Item::Checkbox { text, value, on } => {
                if ui.add_enabled_ui(on.is_some(), |ui| kit::checkbox(ui, theme, value, text.as_str())).inner.changed()
                    && let Some(h) = on
                {
                    events.push((*h, json!(*value)));
                }
            }
            Item::Input { value, on, hint, multiline, key } => {
                let id = key.as_ref().map_or(id, |k| ui.scope_id().with(("xc-script-input", k.as_str())));
                let r = ui
                    .add_enabled_ui(on.is_some(), |ui| {
                        if *multiline {
                            ui.add(kit::TextArea::new(id, value).hint(hint.as_str()).rows(4))
                        } else {
                            ui.push_id(id, |ui| ui.add(kit::TextLine::new(value).hint(hint.as_str()))).inner
                        }
                    })
                    .inner;
                if r.changed()
                    && let Some(h) = on
                {
                    events.push((*h, json!(value)));
                }
            }
            Item::Image { file, width, height, tint } => {
                let size = vec2(width.unwrap_or(64.0), height.unwrap_or(width.unwrap_or(64.0)));
                match plugin_file(root, file) {
                    Some(p) => {
                        let mut image = egui::Image::from_uri(crate::editor::file_uri(&p)).fit_to_exact_size(size);
                        if let Some(c) = tint.as_deref().and_then(|c| color(ui, c)) {
                            image = image.tint(c);
                        }
                        ui.add(image);
                    }
                    None => {
                        ui.label(RichText::new(format!("No image {file}")).small().color(ui.visuals().error_fg_color));
                    }
                }
            }
            Item::Progress { value, text } => {
                let mut bar = egui::ProgressBar::new(value.clamp(0.0, 1.0));
                if !text.is_empty() {
                    bar = bar.text(text.as_str());
                }
                ui.add(bar);
            }
            Item::Row { items } => {
                ui.horizontal_wrapped(|ui| {
                    if items.iter().all(|i| matches!(i, Item::Stat { .. })) {
                        ui.spacing_mut().item_spacing.x = 22.0;
                    }
                    draw(ui, theme, items, root, path, events);
                });
            }
            Item::Toggle { text, value, on } => {
                let changed = kit::line(ui, |ui| {
                    let c = ui.add_enabled_ui(on.is_some(), |ui| kit::toggle(ui, value, theme).accessible_name(text.as_str())).inner.changed();
                    ui.label(RichText::new(text.as_str()).color(theme.text));
                    c
                });
                if changed && let Some(h) = on {
                    events.push((*h, json!(*value)));
                }
            }
            Item::Select { value, choices, on, width } => {
                if ui.add_enabled_ui(on.is_some(), |ui| kit::select(ui, id, *width, value, choices)).inner
                    && let Some(h) = on
                {
                    events.push((*h, json!(value)));
                }
            }
            Item::Number { value, on, min, max, step, suffix } => {
                let mut n = kit::Number::new(value).suffix(suffix.as_str()).speed(step.unwrap_or(1.0));
                if min.is_some() || max.is_some() {
                    n = n.range(min.unwrap_or(f64::MIN)..=max.unwrap_or(f64::MAX));
                }
                let r = ui.add_enabled_ui(on.is_some(), |ui| n.show(ui, theme)).inner;
                if r.changed() && let Some(h) = on {
                    events.push((*h, json!(*value)));
                }
            }
            Item::Slider { value, min, max, on, step, suffix } => {
                let mut s = kit::Slider::new(value, *min..=(*max).max(*min)).suffix(suffix.as_str());
                if let Some(st) = step {
                    s = s.step(*st);
                }
                if ui.add_enabled_ui(on.is_some(), |ui| s.show(ui, theme)).inner && let Some(h) = on {
                    events.push((*h, json!(*value)));
                }
            }
            Item::Color { value, on } => {
                if ui.add_enabled_ui(on.is_some(), |ui| kit::color(ui, theme, id, value)).inner && let Some(h) = on {
                    events.push((*h, json!(value)));
                }
            }
            Item::Note { text, title, tone } => {
                let icons = icon_set(ui);
                kit::note(ui, &icons, theme, tone, title, text);
            }
            Item::Section { text, items, open } => {
                // Unindented: egui's indent guide reads as a broken line here.
                egui::CollapsingHeader::new(RichText::new(text.as_str()).strong()).id_salt(id).default_open(*open).show_unindented(ui, |ui| {
                    ui.add_space(2.0);
                    draw(ui, theme, items, root, path, events);
                    ui.add_space(6.0);
                });
            }
        }
        path.pop();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// A project in memory.
    #[derive(Default)]
    struct FakeHost {
        workspaces: BTreeMap<String, Value>,
        data: BTreeMap<String, String>,
        storage: BTreeMap<String, String>,
        log: Vec<(api::Level, String)>,
        revealed: Vec<(String, String)>,
        ran: Vec<String>,
    }

    impl Host for FakeHost {
        fn project_name(&self) -> Option<String> {
            Some("Test".into())
        }
        fn project_root(&self) -> Option<PathBuf> {
            None
        }
        fn workspaces(&self) -> Vec<api::WorkspaceInfo> {
            self.workspaces.iter().map(|(k, w)| api::WorkspaceInfo { key: k.clone(), name: w["name"].as_str().unwrap_or_default().into() }).collect()
        }
        fn workspace_json(&self, key: &str) -> Option<String> {
            self.workspaces.get(key).map(Value::to_string)
        }
        fn set_workspace_json(&mut self, key: &str, json: &str) -> Result<(), String> {
            self.workspaces.insert(key.into(), serde_json::from_str(json).map_err(|e| e.to_string())?);
            Ok(())
        }
        fn active_workspace(&self) -> Option<String> {
            Some("main".into())
        }
        fn selection(&self) -> Vec<String> {
            vec!["b".into()]
        }
        fn reveal(&mut self, workspace: &str, node: &str, _zoom: Option<f32>) {
            self.revealed.push((workspace.into(), node.into()));
        }
        fn block_name(&self, block: &str) -> Option<String> {
            (block == "core.log").then(|| "Log".into())
        }
        fn project_data(&self, key: &str) -> Option<String> {
            self.data.get(key).cloned()
        }
        fn set_project_data(&mut self, key: &str, json: &str) -> Result<(), String> {
            self.data.insert(key.into(), json.into());
            Ok(())
        }
        fn log(&mut self, level: api::Level, text: &str) {
            self.log.push((level, text.into()));
        }
        fn run_command(&mut self, id: &str) {
            self.ran.push(id.into());
        }
        fn bot_running(&self) -> bool {
            false
        }
        fn storage_get(&self, key: &str) -> Option<String> {
            self.storage.get(key).cloned()
        }
        fn storage_set(&mut self, key: &str, value: &str) {
            self.storage.insert(key.into(), value.into());
        }
    }

    fn host() -> FakeHost {
        let mut h = FakeHost::default();
        h.workspaces.insert(
            "main".into(),
            json!({ "name": "Main", "nodes": { "a": { "block": "core.on_start@1", "at": [0, 0] }, "b": { "block": "core.log@1", "at": [0, 0], "values": { "message": "TODO say hi" } } }, "links": ["a.next > b.in"] }),
        );
        h
    }

    fn plugin_dir(files: &[(&str, &str)]) -> PathBuf {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("xc-script-test-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (name, text) in files {
            let path = dir.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        dir
    }

    const SCRIPT: &str = r#"
        const util = require("./util");
        let clicks = 0;
        xc.command({ id: "t.hello", title: "Hello", run() { xc.log("hello", { from: xc.plugin.name }); xc.run("file.save"); } });
        xc.command({ id: "t.slow", title: "Slow", run() { while (true) {} } });
        xc.command({ id: "t.rename", title: "Rename", run() {
          const ws = xc.workspace("main");
          ws.name = "Renamed";
          xc.setWorkspace("main", ws);
          xc.data.set("renamed", { at: 1 });
        } });
        xc.check(() => xc.blocks().filter((b) => util.hasTodo(b)).map((b) => ({ message: `TODO in ${xc.blockName(b.type)}`, workspace: b.workspace, block: b.id })));
        xc.check(() => "plain text works too");
        xc.panel({ id: "p", title: "Panel", render(ui) {
          ui.heading(xc.project().name);
          ui.button(`Clicked ${clicks}`, () => { clicks++; xc.storage.set("clicks", clicks); });
          ui.row((ui) => { ui.stat("blocks", xc.blocks().length); ui.link("Show", () => xc.reveal("main", "b")); });
        } });
        xc.on("save", () => xc.storage.set("saved", true));
        xc.on("botEvent", (e) => { if (e.type === "block") xc.storage.set("last", e.block); });
    "#;
    const UTIL: &str = "module.exports.hasTodo = (b) => Object.values(b.values).some((v) => String(v).includes('TODO'));";

    #[test]
    fn scripts_add_commands_checks_and_panels() {
        let dir = plugin_dir(&[("editor.js", SCRIPT), ("util.js", UTIL)]);
        let (plugin, files) = load("Test Plugin", &dir, "editor.js");
        let mut p = plugin.unwrap();
        assert_eq!(files.len(), 2, "both files are watched: {files:?}");
        let commands = p.commands().unwrap();
        assert_eq!((commands[0].id.as_str(), commands[0].title.as_str()), ("t.hello", "Hello"));
        assert_eq!(p.panels().unwrap()[0].title, "Panel");

        let mut h = host();
        p.run_command("t.hello", &mut h).unwrap();
        assert_eq!(h.log[0], (api::Level::Info, r#"hello {"from":"Test Plugin"}"#.to_string()));
        assert_eq!(h.ran, vec!["file.save"]);

        // An endless loop is stopped and reported; the plugin keeps working.
        p.run_command("t.slow", &mut h).unwrap();
        assert!(h.log.last().unwrap().1.contains("took too long"), "{:?}", h.log);
        p.run_command("t.rename", &mut h).unwrap();
        assert_eq!(h.workspaces["main"]["name"], "Renamed");
        assert_eq!(h.data["renamed"], r#"{"at":1}"#);

        let problems = p.check(&mut h).unwrap();
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert_eq!((problems[0].message.as_str(), problems[0].node.as_deref()), ("TODO in Log", Some("b")));
        assert_eq!(problems[1].message, "plain text works too");

        // render() builds items; a click runs its handler, and the next render shows it.
        assert_eq!(p.run(Some(&mut h), QUICK, "render", vec![json!("p")]).unwrap()[1]["text"], "Clicked 0");
        p.run(Some(&mut h), SLOW, "event", vec![json!("p"), json!(0), Value::Null]).unwrap();
        let items: Vec<Item> = serde_json::from_value(p.run(Some(&mut h), QUICK, "render", vec![json!("p")]).unwrap()).unwrap();
        assert!(matches!(&items[1], Item::Button { text, .. } if text == "Clicked 1"), "{items:?}");
        assert!(matches!(&items[2], Item::Row { items } if matches!(&items[0], Item::Stat { value, .. } if value == "2")), "{items:?}");
        assert_eq!(h.storage["clicks"], "1");
        p.run(Some(&mut h), SLOW, "event", vec![json!("p"), json!(1), Value::Null]).unwrap();
        assert_eq!(h.revealed, vec![("main".to_string(), "b".to_string())]);

        p.on_save(&mut h).unwrap();
        p.on_bot_event(&BotEvent::Block { workspace: "main".into(), node: "b".into(), phase: "exit".into(), ms: None, error: None }, &mut h).unwrap();
        assert_eq!((h.storage["saved"].as_str(), h.storage["last"].as_str()), ("true", "\"b\""));

        // The panel draws with egui.
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| p.panel_ui("p", ui, &mut h).unwrap()).textures_delta.clear();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn timers_overlays_and_placing_blocks() {
        let dir = plugin_dir(&[
            ("editor.js", r#"
                let ticks = 0, clicks = 0;
                const every = xc.every(10, () => { ticks++; if (ticks === 3) xc.cancel(every); });
                xc.after(0, () => xc.log("after"));
                xc.overlay({ id: "o", width: 50, height: 40, x: -10, y: 10, render(ui) { ui.text(`ticks ${ticks}`); ui.image("pic.svg", { width: 20 }) }, onClick() { clicks++; xc.moveOverlay("o", 5, 6) } });
                xc.on("selection", (e) => xc.log("selected", e.blocks));
                xc.command({ id: "t.place", title: "Place", run() { xc.log(xc.addBlock("main", "core.log", { x: 10, y: 20, values: { message: "hi" } })) } });
                xc.command({ id: "t.state", title: "State", run() { xc.log(ticks, clicks, JSON.stringify(xc.overlayPosition("o"))) } });
            "#),
            ("pic.svg", "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 4 4'/>"),
        ]);
        let mut p = load("P", &dir, "editor.js").0.unwrap();
        let mut h = host();
        // Timers run from tick(), which says when to come back.
        assert!(p.tick(&mut h).unwrap().is_some());
        // Repeating timers run at most every 50 ms.
        for _ in 0..25 {
            std::thread::sleep(Duration::from_millis(15));
            p.tick(&mut h).unwrap();
        }
        assert!(h.log.iter().any(|l| l.1 == "after"), "{:?}", h.log);
        p.run_command("t.state", &mut h).unwrap();
        assert_eq!(h.log.last().unwrap().1, "3 0 null", "the repeating timer stopped after 3");
        assert_eq!(p.state.borrow().timers.len(), 0);

        // The overlay draws over the whole window; a click runs onClick.
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| p.overlay_ui(ui.ctx(), &mut h).unwrap()).textures_delta.clear();
        let view = &p.views["overlay:o"];
        assert!(view.error.is_none() && format!("{:?}", view.items).contains("ticks 3"), "{:?}", view.items);
        p.run(Some(&mut h), SLOW, "overlayClick", vec![json!("o")]).unwrap();
        p.run_command("t.state", &mut h).unwrap();
        assert_eq!(h.log.last().unwrap().1, r#"3 1 {"x":5,"y":6}"#);

        // Selection changes reach xc.on("selection") (not the first one seen).
        p.tick(&mut h).unwrap();
        assert!(!h.log.iter().any(|l| l.1.starts_with("selected")));

        // Placing blocks goes through the editor (the fake one refuses).
        p.run_command("t.place", &mut h).unwrap();
        assert!(h.log.last().unwrap().1.contains("can't place blocks"), "{:?}", h.log.last());
    }

    /// The desk pet example loads, draws and reacts to a click.
    #[test]
    fn desk_pet_example_works() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/plugins/desk-pet");
        let mut p = load("Desk Pet", &dir, "editor.js").0.unwrap();
        let mut h = host();
        assert_eq!(p.registered.overlays[0].id, "pet");
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| p.overlay_ui(ui.ctx(), &mut h).unwrap()).textures_delta.clear();
        let items = format!("{:?}", p.views["overlay:pet"].items);
        assert!(items.contains("art/pet-idle.svg"), "{items}");
        for face in ["idle", "blink", "happy", "sleep", "worried"] {
            assert!(dir.join(format!("art/pet-{face}.svg")).is_file());
        }
        p.run_command("desk-pet.come-back", &mut h).unwrap();
        assert!(p.state.borrow().overlays.contains_key("pet"));
        assert!(h.log.iter().all(|l| l.0 != api::Level::Error), "{:?}", h.log);
    }

    /// The example in DemoBot: its panel draws, its check finds the TODO.
    #[test]
    fn demobot_example_plugin_works() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/DemoBot/plugins/demo.custom");
        let mut p = load("DemoBot custom blocks", &dir, "editor.js").0.unwrap();
        let mut h = host();
        let problems = p.check(&mut h).unwrap();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(problems[0].message, "TODO in Log (message): TODO say hi");
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| p.panel_ui("notes", ui, &mut h).unwrap()).textures_delta.clear();
        let view = &p.views["notes"];
        assert_eq!(view.error, None);
        let text = format!("{:?}", view.items);
        assert!(text.contains("Log: TODO say hi") && text.contains("Ideas, plans"), "{text}");
        p.run_command("demo.log_selection", &mut h).unwrap();
        assert_eq!(h.log.last().unwrap().1, r##"#b Log: {"message":"TODO say hi"}"##);
    }

    #[test]
    fn panels_get_the_kit_elements() {
        // Every kit element a panel can hold builds and draws without an error.
        let script = "xc.panel({ id: 'p', title: 'P', render(ui) {
            ui.toggle('On', true, () => {});
            ui.select('b', { a: 'A', b: 'B' }, () => {});
            ui.select('x', ['x', ['y', 'Why']], () => {}, { width: 120 });
            ui.number(5, () => {}, { min: 0, max: 10, step: 0.5, suffix: ' px' });
            ui.slider(50, 0, 100, () => {}, { suffix: ' %' });
            ui.color('#00D8F0', () => {});
            ui.note('Careful', { tone: 'warning', title: 'Heads up' });
            ui.checkbox('Check', false, () => {});
            ui.button('Go', () => {}, { primary: true });
            ui.input('text', () => {}, { placeholder: 'Type' });
        } });";
        let dir = plugin_dir(&[("editor.js", script)]);
        let mut p = load("P", &dir, "editor.js").0.unwrap();
        let mut h = host();
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| p.panel_ui("p", ui, &mut h).unwrap()).textures_delta.clear();
        let view = &p.views["p"];
        assert!(view.error.is_none(), "{:?}", view.error);
        let kinds: Vec<&str> = view.items.iter().map(|i| match i {
            Item::Toggle { .. } => "toggle",
            Item::Select { choices, .. } if choices.len() == 2 => "select",
            Item::Number { min: Some(_), .. } => "number",
            Item::Slider { .. } => "slider",
            Item::Color { .. } => "color",
            Item::Note { tone, .. } if tone == "warning" => "note",
            _ => "other",
        }).collect();
        assert_eq!(kinds, ["toggle", "select", "select", "number", "slider", "color", "note", "other", "other", "other"]);
    }

    #[test]
    fn script_errors_say_where() {
        // Loading fails with the file and the message.
        let dir = plugin_dir(&[("editor.js", "xc.command({ title: 'no id' });")]);
        let err = load("P", &dir, "editor.js").0.err().unwrap();
        assert!(err.contains("needs an id"), "{err}");
        // The app can't be used while loading, and other folders are out of reach.
        let dir = plugin_dir(&[("editor.js", "xc.project();")]);
        assert!(load("P", &dir, "editor.js").0.err().unwrap().contains("not while the script loads"));
        let dir = plugin_dir(&[("editor.js", "require('../../secret.js');")]);
        assert!(load("P", &dir, "editor.js").0.err().unwrap().contains("outside the plugin's folder"));
        let dir = plugin_dir(&[("editor.js", "require('node:fs');")]);
        assert!(load("P", &dir, "editor.js").0.err().unwrap().contains("only load their own files"));
        // A check that throws becomes a problem; a render that throws shows in the panel.
        let dir = plugin_dir(&[("editor.ts", "const n: number = 1;\nxc.check(() => { throw new Error('boom ' + n) });\nxc.panel({ id: 'p', title: 'P', render() { undefined.x } });")]);
        let mut p = load("P", &dir, "editor.ts").0.unwrap();
        let mut h = host();
        assert_eq!(p.check(&mut h).unwrap()[0].message.lines().next().unwrap(), "A check failed: boom 1");
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| p.panel_ui("p", ui, &mut h).unwrap()).textures_delta.clear();
        assert!(p.views["p"].error.as_deref().is_some_and(|e| e.contains("undefined")), "{:?}", p.views["p"].error);
    }

    #[test]
    fn keeps_requires_inside_the_plugin() {
        let root = Path::new("/plugins/demo");
        assert_eq!(inside(root, Path::new("lib/../util.js")), Some(PathBuf::from("/plugins/demo/util.js")));
        assert_eq!(inside(root, Path::new("../../etc/passwd")), None);
        let err = read_module(root, "", "fs").unwrap_err();
        assert!(err.contains("only load their own files"), "{err}");
    }
}
