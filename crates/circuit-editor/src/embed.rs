//! The `embed` field type: a Discord-style preview (drawn inside blocks and in the
//! builder) and the Embed Builder dialog.


use eframe::egui::{self, Color32, FontId, Painter, Pos2, Rect, RichText, ScrollArea, Stroke, Ui, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::dialog::{self, Part};
use crate::editor::Editor;
use crate::json_view::{self, JsonText};
use crate::markdown::{Kind, Marked};
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
    /// Show when the message was sent, next to the footer.
    pub timestamp: bool,
    /// A set time to show instead: a date such as 2026-12-24T18:00:00Z, or a `${…}`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub time: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NameIcon {
    pub name: String,
    pub icon: String,
    /// The page the author's name links to.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
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

    /// The embed as Discord's JSON. "When it's sent" is written as `"timestamp": "now"`.
    pub fn to_discord(&self) -> Value {
        let mut out = Map::new();
        let mut put = |k: &str, v: Value| {
            out.insert(k.into(), v);
        };
        for (k, v) in [("title", &self.title), ("description", &self.description), ("url", &self.url)] {
            if !v.is_empty() {
                put(k, json!(v));
            }
        }
        if let Some(c) = hex_color(&self.color) {
            put("color", json!(((c.r() as u32) << 16) | ((c.g() as u32) << 8) | c.b() as u32));
        }
        if let Some(a) = &self.author {
            let mut m = json!({ "name": a.name });
            if !a.url.is_empty() {
                m["url"] = json!(a.url);
            }
            if !a.icon.is_empty() {
                m["icon_url"] = json!(a.icon);
            }
            put("author", m);
        }
        if !self.fields.is_empty() {
            put("fields", self.fields.iter().map(|f| json!({ "name": f.name, "value": f.value, "inline": f.inline })).collect());
        }
        if !self.image.is_empty() {
            put("image", json!({ "url": self.image }));
        }
        if !self.thumbnail.is_empty() {
            put("thumbnail", json!({ "url": self.thumbnail }));
        }
        if let Some(f) = &self.footer {
            let mut m = json!({ "text": f.text });
            if !f.icon.is_empty() {
                m["icon_url"] = json!(f.icon);
            }
            put("footer", m);
        }
        if !self.time.is_empty() {
            put("timestamp", json!(self.time));
        } else if self.timestamp {
            put("timestamp", json!("now"));
        }
        Value::Object(out)
    }

    /// An embed in Discord's JSON, or a whole message with `embeds` (its first one).
    pub fn from_discord(v: &Value) -> Result<EmbedData, String> {
        let v = match v.get("embeds").and_then(Value::as_array) {
            Some(list) => list.first().ok_or_else(|| t!("the message has no embeds").to_string())?,
            None => v,
        };
        if !v.is_object() {
            return Err(t!("paste an embed object { … }").into());
        }
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let url_of = |k: &str| v.get(k).map(|i| s(i, "url")).unwrap_or_default();
        let color = match v.get("color") {
            Some(Value::Number(n)) => format!("#{:06X}", n.as_u64().unwrap_or(0) & 0xFF_FFFF),
            Some(Value::String(c)) => c.clone(),
            _ => String::new(),
        };
        let time = s(v, "timestamp");
        Ok(EmbedData {
            color,
            title: s(v, "title"),
            url: s(v, "url"),
            description: s(v, "description"),
            author: v.get("author").map(|a| NameIcon { name: s(a, "name"), icon: s(a, "icon_url"), url: s(a, "url") }),
            fields: v
                .get("fields")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|f| EmbedField { name: s(f, "name"), value: s(f, "value"), inline: f.get("inline").and_then(Value::as_bool).unwrap_or(false) })
                .collect(),
            footer: v.get("footer").map(|f| FooterData { text: s(f, "text"), icon: s(f, "icon_url") }),
            image: url_of("image"),
            thumbnail: url_of("thumbnail"),
            timestamp: time == "now",
            time: if time == "now" { String::new() } else { time },
        })
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
    pieces: Vec<(Pos2, Marked)>,
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
    // Discord's markdown where Discord reads it (None: plain, as in the author and footer).
    let marked = |s: &str, md: Option<Kind>, size: f32, color: Color32, wrap: f32| match md {
        Some(kind) => crate::markdown::layout(painter, s, kind, size * scale, color, wrap),
        // The author is bold in Discord (it comes in STRONG), the footer not.
        None => {
            let font = if color == STRONG { crate::fonts::bold(size * scale) } else { FontId::proportional(size * scale) };
            Marked::plain(painter.layout_job(crate::fonts::usable(painter.ctx(), crate::chips::job(s, font, color, wrap))))
        }
    };
    #[allow(clippy::too_many_arguments)]
    let text = |pieces: &mut Vec<(Pos2, Marked)>, s: &str, md: Option<Kind>, size: f32, color: Color32, wrap: f32, y: &mut f32, x: f32, gap: f32| {
        if s.trim().is_empty() {
            return;
        }
        let m = marked(s, md, size, color, wrap);
        *y += m.size().y + gap * scale;
        pieces.push((pos2(x, *y - m.size().y - gap * scale), m));
    };
    let x0 = pad + 4.0;
    if let Some(a) = &e.author {
        text(&mut pieces, &a.name, None, 12.0, STRONG, inner, &mut y, x0, 6.0);
    }
    let title_color = if e.url.trim().is_empty() { STRONG } else { LINK };
    text(&mut pieces, &e.title, Some(Kind::Inline), 14.5, title_color, inner, &mut y, x0, 6.0);
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
    text(&mut pieces, &desc, Some(Kind::Full), 12.5, TEXT, inner, &mut y, x0, 8.0);

    // Fields: inline ones share a row, up to three.
    let mut row: Vec<&EmbedField> = Vec::new();
    let flush = |row: &mut Vec<&EmbedField>, y: &mut f32, pieces: &mut Vec<(Pos2, Marked)>| {
        if row.is_empty() {
            return;
        }
        let col_w = inner / row.len() as f32;
        let mut tallest: f32 = 0.0;
        for (i, f) in row.iter().enumerate() {
            let x = x0 + col_w * i as f32;
            let n = marked(&f.name, Some(Kind::Inline), 12.0, STRONG, col_w - 8.0);
            let v = marked(&f.value, Some(Kind::Full), 12.0, TEXT, col_w - 8.0);
            let h = n.size().y + 2.0 + v.size().y;
            let name_h = n.size().y;
            pieces.push((pos2(x, *y), n));
            pieces.push((pos2(x, *y + name_h + 2.0), v));
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
    let footer = match (&e.footer, e.timestamp || !e.time.is_empty()) {
        (Some(f), true) if !f.text.is_empty() => format!("{} · Today at 12:00", f.text),
        (Some(f), _) => f.text.clone(),
        (None, true) => "Today at 12:00".into(),
        (None, false) => String::new(),
    };
    text(&mut pieces, &footer, None, 11.0, MUTED, inner, &mut y, x0, 0.0);
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
    for (p, m) in &l.pieces {
        m.paint(painter, at + p.to_vec2());
    }
}

/// An open Embed Builder: which field it edits and the draft.
pub struct EmbedEdit {
    pub ws: String,
    pub node: String,
    pub key: String,
    pub draft: EmbedData,
    /// The draft as JSON, while switched to JSON.
    pub json: Option<JsonText>,
}

pub fn open(ed: &mut Editor, ws: &str, node: &str, key: &str) {
    let value = ed
        .workspace(ws)
        .and_then(|w| w.nodes.get(node))
        .and_then(|n| n.values.get(key).cloned())
        .unwrap_or(Value::Null);
    ed.embed_edit = Some(EmbedEdit { ws: ws.into(), node: node.into(), key: key.into(), draft: EmbedData::from_value(&value), json: None });
}

/// The Embed Builder tab.
pub fn tab(ui: &mut Ui, ed: &mut Editor) {
    let Some((ws, node)) = ed.embed_edit.as_ref().map(|e| (e.ws.clone(), e.node.clone())) else { return };
    // Texts offer the values of earlier blocks after `{`, and list them on the right.
    let values = std::sync::Arc::new(ed.values_for(&ws, &node));
    crate::values::set_scope(ui.ctx(), Some(values.clone()));
    let bot = ed.bot_profile.shown.clone();
    let Some(edit) = &mut ed.embed_edit else { return };
    let theme = ed.theme.clone();
    let icon = ed.reg.icons.get("embed").cloned();
    let (mut apply, mut close, mut cancel) = (false, false, false);
    dialog::tab(
        ui,
        &theme,
        dialog::Spec {
            id: "embed-builder",
            title: t!("Embed Builder"),
            subtitle: Some(t!("${…} in any text holds JavaScript: type ${ for values from earlier blocks")),
            icon: icon.as_deref(),
            width: 1040.0,
        },
        |ui, part| match part {
            Part::Body => {
                ui.horizontal_top(|ui| {
                    let h = dialog::remaining_height(ui, 420.0);
                    ui.allocate_ui_with_layout(vec2(470.0, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                        if let Some(json) = json_view::switch(ui, &theme, edit.json.is_some()) {
                            if json {
                                edit.json = Some(JsonText::new(&edit.draft.to_discord()));
                            } else if edit.json.as_ref().is_none_or(|j| j.error.is_none()) {
                                edit.json = None;
                            }
                        }
                        ui.add_space(8.0);
                        if let Some(jt) = &mut edit.json {
                            let help = t!("The embed as Discord's JSON. Edit it, or paste an embed or a whole message (from an embed generator, say); it applies as soon as it is valid. \"timestamp\": \"now\" shows when it's sent.");
                            if let Some(v) = json_view::editor(ui, &theme, ui.id().with("embed-json"), jt, help) {
                                match EmbedData::from_discord(&v) {
                                    Ok(e) => edit.draft = e,
                                    Err(e) => jt.error = Some(e),
                                }
                            }
                            return;
                        }
                        ScrollArea::vertical().id_salt("embed-form").max_height(dialog::remaining_height(ui, 300.0)).auto_shrink([false, false]).show(ui, |ui| {
                            ui.set_width(452.0);
                            form(ui, &theme, &mut edit.draft);
                        });
                    });
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        dialog::section(ui, &theme, t!("Preview"));
                        preview_message(ui, &theme, &edit.draft, bot.as_ref());
                        ui.add_space(10.0);
                        dialog::section(ui, &theme, t!("Values from earlier blocks"));
                        ScrollArea::vertical().id_salt("embed-values").max_height(dialog::remaining_height(ui, 140.0)).auto_shrink([false, true]).show(ui, |ui| crate::values::list(ui, &theme, &values));
                    });
                });
            }
            Part::Footer => {
                let broken = edit.json.as_ref().is_some_and(|j| j.error.is_some());
                ui.add_enabled_ui(!broken, |ui| {
                    if dialog::primary_button(ui, &theme, t!("Apply and close")).clicked() {
                        (apply, close) = (true, true);
                    }
                    apply |= dialog::secondary_button(ui, &theme, t!("Apply")).on_hover_text(t!("Put it on the block and keep editing")).clicked();
                });
                cancel = dialog::secondary_button(ui, &theme, t!("Cancel")).clicked();
                if broken {
                    ui.label(RichText::new(t!("Fix the JSON first")).size(12.0).color(theme.warning));
                }
            }
        },
    );
    crate::values::set_scope(ui.ctx(), None);
    if apply {
        let edit = ed.embed_edit.as_ref().unwrap();
        let (ws, node, key, value) = (edit.ws.clone(), edit.node.clone(), edit.key.clone(), edit.draft.to_value());
        if close {
            ed.embed_edit = None;
        }
        if let Some(n) = ed.project.as_mut().and_then(|p| p.workspaces.get_mut(&ws)).and_then(|w| w.nodes.get_mut(&node)) {
            n.values.insert(key, value);
            ed.edited = true;
        }
    } else if cancel {
        ed.embed_edit = None;
    }
}

