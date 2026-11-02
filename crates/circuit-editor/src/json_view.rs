//! The Builder | JSON switch the builders share, and the JSON text editor
//! behind it: edits and pasted JSON apply as soon as they are valid.

use eframe::egui::{self, Color32, RichText, Stroke, Ui, vec2};
use serde_json::Value;

use crate::dialog;
use crate::theme::Theme;

/// The JSON a builder shows while switched to JSON, and why it doesn't apply.
#[derive(Clone, Debug, Default)]
pub struct JsonText {
    pub text: String,
    pub error: Option<String>,
}

impl JsonText {
    pub fn new(v: &Value) -> JsonText {
        JsonText { text: serde_json::to_string_pretty(v).unwrap_or_default(), error: None }
    }
}

/// The Builder | JSON switch. Returns the side clicked this frame, if it is
/// not the one showing.
pub fn switch(ui: &mut Ui, theme: &Theme, json: bool) -> Option<bool> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let h = crate::inputs::input_height(ui);
        for (is_json, label) in [(false, t!("Builder")), (true, "JSON")] {
            let on = is_json == json;
            let ink = if on { if theme.dark { Color32::from_rgb(0, 24, 28) } else { Color32::WHITE } } else { theme.text };
            let corner = if is_json { egui::CornerRadius { nw: 0, sw: 0, ne: 7, se: 7 } } else { egui::CornerRadius { nw: 7, sw: 7, ne: 0, se: 0 } };
            let b = egui::Button::new(RichText::new(label).size(13.0).color(ink)).corner_radius(corner).min_size(vec2(82.0, h));
            let fill = if on { theme.accent } else { theme.panel };
            if dialog::styled_button(ui, theme, b, fill, Stroke::new(1.0, if on { theme.accent } else { theme.node_border })).clicked() && !on {
                picked = Some(is_json);
            }
        }
    });
    picked
}

/// The JSON as editable text, with Format and Copy. Returns the parsed value
/// after an edit that parses; a parse error is kept in `jt.error` and shown.
pub fn editor(ui: &mut Ui, theme: &Theme, id: egui::Id, jt: &mut JsonText, height: f32, help: &str) -> Option<Value> {
    let mut parsed = None;
    ui.label(RichText::new(help).size(12.0).color(theme.text_muted));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if dialog::button(ui, theme, t!("Format")).on_hover_text(t!("Indent the JSON neatly")).clicked()
            && let Ok(v) = serde_json::from_str::<Value>(&jt.text)
        {
            jt.text = serde_json::to_string_pretty(&v).unwrap_or_default();
        }
        if dialog::button(ui, theme, t!("Copy")).clicked() {
            ui.ctx().copy_text(jt.text.clone());
        }
        if dialog::button(ui, theme, t!("Clear")).on_hover_text(t!("Empty the box, to paste other JSON in")).clicked() {
            jt.text.clear();
        }
        match &jt.error {
            Some(e) => ui.label(RichText::new(e.as_str()).size(12.0).color(theme.warning)),
            None => ui.label(RichText::new(t!("Applied")).size(12.0).color(theme.action)),
        };
    });
    ui.add_space(6.0);
    let before = jt.text.clone();
    egui::Frame::new().fill(theme.field).stroke(Stroke::new(1.0, if jt.error.is_some() { theme.warning } else { theme.node_border })).corner_radius(7).inner_margin(8).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt(id.with("scroll")).max_height(height).auto_shrink([false, false]).show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut jt.text)
                    .id(id)
                    .code_editor()
                    .frame(egui::Frame::NONE)
                    .desired_width(f32::INFINITY)
                    .min_size(vec2(0.0, height - 4.0)),
            );
        });
    });
    if jt.text != before {
        match serde_json::from_str::<Value>(&jt.text) {
            Ok(v) => {
                jt.error = None;
                parsed = Some(v);
            }
            Err(e) => jt.error = Some(tf!("Line {line}, column {column}: {problem}", line = e.line(), column = e.column(), problem = e.to_string().split(" at line").next().unwrap_or_default())),
        }
    }
    parsed
}
