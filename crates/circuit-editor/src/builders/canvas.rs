//! The `canvas` field type: a picture the bot draws (Build Canvas), its
//! preview (on the block and in the builder) and the Canvas Builder.
//!
//! The design is JSON: a size, a background and layers (shape, circle,
//! picture, text, bar), each placed in the picture's pixels. The bot draws it
//! with the same fonts the editor uses, so the preview looks like the result.

use eframe::egui::{self, Align, Color32, CursorIcon, FontId, Id, Mesh, Painter, Pos2, Rect, RichText, ScrollArea, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ui::kit;
use crate::editor::Editor;
use crate::i18n::n;
use crate::ui::dialog::{self, Part};
use crate::ui::theme::{Theme, hex_color};

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CanvasData {
    pub width: f32,
    pub height: f32,
    /// A colour, or empty for a see-through background.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub background: String,
    /// Bottom first: later layers are drawn over earlier ones.
    pub layers: Vec<Layer>,
    /// What `${…}` shows in the preview, by expression: `user.username` → `Alex`.
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub samples: IndexMap<String, String>,
}

impl Default for CanvasData {
    fn default() -> Self {
        CanvasData { width: 934.0, height: 282.0, background: "#1E1F22".into(), layers: Vec::new(), samples: IndexMap::new() }
    }
}

/// One layer. Which keys matter depends on `kind`: `rect`, `circle`,
/// `image`, `text` or `bar`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Layer {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// 0 to 100; none is 100.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    #[serde(skip_serializing_if = "is_false")]
    pub hidden: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub locked: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub color: String,
    /// A second colour makes a gradient, at `angle` degrees.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub color2: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angle: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub outline: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_width: Option<f32>,
    /// Pictures: a web address, file path or `${…}`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub src: String,
    /// Pictures: `rect`, `rounded` or `circle`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub shape: String,
    /// Pictures: `cover`, `contain` or `stretch`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub fit: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// Texts: `regular`, `bold` or `mono`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub font: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f32>,
    /// Texts: `left`, `center` or `right` within the width.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub align: String,
    #[serde(skip_serializing_if = "is_false")]
    pub shadow: bool,
    /// Texts: one line, made smaller until it fits the width.
    #[serde(skip_serializing_if = "is_false")]
    pub shrink: bool,
    /// Bars: how full, out of `max`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub value: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub max: String,
    /// Bars: the colour of the empty part.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub back: String,
}

/// The kinds of layer, with what the builder calls them.
pub const KINDS: [(&str, &str); 5] = [("rect", n("Shape")), ("circle", n("Circle")), ("image", n("Picture")), ("text", n("Text")), ("bar", n("Bar"))];

impl Layer {
    pub fn new(kind: &str, d: &CanvasData) -> Layer {
        let (w, h) = (d.width, d.height);
        let mut l = Layer { kind: kind.into(), ..Default::default() };
        match kind {
            "rect" => {
                (l.x, l.y, l.w, l.h) = (w * 0.25, h * 0.25, w * 0.5, h * 0.5);
                (l.color, l.radius) = ("#2B2D31".into(), Some(16.0));
            }
            "circle" => {
                let s = w.min(h) * 0.4;
                (l.x, l.y, l.w, l.h) = ((w - s) / 2.0, (h - s) / 2.0, s, s);
                l.color = "#00D8F0".into();
            }
            "image" => {
                let s = w.min(h) * 0.6;
                (l.x, l.y, l.w, l.h) = ((w - s) / 2.0, (h - s) / 2.0, s, s);
                (l.src, l.shape, l.fit) = ("${user.displayAvatarURL({ extension: 'png', size: 256 })}".into(), "circle".into(), "cover".into());
            }
            "bar" => {
                (l.x, l.y, l.w, l.h) = (w * 0.1, h * 0.7, w * 0.8, 30.0);
                (l.value, l.max, l.color, l.back, l.radius) = ("${xp}".into(), "100".into(), "#00D8F0".into(), "#3F4147".into(), Some(15.0));
            }
            _ => {
                (l.x, l.y, l.w, l.h) = (w * 0.1, h * 0.4, w * 0.8, 0.0);
                (l.text, l.size, l.color, l.font) = ("Hello ${user.username}".into(), Some(40.0), "#FFFFFF".into(), "bold".into());
            }
        }
        l
    }

    /// The name shown in the layer list.
    pub fn title(&self) -> String {
        if !self.name.trim().is_empty() {
            return self.name.clone();
        }
        let kind = KINDS.iter().find(|(k, _)| *k == self.kind).map(|(_, n)| crate::i18n::t(n)).unwrap_or("?");
        match self.kind.as_str() {
            "text" if !self.text.is_empty() => format!("{kind}: {}", self.text.chars().take(24).collect::<String>()),
            _ => kind.to_string(),
        }
    }
}

