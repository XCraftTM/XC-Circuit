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
    pub contributes: Contributes,
    /// A native Rust part, loaded by the editor (see the xc-circuit-plugin crate).
    pub native: Option<NativeManifest>,
    /// A JavaScript part that runs in the editor: commands, panels and checks.
    pub editor: Option<EditorManifest>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct EditorManifest {
    /// The script, relative to the plugin folder (`editor.js` or `editor.ts`).
    pub script: String,
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
    /// The oldest XC Circuit version the plugin works with (`0.2.0`), for
    /// plugins that need something newer apps have. Missing: any.
    pub app: Option<String>,
}

/// This app's version: plugins name the oldest one they work with.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Compares versions such as `1.2.0` and `1.10.0` part by part, as numbers.
/// A pre-release (`1.2.0-beta`) counts as its version without the suffix.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |v: &str| -> Vec<u64> { v.trim().trim_start_matches('v').split('-').next().unwrap_or("").split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    let (a, b) = (parts(a), parts(b));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x.cmp(&y);
        }
    }
    std::cmp::Ordering::Equal
}

/// Whether this app is new enough for a plugin's `engines.app`.
pub fn app_supported(min: Option<&str>) -> bool {
    min.is_none_or(|m| compare_versions(APP_VERSION, m) != std::cmp::Ordering::Less)
}

/// Every contribution point is a list of paths or globs relative to the plugin folder.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Contributes {
    pub build_modes: Vec<String>,
    pub blocks: Vec<String>,
    pub pin_types: Vec<String>,
    /// Class catalogs (JSON): the members of the values behind pin types, for
    /// `{#1.user.username}` in texts.
    pub classes: Vec<String>,
    pub themes: Vec<String>,
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
