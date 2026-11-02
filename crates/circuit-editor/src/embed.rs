//! The `embed` field type: a Discord-style preview (drawn inside blocks and in the
//! builder) and the Embed Builder dialog.

use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Galley, Painter, Pos2, Rect, RichText, ScrollArea, Stroke, Ui, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dialog::{self, Part};
use crate::editor::Editor;
use crate::theme::{Theme, hex_color};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EmbedData {
    pub color: String,
    pub title: String,
    pub url: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<NameIcon>,
    pub fields: Vec<EmbedField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer: Option<FooterData>,
    pub image: String,
    pub thumbnail: String,
    pub timestamp: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NameIcon {
    pub name: String,
    pub icon: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FooterData {
    pub text: String,
    pub icon: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EmbedField {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

impl EmbedData {
    pub fn from_value(v: &Value) -> EmbedData {
        serde_json::from_value(v.clone()).unwrap_or_default()
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

// Discord's own colors, so the preview looks like the real thing in every theme.
const BG: Color32 = Color32::from_rgb(0x2B, 0x2D, 0x31);
const TEXT: Color32 = Color32::from_rgb(0xDB, 0xDE, 0xE1);
const STRONG: Color32 = Color32::from_rgb(0xF2, 0xF3, 0xF5);
const MUTED: Color32 = Color32::from_rgb(0x94, 0x9B, 0xA4);
const LINK: Color32 = Color32::from_rgb(0x00, 0xA8, 0xFC);

/// A laid-out embed: galleys at offsets from the card's top-left corner.
pub struct EmbedLayout {
    pub height: f32,
    pub bar: Color32,
    pieces: Vec<(Pos2, Arc<Galley>)>,
    image_slot: Option<Rect>,
    thumb_slot: Option<Rect>,
}

/// Lays out an embed at `width`. `max_desc_lines` shortens long descriptions (blocks use 4).
pub fn layout(painter: &Painter, e: &EmbedData, width: f32, max_desc_lines: Option<usize>, scale: f32) -> EmbedLayout {
    let pad = 12.0 * scale;
    let thumb = if e.thumbnail.trim().is_empty() { 0.0 } else { 64.0 * scale };
    let inner = (width - pad * 2.0 - 4.0 - if thumb > 0.0 { thumb + 12.0 * scale } else { 0.0 }).max(40.0);
    let mut y = pad;
    let mut pieces = Vec::new();
    let text = |pieces: &mut Vec<(Pos2, Arc<Galley>)>, s: &str, size: f32, color: Color32, wrap: f32, y: &mut f32, x: f32, gap: f32| {
        if s.trim().is_empty() {
            return;
        }
        let g = painter.layout(s.to_string(), FontId::proportional(size * scale), color, wrap);
        pieces.push((pos2(x, *y), g.clone()));
        *y += g.size().y + gap * scale;
    };
    let x0 = pad + 4.0;
    if let Some(a) = &e.author {
        text(&mut pieces, &a.name, 12.0, STRONG, inner, &mut y, x0, 6.0);
    }
    let title_color = if e.url.trim().is_empty() { STRONG } else { LINK };
    text(&mut pieces, &e.title, 14.5, title_color, inner, &mut y, x0, 6.0);
    let mut desc = e.description.clone();
    if let Some(max) = max_desc_lines {
        let lines: Vec<&str> = desc.lines().collect();
        if lines.len() > max {
            desc = format!("{}\n…", lines[..max].join("\n"));
        }
        if desc.chars().count() > max * 60 {
            desc = desc.chars().take(max * 60).collect::<String>() + "…";
        }
    }
    text(&mut pieces, &desc, 12.5, TEXT, inner, &mut y, x0, 8.0);

    // Fields: inline ones share a row, up to three.
    let mut row: Vec<&EmbedField> = Vec::new();
    let flush = |row: &mut Vec<&EmbedField>, y: &mut f32, pieces: &mut Vec<(Pos2, Arc<Galley>)>| {
        if row.is_empty() {
            return;
        }
        let col_w = inner / row.len() as f32;
        let mut tallest: f32 = 0.0;
        for (i, f) in row.iter().enumerate() {
            let x = x0 + col_w * i as f32;
            let n = painter.layout(f.name.clone(), FontId::proportional(12.0 * scale), STRONG, col_w - 8.0);
            let v = painter.layout(f.value.clone(), FontId::proportional(12.0 * scale), TEXT, col_w - 8.0);
            let h = n.size().y + 2.0 + v.size().y;
            pieces.push((pos2(x, *y), n.clone()));
            pieces.push((pos2(x, *y + n.size().y + 2.0), v));
            tallest = tallest.max(h);
        }
        *y += tallest + 8.0 * scale;
        row.clear();
    };
    for f in &e.fields {
        if !f.inline || row.len() == 3 {
            flush(&mut row, &mut y, &mut pieces);
        }
        row.push(f);
        if !f.inline {
            flush(&mut row, &mut y, &mut pieces);
        }
    }
    flush(&mut row, &mut y, &mut pieces);

    let image_slot = (!e.image.trim().is_empty()).then(|| {
        let r = Rect::from_min_size(pos2(x0, y), vec2(inner, 90.0 * scale));
        y += 90.0 * scale + 8.0 * scale;
        r
    });
    let footer = match (&e.footer, e.timestamp) {
        (Some(f), true) if !f.text.is_empty() => format!("{} · Today at 12:00", f.text),
        (Some(f), _) => f.text.clone(),
        (None, true) => "Today at 12:00".into(),
        (None, false) => String::new(),
    };
    text(&mut pieces, &footer, 11.0, MUTED, inner, &mut y, x0, 0.0);
    let thumb_slot = (thumb > 0.0).then(|| Rect::from_min_size(pos2(width - pad - thumb, pad), vec2(thumb, thumb)));
    let height = (y + pad).max(pad * 2.0 + thumb).max(36.0 * scale);
    let bar = hex_color(&e.color).unwrap_or(Color32::from_rgb(0x1E, 0x1F, 0x22));
    EmbedLayout { height, bar, pieces, image_slot, thumb_slot }
}

pub fn paint(painter: &Painter, at: Pos2, width: f32, l: &EmbedLayout) {
    let card = Rect::from_min_size(at, vec2(width, l.height));
    painter.rect_filled(card, 4.0, BG);
    painter.rect_filled(Rect::from_min_size(at, vec2(4.0, l.height)), egui::CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 }, l.bar);
    for slot in [l.image_slot, l.thumb_slot].into_iter().flatten() {
        let r = slot.translate(at.to_vec2());
        painter.rect(r, 4.0, Color32::from_rgb(0x1E, 0x1F, 0x22), Stroke::new(1.0, Color32::from_rgb(0x3F, 0x41, 0x47)), egui::StrokeKind::Inside);
        painter.text(r.center(), egui::Align2::CENTER_CENTER, "image", FontId::proportional(11.0), MUTED);
    }
    for (p, g) in &l.pieces {
        painter.galley(at + p.to_vec2(), g.clone(), TEXT);
    }
}

/// An open Embed Builder: which field it edits and the draft.
pub struct EmbedEdit {
    pub ws: String,
    pub node: String,
    pub key: String,
    pub draft: EmbedData,
}

pub fn open(ed: &mut Editor, ws: &str, node: &str, key: &str) {
    let value = ed
        .workspace(ws)
        .and_then(|w| w.nodes.get(node))
        .and_then(|n| n.values.get(key).cloned())
        .unwrap_or(Value::Null);
    ed.embed_edit = Some(EmbedEdit { ws: ws.into(), node: node.into(), key: key.into(), draft: EmbedData::from_value(&value) });
}

/// The Embed Builder tab.
pub fn tab(ui: &mut Ui, ed: &mut Editor) {
    let Some(edit) = &mut ed.embed_edit else { return };
    let theme = ed.theme.clone();
    let icon = ed.reg.icons.get("embed").cloned();
    let (mut apply, mut cancel) = (false, false);
    dialog::tab(
        ui,
        &theme,
        dialog::Spec {
            id: "embed-builder",
            title: "Embed Builder",
            subtitle: Some("Type {name} anywhere to add an input with that name"),
            icon: icon.as_deref(),
            width: 1040.0,
        },
        |ui, part| match part {
            Part::Body => {
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(vec2(470.0, 560.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ScrollArea::vertical().id_salt("embed-form").max_height(560.0).auto_shrink([false, false]).show(ui, |ui| {
                            ui.set_width(452.0);
                            form(ui, &theme, &mut edit.draft);
                        });
                    });
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        dialog::section(ui, &theme, "Preview");
                        preview_message(ui, &theme, &edit.draft);
                    });
                });
            }
            Part::Footer => {
                apply = dialog::primary_button(ui, &theme, "Apply to block").clicked();
                cancel = dialog::secondary_button(ui, &theme, "Cancel").clicked();
            }
        },
    );
    if apply {
        let edit = ed.embed_edit.take().unwrap();
        if let Some(n) = ed.project.as_mut().and_then(|p| p.workspaces.get_mut(&edit.ws)).and_then(|w| w.nodes.get_mut(&edit.node)) {
            n.values.insert(edit.key, edit.draft.to_value());
            ed.dirty = true;
        }
    } else if cancel {
        ed.embed_edit = None;
    }
}

fn form(ui: &mut Ui, theme: &Theme, e: &mut EmbedData) {
    dialog::section(ui, theme, "Color");
    ui.horizontal(|ui| {
        let mut c = hex_color(&e.color).unwrap_or(Color32::from_rgb(0, 0xD8, 0xF0));
        if crate::color::color_button(ui, ui.id().with("embed-color"), &mut c, theme, None) {
            e.color = crate::color::to_hex(c);
        }
        for preset in ["#00D8F0", "#3DFF6E", "#5865F2", "#FEE75C", "#ED4245", "#EB459E"] {
            let col = hex_color(preset).unwrap();
            let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), egui::Sense::click());
            ui.painter().rect_filled(r, 5.0, col);
            if e.color.eq_ignore_ascii_case(preset) {
                ui.painter().rect_stroke(r.expand(2.0), 6.0, Stroke::new(2.0, theme.text), egui::StrokeKind::Outside);
            }
            if resp.clicked() {
                e.color = preset.into();
            }
        }
    });
    ui.add_space(10.0);

    dialog::section(ui, theme, "Author");
    let author = e.author.get_or_insert_with(NameIcon::default);
    two(ui, theme, ("Name", &mut author.name, "{user}"), ("Icon URL", &mut author.icon, "https://…"));
    if author.name.is_empty() && author.icon.is_empty() {
        e.author = None;
    }

    dialog::section(ui, theme, "Body");
    dialog::field(ui, theme, "Title", None, |ui| dialog::text_input(ui, &mut e.title, "Rank of {user}", false));
    dialog::field(ui, theme, "Title link", None, |ui| dialog::text_input(ui, &mut e.url, "https://…", false));
    dialog::field(ui, theme, "Description", Some("Markdown works: **bold**, *italic*, `code`. Drag the corner to make the box taller."), |ui| {
        ui.add(crate::inputs::TextArea::new("embed-description", &mut e.description).hint("You have **{xp} XP**…").rows(5))
    });

    dialog::section(ui, theme, &format!("Fields · {} of 25", e.fields.len()));
    let mut remove = None;
    let mut swap = None;
    let count = e.fields.len();
    // Every row gets the same width, measured once: sizing each row from what is
    // left after the previous one made every row a little wider than the last.
    let inner_w = ui.available_width() - 2.0 * 10.0 - 2.0;
    let inputs_w = (inner_w - FIELD_CONTROLS_W).max(160.0);
    let name_w = (inputs_w * 0.38).floor();
    for (i, f) in e.fields.iter_mut().enumerate() {
        egui::Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(8).inner_margin(10).show(ui, |ui| {
            ui.set_width(inner_w);
            ui.horizontal(|ui| {
                ui.add(crate::inputs::TextLine::new(&mut f.name).hint("Name").width(name_w));
                ui.add(crate::inputs::TextLine::new(&mut f.value).hint("Value").width(inputs_w - name_w - 8.0));
                crate::widgets::toggle(ui, &mut f.inline, theme).on_hover_text("Inline: share a row with other inline fields");
                if ui.add_enabled(i > 0, egui::Button::new("Up").small()).clicked() {
                    swap = Some((i, i - 1));
                }
                if ui.add_enabled(i + 1 < count, egui::Button::new("Down").small()).clicked() {
                    swap = Some((i, i + 1));
                }
                if ui.add(egui::Button::new(RichText::new("Remove").color(theme.error)).small()).clicked() {
                    remove = Some(i);
                }
            });
        });
        ui.add_space(6.0);
    }
    if let Some((a, b)) = swap {
        e.fields.swap(a, b);
    }
    if let Some(i) = remove {
        e.fields.remove(i);
    }
    if e.fields.len() < 25 && dialog::secondary_button(ui, theme, "+ Add field").clicked() {
        e.fields.push(EmbedField { inline: true, ..Default::default() });
    }
    ui.add_space(12.0);

    dialog::section(ui, theme, "Images");
    two(ui, theme, ("Image URL", &mut e.image, "https://…"), ("Thumbnail URL", &mut e.thumbnail, "https://… or {avatar}"));

    dialog::section(ui, theme, "Footer");
    let footer = e.footer.get_or_insert_with(FooterData::default);
    two(ui, theme, ("Text", &mut footer.text, "DemoBot"), ("Icon URL", &mut footer.icon, "https://…"));
    if footer.text.is_empty() && footer.icon.is_empty() {
        e.footer = None;
    }
    dialog::toggle_row(ui, theme, &mut e.timestamp, "Timestamp", "Show when the embed was sent, next to the footer.");
}