impl CanvasData {
    pub fn from_value(v: &Value) -> CanvasData {
        serde_json::from_value(v.clone()).unwrap_or_default()
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// Every `${…}` in the layers, in order, once each.
    pub fn expressions(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut add = |s: &str| {
            for e in expressions_in(s) {
                if !out.contains(&e) {
                    out.push(e);
                }
            }
        };
        add(&self.background);
        for l in &self.layers {
            for s in [&l.text, &l.src, &l.value, &l.max, &l.color, &l.color2, &l.back, &l.outline] {
                add(s);
            }
        }
        out
    }
}

/// The expressions of the `${…}` in `s`.
pub fn expressions_in(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        let after = &rest[start + 2..];
        let mut depth = 0;
        let mut end = None;
        for (i, c) in after.char_indices() {
            match c {
                '{' => depth += 1,
                '}' if depth == 0 => {
                    end = Some(i);
                    break;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        let Some(end) = end else { break };
        out.push(after[..end].trim().to_string());
        rest = &after[end + 1..];
    }
    out
}

/// `s` as the preview shows it: each `${…}` as its preview value, or else its expression.
pub fn shown(s: &str, samples: &IndexMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = s;
    for e in expressions_in(s) {
        let start = rest.find("${").unwrap_or(rest.len());
        out.push_str(&rest[..start]);
        out.push_str(samples.get(&e).map(String::as_str).filter(|v| !v.is_empty()).unwrap_or(&e));
        // Past this ${…}: its closing brace is the first one that balances it.
        let after = &rest[start + 2..];
        let mut depth = 0;
        let mut cut = after.len();
        for (i, c) in after.char_indices() {
            match c {
                '{' => depth += 1,
                '}' if depth == 0 => {
                    cut = i + 1;
                    break;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        rest = &after[cut..];
    }
    out.push_str(rest);
    out
}

/// A colour of the design: hex, or a `${…}` whose preview value is one.
fn colour(s: &str, samples: &IndexMap<String, String>, fallback: Color32) -> Color32 {
    let s = shown(s, samples);
    let s = s.trim();
    if s.is_empty() {
        return fallback;
    }
    if let Ok(n) = s.parse::<u32>() {
        return Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8);
    }
    hex_color(s).unwrap_or(fallback)
}

fn number(s: &str, samples: &IndexMap<String, String>) -> Option<f32> {
    shown(s, samples).trim().parse().ok()
}

/// The points around a rectangle with round corners (convex), or an ellipse.
fn outline_points(shape: &str, r: Rect, radius: f32) -> Vec<Pos2> {
    let mut pts = Vec::new();
    if shape == "circle" {
        let n = 64;
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            pts.push(r.center() + vec2(a.cos() * r.width() / 2.0, a.sin() * r.height() / 2.0));
        }
        return pts;
    }
    let rad = radius.clamp(0.0, r.width().min(r.height()) / 2.0);
    if rad < 0.5 {
        return vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
    }
    let corners = [(r.right_bottom() - vec2(rad, rad), 0.0), (r.left_bottom() + vec2(rad, -rad), 90.0), (r.left_top() + vec2(rad, rad), 180.0), (r.right_top() + vec2(-rad, rad), 270.0)];
    for (c, from) in corners {
        for i in 0..=8 {
            let a = (from + i as f32 * 90.0 / 8.0f32).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * rad);
        }
    }
    pts
}

/// Fills a convex outline: one colour, or a gradient from `a` to `b` at `angle` degrees.
fn fill_points(painter: &Painter, pts: &[Pos2], bounds: Rect, a: Color32, b: Option<Color32>, angle: f32) {
    let Some(b) = b else {
        painter.add(Shape::convex_polygon(pts.to_vec(), a, Stroke::NONE));
        return;
    };
    let dir = vec2(angle.to_radians().cos(), angle.to_radians().sin());
    let half = (bounds.width() * dir.x.abs() + bounds.height() * dir.y.abs()) / 2.0;
    let at = |p: Pos2| {
        let t = if half > 0.0 { ((p - bounds.center()).dot(dir) / half + 1.0) / 2.0 } else { 0.0 };
        crate::ui::theme::mix(a, b, t.clamp(0.0, 1.0))
    };
    let mut mesh = Mesh::default();
    let center = pts.iter().fold(Vec2::ZERO, |s, p| s + p.to_vec2()) / pts.len().max(1) as f32;
    mesh.colored_vertex(center.to_pos2(), at(center.to_pos2()));
    for p in pts {
        mesh.colored_vertex(*p, at(*p));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(Shape::mesh(mesh));
}

fn stroke_points(painter: &Painter, pts: &[Pos2], stroke: Stroke) {
    if stroke.width > 0.0 {
        painter.add(Shape::closed_line(pts.to_vec(), stroke));
    }
}

fn font_for(font: &str, size: f32) -> FontId {
    match font {
        "bold" => crate::ui::fonts::bold(size),
        "mono" => FontId::monospace(size),
        _ => FontId::proportional(size),
    }
}

fn text_job(painter: &Painter, text: &str, font: FontId, color: Color32, wrap: f32, align: Align) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(text.to_string(), egui::TextFormat::simple(font, color));
    job.wrap.max_width = if wrap > 0.0 { wrap } else { f32::INFINITY };
    job.halign = align;
    painter.layout_job(crate::ui::fonts::usable(painter.ctx(), job))
}

/// Paints the design into `rect` (its size decides the scale). Gives each
/// layer's box on screen, for clicking and the selection.
pub fn paint(painter: &Painter, rect: Rect, d: &CanvasData) -> Vec<Rect> {
    let scale = rect.width() / d.width.max(1.0);
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    // A see-through background shows as squares.
    match hex_color(&shown(&d.background, &d.samples)) {
        Some(bg) if !d.background.trim().is_empty() => {
            painter.rect_filled(rect, 0.0, bg);
        }
        _ => {
            let cell = 12.0;
            painter.rect_filled(rect, 0.0, Color32::from_gray(58));
            let (cols, rows) = ((rect.width() / cell).ceil() as i32, (rect.height() / cell).ceil() as i32);
            for y in 0..rows {
                for x in (y % 2..cols).step_by(2) {
                    painter.rect_filled(Rect::from_min_size(rect.min + vec2(x as f32 * cell, y as f32 * cell), vec2(cell, cell)), 0.0, Color32::from_gray(74));
                }
            }
        }
    }
    let s = &d.samples;
    let mut boxes = Vec::with_capacity(d.layers.len());
    for l in &d.layers {
        let r = Rect::from_min_size(rect.min + vec2(l.x, l.y) * scale, vec2(l.w, l.h) * scale);
        if l.hidden {
            boxes.push(r);
            continue;
        }
        let alpha = (l.opacity.unwrap_or(100.0) / 100.0).clamp(0.0, 1.0);
        let fade = |c: Color32| c.gamma_multiply(alpha);
        let main = fade(colour(&l.color, s, Color32::WHITE));
        let second = (!l.color2.is_empty()).then(|| fade(colour(&l.color2, s, Color32::WHITE)));
        let angle = l.angle.unwrap_or(0.0);
        let line = Stroke::new(l.outline_width.unwrap_or(0.0) * scale, fade(colour(&l.outline, s, Color32::TRANSPARENT)));
        let line = if l.outline.is_empty() { Stroke::NONE } else { line };
        let radius = l.radius.unwrap_or(0.0) * scale;
        match l.kind.as_str() {
            "rect" | "circle" => {
                let pts = outline_points(if l.kind == "circle" { "circle" } else { "rect" }, r, radius);
                fill_points(&painter, &pts, r, main, second, angle);
                stroke_points(&painter, &pts, line);
                boxes.push(r);
            }
            "image" => {
                // Pictures come when the bot runs; the preview shows where.
                let shape = if l.shape == "circle" { "circle" } else { "rect" };
                let rad = if l.shape == "rounded" { l.radius.unwrap_or(16.0) * scale } else { 0.0 };
                let pts = outline_points(shape, r, rad);
                fill_points(&painter, &pts, r, fade(Color32::from_rgb(0x4E, 0x50, 0x58)), Some(fade(Color32::from_rgb(0x38, 0x3A, 0x40))), 90.0);
                // A person, as most pictures here are avatars.
                let head = r.center() - vec2(0.0, r.height() * 0.12);
                let unit = r.width().min(r.height());
                painter.circle_filled(head, unit * 0.16, fade(Color32::from_rgb(0x8E, 0x92, 0x9B)));
                let body = Rect::from_center_size(r.center() + vec2(0.0, unit * 0.3), vec2(unit * 0.56, unit * 0.36));
                painter.add(Shape::convex_polygon(outline_points("circle", body, 0.0), fade(Color32::from_rgb(0x8E, 0x92, 0x9B)), Stroke::NONE));
                stroke_points(&painter, &pts, line);
                boxes.push(r);
            }
            "bar" => {
                let max = number(&l.max, s).filter(|m| *m != 0.0).unwrap_or(100.0);
                let part = (number(&l.value, s).unwrap_or(max / 2.0) / max).clamp(0.0, 1.0);
                let pts = outline_points("rect", r, radius);
                fill_points(&painter, &pts, r, fade(colour(&l.back, s, Color32::from_rgb(0x48, 0x4B, 0x51))), None, 0.0);
                if part > 0.0 {
                    let full = Rect::from_min_size(r.min, vec2(r.width() * part, r.height()));
                    let pts_full = outline_points("rect", full, radius.min(full.width() / 2.0));
                    fill_points(&painter, &pts_full, full, main, second, angle);
                }
                stroke_points(&painter, &pts, line);
                boxes.push(r);
            }
            "text" => {
                let text = shown(&l.text, s);
                let mut size = l.size.unwrap_or(32.0).max(4.0);
                let width = l.w.max(0.0);
                let align = match l.align.as_str() {
                    "center" => Align::Center,
                    "right" => Align::Max,
                    _ => Align::Min,
                };
                // Shrink to fit, as the bot does: one line, smaller until it fits.
                if l.shrink && width > 0.0 && !text.contains('\n') {
                    while size > 8.0 && text_job(&painter, &text, font_for(&l.font, size * scale), main, 0.0, Align::Min).size().x > width * scale {
                        size -= 1.0;
                    }
                }
                let wrap = if l.shrink { 0.0 } else { width * scale };
                let galley = text_job(&painter, &text, font_for(&l.font, size * scale), main, wrap, align);
                let x = match align {
                    Align::Center => r.left() + width * scale / 2.0,
                    Align::Max => r.left() + width * scale,
                    _ => r.left(),
                };
                let at = pos2(x, r.top());
                if l.shadow {
                    painter.galley_with_override_text_color(at + vec2(0.0, (size * scale * 0.06).max(1.0)), galley.clone(), fade(Color32::from_black_alpha(150)));
                }
                if line.width > 0.0 {
                    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (-0.7, -0.7), (0.7, -0.7), (-0.7, 0.7), (0.7, 0.7)] {
                        painter.galley_with_override_text_color(at + vec2(dx, dy) * line.width, galley.clone(), line.color);
                    }
                }
                let drawn = galley.rect.translate(at.to_vec2());
                painter.galley(at, galley, main);
                let w = if width > 0.0 { width * scale } else { drawn.width() };
                boxes.push(Rect::from_min_size(r.min, vec2(w, drawn.height().max(8.0))));
            }
            _ => boxes.push(r),
        }
    }
    boxes
}

/// The design's height on a block that is `width` wide.
pub fn block_height(d: &CanvasData, width: f32) -> f32 {
    (width * d.height / d.width.max(1.0)).min(220.0)
}

/// Paints the design on a block, fitted into `width` × at most 220.
pub fn paint_on_block(painter: &Painter, at: Pos2, width: f32, d: &CanvasData) {
    let h = block_height(d, width);
    let w = (h * d.width / d.height.max(1.0)).min(width);
    let rect = Rect::from_min_size(at + vec2((width - w) / 2.0, 0.0), vec2(w, h));
    paint(painter, rect, d);
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::from_gray(70)), egui::StrokeKind::Outside);
}

