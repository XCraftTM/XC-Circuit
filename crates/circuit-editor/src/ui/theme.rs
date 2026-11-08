//! Turns a theme file into egui visuals, dock styling and canvas colors.

use std::collections::HashMap;

use circuit_core::PinType;
use circuit_plugins::theme::parse_hex;
use circuit_plugins::{Registry, ThemeDef};
use eframe::egui::{self, Color32, CornerRadius, Margin, Shadow, Stroke, Visuals};

/// Where `apply` leaves the theme in egui's memory.
pub(crate) const KIT_THEME: &str = "xc-kit-theme";

#[derive(Clone, Debug)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub dark: bool,
    pub frame: Color32,
    pub panel: Color32,
    pub panel_header: Color32,
    pub canvas: Color32,
    pub canvas_dots: Color32,
    pub node: Color32,
    pub node_header: Color32,
    pub node_border: Color32,
    pub field: Color32,
    pub line: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub action: Color32,
    pub status_bar: Color32,
    pub status_text: Color32,
    pub error: Color32,
    pub warning: Color32,
    pub pin_colors: HashMap<String, Color32>,
    pub node_radius: f32,
    pub wire_width: f32,
    /// Touch mode (a preference, not part of the theme file): bigger
    /// controls and more room between them.
    pub touch: bool,
    /// Lighter drawing, for older computers (preferences, or all of it by
    /// itself when the app draws in software): which parts are left out.
    pub lighter: Lighter,
}

/// The parts of lighter drawing, each chosen on its own: `true` leaves that
/// part out (or keeps it still), so there's less to draw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Lighter {
    /// No shadows under windows, menus and blocks.
    pub shadows: bool,
    /// No fading in and out: things appear and go at once.
    pub animations: bool,
    /// Glows (empty required fields, blocks that just ran) that don't pulse,
    /// so the app doesn't draw itself again 30 times a second for them.
    pub glows: bool,
    /// Edges without their soft fringe, which about halves the triangles.
    pub smoothing: bool,
}

impl Lighter {
    pub const ALL: Lighter = Lighter { shadows: true, animations: true, glows: true, smoothing: true };
    pub const NONE: Lighter = Lighter { shadows: false, animations: false, glows: false, smoothing: false };

    fn parts(self) -> [bool; 4] {
        [self.shadows, self.animations, self.glows, self.smoothing]
    }

    pub fn all(self) -> bool {
        self.parts().iter().all(|p| *p)
    }

    pub fn any(self) -> bool {
        self.parts().iter().any(|p| *p)
    }
}

impl Theme {
    /// Used only when no theme file could be loaded at all.
    pub fn fallback() -> Theme {
        Theme::from_def(&ThemeDef { id: "fallback".into(), name: "Fallback".into(), base: "dark".into(), ..Default::default() })
    }

    pub fn from_def(def: &ThemeDef) -> Theme {
        let dark = def.is_dark();
        let c = |key: &str, dark_default: &str, light_default: &str| -> Color32 {
            let hex = def.colors.get(key).map(String::as_str).unwrap_or(if dark { dark_default } else { light_default });
            hex_color(hex).unwrap_or(Color32::MAGENTA)
        };
        let pin_colors = def.pin_colors.iter().filter_map(|(k, v)| hex_color(v).map(|c| (k.clone(), c))).collect();
        Theme {
            id: def.id.clone(),
            name: def.name.clone(),
            dark,
            frame: c("frame", "#18191C", "#DCE1E5"),
            panel: c("panel", "#232529", "#F4F6F8"),
            panel_header: c("panelHeader", "#2B2E33", "#E7EBEE"),
            canvas: c("canvas", "#1E2023", "#E8ECEF"),
            canvas_dots: c("canvasDots", "#FFFFFF16", "#00000018"),
            node: c("node", "#2C2F34", "#FFFFFF"),
            node_header: c("nodeHeader", "#363A41", "#F0F3F5"),
            node_border: c("nodeBorder", "#454A52", "#C9D1D7"),
            field: c("field", "#40454D", "#E4E9ED"),
            line: c("line", "#34383E", "#D2D9DF"),
            text: c("text", "#E7EAED", "#141A1F"),
            text_muted: c("textMuted", "#A9B1B9", "#4C5863"),
            accent: c("accent", "#00D8F0", "#008C9C"),
            action: c("action", "#3DFF6E", "#13A43B"),
            status_bar: c("statusBar", "#26282C", "#E8ECEF"),
            status_text: c("statusText", "#AEB3BA", "#4A535C"),
            error: c("error", "#FF6F91", "#C42A52"),
            warning: c("warning", "#FFB454", "#B26A00"),
            pin_colors,
            node_radius: def.shape.node_radius,
            wire_width: def.shape.wire_width,
            touch: false,
            lighter: Lighter::NONE,
        }
    }

    /// Color of a pin or wire of the given type.
    pub fn pin_color(&self, ty: &PinType, reg: &Registry) -> Color32 {
        let base = |key: &str, default: &str| self.pin_colors.get(key).copied().unwrap_or_else(|| hex_color(default).unwrap());
        match ty.required() {
            PinType::Action => self.action,
            PinType::Text => base("text", "#F5C451"),
            PinType::Number => base("number", "#4FB3FF"),
            PinType::Boolean => base("boolean", "#FF6F91"),
            PinType::Date => base("date", "#FF9F5A"),
            PinType::Any => base("any", "#9AA3AB"),
            PinType::Object => base("object", "#3DD9C1"),
            PinType::List(_) => base("list", "#B98CFF"),
            PinType::Union(ts) => ts.first().map(|t| self.pin_color(t, reg)).unwrap_or(self.text_muted),
            PinType::Optional(t) => self.pin_color(t, reg),
            PinType::Named(id) => self
                .pin_colors
                .get(id)
                .copied()
                .or_else(|| reg.types.get(id).and_then(|t| t.color.as_deref()).and_then(hex_color))
                .unwrap_or_else(|| base("object", "#3DD9C1")),
        }
    }

