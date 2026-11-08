//! Inline editors for field values, shared by blocks on the canvas and the inspector.

use circuit_core::{FieldDef, PinType};
use eframe::egui::{self, Color32, ComboBox, DragValue, TextEdit, Ui};
use serde_json::{Number, Value};

use crate::ui::kit;
use crate::ui::theme::{Theme, hex_color};

/// Whether a field can be edited inline at all (lists, objects and plugin types must be connected).
pub fn is_inline_editable(field: &FieldDef) -> bool {
    field.is_select()
        || field.is_multiselect()
        // A channel (or another value found by its id): its id, typed in.
        || field.by_id
        || matches!(field.type_name.as_str(), "color" | "textarea")
        || matches!(field.ty.required(), PinType::Text | PinType::Number | PinType::Boolean | PinType::Any | PinType::Date)
}

/// Whether a field is edited as free text (and so can be expanded on the canvas).
pub fn is_plain_text(field: &FieldDef) -> bool {
    !field.is_select() && !field.is_multiselect() && !field.by_id && field.suggest.is_none() && field.type_name != "color" && is_inline_editable(field) && !matches!(field.ty.required(), PinType::Boolean | PinType::Number)
}

/// Where the project's custom ids are kept for this frame (see `share_suggestions`).
const SUGGESTIONS: &str = "custom-id-suggestions";

/// Puts the project's custom ids, by kind, where `suggest` fields find them.
pub fn share_suggestions(ctx: &egui::Context, ids: std::collections::BTreeMap<String, Vec<String>>) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(SUGGESTIONS), std::sync::Arc::new(ids)));
}

