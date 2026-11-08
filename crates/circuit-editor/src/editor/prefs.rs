//! App preferences, edited on the Settings page and kept between sessions.

use serde::{Deserialize, Serialize};

use crate::ui::theme::Theme;

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
    /// The Node.js bots run with; `None` uses `node` from the PATH.
    pub node_path: Option<String>,
    /// The First Start page was finished or skipped.
    pub setup_done: bool,
    /// A snapshot of the project on every save.
    pub backups: bool,
    /// Snapshots kept: the newest this many …
    pub backup_keep: usize,
    /// … plus one per day for this many days.
    pub backup_days: u32,
    /// Bigger controls with more room between them, for fingers.
    pub touch: bool,
    /// The interface language's code (`en`, `de`).
    pub language: String,
    /// The activity bar's icons, in order, by key: Workspaces, Stored Data and
    /// Plugins to start with, and what's been dragged there from the dock.
    pub activity: Vec<String>,
    /// The one of them the side bar shows; `None` folds the side bar away.
    pub side_shown: Option<String>,
    /// Another one shown in the side bar's lower half, under it.
    pub side_lower: Option<String>,
    /// How much of the side bar's height the upper one takes, with a lower one.
    pub side_split: f32,
    /// Plugins turned off, by id: found but not loaded.
    pub disabled_plugins: Vec<String>,
    /// Optional plugins that come with the app and were removed, by id: not
    /// loaded or listed (their files come back with every update), until
    /// added back.
    pub removed_plugins: Vec<String>,
    /// The Console's font, an installed font family by name; `None` for the
    /// built-in monospaced one.
    pub console_font: Option<String>,
    /// The Console's text size, in points.
    pub console_font_size: f32,
    /// Room between the Console's lines, in points.
    pub console_line_spacing: f32,
    /// Where File › New Project put the last project.
    pub new_project_location: Option<String>,
    /// Projects opened lately, the latest first (folders).
    pub recent_projects: Vec<String>,
    /// Look for a newer version on GitHub at start.
    pub check_updates: bool,
    /// A newer version the user doesn't want to hear about at start.
    pub skipped_update: Option<String>,
    /// Dropped blocks and notes line up with the canvas grid.
    pub snap_to_grid: bool,
    /// Blocks placed lately, the latest first (block ids), for the picker.
    pub recent_blocks: Vec<String>,
    /// Blocks starred in the picker, in the order they were starred.
    pub favorite_blocks: Vec<String>,
    /// Keyboard shortcuts changed in Settings › Shortcuts (see `keymap`).
    pub shortcuts: crate::editor::keymap::Custom,
    /// When the newest crash the user was told about happened (seconds).
    pub last_crash_seen: u64,
    /// Projects (folders) whose Run buttons act on their server: Server is
    /// picked beside Run.
    pub remote_bots: Vec<String>,
    /// The builders' and editors' windows (Embed Builder, Bot settings …)
    /// where they were last and as big: x, y, width and height, by kind.
    pub window_places: std::collections::BTreeMap<String, [f32; 4]>,
    /// Lighter drawing for older computers: `None` lets the app decide (on
    /// when it draws in software), or on or off by hand.
    pub lighter_drawing: Option<bool>,
}

/// How many projects File › Open Recent and the start page remember.
pub const RECENT_KEPT: usize = 10;

impl Prefs {
    /// Puts a project first in the recent list.
    pub fn remember_project(&mut self, root: &std::path::Path) {
        let path = root.display().to_string();
        self.recent_projects.retain(|p| !same_folder(p, &path));
        self.recent_projects.insert(0, path);
        self.recent_projects.truncate(RECENT_KEPT);
    }

    /// Takes a project off the recent list.
    pub fn forget_project(&mut self, root: &str) {
        self.recent_projects.retain(|p| !same_folder(p, root));
    }
}

