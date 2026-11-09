//! What editor plugins see of the editor ([`Host`]) and what the editor
//! calls on them ([`PluginObject`]): commands, panels, checks and the bot's
//! events. Project data passes as JSON text, in the project files' shape.

use std::path::PathBuf;

use eframe::egui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Warn,
    Error,
}

/// A command shown in the Plugins menu and the command palette.
#[derive(Clone, Debug)]
pub struct Command {
    pub id: String,
    pub title: String,
    pub description: String,
}

impl Command {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Command {
        Command { id: id.into(), title: title.into(), description: String::new() }
    }

    pub fn description(mut self, d: impl Into<String>) -> Command {
        self.description = d.into();
        self
    }
}

/// A dockable panel, opened from the Plugins menu or the command palette.
#[derive(Clone, Debug)]
pub struct PanelInfo {
    pub id: String,
    pub title: String,
}

impl PanelInfo {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> PanelInfo {
        PanelInfo { id: id.into(), title: title.into() }
    }
}

/// An entry for the Problems panel. Point it at a block to make it clickable.
#[derive(Clone, Debug)]
pub struct Problem {
    pub message: String,
    pub workspace: Option<String>,
    pub node: Option<String>,
}

impl Problem {
    pub fn new(message: impl Into<String>) -> Problem {
        Problem { message: message.into(), workspace: None, node: None }
    }

    pub fn at(mut self, workspace: impl Into<String>, node: impl Into<String>) -> Problem {
        self.workspace = Some(workspace.into());
        self.node = Some(node.into());
        self
    }
}

/// Something the running bot reported.
#[derive(Clone, Debug)]
pub enum BotEvent {
    Started,
    Stopped,
    Log { level: Level, text: String },
    /// A block started (`phase` "enter") or finished ("exit"), or failed (`error`).
    Block { workspace: String, node: String, phase: String, ms: Option<f64>, error: Option<String> },
}

