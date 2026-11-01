//! Native XC Circuit plugins.
//!
//! A native plugin is a Rust library built as a `cdylib` (`.dll` on Windows,
//! `.so` on Linux) that the editor loads. It draws its panels with egui
//! directly, so it looks like the rest of the app and follows the theme.
//!
//! ```ignore
//! use xc_circuit_plugin::{egui, Command, Host, Plugin, PanelInfo};
//!
//! #[derive(Default)]
//! struct Hello { clicks: u32 }
//!
//! impl Plugin for Hello {
//!     fn commands(&self) -> Vec<Command> {
//!         vec![Command::new("hello.greet", "Say hello")]
//!     }
//!     fn run_command(&mut self, _id: &str, host: &mut dyn Host) {
//!         host.log(xc_circuit_plugin::Level::Info, "Hello from Rust!");
//!     }
//!     fn panels(&self) -> Vec<PanelInfo> {
//!         vec![PanelInfo::new("hello", "Hello")]
//!     }
//!     fn panel_ui(&mut self, _id: &str, ui: &mut egui::Ui, _host: &mut dyn Host) {
//!         if ui.button(format!("Clicked {} times", self.clicks)).clicked() {
//!             self.clicks += 1;
//!         }
//!     }
//! }
//!
//! xc_circuit_plugin::export_plugin!(Hello);
//! ```
//!
//! Native plugins share memory with the editor, so a plugin must use the very
//! same build of egui as the editor: same Rust compiler, same crate versions
//! and features, same profile (debug or release). Build plugins as members of
//! the XC Circuit workspace, together with the editor (`cargo build` at the
//! workspace root). The editor checks this before loading and says what differs.

use std::any::Any;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::OnceLock;

pub use egui;

/// Bumped whenever the traits below change.
pub const ABI_VERSION: u32 = 1;

/// Symbol names the editor looks up.
pub const FINGERPRINT_SYMBOL: &[u8] = b"xc_circuit_plugin_fingerprint";
pub const CREATE_SYMBOL: &[u8] = b"xc_circuit_plugin_create";

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
}

/// Implement this for your plugin type, then call [`export_plugin!`].
/// Every method has a default, so implement only what you need.
pub trait Plugin: Send + 'static {
    fn commands(&self) -> Vec<Command> {
        Vec::new()
    }
    fn run_command(&mut self, _id: &str, _host: &mut dyn Host) {}
    fn panels(&self) -> Vec<PanelInfo> {
        Vec::new()
    }
    fn panel_ui(&mut self, _id: &str, _ui: &mut egui::Ui, _host: &mut dyn Host) {}
    /// Called about once a second; the results show in the Problems panel.
    fn check(&mut self, _host: &mut dyn Host) -> Vec<Problem> {
        Vec::new()
    }
    fn on_bot_event(&mut self, _event: &BotEvent, _host: &mut dyn Host) {}
    fn on_save(&mut self, _host: &mut dyn Host) {}
}

/// What the editor calls. Each call catches the plugin's panics on the
/// plugin's side and reports them as errors.
#[doc(hidden)]
pub trait PluginObject {
    fn commands(&self) -> Result<Vec<Command>, String>;
    fn run_command(&mut self, id: &str, host: &mut dyn Host) -> Result<(), String>;
    fn panels(&self) -> Result<Vec<PanelInfo>, String>;
    fn panel_ui(&mut self, id: &str, ui: &mut egui::Ui, host: &mut dyn Host) -> Result<(), String>;
    fn check(&mut self, host: &mut dyn Host) -> Result<Vec<Problem>, String>;
    fn on_bot_event(&mut self, event: &BotEvent, host: &mut dyn Host) -> Result<(), String>;
    fn on_save(&mut self, host: &mut dyn Host) -> Result<(), String>;
}

#[doc(hidden)]
pub struct Guarded<T>(pub T);

fn guard<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|p| panic_message(&*p))
}

fn panic_message(p: &(dyn Any + Send)) -> String {
    p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "the plugin panicked".into())
}

impl<T: Plugin> PluginObject for Guarded<T> {
    fn commands(&self) -> Result<Vec<Command>, String> {
        guard(|| self.0.commands())
    }
    fn run_command(&mut self, id: &str, host: &mut dyn Host) -> Result<(), String> {
        guard(|| self.0.run_command(id, host))
    }
    fn panels(&self) -> Result<Vec<PanelInfo>, String> {
        guard(|| self.0.panels())
    }
    fn panel_ui(&mut self, id: &str, ui: &mut egui::Ui, host: &mut dyn Host) -> Result<(), String> {
        guard(|| self.0.panel_ui(id, ui, host))
    }
    fn check(&mut self, host: &mut dyn Host) -> Result<Vec<Problem>, String> {
        guard(|| self.0.check(host))
    }
    fn on_bot_event(&mut self, event: &BotEvent, host: &mut dyn Host) -> Result<(), String> {
        guard(|| self.0.on_bot_event(event, host))
    }
    fn on_save(&mut self, host: &mut dyn Host) -> Result<(), String> {
        guard(|| self.0.on_save(host))
    }
}

/// The signature of the `xc_circuit_plugin_create` symbol.
pub type CreateFn = fn() -> Result<Box<dyn PluginObject>, String>;