// ───── The Canvas Builder ─────

#[derive(Clone, Copy, PartialEq)]
enum Drag {
    Move,
    Resize,
}

/// An open Canvas Builder: which field it edits and the draft.
pub struct CanvasEdit {
    pub ws: String,
    pub node: String,
    pub key: String,
    pub draft: CanvasData,
    selected: Option<usize>,
    /// A drag in the preview: what it does, where it started, the layer's box then.
    drag: Option<(Drag, Pos2, [f32; 4])>,
}

impl CanvasEdit {
    pub fn new(ws: &str, node: &str, key: &str, value: &Value) -> CanvasEdit {
        let draft = CanvasData::from_value(value);
        let selected = (!draft.layers.is_empty()).then(|| draft.layers.len() - 1);
        CanvasEdit { ws: ws.into(), node: node.into(), key: key.into(), draft, selected, drag: None }
    }
}

/// Opens the builder for a block's field; an unset field starts from the block's default.
pub fn open(ed: &mut Editor, ws: &str, node: &str, key: &str) {
    let n = ed.workspace(ws).and_then(|w| w.nodes.get(node));
    let value = n.and_then(|n| n.values.get(key).cloned().or_else(|| ed.reg.block(&n.block).and_then(|d| d.fields.get(key)).and_then(|f| f.default.clone()))).unwrap_or(Value::Null);
    ed.canvas_edit = Some(CanvasEdit::new(ws, node, key, &value));
}