/// Room for a field row's inline switch and its Up, Down and Remove buttons.
const FIELD_CONTROLS_W: f32 = 226.0;

fn two(ui: &mut Ui, theme: &Theme, a: (&str, &mut String, &str), b: (&str, &mut String, &str)) {
    ui.columns(2, |cols| {
        dialog::field(&mut cols[0], theme, a.0, None, |ui| dialog::text_input(ui, a.1, a.2, false));
        dialog::field(&mut cols[1], theme, b.0, None, |ui| dialog::text_input(ui, b.1, b.2, false));
    });
}

/// The embed as it appears in a Discord message, with the bot's name above it.
fn preview_message(ui: &mut Ui, theme: &Theme, e: &EmbedData) {
    let width = ui.available_width().min(500.0);
    let painter = ui.painter().clone();
    let card_w = width - 76.0;
    let l = layout(&painter, e, card_w, None, 1.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, l.height + 70.0), egui::Sense::hover());
    painter.rect_filled(rect, 8.0, Color32::from_rgb(0x31, 0x33, 0x38));
    let avatar = rect.min + vec2(14.0, 16.0);
    painter.circle_filled(avatar + vec2(18.0, 18.0), 18.0, theme.accent);
    painter.text(avatar + vec2(48.0, 2.0), egui::Align2::LEFT_TOP, "Your Bot", FontId::proportional(14.0), STRONG);
    painter.text(avatar + vec2(118.0, 4.0), egui::Align2::LEFT_TOP, "APP · Today at 12:00", FontId::proportional(11.0), MUTED);
    paint(&painter, rect.min + vec2(62.0, 44.0), card_w, &l);
}
