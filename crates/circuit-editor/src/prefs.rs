//! App preferences, edited on the Settings page and kept between sessions.

use serde::{Deserialize, Serialize};

use crate::theme::Theme;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// `#RRGGBB` that replaces the theme's accent color.
    pub accent: Option<String>,
    /// Size of the whole interface, 1.0 = 100%.
    pub ui_scale: f32,
    pub show_grid: bool,
    /// Wire thickness; `None` uses the theme's.
    pub wire_width: Option<f32>,
    /// Saving restarts a running bot.
    pub auto_restart: bool,
    pub reopen_last_project: bool,
    pub activity_bar: bool,
    pub system_titlebar: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { accent: None, ui_scale: 1.0, show_grid: true, wire_width: None, auto_restart: true, reopen_last_project: true, activity_bar: true, system_titlebar: false }
    }
}

/// Interface sizes offered on the Settings page.
pub const UI_SCALES: [f32; 6] = [0.8, 0.9, 1.0, 1.1, 1.25, 1.5];

/// Accent colors offered next to the color picker.
pub const ACCENTS: [(&str, &str); 8] = [
    ("Aqua", "#00D8F0"),
    ("Mint", "#3DDC97"),
    ("Blurple", "#5865F2"),
    ("Violet", "#A66BFF"),
    ("Pink", "#FF5CA8"),
    ("Coral", "#FF7A59"),
    ("Amber", "#FFB547"),
    ("Lime", "#B4E33D"),
];

impl Prefs {
    /// Puts the overrides on top of a theme's own colors and sizes.
    pub fn style(&self, theme: &mut Theme) {
        if let Some(c) = self.accent.as_deref().and_then(crate::color::parse_hex) {
            theme.accent = c;
        }
        if let Some(w) = self.wire_width {
            theme.wire_width = w;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_apply_to_any_theme() {
        let mut theme = Theme::fallback();
        let original = theme.accent;
        Prefs::default().style(&mut theme);
        assert_eq!(theme.accent, original, "no override, no change");
        let prefs = Prefs { accent: Some("#FF5CA8".into()), wire_width: Some(3.5), ..Default::default() };
        prefs.style(&mut theme);
        assert_eq!(crate::color::to_hex(theme.accent), "#FF5CA8");
        assert_eq!(theme.wire_width, 3.5);
        // Old saved settings without the newer fields still load.
        let old: Prefs = serde_json::from_str(r#"{"show_grid": false}"#).unwrap();
        assert!(!old.show_grid && old.auto_restart && old.ui_scale == 1.0);
    }
}
