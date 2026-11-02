//! Inline editors for field values, shared by blocks on the canvas and the inspector.

use circuit_core::{FieldDef, PinType};
use eframe::egui::{self, Color32, ComboBox, DragValue, Response, Sense, TextEdit, Ui, Vec2};
use serde_json::{Number, Value};

use crate::theme::{Theme, hex_color};

/// Whether a field can be edited inline at all (lists, objects and plugin types must be connected).
pub fn is_inline_editable(field: &FieldDef) -> bool {
    field.is_select()
        || matches!(field.type_name.as_str(), "color" | "textarea")
        || matches!(field.ty.required(), PinType::Text | PinType::Number | PinType::Boolean | PinType::Any | PinType::Date)
}

/// Whether a field is edited as free text (and so can be expanded on the canvas).
pub fn is_plain_text(field: &FieldDef) -> bool {
    !field.is_select() && field.type_name != "color" && is_inline_editable(field) && !matches!(field.ty.required(), PinType::Boolean | PinType::Number)
}

/// A field value as the text a text box shows.
pub fn as_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Draws an editor for one field value. Returns true when the value changed.
pub fn value_editor(ui: &mut Ui, id: egui::Id, field: &FieldDef, value: &mut Value, width: f32, multiline: bool, theme: &Theme) -> bool {
    if field.is_select() {
        return select(ui, id, field, value, width);
    }
    if field.type_name == "color" {
        return color(ui, id, value, width, theme);
    }
    match field.ty.required() {
        PinType::Boolean => {
            let mut b = value.as_bool().unwrap_or(false);
            let changed = toggle(ui, &mut b, theme).changed();
            if changed {
                *value = Value::Bool(b);
            }
            changed
        }
        PinType::Number => {
            let mut n = value.as_f64().or_else(|| value.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0);
            let mut drag = DragValue::new(&mut n).speed(field.step.unwrap_or(0.1).max(0.01));
            if let Some(min) = field.min {
                drag = drag.range(min..=field.max.unwrap_or(f64::MAX));
            }
            let changed = ui.add_sized([width, 22.0], drag).changed();
            if changed {
                *value = Number::from_f64(n).map(Value::Number).unwrap_or(Value::Null);
            }
            changed
        }
        _ if is_inline_editable(field) => {
            let mut s = match &*value {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            let hint = field.placeholder.clone().unwrap_or_default();
            let changed = if multiline && field.type_name == "textarea" {
                ui.add(crate::inputs::TextArea::new(id, &mut s).hint(hint).rows(3)).changed()
            } else if multiline {
                ui.add(crate::inputs::TextLine::new(&mut s).hint(hint).width(width).compact()).changed()
            } else {
                ui.add(TextEdit::singleline(&mut s).desired_width(width).hint_text(hint)).changed()
            };
            if changed {
                *value = Value::String(s);
            }
            changed
        }
        _ => {
            ui.label(egui::RichText::new(t!("connect a value")).italics().color(theme.text_muted));
            false
        }
    }
}

fn select(ui: &mut Ui, id: egui::Id, field: &FieldDef, value: &mut Value, width: f32) -> bool {
    // Choice texts are block labels: translated like field names.
    let choices: Vec<(Option<String>, String, String)> = field
        .choice_list()
        .into_iter()
        .map(|(g, k, l)| (g.map(|g| crate::i18n::label(&g).into_owned()), k, crate::i18n::label(&l).into_owned()))
        .collect();
    let current = value.as_str().unwrap_or_default().to_string();
    let label = choices.iter().find(|(_, k, _)| *k == current).map(|(_, _, l)| l.clone()).unwrap_or(current.clone());
    let mut picked = None;
    ComboBox::from_id_salt(id).width(width).selected_text(label).show_ui(ui, |ui| {
        let mut last_group: Option<&String> = None;
        for (group, key, text) in &choices {
            if group.as_ref() != last_group {
                if let Some(g) = group {
                    ui.label(egui::RichText::new(g.to_uppercase()).small().weak());
                }
                last_group = group.as_ref();
            }
            if ui.selectable_label(*key == current, text).clicked() {
                picked = Some(key.clone());
            }
        }
    });
    match picked {
        Some(k) if k != current => {
            *value = Value::String(k);
            true
        }
        _ => false,
    }
}

fn color(ui: &mut Ui, id: egui::Id, value: &mut Value, width: f32, theme: &Theme) -> bool {
    let mut c = value.as_str().and_then(hex_color).unwrap_or(Color32::WHITE);
    let changed = crate::color::color_button(ui, id, &mut c, theme, Some(width.clamp(96.0, 180.0)));
    if changed {
        *value = Value::String(crate::color::to_hex(c));
    }
    changed
}

/// A pill-shaped on/off switch in the theme's Action green.
pub fn toggle(ui: &mut Ui, on: &mut bool, theme: &Theme) -> Response {
    let size = if theme.touch { Vec2::new(44.0, 24.0) } else { Vec2::new(30.0, 17.0) };
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(response.id, *on);
    let bg = crate::theme::mix(theme.field, theme.action, t);
    let painter = ui.painter();
    painter.rect_filled(rect, rect.height() / 2.0, bg);
    let r = rect.height() / 2.0 - 2.5;
    let x = egui::lerp((rect.left() + r + 2.5)..=(rect.right() - r - 2.5), t);
    let knob = if *on { Color32::from_rgb(4, 33, 13) } else { theme.text_muted };
    painter.circle_filled(egui::pos2(x, rect.center().y), r, knob);
    response
}
