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
    fn reveal(&mut self, workspace: &str, node: &str);
    /// The display name of a block type such as `discord.send_message`.
    fn block_name(&self, block: &str) -> Option<String>;
    /// A value from the project's `data` section, as JSON.
    fn project_data(&self, key: &str) -> Option<String>;
    fn set_project_data(&mut self, key: &str, json: &str) -> Result<(), String>;
    fn log(&mut self, level: Level, text: &str);
    /// Runs an editor command (`bot.run`, `bot.stop`, `file.save`) or another plugin's command.
    fn run_command(&mut self, id: &str);
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
}
