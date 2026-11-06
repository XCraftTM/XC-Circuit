//! Inline editors for field values, shared by blocks on the canvas and the inspector.

use circuit_core::{FieldDef, PinType};
use eframe::egui::{self, Color32, ComboBox, DragValue, Response, Sense, TextEdit, Ui, Vec2};
use serde_json::{Number, Value};

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
                ui.add(crate::ui::inputs::TextArea::new(id, &mut s).hint(hint).rows(3)).changed()
            } else if multiline {
                ui.add(crate::ui::inputs::TextLine::new(&mut s).hint(hint).width(width).compact()).changed()
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

/// A row with a text box and a control after it (a button): the control takes
/// what it needs, in any language, and the text box the rest of the width.
/// The row is as tall as a text box.
/// A drawn check mark: the interface font has no ✓.
pub fn check_mark(painter: &egui::Painter, center: egui::Pos2, size: f32, color: Color32) {
    let s = size / 2.0;
    painter.line(vec![center + egui::vec2(-s, 0.0), center + egui::vec2(-s * 0.3, s * 0.65), center + egui::vec2(s, -s * 0.6)], egui::Stroke::new(size / 6.0 + 0.6, color));
}

/// A check mark and `text`, in `color`: something that worked.
pub fn done_label(ui: &mut Ui, text: impl Into<egui::RichText>, color: Color32) -> Response {
    let text = text.into().color(color);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let size = ui.text_style_height(&egui::TextStyle::Body);
        let (r, _) = ui.allocate_exact_size(egui::vec2(size * 0.8, size), Sense::hover());
        check_mark(ui.painter(), r.center(), size * 0.62, color);
        ui.label(text);
    })
    .response
}

/// A row of a label and controls, all in its middle. egui centres each item
/// in the height the row has when the item is added, so a label put before
/// a taller control (a dropdown, a slider, a field) would sit at the top:
/// the row gets its full height first. (It starts at that height too: in
/// another row, a shorter start would be centred there and then grow down.)
pub fn line<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let h = crate::ui::inputs::input_height(ui).max(ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(egui::vec2(ui.available_size_before_wrap().x, h), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_height(h);
        add(ui)
    })
    .inner
}

pub fn field_with_button(ui: &mut Ui, field: impl FnOnce(&mut Ui), button: impl FnOnce(&mut Ui)) {
    let size = egui::vec2(ui.available_width(), crate::ui::inputs::input_height(ui).max(ui.spacing().interact_size.y));
    ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        button(ui);
        field(ui);
    });
}

/// A dropdown as tall as a text box, so the two line up in a row.
pub fn dropdown<R>(ui: &mut Ui, id_salt: egui::Id, width: f32, selected: impl Into<egui::WidgetText>, contents: impl FnOnce(&mut Ui) -> R) -> Response {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = crate::ui::inputs::input_height(ui);
        ComboBox::from_id_salt(id_salt).width(width).selected_text(selected).show_ui(ui, contents).response
    })
    .inner
}

/// A dropdown with a checkbox per option, for picking any number of them, as
/// tall as a text box. Closed, it names the first one picked (and how many
/// more), or says `none`. Options are (key, English label). Returns whether
/// the picks changed.
pub fn multi_select<K: PartialEq + Clone>(ui: &mut Ui, id_salt: egui::Id, width: f32, options: &[(K, &'static str)], picked: &mut Vec<K>, none: &str) -> bool {
    let name = |k: &K| options.iter().find(|(o, _)| o == k).map(|(_, l)| crate::i18n::t(l)).unwrap_or_default().to_string();
    let summary = match picked.len() {
        0 => none.to_string(),
        1 => name(&picked[0]),
        n => tf!("{first} + {more} more", first = name(&picked[0]), more = n - 1),
    };
    let mut changed = false;
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = crate::ui::inputs::input_height(ui);
        ComboBox::from_id_salt(id_salt).width(width).selected_text(summary).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show_ui(ui, |ui| {
            for (key, label) in options {
                let mut on = picked.contains(key);
                if ui.checkbox(&mut on, crate::i18n::t(label)).changed() {
                    if on {
                        picked.push(key.clone());
                    } else {
                        picked.retain(|k| k != key);
                    }
                    changed = true;
                }
            }
        });
    });
    changed
}

fn select(ui: &mut Ui, id: egui::Id, field: &FieldDef, value: &mut Value, width: f32) -> bool {
    let choices: Vec<(Option<String>, String, String)> = field.choice_list();
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
    let changed = crate::builders::color::color_button(ui, id, &mut c, theme, Some(width.clamp(96.0, 180.0)));
    if changed {
        *value = Value::String(crate::builders::color::to_hex(c));
    }
    changed
}

/// A pill-shaped on/off switch in the theme's Action green, one size
/// everywhere: big enough to hit with a finger.
pub fn toggle(ui: &mut Ui, on: &mut bool, theme: &Theme) -> Response {
    let size = Vec2::new(44.0, 24.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(response.id, *on);
    let bg = crate::ui::theme::mix(theme.field, theme.action, t);
    let painter = ui.painter();
    painter.rect_filled(rect, rect.height() / 2.0, bg);
    let r = rect.height() / 2.0 - 2.5;
    let x = egui::lerp((rect.left() + r + 2.5)..=(rect.right() - r - 2.5), t);
    let knob = if *on { Color32::from_rgb(4, 33, 13) } else { theme.text_muted };
    painter.circle_filled(egui::pos2(x, rect.center().y), r, knob);
    response
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
                    field_with_button(ui, |ui| field = ui.add(crate::ui::inputs::TextLine::new(&mut text).hint("Filter keys")).rect, |ui| button = crate::ui::dialog::button(ui, &theme, label).rect);
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