/// Builds the plugin, catching a panic in its `Default` on the plugin's side.
#[doc(hidden)]
pub fn create<T: Plugin + Default>() -> Result<Box<dyn PluginObject>, String> {
    guard(|| Box::new(Guarded(T::default())) as Box<dyn PluginObject>)
}
/// The signature of the `xc_circuit_plugin_fingerprint` symbol.
pub type FingerprintFn = extern "C" fn() -> *const c_char;

/// Exports a [`Plugin`] type (which must implement `Default`) from a `cdylib`.
#[macro_export]
macro_rules! export_plugin {
    ($ty:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn xc_circuit_plugin_fingerprint() -> *const ::std::ffi::c_char {
            $crate::fingerprint_cstr().as_ptr()
        }

        #[unsafe(no_mangle)]
        pub fn xc_circuit_plugin_create() -> ::std::result::Result<::std::boxed::Box<dyn $crate::PluginObject>, ::std::string::String> {
            $crate::create::<$ty>()
        }
    };
}

/// Describes this build: everything that has to match between the editor and
/// a plugin for them to share egui and Rust types. One `key=value` per line.
pub fn fingerprint() -> String {
    use std::mem::size_of;
    let layout = [
        ("Ui", size_of::<egui::Ui>()),
        ("Style", size_of::<egui::Style>()),
        ("Visuals", size_of::<egui::Visuals>()),
        ("Memory", size_of::<egui::Memory>()),
        ("Response", size_of::<egui::Response>()),
        ("InputState", size_of::<egui::InputState>()),
        ("Shape", size_of::<egui::Shape>()),
        ("Options", size_of::<egui::Options>()),
        ("Command", size_of::<Command>()),
        ("BotEvent", size_of::<BotEvent>()),
    ]
    .iter()
    .map(|(n, s)| format!("{n}:{s}"))
    .collect::<Vec<_>>()
    .join(",");
    [
        format!("abi={ABI_VERSION}"),
        format!("api={}", env!("CARGO_PKG_VERSION")),
        format!("rustc={}", env!("XC_PLUGIN_RUSTC")),
        format!("target={}", env!("XC_PLUGIN_TARGET")),
        format!("profile={}", if cfg!(debug_assertions) { "debug" } else { "release" }),
        format!("deps={}", env!("XC_PLUGIN_DEPS")),
        format!("layout={layout}"),
        // Type ids differ whenever egui or any crate under it was built with
        // other features, even at the same versions.
        format!("types={:?},{:?},{:?}", std::any::TypeId::of::<egui::Context>(), std::any::TypeId::of::<egui::Memory>(), std::any::TypeId::of::<egui::epaint::text::Fonts>()),
    ]
    .join("\n")
}

#[doc(hidden)]
pub fn fingerprint_cstr() -> &'static CStr {
    static FP: OnceLock<CString> = OnceLock::new();
    FP.get_or_init(|| CString::new(fingerprint()).unwrap_or_default())
}

/// Explains how a plugin's fingerprint differs from the editor's, or `None`
/// when they match.
pub fn compare_fingerprints(editor: &str, plugin: &str) -> Option<String> {
    let parse = |s: &str| -> Vec<(String, String)> { s.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect() };
    let (e, p) = (parse(editor), parse(plugin));
    let get = |list: &[(String, String)], key: &str| list.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or_default();
    let mut diffs = Vec::new();
    for (key, ev) in &e {
        let pv = get(&p, key);
        if *ev == pv {
            continue;
        }
        if key == "types" {
            diffs.push("it was built against a different build of egui; build it together with the editor: `cargo plugin <crate>` in the XC Circuit workspace".into());
        } else if key == "deps" && ev != "unknown" && pv != "unknown" {
            let (es, ps): (Vec<&str>, Vec<&str>) = (ev.split(',').collect(), pv.split(',').collect());
            let only = |a: &[&str], b: &[&str]| a.iter().filter(|x| !b.contains(x)).copied().collect::<Vec<_>>().join(", ");
            diffs.push(format!("egui crates differ (editor has {}; plugin has {})", only(&es, &ps), only(&ps, &es)));
        } else {
            diffs.push(format!("{key}: editor {ev}, plugin {}", if pv.is_empty() { "(missing)" } else { &pv }));
        }
    }
    (!diffs.is_empty()).then(|| diffs.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_lists_versions() {
        let fp = fingerprint();
        assert!(fp.contains("deps=") && fp.contains("egui@0.36"), "{fp}");
        assert!(fp.contains("rustc=rustc "), "{fp}");
        assert_eq!(compare_fingerprints(&fp, &fp), None);
    }

    #[test]
    fn explains_differences() {
        let editor = "abi=1\nprofile=release\ndeps=egui@0.36.0,ahash@0.8.12";
        let plugin = "abi=1\nprofile=debug\ndeps=egui@0.36.0,ahash@0.8.11";
        let why = compare_fingerprints(editor, plugin).unwrap();
        assert!(why.contains("profile: editor release, plugin debug"), "{why}");
        assert!(why.contains("editor has ahash@0.8.12; plugin has ahash@0.8.11"), "{why}");
    }

    #[test]
    fn panics_become_errors() {
        #[derive(Default)]
        struct Bad;
        impl Plugin for Bad {
            fn commands(&self) -> Vec<Command> {
                panic!("boom")
            }
        }
        let obj = Guarded(Bad);
        assert_eq!(obj.commands().unwrap_err(), "boom");
    }
}