    pub fn apply(&self, ctx: &egui::Context) {
        // For code that only has a ui (plugins' panels): kit::theme_of.
        ctx.data_mut(|d| d.insert_temp(egui::Id::unique(KIT_THEME), self.clone()));
        let mut v = if self.dark { Visuals::dark() } else { Visuals::light() };
        v.interact_cursor = Some(egui::CursorIcon::PointingHand);
        let radius = CornerRadius::same(6);
        v.panel_fill = self.panel;
        v.window_fill = self.panel_header;
        v.window_stroke = Stroke::new(1.0, self.node_border);
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(8);
        v.window_shadow = Shadow { offset: [0, 10], blur: 28, spread: 0, color: Color32::from_black_alpha(110) };
        v.popup_shadow = Shadow { offset: [0, 8], blur: 20, spread: 0, color: Color32::from_black_alpha(90) };
        v.extreme_bg_color = self.field;
        v.text_edit_bg_color = Some(self.field);
        v.faint_bg_color = self.panel_header;
        v.code_bg_color = self.field;
        v.hyperlink_color = self.accent;
        v.warn_fg_color = self.warning;
        v.error_fg_color = self.error;
        // Strong enough to see marked text at a glance on the input fields.
        v.selection.bg_fill = with_alpha(self.accent, if self.dark { 140 } else { 110 });
        v.selection.stroke = Stroke::new(1.0, self.text);

        let w = &mut v.widgets;
        w.noninteractive.bg_fill = self.panel;
        w.noninteractive.weak_bg_fill = self.panel;
        w.noninteractive.bg_stroke = Stroke::new(1.0, self.line);
        w.noninteractive.fg_stroke = Stroke::new(1.0, self.text);
        w.noninteractive.corner_radius = radius;
        for (state, bg, weak) in [
            (&mut w.inactive, self.field, self.panel_header),
            (&mut w.hovered, mix(self.field, self.text, 0.08), mix(self.panel_header, self.text, 0.08)),
            (&mut w.active, mix(self.field, self.text, 0.14), mix(self.panel_header, self.text, 0.14)),
            (&mut w.open, mix(self.field, self.text, 0.08), mix(self.panel_header, self.text, 0.08)),
        ] {
            state.bg_fill = bg;
            state.weak_bg_fill = weak;
            state.fg_stroke = Stroke::new(1.0, self.text);
            state.corner_radius = radius;
        }
        // Buttons and other controls always show their frame, so they read as
        // something to press (menus draw their items without one).
        w.inactive.bg_stroke = Stroke::new(1.0, self.node_border);
        w.hovered.bg_stroke = Stroke::new(1.0, self.node_border);
        w.active.bg_stroke = Stroke::new(1.0, self.accent);
        w.open.bg_stroke = Stroke::new(1.0, self.node_border);

        if self.lighter.shadows {
            v.window_shadow = Shadow::NONE;
            v.popup_shadow = Shadow::NONE;
        }
        let theme = if self.dark { egui::Theme::Dark } else { egui::Theme::Light };
        ctx.set_visuals_of(theme, v);
        let lighter = self.lighter;
        ctx.tessellation_options_mut(|t| t.feathering = !lighter.smoothing);
        ctx.set_theme(theme);
        let touch = self.touch;
        ctx.all_styles_mut(|s| {
            // Lighter drawing: things appear and go at once, no fading.
            s.animation_time = if lighter.animations { 0.0 } else { egui::Style::default().animation_time };
            if touch {
                // Everything a finger presses is at least 40 points tall.
                s.spacing.item_spacing = egui::vec2(12.0, 10.0);
                s.spacing.button_padding = egui::vec2(16.0, 10.0);
                s.spacing.interact_size = egui::vec2(48.0, 40.0);
                s.spacing.icon_width = 22.0;
                s.spacing.icon_width_inner = 12.0;
                s.spacing.icon_spacing = 8.0;
                s.spacing.menu_margin = Margin::same(10);
                s.spacing.scroll.bar_width = 14.0;
                s.spacing.combo_height = 320.0;
                s.interaction.resize_grab_radius_side = 10.0;
                s.interaction.resize_grab_radius_corner = 16.0;
            } else {
                compact(s);
                let i = egui::style::Interaction::default();
                s.interaction.resize_grab_radius_side = i.resize_grab_radius_side;
                s.interaction.resize_grab_radius_corner = i.resize_grab_radius_corner;
            }
        });
    }
}

/// The normal (not touch) sizes. Also used inside places with a fixed size in
/// touch mode: the title bar, and the controls on blocks, which the canvas zoom
/// already makes bigger or smaller.
pub fn compact(s: &mut egui::Style) {
    let d = egui::style::Spacing::default();
    s.spacing.item_spacing = egui::vec2(8.0, 6.0);
    s.spacing.button_padding = egui::vec2(12.0, 6.0);
    s.spacing.interact_size = d.interact_size;
    s.spacing.icon_width = d.icon_width;
    s.spacing.icon_width_inner = d.icon_width_inner;
    s.spacing.icon_spacing = d.icon_spacing;
    s.spacing.menu_margin = Margin::same(6);
    s.spacing.scroll.bar_width = d.scroll.bar_width;
    s.spacing.combo_height = d.combo_height;
}

pub fn hex_color(s: &str) -> Option<Color32> {
    parse_hex(s).map(|[r, g, b, a]| Color32::from_rgba_unmultiplied(r, g, b, a))
}

pub fn with_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

/// Blends `a` toward `b` by `t` (0..1).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}