/// Ready-made designs to start from.
pub fn starters() -> Vec<(&'static str, CanvasData)> {
    let rank = json!({
        "width": 934, "height": 282, "background": "#1E1F22",
        "layers": [
            { "type": "rect", "name": "Card", "x": 16, "y": 16, "w": 902, "h": 250, "radius": 24, "color": "#2B2D31" },
            { "type": "image", "name": "Avatar", "x": 48, "y": 51, "w": 180, "h": 180, "shape": "circle", "fit": "cover", "src": "${user.displayAvatarURL({ extension: 'png', size: 256 })}", "outline": "#00D8F0", "outlineWidth": 6 },
            { "type": "text", "name": "Name", "x": 264, "y": 64, "w": 620, "text": "${user.username}", "font": "bold", "size": 46, "color": "#FFFFFF", "shrink": true, "shadow": true },
            { "type": "text", "name": "Level", "x": 264, "y": 128, "w": 620, "text": "Level ${level} · ${xp} / ${needed} XP", "size": 28, "color": "#B5BAC1" },
            { "type": "bar", "name": "Progress", "x": 264, "y": 184, "w": 620, "h": 34, "radius": 17, "value": "${xp}", "max": "${needed}", "color": "#00D8F0", "color2": "#3DFF6E", "back": "#3F4147" }
        ],
        "samples": { "user.username": "Alex", "level": "12", "xp": "750", "needed": "1000" }
    });
    let welcome = json!({
        "width": 1024, "height": 450, "background": "#23272A",
        "layers": [
            { "type": "rect", "name": "Background", "x": 0, "y": 0, "w": 1024, "h": 450, "color": "#5865F2", "color2": "#00D8F0", "angle": 30 },
            { "type": "rect", "name": "Shade", "x": 0, "y": 0, "w": 1024, "h": 450, "color": "#000000", "opacity": 35 },
            { "type": "image", "name": "Avatar", "x": 412, "y": 50, "w": 200, "h": 200, "shape": "circle", "fit": "cover", "src": "${user.displayAvatarURL({ extension: 'png', size: 256 })}", "outline": "#FFFFFF", "outlineWidth": 8 },
            { "type": "text", "name": "Welcome", "x": 62, "y": 280, "w": 900, "text": "Welcome, ${user.username}!", "font": "bold", "size": 56, "color": "#FFFFFF", "align": "center", "shrink": true, "shadow": true },
            { "type": "text", "name": "Count", "x": 62, "y": 360, "w": 900, "text": "You're member #${server.memberCount} of ${server.name}", "size": 30, "color": "#E3E5E8", "align": "center", "shadow": true }
        ],
        "samples": { "user.username": "Alex", "server.memberCount": "128", "server.name": "XC Lounge" }
    });
    let profile = json!({
        "width": 800, "height": 400, "background": "#111214",
        "layers": [
            { "type": "rect", "name": "Banner", "x": 0, "y": 0, "w": 800, "h": 140, "color": "#EB459E", "color2": "#5865F2" },
            { "type": "image", "name": "Avatar", "x": 40, "y": 70, "w": 150, "h": 150, "shape": "circle", "fit": "cover", "src": "${user.displayAvatarURL({ extension: 'png', size: 256 })}", "outline": "#111214", "outlineWidth": 10 },
            { "type": "text", "name": "Name", "x": 40, "y": 236, "w": 720, "text": "${user.username}", "font": "bold", "size": 40, "color": "#FFFFFF", "shrink": true },
            { "type": "text", "name": "About", "x": 40, "y": 292, "w": 720, "text": "${about}", "size": 24, "color": "#B5BAC1" }
        ],
        "samples": { "user.username": "Alex", "about": "Builds bots with blocks." }
    });
    let empty = json!({ "width": 900, "height": 300, "background": "#2B2D31", "layers": [] });
    vec![
        (n("Rank card"), CanvasData::from_value(&rank)),
        (n("Welcome card"), CanvasData::from_value(&welcome)),
        (n("Profile card"), CanvasData::from_value(&profile)),
        (n("Empty"), CanvasData::from_value(&empty)),
    ]
}

const SIZES: [(&str, f32, f32); 5] = [(n("Rank card 934 × 282"), 934.0, 282.0), (n("Welcome 1024 × 450"), 1024.0, 450.0), (n("Banner 1100 × 400"), 1100.0, 400.0), (n("Square 800 × 800"), 800.0, 800.0), (n("Wide 1200 × 630"), 1200.0, 630.0)];

