//! `plugin.toml`.

use indexmap::IndexMap;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub icon: Option<String>,
    pub engines: Engines,
    pub dependencies: IndexMap<String, String>,
    /// Block ids from other plugins this plugin may replace.
    pub overrides: Vec<String>,
    pub permissions: Permissions,
    pub contributes: Contributes,
    /// A native Rust part, loaded by the editor (see the xc-circuit-plugin crate).
    pub native: Option<NativeManifest>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct NativeManifest {
    /// Library name without prefix or extension: `xc_stats` means `xc_stats.dll` or `libxc_stats.so`.
    pub library: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Engines {
    pub circuit: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Permissions {
    pub fs: Vec<String>,
    pub network: Vec<String>,
    pub process: bool,
    pub clipboard: bool,
}

/// Every contribution point is a list of paths or globs relative to the plugin folder.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Contributes {
    pub build_modes: Vec<String>,
    pub blocks: Vec<String>,
    pub pin_types: Vec<String>,
    pub field_types: Vec<String>,
    pub project_editors: Vec<String>,
    pub runtime_libs: Vec<String>,
    pub storage_backends: Vec<String>,
    pub panels: Vec<String>,
    pub widgets: Vec<String>,
    pub commands: Vec<String>,
    pub menus: Vec<String>,
    pub keybindings: Vec<String>,
    pub themes: Vec<String>,
    pub layouts: Vec<String>,
    pub exporters: Vec<String>,
    pub importers: Vec<String>,
    pub templates: Vec<String>,
    pub icons: Vec<String>,
    pub locales: Vec<String>,
}

/// The host API version this build implements.
pub const HOST_API: (u64, u64) = (1, 0);

/// Checks `engines.circuit` (`^1.0`, `1.x`, `>=1.0`). A missing range is accepted.
pub fn engine_supported(range: Option<&str>) -> bool {
    let Some(range) = range else { return true };
    let digits: String = range.chars().skip_while(|c| !c.is_ascii_digit()).collect();
    let mut parts = digits.split('.');
    let major: u64 = parts.next().and_then(|m| m.parse().ok()).unwrap_or(0);
    let minor: u64 = parts.next().and_then(|m| m.parse().ok()).unwrap_or(0);
    if range.trim_start().starts_with(">=") {
        return (HOST_API.0, HOST_API.1) >= (major, minor);
    }
    major == HOST_API.0 && HOST_API.1 >= minor
}