/// A workspace in the open project.
/// Something AI agents can do through the app's MCP server (`xc.tool`).
#[derive(Clone, Debug)]
pub struct Tool {
    pub name: String,
    pub description: String,
    /// What it takes, as a JSON Schema object.
    pub input: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct WorkspaceInfo {
    pub key: String,
    pub name: String,
}

/// What the editor lets a plugin see and do. Project data is passed as JSON
/// text in the same shape as the project files.
pub trait Host {
    fn project_name(&self) -> Option<String>;
    fn project_root(&self) -> Option<PathBuf>;
    fn workspaces(&self) -> Vec<WorkspaceInfo>;
    /// A workspace as JSON (`name`, `nodes`, `links`, `zones`).
    fn workspace_json(&self, key: &str) -> Option<String>;
    /// Replaces a workspace. The change can be undone like any edit.
    fn set_workspace_json(&mut self, key: &str, json: &str) -> Result<(), String>;
    fn active_workspace(&self) -> Option<String>;
    /// Selected block ids in the active workspace.
    fn selection(&self) -> Vec<String>;
    /// Opens a workspace, selects a block and centers the canvas on it.
    /// Opens a workspace and centres it on a block, at `zoom` (1 is 100%) when given.
    fn reveal(&mut self, workspace: &str, node: &str, zoom: Option<f32>);
    /// The display name of a block type such as `discord.send_message`.
    fn block_name(&self, block: &str) -> Option<String>;
    /// A value from the project's `data` section, as JSON.
    fn project_data(&self, key: &str) -> Option<String>;
    fn set_project_data(&mut self, key: &str, json: &str) -> Result<(), String>;
    fn log(&mut self, level: Level, text: &str);
    /// Runs an editor command (`bot.run`, `bot.stop`, `file.save`) or another
    /// plugin's command. `by_user`: asked for from a command, a click or an
    /// agent's tool, not by the script on its own (a timer, a check, while
    /// drawing or loading). An error when it isn't allowed that way.
    fn run_command(&mut self, id: &str, by_user: bool) -> Result<(), String>;
    fn bot_running(&self) -> bool;
    /// This plugin's own small key-value store, kept between sessions.
    fn storage_get(&self, key: &str) -> Option<String>;
    fn storage_set(&mut self, key: &str, value: &str);
    /// Places a block (`core.log`) at `at` on a workspace's canvas, with
    /// `values_json` (an object) over its defaults. Returns its id.
    fn add_block(&mut self, _workspace: &str, _block: &str, _at: [f32; 2], _values_json: &str) -> Result<String, String> {
        Err("this editor can't place blocks for plugins".into())
    }
    /// Selects blocks of a workspace.
    fn select(&mut self, _workspace: &str, _nodes: &[String]) {}
    /// The installed block types matching `query` (words in their name,
    /// aliases, tags, category or description; empty: all).
    fn block_types(&self, _query: &str) -> serde_json::Value {
        serde_json::Value::Array(Vec::new())
    }
    /// A block type: its fields, outputs and flows.
    fn block_type(&self, _block: &str) -> Option<serde_json::Value> {
        None
    }
    /// A block on a workspace as it is now: its values and the pins it has
    /// (a block's `define` can add some), with their types.
    fn block_pins(&mut self, _workspace: &str, _node: &str) -> Result<serde_json::Value, String> {
        Err("this editor can't describe blocks".into())
    }
    /// Connects two pins (`node.pin`), checked like a wire drawn on the canvas.
    fn connect(&mut self, _workspace: &str, _from: &str, _to: &str) -> Result<(), String> {
        Err("this editor can't connect blocks for plugins".into())
    }
    /// Makes a workspace (in a sidebar folder, when given). Returns its key.
    fn add_workspace(&mut self, _name: &str, _folder: Option<&str>) -> Result<String, String> {
        Err("this editor can't make workspaces for plugins".into())
    }
    /// What the Problems panel lists: [{ source, message, workspace, block }].
    fn problems(&mut self) -> serde_json::Value {
        serde_json::Value::Array(Vec::new())
    }
    /// The Console's last `lines` lines: [{ time, level, text, workspace, block }].
    fn console(&self, _lines: usize) -> serde_json::Value {
        serde_json::Value::Array(Vec::new())
    }
    /// `text` in the app's language, from the plugins' translations.
    fn translate(&self, text: &str) -> String {
        text.to_string()
    }
    /// Puts text on the clipboard.
    fn copy_text(&mut self, _text: &str) {}
    /// Where a workspace's blocks are and how big, as the canvas last drew
    /// them: { id: [x, y, width, height] }. Empty before it was shown.
    fn block_rects(&self, _workspace: &str) -> serde_json::Value {
        serde_json::json!({})
    }
    /// A value for a block's field in the shape the block keeps: an embed,
    /// layout or canvas as the builders save it (Discord's own JSON works
    /// for embeds and layouts). Other fields' values stay as they are.
    fn field_value(&self, _block: &str, _field: &str, value: &serde_json::Value) -> Result<serde_json::Value, String> {
        Ok(value.clone())
    }
    /// Opens the builder for a block's embed, layout or canvas field (or
    /// Slash Commands, for a Slash Command block).
    fn open_builder(&mut self, _workspace: &str, _node: &str, _field: &str) -> Result<String, String> {
        Err("this editor can't open builders for plugins".into())
    }
    /// The AI clients the app's MCP server can be added to, and whether it is.
    fn agent_clients(&self) -> serde_json::Value {
        serde_json::Value::Array(Vec::new())
    }
    /// Adds the MCP server to an AI client (or takes it out). Says what happened.
    fn agent_install(&mut self, _client: &str, _remove: bool) -> Result<String, String> {
        Err("this editor can't set up AI clients".into())
    }
    /// The MCP server's entry for a client's config file, as JSON text.
    fn agent_snippet(&self) -> String {
        String::new()
    }
    /// Whether the MCP server is listening, and how many agents are connected.
    fn agent_link(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
    /// The app's settings (Settings page): theme, sizes, the Console's font,
    /// shortcuts … Nothing secret is in them.
    fn app_settings(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
    /// Bot › Settings of the open project: each setting with its page, type,
    /// choices, default and value. Secrets (the token) only say whether one
    /// is saved, never what it is.
    fn bot_settings(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
    /// Changes Bot › Settings (`{ key: value }`); secrets can't be set this
    /// way. Returns the keys changed.
    fn set_bot_settings(&mut self, _values: &serde_json::Value) -> Result<serde_json::Value, String> {
        Err("this editor can't change bot settings for plugins".into())
    }
    /// The bot's server (Bot › Server…): where it uploads, whether Run acts
    /// there, the server's state, the last upload and the server console's
    /// last lines. Never passwords or keys.
    fn server(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
    /// The installed plugins (and turned-off ones), with their blocks.
    fn plugins(&self) -> serde_json::Value {
        serde_json::Value::Array(Vec::new())
    }
    /// A block type's source file: its plugin, the file and the code.
    fn block_source(&self, _block: &str) -> Result<serde_json::Value, String> {
        Err("this editor can't show block sources".into())
    }
}

/// What the editor calls on a plugin (a script, see [`crate::plugins::script`]).
/// Each call reports what went wrong as an error.
pub trait PluginObject {
    fn commands(&self) -> Result<Vec<Command>, String>;
    fn run_command(&mut self, id: &str, host: &mut dyn Host) -> Result<(), String>;
    fn panels(&self) -> Result<Vec<PanelInfo>, String>;
    fn panel_ui(&mut self, id: &str, ui: &mut egui::Ui, host: &mut dyn Host) -> Result<(), String>;
    fn check(&mut self, host: &mut dyn Host) -> Result<Vec<Problem>, String>;
    fn on_bot_event(&mut self, event: &BotEvent, host: &mut dyn Host) -> Result<(), String>;
    fn on_save(&mut self, host: &mut dyn Host) -> Result<(), String>;
    fn tick(&mut self, host: &mut dyn Host) -> Result<Option<std::time::Duration>, String>;
    fn overlay_ui(&mut self, ctx: &egui::Context, host: &mut dyn Host) -> Result<(), String>;
    /// What it offers AI agents.
    fn tools(&self) -> Vec<Tool> {
        Vec::new()
    }
    /// Runs one of its tools with `args` (an object). Its answer: text, or
    /// any JSON value.
    fn call_tool(&mut self, name: &str, _args: &serde_json::Value, _host: &mut dyn Host) -> Result<serde_json::Value, String> {
        Err(format!("no tool `{name}`"))
    }
}