/// Whether two paths name the same folder (Windows: any capitals, either slash).
fn same_folder(a: &str, b: &str) -> bool {
    let norm = |p: &str| p.trim_end_matches(['/', '\\']).replace('\\', "/").to_lowercase();
    norm(a) == norm(b)
}

/// The views of the side bar, one shown at a time (picked in the activity bar).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SideView {
    Workspaces,
    StoredData,
    Plugins,
}

impl SideView {
    pub const ALL: [SideView; 3] = [SideView::Workspaces, SideView::StoredData, SideView::Plugins];

    /// The name, in the activity bar's tooltips and the side bar's heading.
    pub fn name(self) -> &'static str {
        match self {
            SideView::Workspaces => t!("Workspaces"),
            SideView::StoredData => t!("Stored Data"),
            SideView::Plugins => t!("Plugins"),
        }
    }

    /// Its key in [`Prefs::activity`].
    pub fn key(self) -> &'static str {
        match self {
            SideView::Workspaces => "workspaces",
            SideView::StoredData => "stored-data",
            SideView::Plugins => "plugins",
        }
    }

    pub fn from_key(key: &str) -> Option<SideView> {
        SideView::ALL.into_iter().find(|v| v.key() == key)
    }

    pub fn icon(self) -> &'static str {
        match self {
            SideView::Workspaces => "layers",
            SideView::StoredData => "database",
            SideView::Plugins => "plug",
        }
    }
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { accent: None, ui_scale: 1.0, show_grid: true, wire_width: None, auto_restart: true, reopen_last_project: true, activity_bar: true, system_titlebar: false, node_path: None, setup_done: false, backups: true, backup_keep: 20, backup_days: 14, touch: false, language: crate::i18n::ENGLISH.into(), activity: SideView::ALL.iter().map(|v| v.key().to_string()).collect(), side_shown: Some(SideView::Workspaces.key().into()), side_lower: None, side_split: 0.5, disabled_plugins: Vec::new(), removed_plugins: Vec::new(), console_font: None, console_font_size: CONSOLE_FONT_SIZE, console_line_spacing: CONSOLE_LINE_SPACING, new_project_location: None, recent_projects: Vec::new(), check_updates: true, skipped_update: None, snap_to_grid: false, recent_blocks: Vec::new(), favorite_blocks: Vec::new(), shortcuts: Default::default(), last_crash_seen: 0, remote_bots: Vec::new(), window_places: Default::default(), lighter_drawing: None }
    }
}

/// Whether the app draws in software (no graphics card driver it could use),
/// as found when it started: lighter drawing is then on unless turned off.
static SOFTWARE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_software_drawing(on: bool) {
    SOFTWARE.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn software_drawing() -> bool {
    SOFTWARE.load(std::sync::atomic::Ordering::Relaxed)
}

/// The Console's text size to start with (egui's monospaced size).
pub const CONSOLE_FONT_SIZE: f32 = 13.0;

/// Room between the Console's lines to start with.
pub const CONSOLE_LINE_SPACING: f32 = 2.0;

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
    /// Tells the runner which Node.js to use.
    pub fn apply_node(&self) {
        circuit_runner::node::set_node_path(self.node_path.as_ref().map(std::path::PathBuf::from));
    }

    /// Puts the overrides on top of a theme's own colors and sizes.
    pub fn style(&self, theme: &mut Theme) {
        if let Some(c) = self.accent.as_deref().and_then(crate::builders::color::parse_hex) {
            theme.accent = c;
        }
        if let Some(w) = self.wire_width {
            theme.wire_width = w;
        }
        theme.touch = self.touch;
        theme.lighter = self.lighter_drawing.unwrap_or(software_drawing());
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
        assert_eq!(crate::builders::color::to_hex(theme.accent), "#FF5CA8");
        assert_eq!(theme.wire_width, 3.5);
        // Old saved settings without the newer fields still load.
        let old: Prefs = serde_json::from_str(r#"{"show_grid": false}"#).unwrap();
        assert!(!old.show_grid && old.auto_restart && old.ui_scale == 1.0);
    }
}