fn form(ui: &mut Ui, theme: &Theme, e: &mut EmbedData) {
    dialog::section(ui, theme, t!("Color"));
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

    dialog::section(ui, theme, t!("Author"));
    let author = e.author.get_or_insert_with(NameIcon::default);
    two(ui, theme, (t!("Name"), &mut author.name, "${user}"), (t!("Icon URL"), &mut author.icon, "https://…"));
    dialog::field(ui, theme, t!("Name link"), None, |ui| dialog::text_input(ui, &mut author.url, "https://…", false));
    if author.name.is_empty() && author.icon.is_empty() && author.url.is_empty() {
        e.author = None;
    }

    dialog::section(ui, theme, t!("Body"));
    dialog::field(ui, theme, t!("Title"), None, |ui| dialog::text_input(ui, &mut e.title, t!("Rank of ${user}"), false));
    dialog::field(ui, theme, t!("Title link"), None, |ui| dialog::text_input(ui, &mut e.url, "https://…", false));
    dialog::field(ui, theme, t!("Description"), Some(t!("Markdown works: **bold**, *italic*, `code`. Drag the corner to make the box taller.")), |ui| {
        ui.add(crate::inputs::TextArea::new("embed-description", &mut e.description).hint(t!("You have **${xp} XP**…")).rows(5))
    });

    dialog::section(ui, theme, &format!("Fields · {} of 25", e.fields.len()));
    let mut remove = None;
    let mut swap = None;
    let count = e.fields.len();
    // Every row gets the same width, measured once: sizing each row from what is
    // left after the previous one made every row a little wider than the last.
    let inner_w = ui.available_width() - 2.0 * 10.0 - 2.0;
    let name_w = (inner_w * 0.38).floor();
    for (i, f) in e.fields.iter_mut().enumerate() {
        egui::Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(8).inner_margin(10).show(ui, |ui| {
            ui.set_width(inner_w);
            ui.horizontal(|ui| {
                ui.add(crate::inputs::TextLine::new(&mut f.name).hint(t!("Name")).width(name_w));
                ui.add(crate::inputs::TextLine::new(&mut f.value).hint(t!("Value")).width(inner_w - name_w - 8.0));
            });
            ui.add_space(4.0);
            // The switch and buttons below, so the texts get the whole width.
            ui.horizontal(|ui| {
                ui.set_min_height(crate::inputs::input_height(ui));
                ui.label(RichText::new(t!("Inline")).size(12.0).color(theme.text_muted));
                crate::widgets::toggle(ui, &mut f.inline, theme).on_hover_text(t!("Inline: share a row with other inline fields"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if dialog::danger_button(ui, theme, t!("Remove")).clicked() {
                        remove = Some(i);
                    }
                    if ui.add_enabled_ui(i + 1 < count, |ui| dialog::button(ui, theme, t!("Down"))).inner.clicked() {
                        swap = Some((i, i + 1));
                    }
                    if ui.add_enabled_ui(i > 0, |ui| dialog::button(ui, theme, t!("Up"))).inner.clicked() {
                        swap = Some((i, i - 1));
                    }
                });
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
    if e.fields.len() < 25 && dialog::secondary_button(ui, theme, t!("+ Add field")).clicked() {
        e.fields.push(EmbedField { inline: true, ..Default::default() });
    }
    ui.add_space(12.0);

    dialog::section(ui, theme, t!("Images"));
    two(ui, theme, (t!("Image URL"), &mut e.image, "https://…"), (t!("Thumbnail URL"), &mut e.thumbnail, "https://… or ${avatar}"));

    dialog::section(ui, theme, t!("Footer"));
    let footer = e.footer.get_or_insert_with(FooterData::default);
    two(ui, theme, (t!("Text"), &mut footer.text, "DemoBot"), (t!("Icon URL"), &mut footer.icon, "https://…"));
    if footer.text.is_empty() && footer.icon.is_empty() {
        e.footer = None;
    }
    dialog::section(ui, theme, t!("Timestamp"));
    ui.horizontal(|ui| {
        ui.set_min_height(crate::inputs::input_height(ui));
        // None, when it's sent, or a set time (kept in `time`).
        let set_id = ui.id().with("set-time");
        let mut kind = if !e.time.is_empty() || ui.memory(|m| m.data.get_temp::<bool>(set_id).unwrap_or(false)) { 2 } else if e.timestamp { 1 } else { 0 };
        let labels = [t!("None"), t!("When it's sent"), t!("A set time")];
        crate::widgets::dropdown(ui, ui.id().with("timestamp"), 160.0, labels[kind], |ui| {
            for (i, l) in labels.iter().enumerate() {
                ui.selectable_value(&mut kind, i, *l);
            }
        });
        e.timestamp = kind == 1;
        if kind != 2 {
            e.time.clear();
        }
        ui.memory_mut(|m| m.data.insert_temp(set_id, kind == 2));
        if kind == 2 {
            let w = ui.available_width();
            ui.add(crate::inputs::TextLine::new(&mut e.time).hint("2026-12-24T18:00:00Z or ${date}").width(w));
        }
    });
    ui.label(RichText::new(t!("Shown next to the footer, in each reader's own time zone.")).size(11.5).color(theme.text_muted));
}

fn two(ui: &mut Ui, theme: &Theme, a: (&str, &mut String, &str), b: (&str, &mut String, &str)) {
    ui.columns(2, |cols| {
        dialog::field(&mut cols[0], theme, a.0, None, |ui| dialog::text_input(ui, a.1, a.2, false));
        dialog::field(&mut cols[1], theme, b.0, None, |ui| dialog::text_input(ui, b.1, b.2, false));
    });
}

/// The embed as it appears in a Discord message, with the bot's name above it.
fn preview_message(ui: &mut Ui, theme: &Theme, e: &EmbedData, bot: Option<&crate::bot_profile::BotProfile>) {
    let width = ui.available_width().min(500.0);
    let painter = ui.painter().clone();
    let card_w = width - 76.0;
    let l = layout(&painter, e, card_w, None, 1.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, l.height + 70.0), egui::Sense::hover());
    painter.rect_filled(rect, 8.0, Color32::from_rgb(0x31, 0x33, 0x38));
    let avatar = rect.min + vec2(14.0, 16.0);
    crate::bot_profile::paint_avatar(ui, Rect::from_min_size(avatar, vec2(36.0, 36.0)), bot, theme.accent);
    let name = painter.text(avatar + vec2(48.0, 2.0), egui::Align2::LEFT_TOP, crate::bot_profile::name(bot), FontId::proportional(14.0), STRONG);
    painter.text(pos2(name.right() + 8.0, avatar.y + 4.0), egui::Align2::LEFT_TOP, "APP · Today at 12:00", FontId::proportional(11.0), MUTED);
    paint(&painter, rect.min + vec2(62.0, 44.0), card_w, &l);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_go_to_discord_json_and_back() {
        let e = EmbedData {
            color: "#5865F2".into(),
            title: "Rank of ${user}".into(),
            url: "https://example.com".into(),
            description: "**${xp}** XP".into(),
            author: Some(NameIcon { name: "${user}".into(), icon: "${avatar}".into(), url: "https://a.example".into() }),
            fields: vec![EmbedField { name: "Level".into(), value: "${level}".into(), inline: true }],
            footer: Some(FooterData { text: "DemoBot".into(), icon: String::new() }),
            image: "https://i.example/a.png".into(),
            thumbnail: String::new(),
            timestamp: true,
            time: String::new(),
        };
        let v = e.to_discord();
        assert_eq!(v["color"], 0x5865F2);
        assert_eq!(v["author"]["icon_url"], "${avatar}");
        assert_eq!(v["timestamp"], "now");
        assert_eq!(EmbedData::from_discord(&v).unwrap(), e);
        // A whole message from an embed generator: its first embed, with a set time.
        let message = json!({ "content": "hi", "embeds": [{ "title": "T", "color": 16711680, "timestamp": "2026-12-24T18:00:00.000Z" }] });
        let back = EmbedData::from_discord(&message).unwrap();
        assert_eq!((back.title.as_str(), back.color.as_str(), back.time.as_str(), back.timestamp), ("T", "#FF0000", "2026-12-24T18:00:00.000Z", false));
        assert!(EmbedData::from_discord(&json!([1])).is_err());
    }
}
