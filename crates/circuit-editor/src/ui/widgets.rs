//! Inline editors for field values, shared by blocks on the canvas and the inspector.

use circuit_core::{FieldDef, PinType};
use eframe::egui::{self, Color32, ComboBox, DragValue, TextEdit, Ui};
use serde_json::{Number, Value};

use crate::ui::kit;
use crate::ui::theme::{Theme, hex_color};

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
    // Every widget inside gets an id from `id`, not from its place among other
    // widgets: number boxes and text boxes keep their editing state across frames.
    ui.push_id(id, |ui| editor(ui, id, field, value, width, multiline, theme)).inner
}

fn editor(ui: &mut Ui, id: egui::Id, field: &FieldDef, value: &mut Value, width: f32, multiline: bool, theme: &Theme) -> bool {
    if field.is_select() {
        return select(ui, id, field, value, width, multiline);
    }
    if field.type_name == "color" {
        return color(ui, id, value, width, theme);
    }
    match field.ty.required() {
        PinType::Boolean => {
            let mut b = value.as_bool().unwrap_or(false);
            let changed = crate::ui::kit::toggle(ui, &mut b, theme).changed();
            if changed {
                *value = Value::Bool(b);
            }
            changed
        }
        PinType::Number => {
            let mut n = value.as_f64().or_else(|| value.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0);
            let speed = field.step.unwrap_or(0.1).max(0.01);
            let range = field.min.map(|min| min..=field.max.unwrap_or(f64::MAX));
            // The Inspector (multiline) draws with the kit; a block on the canvas stays compact.
            let changed = if multiline {
                let mut number = kit::Number::new(&mut n).speed(speed).width(width);
                if let Some(r) = range {
                    number = number.range(r);
                }
                number.show(ui, theme).changed()
            } else {
                let mut drag = DragValue::new(&mut n).speed(speed);
                if let Some(r) = range {
                    drag = drag.range(r);
                }
                let resp = ui.add_sized([width, 22.0], drag);
                kit::named(ui, resp).changed()
            };
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
                ui.add(crate::ui::kit::TextArea::new(id, &mut s).hint(hint).rows(3)).changed()
            } else if multiline {
                ui.add(crate::ui::kit::TextLine::new(&mut s).hint(hint).width(width).compact()).changed()
            } else {
                let resp = ui.add(TextEdit::singleline(&mut s).desired_width(width).hint_text(hint));
                kit::named(ui, resp).changed()
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

/// How wide a choice field's dropdown is with its current choice showing
/// (None for other fields), so a block can be made wide enough for it.
pub fn select_width(ui: &Ui, field: &FieldDef, value: &Value) -> Option<f32> {
    if !field.is_select() {
        return None;
    }
    let current = value.as_str().unwrap_or_default();
    let choices = field.choice_list();
    let label = choices.iter().find(|(_, k, _)| k == current).map(|(_, _, l)| l.as_str()).unwrap_or(current);
    let font = egui::TextStyle::Button.resolve(ui.style());
    let text = ui.fonts_mut(|f| f.layout_no_wrap(label.to_string(), font, egui::Color32::WHITE)).size().x;
    let s = ui.spacing();
    Some(text + s.icon_spacing + s.icon_width + 2.0 * s.button_padding.x + 14.0)
}

fn select(ui: &mut Ui, id: egui::Id, field: &FieldDef, value: &mut Value, width: f32, full: bool) -> bool {
    let choices: Vec<(Option<String>, String, String)> = field.choice_list();
    let current = value.as_str().unwrap_or_default().to_string();
    let label = choices.iter().find(|(_, k, _)| *k == current).map(|(_, _, l)| l.clone()).unwrap_or(current.clone());
    let mut picked = None;
    let list = |ui: &mut Ui| {
        let mut last_group: Option<&String> = None;
        for (group, key, text) in &choices {
            if group.as_ref() != last_group {
                if let Some(g) = group {
                    ui.label(egui::RichText::new(g.to_uppercase()).small().weak());
                }
                last_group = group.as_ref();
            }
            if kit::option(ui, *key == current, text).clicked() {
                picked = Some(key.clone());
            }
        }
    };
    if full {
        kit::dropdown(ui, id, width, label, list);
    } else {
        let resp = ComboBox::from_id_salt(id).width(width).selected_text(label).show_ui(ui, list).response;
        kit::named(ui, resp);
    }
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
    let changed = crate::builders::color::color_button(ui, id, &mut c, theme, Some(width.clamp(96.0, 180.0)));
    if changed {
        *value = Value::String(crate::builders::color::to_hex(c));
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The button keeps its whole text and stays inside the row, however long
    /// it is in this language, and the text box is just as tall.
    #[test]
    fn field_with_button_fits_any_button() {
        let theme = Theme::from_def(&circuit_plugins::ThemeDef::default());
        for (width, label) in [(240.0, "Aktualisieren"), (320.0, "Refresh"), (300.0, "Durchsuchen …")] {
            let ctx = egui::Context::default();
            let (mut field, mut button) = (egui::Rect::NOTHING, egui::Rect::NOTHING);
            for _ in 0..3 {
                let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 300.0))), ..Default::default() };
                let mut out = ctx.run_ui(input, |ui| {
                    let mut text = String::new();
                    crate::ui::kit::field_with_button(ui, |ui| field = ui.add(crate::ui::kit::TextLine::new(&mut text).hint("Filter keys")).rect, |ui| button = crate::ui::kit::button(ui, &theme, label).rect);
                });
                out.textures_delta.clear();
            }
            let row = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 300.0));
            assert!(button.right() <= row.right() + 0.5 && field.left() >= row.left() - 0.5, "{label} at {width}: {field:?} {button:?}");
            assert!(field.right() <= button.left(), "{label}: they don't overlap");
            assert!((field.height() - button.height()).abs() < 0.5, "{label}: same height, {field:?} {button:?}");
            assert!(field.width() > 100.0, "{label}: the text box gets the rest");
        }
    }
}