/// A custom id field of an event: the ids of its kind the project uses, then
/// Custom… for any other (one made while the bot runs, say).
fn suggested(ui: &mut Ui, id: egui::Id, field: &FieldDef, kind: &str, value: &mut Value, width: f32, full: bool) -> bool {
    let all: Option<std::sync::Arc<std::collections::BTreeMap<String, Vec<String>>>> = ui.ctx().data(|d| d.get_temp(egui::Id::unique(SUGGESTIONS)));
    let ids = all.as_ref().and_then(|a| a.get(kind)).cloned().unwrap_or_default();
    let mut f = field.clone();
    f.choices = Some(Value::Object(ids.iter().map(|i| (i.clone(), Value::String(i.clone()))).collect()));
    f.custom = true;
    f.placeholder = field.placeholder.clone().or_else(|| Some(t!("my-button").to_string()));
    select_or_custom(ui, id, &f, value, width, full)
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
    if let Some(kind) = field.suggest.as_deref() {
        return suggested(ui, id, field, kind, value, width, multiline);
    }
    if field.is_select() {
        return select(ui, id, field, value, width, multiline);
    }
    if field.is_multiselect() {
        return multiselect(ui, id, field, value, width);
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
            let hint = field.placeholder.clone().unwrap_or_else(|| if field.by_id { id_hint(field) } else { String::new() });
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

/// What an id box asks for: "Channel id", "Thread id".
pub fn id_hint(field: &FieldDef) -> String {
    let kind = field.type_name.trim_end_matches('?').rsplit('.').next().unwrap_or("").replace('_', " ");
    match kind.as_str() {
        "thread" => t!("Thread id").to_string(),
        "voice channel" => t!("Voice channel id").to_string(),
        "channel" => t!("Channel id").to_string(),
        _ => t!("Id").to_string(),
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
    if field.custom {
        return select_or_custom(ui, id, field, value, width, full);
    }
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

/// A select with Custom… at the end of its list: picking it (or a value that
/// isn't one of the choices) shows a text box under the list for any other
/// value. Custom stays chosen while typing, even when the text matches a choice.
fn select_or_custom(ui: &mut Ui, id: egui::Id, field: &FieldDef, value: &mut Value, width: f32, full: bool) -> bool {
    let choices: Vec<(Option<String>, String, String)> = field.choice_list();
    let current = match &*value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    };
    let sticky = id.with("custom");
    let mut custom = ui.data(|d| d.get_temp::<bool>(sticky)).unwrap_or(false) || !choices.iter().any(|(_, k, _)| *k == current);
    let label = if custom { t!("Custom…").to_string() } else { choices.iter().find(|(_, k, _)| *k == current).map(|(_, _, l)| l.clone()).unwrap_or_default() };
    let (mut picked, mut chose_custom) = (None, false);
    let was_custom = custom;
    let mut changed = false;
    ui.vertical(|ui| {
        {
            let list = |ui: &mut Ui| {
                let mut last_group: Option<&String> = None;
                for (group, key, text) in &choices {
                    if group.as_ref() != last_group {
                        if let Some(g) = group {
                            ui.label(egui::RichText::new(g.to_uppercase()).small().weak());
                        }
                        last_group = group.as_ref();
                    }
                    if kit::option(ui, !was_custom && *key == current, text).clicked() {
                        picked = Some(key.clone());
                    }
                }
                ui.separator();
                if kit::option(ui, was_custom, t!("Custom…")).clicked() {
                    chose_custom = true;
                }
            };
            if full {
                kit::dropdown(ui, id, width, label, list);
            } else {
                let resp = ComboBox::from_id_salt(id).width(width).selected_text(label).show_ui(ui, list).response;
                kit::named(ui, resp);
            }
        }
        if let Some(k) = picked {
            ui.data_mut(|d| d.insert_temp(sticky, false));
            custom = false;
            if k != current {
                *value = Value::String(k);
                changed = true;
            }
        } else if chose_custom {
            ui.data_mut(|d| d.insert_temp(sticky, true));
            custom = true;
        }
        if custom {
            let mut text = current.clone();
            let hint = field.placeholder.clone().unwrap_or_else(|| t!("Type it…").to_string());
            let edited = if full {
                ui.add(kit::TextLine::new(&mut text).hint(hint).width(width).compact()).changed()
            } else {
                let resp = ui.add(TextEdit::singleline(&mut text).id(id.with("custom-text")).desired_width(width).hint_text(hint));
                kit::named(ui, resp).changed()
            };
            if edited {
                *value = Value::String(text);
                changed = true;
            }
        }
    });
    changed
}

/// The picked choices of a multi-select value, by key: a list, or (as older
/// projects typed them) text with commas whose items name a key or a label.
/// Items that match no choice are kept as they are.
pub fn chosen(field: &FieldDef, value: &Value) -> Vec<String> {
    let squash = |s: &str| s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
    let choices = field.choice_list();
    let items: Vec<String> = match value {
        Value::Array(list) => list.iter().map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).collect(),
        Value::String(text) => text.split([',', '\n']).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect(),
        _ => Vec::new(),
    };
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let key = choices.iter().find(|(_, k, l)| *k == item || squash(k) == squash(&item) || squash(l) == squash(&item)).map(|(_, k, _)| k.clone()).unwrap_or(item);
        if !out.contains(&key) {
            out.push(key);
        }
    }
    out
}

/// A dropdown of checkboxes, with a search when there are many.
fn multiselect(ui: &mut Ui, id: egui::Id, field: &FieldDef, value: &mut Value, width: f32) -> bool {
    let choices = field.choice_list();
    let mut picked = chosen(field, value);
    let label_of = |k: &str| choices.iter().find(|(_, key, _)| key == k).map(|(_, _, l)| l.clone()).unwrap_or_else(|| k.to_string());
    // Every pick, for the tooltip when the box is too narrow to show them.
    let all = picked.iter().map(|k| label_of(k)).collect::<Vec<_>>().join(", ");
    let shown = match picked.len() {
        0 => t!("None").to_string(),
        1..=2 => picked.iter().map(|k| label_of(k)).collect::<Vec<_>>().join(", "),
        n => tf!("{first} and {count} more", first = label_of(&picked[0]), count = n - 1),
    };
    let mut changed = false;
    let search_id = id.with("search");
    let resp = ComboBox::from_id_salt(id)
        .width(width)
        .selected_text(shown)
        // Long lists of picks end in … instead of widening the block.
        .truncate()
        // Ticking boxes leaves it open; a click outside closes it.
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            ui.set_min_width(width.max(220.0));
            let mut search: String = ui.data(|d| d.get_temp(search_id)).unwrap_or_default();
            if choices.len() > 8 {
                ui.add(kit::TextLine::new(&mut search).hint(t!("Search…")).compact());
                ui.add_space(4.0);
            }
            let wanted = search.trim().to_lowercase();
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                let mut last_group: Option<&String> = None;
                for (group, key, text) in &choices {
                    if !wanted.is_empty() && !text.to_lowercase().contains(&wanted) && !key.to_lowercase().contains(&wanted) {
                        continue;
                    }
                    if group.as_ref() != last_group {
                        if let Some(g) = group {
                            ui.label(egui::RichText::new(g.to_uppercase()).small().weak());
                        }
                        last_group = group.as_ref();
                    }
                    let mut on = picked.contains(key);
                    if ui.checkbox(&mut on, text).changed() {
                        if on {
                            picked.push(key.clone());
                        } else {
                            picked.retain(|k| k != key);
                        }
                        changed = true;
                    }
                }
            });
            if !picked.is_empty() {
                ui.add_space(4.0);
                if ui.link(t!("Clear")).clicked() {
                    picked.clear();
                    changed = true;
                }
            }
            ui.data_mut(|d| d.insert_temp(search_id, search));
        })
        .response;
    let resp = if picked.is_empty() { resp } else { resp.on_hover_text(all) };
    kit::named(ui, resp);
    if changed {
        // In the choices' order; ones that match no choice after them.
        let index = |k: &String| choices.iter().position(|(_, key, _)| key == k).unwrap_or(usize::MAX);
        picked.sort_by_key(index);
        *value = Value::Array(picked.into_iter().map(Value::String).collect());
    }
    changed
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

    /// A channel field switched off its pin is a box for the channel's id.
    #[test]
    fn id_fields_are_typed_as_an_id() {
        let mut f = FieldDef { type_name: "discord.channel?".into(), ..Default::default() };
        f.ty = PinType::parse(&f.type_name);
        assert!(!is_inline_editable(&f));
        f.by_id = true;
        assert!(is_inline_editable(&f) && !is_plain_text(&f));
        assert_eq!(id_hint(&f), "Channel id");
        f.type_name = "discord.voice_channel".into();
        assert_eq!(id_hint(&f), "Voice channel id");
    }

    /// A multi-select reads a list of keys, and the text older projects typed
    /// ("View Channel, send messages"): by key or label, any case.
    #[test]
    fn multiselect_reads_lists_and_typed_text() {
        let field: FieldDef = serde_json::from_value(serde_json::json!({ "type": "multiselect", "choices": { "ViewChannel": "View Channel", "SendMessages": "Send Messages", "ManageRoles": "Manage Roles" } })).unwrap();
        assert!(field.is_multiselect() && !field.is_select());
        assert_eq!(circuit_core::types::PinType::parse("multiselect"), circuit_core::types::PinType::List(Box::new(circuit_core::types::PinType::Text)), "it takes a list of texts by wire");
        assert_eq!(chosen(&field, &serde_json::json!(["SendMessages"])), ["SendMessages"]);
        assert_eq!(chosen(&field, &serde_json::json!("View Channel, send messages, Mystery")), ["ViewChannel", "SendMessages", "Mystery"], "unknown ones are kept");
        assert!(chosen(&field, &Value::Null).is_empty());
    }

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