/// The Canvas Builder tab.
pub fn tab(ui: &mut Ui, ed: &mut Editor) {
    let Some((ws, node)) = ed.canvas_edit.as_ref().map(|e| (e.ws.clone(), e.node.clone())) else { return };
    // Texts offer the values of earlier blocks after `${`.
    let values = std::sync::Arc::new(ed.values_for(&ws, &node));
    crate::editor::values::set_scope(ui.ctx(), Some(values.clone()));
    let theme = ed.theme.clone();
    let icon = ed.reg.icons.get("palette").cloned();
    let icons = Icons::new(ed);
    let Some(edit) = &mut ed.canvas_edit else { return };
    let (mut apply, mut close, mut cancel) = (false, false, false);
    dialog::tab(
        ui,
        &theme,
        dialog::Spec {
            id: "canvas-builder",
            title: t!("Canvas Builder"),
            subtitle: Some(t!("Click a layer to choose it, drag it to move it, drag its corner to resize it. ${…} holds JavaScript.")),
            icon: icon.as_deref(),
            width: 1240.0,
        },
        |ui, part| match part {
            Part::Body => {
                let h = dialog::remaining_height(ui, 460.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(vec2(250.0, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ScrollArea::vertical().id_salt("canvas-left").auto_shrink([false, false]).show(ui, |ui| left_panel(ui, &theme, edit, &icons));
                    });
                    ui.add_space(12.0);
                    let middle = (ui.available_width() - 300.0 - 12.0).max(300.0);
                    ui.allocate_ui_with_layout(vec2(middle, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                        preview(ui, &theme, edit);
                        ui.add_space(10.0);
                        ScrollArea::vertical().id_salt("canvas-samples").auto_shrink([false, false]).show(ui, |ui| samples(ui, &theme, &mut edit.draft));
                    });
                    ui.add_space(12.0);
                    ui.allocate_ui_with_layout(vec2(300.0, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ScrollArea::vertical().id_salt("canvas-right").auto_shrink([false, false]).show(ui, |ui| {
                            ui.set_width(286.0);
                            properties(ui, &theme, edit);
                        });
                    });
                });
            }
            Part::Footer => {
                if kit::primary_button(ui, &theme, t!("Apply and close")).clicked() {
                    (apply, close) = (true, true);
                }
                apply |= kit::secondary_button(ui, &theme, t!("Apply")).on_hover_text(t!("Put it on the block and keep editing")).clicked();
                cancel = kit::secondary_button(ui, &theme, t!("Cancel")).clicked();
            }
        },
    );
    crate::editor::values::set_scope(ui.ctx(), None);
    if apply {
        let edit = ed.canvas_edit.as_ref().unwrap();
        let (ws, node, key, value) = (edit.ws.clone(), edit.node.clone(), edit.key.clone(), edit.draft.to_value());
        if close {
            ed.canvas_edit = None;
        }
        if let Some(n) = ed.project.as_mut().and_then(|p| p.workspaces.get_mut(&ws)).and_then(|w| w.nodes.get_mut(&node)) {
            n.values.insert(key, value);
            ed.edited = true;
        }
    } else if cancel {
        ed.canvas_edit = None;
    }
}

/// The icons of the layer list's buttons.
struct Icons {
    trash: Option<std::path::PathBuf>,
    copy: Option<std::path::PathBuf>,
    eye: Option<std::path::PathBuf>,
    eye_off: Option<std::path::PathBuf>,
}

impl Icons {
    fn new(ed: &Editor) -> Icons {
        let get = |n: &str| ed.reg.icons.get(n).cloned();
        Icons { trash: get("trash"), copy: get("copy"), eye: get("eye"), eye_off: get("eye-off") }
    }
}

/// A small framed button with an icon (or text), for the layer list.
fn tiny(ui: &mut Ui, theme: &Theme, icon: Option<&std::path::Path>, text: &str, tip: &str, enabled: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    let ink = if enabled { if resp.hovered() { theme.text } else { theme.text_muted } } else { crate::ui::theme::with_alpha(theme.text_muted, 70) };
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, 5.0, crate::ui::theme::mix(theme.panel_header, theme.text, 0.1));
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    match (icon, text) {
        (Some(path), _) => egui::Image::from_uri(crate::editor::file_uri(path)).tint(ink).paint_at(ui, rect.shrink(5.0)),
        // Arrows are drawn: the fonts have none.
        (None, "up" | "down") => {
            let c = rect.center();
            let dy = if text == "up" { -2.5 } else { 2.5 };
            let stroke = Stroke::new(1.6, ink);
            ui.painter().line_segment([c + vec2(-4.0, -dy), c + vec2(0.0, dy)], stroke);
            ui.painter().line_segment([c + vec2(0.0, dy), c + vec2(4.0, -dy)], stroke);
        }
        (None, _) => {
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, FontId::proportional(13.0), ink);
        }
    }
    enabled && resp.on_hover_text(tip).clicked()
}

/// Size, background, a design to start from, and the layers.
fn left_panel(ui: &mut Ui, theme: &Theme, edit: &mut CanvasEdit, icons: &Icons) {
    let d = &mut edit.draft;
    kit::section(ui, theme, t!("Start from"));
    let mut start = None;
    crate::ui::kit::dropdown(ui, Id::unique("canvas-start"), 220.0, t!("Choose a design…"), |ui| {
        for (i, (name, _)) in starters().iter().enumerate() {
            if kit::option(ui, false, crate::i18n::t(name)).clicked() {
                start = Some(i);
            }
        }
    })
    .on_hover_text(t!("Replaces the design; Cancel brings yours back."));
    if let Some(i) = start {
        *d = starters().swap_remove(i).1;
        edit.selected = (!d.layers.is_empty()).then(|| d.layers.len() - 1);
    }

    kit::section(ui, theme, t!("Size"));
    crate::ui::kit::line(ui, |ui| {
        kit::Number::new(&mut d.width).range(16.0..=4096.0).suffix(" px").width(64.0).show(ui, theme);
        ui.label("×");
        kit::Number::new(&mut d.height).range(16.0..=4096.0).suffix(" px").width(64.0).show(ui, theme);
    });
    crate::ui::kit::dropdown(ui, Id::unique("canvas-sizes"), 220.0, t!("Common sizes"), |ui| {
        for (name, w, h) in SIZES {
            if kit::option(ui, d.width == w && d.height == h, crate::i18n::t(name)).clicked() {
                (d.width, d.height) = (w, h);
            }
        }
    });
    kit::section(ui, theme, t!("Background"));
    crate::ui::kit::line(ui, |ui| {
        let mut clear = d.background.trim().is_empty();
        if !clear {
            colour_edit(ui, theme, Id::unique("canvas-bg"), &mut d.background);
        }
        if kit::checkbox(ui, theme, &mut clear, t!("See-through")).changed() {
            d.background = if clear { String::new() } else { "#2B2D31".into() };
        }
    });

    kit::section(ui, theme, t!("Layers"));
    kit::hint(ui, theme, t!("Top ones are drawn over the ones below."));
    ui.add_space(4.0);
    let count = d.layers.len();
    let mut action: Option<(usize, &str)> = None;
    for i in (0..count).rev() {
        let l = &d.layers[i];
        let selected = edit.selected == Some(i);
        egui::Frame::new()
            .fill(if selected { crate::ui::theme::with_alpha(theme.accent, 40) } else { theme.panel_header })
            .stroke(Stroke::new(1.0, if selected { theme.accent } else { theme.line }))
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.set_min_height(22.0);
                    let title = RichText::new(l.title()).size(12.5).color(if l.hidden { theme.text_muted } else { theme.text });
                    if ui.add(egui::Label::new(title).truncate().sense(Sense::click())).clicked() {
                        action = Some((i, "select"));
                    }
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        if tiny(ui, theme, icons.trash.as_deref(), "×", t!("Delete"), true) {
                            action = Some((i, "delete"));
                        }
                        if tiny(ui, theme, icons.copy.as_deref(), "+", t!("Duplicate"), true) {
                            action = Some((i, "copy"));
                        }
                        if tiny(ui, theme, None, "down", t!("Down"), i > 0) {
                            action = Some((i, "down"));
                        }
                        if tiny(ui, theme, None, "up", t!("Up"), i + 1 < count) {
                            action = Some((i, "up"));
                        }
                        let eye = if l.hidden { icons.eye_off.as_deref() } else { icons.eye.as_deref() };
                        if tiny(ui, theme, eye, "o", if l.hidden { t!("Show") } else { t!("Hide") }, true) {
                            action = Some((i, "hide"));
                        }
                    });
                });
            });
        ui.add_space(3.0);
    }
    if let Some((i, what)) = action {
        match what {
            "select" => edit.selected = Some(i),
            "delete" => {
                d.layers.remove(i);
                edit.selected = (!d.layers.is_empty()).then(|| i.min(d.layers.len() - 1));
            }
            "copy" => {
                let mut l = d.layers[i].clone();
                (l.x, l.y) = (l.x + 16.0, l.y + 16.0);
                d.layers.insert(i + 1, l);
                edit.selected = Some(i + 1);
            }
            "up" => {
                d.layers.swap(i, i + 1);
                edit.selected = Some(i + 1);
            }
            "down" => {
                d.layers.swap(i, i - 1);
                edit.selected = Some(i - 1);
            }
            "hide" => d.layers[i].hidden = !d.layers[i].hidden,
            _ => {}
        }
    }
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        for (kind, name) in KINDS {
            if kit::button(ui, theme, format!("+ {}", crate::i18n::t(name))).clicked() {
                d.layers.push(Layer::new(kind, d));
                edit.selected = Some(d.layers.len() - 1);
            }
        }
    });
}

