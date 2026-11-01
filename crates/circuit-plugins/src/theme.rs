//! Theme files: color tokens plus a few shape and motion settings.

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ThemeDef {
    pub id: String,
    pub name: String,
    /// `dark` or `light`: the fallback for tokens this file doesn't set.
    pub base: String,
    pub colors: HashMap<String, String>,
    /// Overrides for pin type colors, keyed by type id (`text`, `discord.member`).
    pub pin_colors: HashMap<String, String>,
    pub shape: ThemeShape,
    pub motion: ThemeMotion,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ThemeShape {
    pub node_radius: f32,
    pub wire_width: f32,
    pub font: Option<String>,
    pub mono_font: Option<String>,
}

impl Default for ThemeShape {
    fn default() -> Self {
        Self { node_radius: 9.0, wire_width: 2.6, font: None, mono_font: None }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ThemeMotion {
    /// 0 turns animation off.
    pub speed: f32,
}

impl Default for ThemeMotion {
    fn default() -> Self {
        Self { speed: 1.0 }
    }
}

impl ThemeDef {
    pub fn is_dark(&self) -> bool {
        self.base != "light"
    }
}

/// Parses `#RRGGBB` or `#RRGGBBAA`.
pub fn parse_hex(s: &str) -> Option<[u8; 4]> {
    let h = s.trim().trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    match h.len() {
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
}