/// The design, fitted into the space, where layers are chosen, moved and resized.
fn preview(ui: &mut Ui, theme: &Theme, edit: &mut CanvasEdit) {
    let d = &edit.draft;
    let room = vec2(ui.available_width(), (ui.available_height() - 160.0).max(200.0));
    let scale = (room.x / d.width.max(1.0)).min(room.y / d.height.max(1.0)).min(1.0);
    let size = vec2(d.width, d.height) * scale;
    let (area, resp) = ui.allocate_exact_size(vec2(room.x, size.y), Sense::click_and_drag());
    let rect = Rect::from_min_size(pos2(area.center().x - size.x / 2.0, area.top()), size);
    let painter = ui.painter_at(area);
    let boxes = paint(&painter, rect, d);
    kit::hint(ui, theme, tf!("{w} × {h} px · shown at {percent} %", w = d.width, h = d.height, percent = (scale * 100.0).round()));

    let handle_of = |b: Rect| Rect::from_center_size(b.right_bottom(), vec2(12.0, 12.0));
    let hit = |p: Pos2| (0..d.layers.len()).rev().find(|&i| !d.layers[i].hidden && !d.layers[i].locked && boxes[i].expand(3.0).contains(p));
    if let Some(i) = edit.selected.filter(|&i| i < boxes.len()) {
        let b = boxes[i];
        painter.rect_stroke(b, 0.0, Stroke::new(1.5, theme.accent), egui::StrokeKind::Outside);
        if !d.layers[i].locked {
            painter.rect_filled(handle_of(b), 2.0, theme.accent);
        }
    }
    if let Some(p) = resp.hover_pos() {
        let on_handle = edit.selected.filter(|&i| i < boxes.len()).is_some_and(|i| handle_of(boxes[i]).expand(2.0).contains(p));
        if on_handle {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe);
        } else if hit(p).is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Move);
        }
    }
    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        edit.selected = hit(p);
    }
    if resp.drag_started()
        && let Some(p) = resp.interact_pointer_pos()
    {
        let on_handle = edit.selected.filter(|&i| i < boxes.len()).filter(|&i| handle_of(boxes[i]).expand(2.0).contains(p));
        let (target, kind) = match on_handle {
            Some(i) => (Some(i), Drag::Resize),
            None => (hit(p), Drag::Move),
        };
        if let Some(i) = target {
            edit.selected = Some(i);
            let l = &d.layers[i];
            edit.drag = Some((kind, p, [l.x, l.y, l.w, l.h]));
        }
    }
    if let (Some((kind, from, [x, y, w, h])), Some(i), Some(p)) = (edit.drag, edit.selected, resp.interact_pointer_pos()) {
        let delta = (p - from) / scale.max(0.01);
        if let Some(l) = edit.draft.layers.get_mut(i) {
            match kind {
                Drag::Move => (l.x, l.y) = ((x + delta.x).round(), (y + delta.y).round()),
                Drag::Resize => {
                    l.w = (w + delta.x).round().max(4.0);
                    // Texts are as tall as their lines; circles and pictures keep their shape with Shift.
                    if l.kind != "text" {
                        l.h = (h + delta.y).round().max(4.0);
                        if ui.input(|i| i.modifiers.shift) && w > 0.0 {
                            l.h = (l.w * h / w).round();
                        }
                    }
                }
            }
        }
    }
    if resp.drag_stopped() {
        edit.drag = None;
    }
    // Arrow keys move the chosen layer (Shift: by 10), when no text box is being typed in.
    if resp.hovered() && ui.memory(|m| m.focused().is_none())
        && let Some(l) = edit.selected.and_then(|i| edit.draft.layers.get_mut(i))
    {
        let step = if ui.input(|i| i.modifiers.shift) { 10.0 } else { 1.0 };
        ui.input(|i| {
            for (key, dx, dy) in [(egui::Key::ArrowLeft, -1.0, 0.0), (egui::Key::ArrowRight, 1.0, 0.0), (egui::Key::ArrowUp, 0.0, -1.0), (egui::Key::ArrowDown, 0.0, 1.0)] {
                if i.key_pressed(key) {
                    l.x += dx * step;
                    l.y += dy * step;
                }
            }
        });
    }
}

/// Preview values for each `${…}`.
fn samples(ui: &mut Ui, theme: &Theme, d: &mut CanvasData) {
    let exprs = d.expressions();
    kit::section(ui, theme, t!("Preview values"));
    if exprs.is_empty() {
        kit::hint(ui, theme, t!("Texts and pictures with ${…} get a box here, to see the design with a value in it."));
        return;
    }
    kit::hint(ui, theme, t!("Only for the preview: the bot fills in the real values."));
    egui::Grid::new("canvas-samples-grid").num_columns(2).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
        for e in &exprs {
            ui.label(RichText::new(format!("${{{e}}}")).monospace().size(11.5).color(theme.text));
            let value = d.samples.entry(e.clone()).or_default();
            ui.add(crate::ui::kit::TextLine::new(value).hint(e.as_str()).width(220.0));
            ui.end_row();
        }
    });
    // Values for ${…} no longer used are dropped.
    d.samples.retain(|k, v| exprs.contains(k) && !v.is_empty());
}

/// A colour: a swatch with the picker, or (for a colour from the bot, such
/// as a role's) a `${…}` text. The small button switches between them.
fn colour_edit(ui: &mut Ui, theme: &Theme, id: Id, value: &mut String) {
    crate::ui::kit::line(ui, |ui| {
        let expression = value.contains("${");
        if expression {
            ui.add(crate::ui::kit::TextLine::new(value).hint("${role.hexColor}").width(200.0));
        } else {
            kit::color(ui, theme, id, value);
        }
        let tip = if expression { t!("Pick a colour instead") } else { t!("A colour from the bot: ${…}, such as ${role.hexColor}") };
        if tiny(ui, theme, None, if expression { "#" } else { "${}" }, tip, true) {
            *value = if expression { "#FFFFFF".into() } else { "${}".into() };
        }
    });
}

fn label(ui: &mut Ui, theme: &Theme, text: &str) {
    kit::caption(ui, theme, text);
}

fn choose(ui: &mut Ui, id: &str, value: &mut String, choices: &[(&'static str, &'static str)], width: f32) {
    let translated: Vec<(&str, &str)> = choices.iter().map(|(v, l)| (*v, crate::i18n::t(l))).collect();
    kit::select(ui, id, width, value, &translated);
}

fn opt_number(ui: &mut Ui, theme: &Theme, value: &mut Option<f32>, default: f32, range: std::ops::RangeInclusive<f32>, suffix: &str) {
    let mut v = value.unwrap_or(default);
    if kit::Number::new(&mut v).range(range).suffix(suffix).show(ui, theme).changed() {
        *value = Some(v);
    }
}

/// The chosen layer's settings.
fn properties(ui: &mut Ui, theme: &Theme, edit: &mut CanvasEdit) {
    let Some(i) = edit.selected.filter(|&i| i < edit.draft.layers.len()) else {
        kit::section(ui, theme, t!("Layer"));
        ui.label(RichText::new(t!("Choose a layer in the picture or the list, or add one.")).size(12.0).color(theme.text_muted));
        return;
    };
    let l = &mut edit.draft.layers[i];
    let kind = KINDS.iter().find(|(k, _)| *k == l.kind).map(|(_, n)| crate::i18n::t(n)).unwrap_or("?");
    kit::section(ui, theme, kind);
    label(ui, theme, t!("Name"));
    let hint = l.title();
    ui.add(crate::ui::kit::TextLine::new(&mut l.name).hint(hint).width(280.0));
    ui.add_space(4.0);
    egui::Grid::new("canvas-box").num_columns(4).spacing(vec2(6.0, 4.0)).show(ui, |ui| {
        for (name, v) in [("X", &mut l.x), ("Y", &mut l.y)] {
            ui.label(name);
            kit::Number::new(v).show(ui, theme);
        }
        ui.end_row();
        ui.label(t!("Width"));
        kit::Number::new(&mut l.w).range(0.0..=4096.0).show(ui, theme);
        if l.kind != "text" {
            ui.label(t!("Height"));
            kit::Number::new(&mut l.h).range(0.0..=4096.0).show(ui, theme);
        }
        ui.end_row();
    });
    crate::ui::kit::line(ui, |ui| {
        // Named like Width and Height above.
        ui.label(t!("Opacity"));
        let mut o = l.opacity.unwrap_or(100.0);
        if kit::Slider::new(&mut o, 0.0..=100.0).decimals(0).suffix(" %").show(ui, theme) {
            l.opacity = (o < 100.0).then_some(o);
        }
    });
    kit::checkbox(ui, theme, &mut l.locked, t!("Locked: not moved by dragging in the picture"));
    ui.add_space(6.0);

    let id = |k: &str| Id::unique(("canvas-prop", i, k.to_string()));
    match l.kind.as_str() {
        "text" => {
            label(ui, theme, t!("Text"));
            ui.add(crate::ui::kit::TextArea::new(("canvas-text", i), &mut l.text).hint("Hello ${user.username}").rows(3));
            crate::ui::kit::line(ui, |ui| {
                choose(ui, &format!("canvas-font-{i}"), &mut l.font, &[("regular", n("Regular")), ("bold", n("Bold")), ("mono", n("Monospace"))], 120.0);
                opt_number(ui, theme, &mut l.size, 32.0, 4.0..=400.0, " px");
            });
            crate::ui::kit::line(ui, |ui| {
                label(ui, theme, t!("Align"));
                choose(ui, &format!("canvas-align-{i}"), &mut l.align, &[("left", n("Left")), ("center", n("Center")), ("right", n("Right"))], 110.0);
            });
            label(ui, theme, t!("Colour"));
            colour_edit(ui, theme, id("color"), &mut l.color);
            kit::checkbox(ui, theme, &mut l.shrink, t!("Shrink to fit the width (one line)"));
            kit::checkbox(ui, theme, &mut l.shadow, t!("Shadow"));
        }
        "image" => {
            label(ui, theme, t!("Picture"));
            ui.add(crate::ui::kit::TextArea::new(("canvas-src", i), &mut l.src).hint("https://… or ${user.displayAvatarURL({ extension: 'png' })}").rows(2));
            label(ui, theme, t!("A web address, a file path, or ${…}. The preview shows where it goes; the bot loads it."));
            crate::ui::kit::line(ui, |ui| {
                label(ui, theme, t!("Shape"));
                choose(ui, &format!("canvas-shape-{i}"), &mut l.shape, &[("rect", n("Square corners")), ("rounded", n("Round corners")), ("circle", n("Circle"))], 140.0);
            });
            if l.shape == "rounded" {
                crate::ui::kit::line(ui, |ui| {
                    label(ui, theme, t!("Corner radius"));
                    opt_number(ui, theme, &mut l.radius, 16.0, 0.0..=2000.0, " px");
                });
            }
            crate::ui::kit::line(ui, |ui| {
                label(ui, theme, t!("Fit"));
                choose(ui, &format!("canvas-fit-{i}"), &mut l.fit, &[("cover", n("Fill (cut off the rest)")), ("contain", n("Whole picture")), ("stretch", n("Stretch"))], 180.0);
            });
        }
        "bar" => {
            label(ui, theme, t!("Value"));
            ui.add(crate::ui::kit::TextLine::new(&mut l.value).hint("${xp}").width(280.0));
            label(ui, theme, t!("Out of"));
            ui.add(crate::ui::kit::TextLine::new(&mut l.max).hint("100").width(280.0));
            label(ui, theme, t!("Colour"));
            colour_edit(ui, theme, id("color"), &mut l.color);
            gradient(ui, theme, l, id("color2"));
            label(ui, theme, t!("Empty part"));
            colour_edit(ui, theme, id("back"), &mut l.back);
            crate::ui::kit::line(ui, |ui| {
                label(ui, theme, t!("Corner radius"));
                opt_number(ui, theme, &mut l.radius, 0.0, 0.0..=2000.0, " px");
            });
        }
        _ => {
            label(ui, theme, t!("Colour"));
            colour_edit(ui, theme, id("color"), &mut l.color);
            gradient(ui, theme, l, id("color2"));
            if l.kind == "rect" {
                crate::ui::kit::line(ui, |ui| {
                    label(ui, theme, t!("Corner radius"));
                    opt_number(ui, theme, &mut l.radius, 0.0, 0.0..=2000.0, " px");
                });
            }
        }
    }
    ui.add_space(6.0);
    let mut has_outline = !l.outline.is_empty();
    if kit::checkbox(ui, theme, &mut has_outline, t!("Outline")).changed() {
        if has_outline {
            l.outline = "#FFFFFF".into();
            l.outline_width.get_or_insert(4.0);
        } else {
            l.outline.clear();
        }
    }
    if has_outline {
        crate::ui::kit::line(ui, |ui| {
            colour_edit(ui, theme, id("outline"), &mut l.outline);
            opt_number(ui, theme, &mut l.outline_width, 4.0, 0.0..=200.0, " px");
        });
    }
}

/// A second colour, for a gradient at an angle.
fn gradient(ui: &mut Ui, theme: &Theme, l: &mut Layer, id: Id) {
    let mut on = !l.color2.is_empty();
    if kit::checkbox(ui, theme, &mut on, t!("Gradient")).changed() {
        l.color2 = if on { "#3DFF6E".into() } else { String::new() };
    }
    if on {
        crate::ui::kit::line(ui, |ui| {
            colour_edit(ui, theme, id, &mut l.color2);
            opt_number(ui, theme, &mut l.angle, 0.0, -360.0..=360.0, "°");
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_fills_in_expressions() {
        let s = "Hi ${user.username}, ${user.displayAvatarURL({ size: 256 })} and ${ level }";
        assert_eq!(expressions_in(s), ["user.username", "user.displayAvatarURL({ size: 256 })", "level"]);
        let mut samples = IndexMap::new();
        samples.insert("user.username".to_string(), "Alex".to_string());
        assert_eq!(shown(s, &samples), "Hi Alex, user.displayAvatarURL({ size: 256 }) and level");
        assert_eq!(shown("no values", &samples), "no values");
        assert_eq!(shown("${broken", &samples), "${broken");
    }

    /// The block's default design and the editor's Rank card are the same,
    /// and every starter reads back as written.
    #[test]
    fn starters_read_back() {
        let block = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/xc.core/blocks/design/build_canvas.js")).unwrap();
        for (name, d) in starters() {
            assert_eq!(CanvasData::from_value(&d.to_value()), d, "{name}");
            if name == "Rank card" {
                for l in &d.layers {
                    assert!(block.contains(&format!("name: \"{}\"", l.name)), "{} is in the block's default", l.name);
                }
                assert_eq!(d.expressions(), ["user.displayAvatarURL({ extension: 'png', size: 256 })", "user.username", "level", "xp", "needed"]);
            }
        }
        // Unknown or broken JSON gives an empty design rather than failing.
        assert_eq!(CanvasData::from_value(&json!("nonsense")), CanvasData::default());
    }

    /// Every starter and every kind of layer paints, and gives a box per layer.
    #[test]
    fn paints_every_layer() {
        let ctx = egui::Context::default();
        crate::ui::fonts::install(&ctx);
        let mut d = CanvasData::default();
        for (kind, _) in KINDS {
            d.layers.push(Layer::new(kind, &d));
        }
        d.layers[0].color2 = "#FF0000".into();
        d.layers[3].shrink = true;
        d.layers[3].outline = "#000000".into();
        d.layers[3].outline_width = Some(3.0);
        d.background.clear();
        let mut all: Vec<CanvasData> = starters().into_iter().map(|(_, d)| d).collect();
        all.push(d);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            for d in &all {
                let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(467.0, 467.0 * d.height / d.width));
                let boxes = paint(ui.painter(), rect, d);
                assert_eq!(boxes.len(), d.layers.len());
                assert!(boxes.iter().all(|b| b.is_finite()));
                paint_on_block(ui.painter(), pos2(0.0, 0.0), 272.0, d);
            }
        });
        out.textures_delta.clear();
    }
}
