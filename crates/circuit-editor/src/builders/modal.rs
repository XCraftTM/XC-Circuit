//! The `modal` field type: a form (Discord calls it a modal) with up to five
//! questions and texts, and the Modal Builder that designs it.
//!
//! A form is kept as Discord's own JSON, `{ "title", "custom_id",
//! "components": [...] }`, where each question is a label (type 18) around a
//! text input, a select menu or a file upload, and texts are type 10. The
//! builder edits that JSON in place, so settings it has no control for survive
//! (they can be edited as JSON). Older forms that put text inputs in rows are
//! turned into questions when read.

use eframe::egui::{self, Color32, Frame, Margin, RichText, ScrollArea, Stroke, Ui, vec2};
use serde_json::{Value, json};

use crate::builders::json_view::{self, JsonText};
use crate::builders::layout::{card, discord, label_column, line, list_mut, list_of, markdown, number, select_editor, text_area, text_of, ty};
use crate::editor::Editor;
use crate::i18n::n;
use crate::ui::dialog::{self, Part};
use crate::ui::kit;
use crate::ui::theme::Theme;

/// Discord's numbers for the parts of a form.
mod kind {
    pub const ROW: u64 = 1;
    pub const STRING_SELECT: u64 = 3;
    pub const TEXT_INPUT: u64 = 4;
    pub const TEXT: u64 = 10;
    pub const LABEL: u64 = 18;
    pub const FILE_UPLOAD: u64 = 19;
    /// User, role, user-or-role and channel menus.
    pub const PICKERS: [u64; 4] = [5, 6, 7, 8];
}

/// Discord's limits for a form.
const MAX_PARTS: usize = 5;
const MAX_TITLE: usize = 45;
const MAX_QUESTION: usize = 45;
const MAX_HINT: usize = 100;

/// What can be added to a form.
const ADDABLE: [(&str, &str); 4] = [("text-input", n("Question: a box to type into")), ("select", n("Question: a select menu")), ("file", n("Question: a file upload")), ("text", n("Text"))];

const STYLES: [(u64, &str); 2] = [(1, n("One line")), (2, n("Several lines"))];

// ---- The JSON ---------------------------------------------------------------

/// A form's parts: questions and texts.
pub fn parts(form: &Value) -> &[Value] {
    list_of(form, "components")
}

fn is_select(c: &Value) -> bool {
    ty(c) == kind::STRING_SELECT || kind::PICKERS.contains(&ty(c))
}

/// The input of a question: its text input, menu or upload.
fn input_of(c: &Value) -> Option<&Value> {
    (ty(c) == kind::LABEL).then(|| c.get("component")).flatten()
}

/// A form from JSON: Discord's modal (`custom_id`, `title`, `components`), or
/// the list of components alone. Text inputs in rows (the older way) become
/// questions.
pub fn from_json(v: &Value) -> Result<Value, String> {
    let (mut form, list) = match v {
        Value::Array(list) => (json!({}), list.clone()),
        Value::Object(m) => {
            let list = m.get("components").and_then(Value::as_array).cloned().ok_or_else(|| t!("a form needs \"components\": [ … ]").to_string())?;
            (v.clone(), list)
        }
        _ => return Err(t!("paste a modal with \"title\" and \"components\", or a list of components").into()),
    };
    let mut out = Vec::new();
    for c in list {
        if ty(&c) == 0 {
            return Err(tf!("every component needs a \"type\": {json}", json = c.to_string().chars().take(60).collect::<String>()));
        }
        // A row with a text input: the input's label becomes the question.
        if ty(&c) == kind::ROW {
            for mut input in list_of(&c, "components").iter().cloned() {
                let question = input.as_object_mut().and_then(|m| m.remove("label")).unwrap_or(json!(""));
                out.push(json!({ "type": kind::LABEL, "label": question, "component": input }));
            }
            continue;
        }
        out.push(c);
    }
    form["components"] = Value::Array(out);
    for (key, fallback) in [("title", "Tell us more"), ("custom_id", "my-form")] {
        if !form.get(key).is_some_and(Value::is_string) {
            form[key] = json!(fallback);
        }
    }
    Ok(form)
}

/// The ids of the answers, in order, as Modal Submitted gives them.
pub fn answer_ids(form: &Value) -> Vec<String> {
    parts(form).iter().filter_map(input_of).map(|i| text_of(i, "custom_id").to_string()).filter(|id| !id.is_empty()).collect()
}

/// Everything Discord would refuse the form for.
pub fn problems(form: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let title = text_of(form, "title").trim().chars().count();
    if title == 0 || title > MAX_TITLE {
        out.push(tf!("The title needs 1 to {max} characters.", max = MAX_TITLE));
    }
    let id = text_of(form, "custom_id").trim().chars().count();
    if id == 0 || id > 100 {
        out.push(t!("The form needs a custom id of up to 100 characters.").into());
    }
    let list = parts(form);
    if list.len() > MAX_PARTS {
        out.push(tf!("{count} parts: Discord allows {max}.", count = list.len(), max = MAX_PARTS));
    }
    if !list.iter().any(|c| ty(c) == kind::LABEL) {
        out.push(t!("Add at least one question.").into());
    }
    let mut ids: Vec<String> = Vec::new();
    for c in list {
        match ty(c) {
            kind::TEXT if text_of(c, "content").trim().is_empty() => out.push(t!("A text is empty.").into()),
            kind::LABEL => {
                let q = text_of(c, "label").trim().chars().count();
                if q == 0 || q > MAX_QUESTION {
                    out.push(tf!("A question needs 1 to {max} characters.", max = MAX_QUESTION));
                }
                if text_of(c, "description").chars().count() > MAX_HINT {
                    out.push(tf!("A hint has up to {max} characters.", max = MAX_HINT));
                }
                let Some(input) = c.get("component") else {
                    out.push(t!("A question needs a box, a menu or a file upload.").into());
                    continue;
                };
                let id = text_of(input, "custom_id").trim();
                if id.is_empty() || id.chars().count() > 100 {
                    out.push(t!("Every question needs an answer id of up to 100 characters.").into());
                } else if ids.iter().any(|x| x == id) {
                    out.push(tf!("The answer id “{id}” is used twice.", id = id));
                } else {
                    ids.push(id.to_string());
                }
                let num = |k: &str| input.get(k).and_then(Value::as_u64);
                match ty(input) {
                    kind::TEXT_INPUT => {
                        if num("max_length").is_some_and(|m| m == 0 || m > 4000) || num("min_length").is_some_and(|m| m > 4000) {
                            out.push(t!("A box takes up to 4000 characters.").into());
                        }
                        if let (Some(a), Some(b)) = (num("min_length"), num("max_length"))
                            && a > b
                        {
                            out.push(t!("A box's least number of characters is above its most.").into());
                        }
                    }
                    kind::STRING_SELECT => {
                        let options = list_of(input, "options").len();
                        if options == 0 || options > 25 {
                            out.push(t!("A select menu has 1 to 25 options.").into());
                        }
                    }
                    kind::FILE_UPLOAD => {
                        if num("max_values").is_some_and(|m| m == 0 || m > 10) {
                            out.push(t!("A file upload takes 1 to 10 files.").into());
                        }
                    }
                    t if kind::PICKERS.contains(&t) => {}
                    _ => out.push(t!("A question needs a box, a menu or a file upload.").into()),
                }
            }
            _ => {}
        }
    }
    out.dedup();
    out
}

fn kind_name(c: &Value) -> String {
    match (ty(c), input_of(c).map(ty)) {
        (kind::TEXT, _) => t!("Text").into(),
        (kind::LABEL, Some(kind::TEXT_INPUT)) => t!("Question: a box").into(),
        (kind::LABEL, Some(kind::FILE_UPLOAD)) => t!("Question: a file upload").into(),
        (kind::LABEL, Some(t)) if t == kind::STRING_SELECT || kind::PICKERS.contains(&t) => t!("Question: a select menu").into(),
        (kind::LABEL, _) => t!("Question").into(),
        (n, _) => tf!("Component type {number}", number = n),
    }
}

/// A new part of the kind picked in the Add menu. `n` keeps answer ids apart.
fn new_part(what: &str, n: usize) -> Value {
    match what {
        "select" => json!({ "type": kind::LABEL, "label": "Pick one", "component": { "type": kind::STRING_SELECT, "custom_id": format!("choice{n}"), "options": [{ "label": "First", "value": "first" }] } }),
        "file" => json!({ "type": kind::LABEL, "label": "Attach a file", "component": { "type": kind::FILE_UPLOAD, "custom_id": format!("files{n}"), "min_values": 1, "max_values": 1 } }),
        "text" => json!({ "type": kind::TEXT, "content": "Please fill in every field." }),
        _ => json!({ "type": kind::LABEL, "label": "Your answer", "component": { "type": kind::TEXT_INPUT, "custom_id": format!("answer{n}"), "style": 1 } }),
    }
}

// ---- The Modal Builder ------------------------------------------------------

/// An open Modal Builder: which field it edits and the draft.
pub struct ModalEdit {
    pub ws: String,
    pub node: String,
    pub key: String,
    pub draft: Value,
    /// The draft as JSON, while switched to JSON.
    pub json: Option<JsonText>,
}

impl ModalEdit {
    pub fn new(ws: &str, node: &str, key: &str, value: &Value) -> ModalEdit {
        let draft = from_json(value).unwrap_or_else(|_| from_json(&json!([])).unwrap());
        ModalEdit { ws: ws.into(), node: node.into(), key: key.into(), draft, json: None }
    }
}

pub fn open(ed: &mut Editor, ws: &str, node: &str, key: &str) {
    let value = ed.workspace(ws).and_then(|w| w.nodes.get(node)).and_then(|n| n.values.get(key).cloned()).unwrap_or(Value::Null);
    let value = if value.is_null() { field_default(ed, ws, node, key) } else { value };
    ed.modal_edit = Some(ModalEdit::new(ws, node, key, &value));
}

/// The field's default, for a block whose form wasn't changed yet.
fn field_default(ed: &Editor, ws: &str, node: &str, key: &str) -> Value {
    let block = ed.workspace(ws).and_then(|w| w.nodes.get(node)).map(|n| n.block.clone());
    block.and_then(|b| ed.reg.block(&b)).and_then(|d| d.fields.get(key)).and_then(|f| f.default.clone()).unwrap_or(Value::Null)
}

/// The Modal Builder tab.
pub fn tab(ui: &mut Ui, ed: &mut Editor) {
    let Some((ws, node)) = ed.modal_edit.as_ref().map(|e| (e.ws.clone(), e.node.clone())) else { return };
    // Texts offer the values of earlier blocks after `{`, and list them on the right.
    let values = std::sync::Arc::new(ed.values_for(&ws, &node));
    crate::editor::values::set_scope(ui.ctx(), Some(values.clone()));
    let Some(edit) = &mut ed.modal_edit else { return };
    let theme = ed.theme.clone();
    let icon = ed.reg.icons.get("text").cloned();
    let (mut apply, mut close, mut cancel) = (false, false, false);
    dialog::tab(
        ui,
        &theme,
        dialog::Spec {
            id: "modal-builder",
            title: t!("Modal Builder"),
            subtitle: Some(t!("A form that pops up in Discord: up to 5 questions and texts. The answers arrive at Modal Submitted. ${…} in any text holds JavaScript")),
            icon: icon.as_deref(),
            width: 1180.0,
        },
        |ui, part| match part {
            Part::Body if ui.available_width() < 600.0 + 16.0 + 400.0 => {
                kit::section(ui, &theme, t!("Preview"));
                dialog::preview_on_top(ui, "modal-preview", 0.45, |ui| preview(ui, &edit.draft, 440.0));
                form(ui, &theme, edit, Some(&values));
            }
            Part::Body => {
                ui.horizontal_top(|ui| {
                    let h = dialog::remaining_height(ui, 420.0);
                    ui.allocate_ui_with_layout(vec2(600.0, h), egui::Layout::top_down(egui::Align::Min), |ui| form(ui, &theme, edit, None));
                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        kit::section(ui, &theme, t!("Preview"));
                        let h = (dialog::remaining_height(ui, 300.0) * 0.6).max(220.0);
                        ScrollArea::vertical().id_salt("modal-preview").max_height(h).auto_shrink([false, true]).show(ui, |ui| preview(ui, &edit.draft, ui.available_width().min(460.0)));
                        ui.add_space(10.0);
                        kit::section(ui, &theme, t!("Values from earlier blocks"));
                        ScrollArea::vertical().id_salt("modal-values").max_height(dialog::remaining_height(ui, 140.0)).auto_shrink([false, true]).show(ui, |ui| crate::editor::values::list(ui, &theme, &values));
                    });
                });
            }
            Part::Footer => {
                let broken = edit.json.as_ref().is_some_and(|j| j.error.is_some());
                ui.add_enabled_ui(!broken, |ui| {
                    if kit::primary_button(ui, &theme, t!("Apply and close")).clicked() {
                        (apply, close) = (true, true);
                    }
                    apply |= kit::secondary_button(ui, &theme, t!("Apply")).on_hover_text(t!("Put it on the block and keep editing")).clicked();
                });
                cancel = kit::secondary_button(ui, &theme, t!("Cancel")).clicked();
                if broken {
                    ui.label(RichText::new(t!("Fix the JSON first")).size(12.0).color(theme.warning));
                }
            }
        },
    );
    crate::editor::values::set_scope(ui.ctx(), None);
    if apply {
        let edit = ed.modal_edit.as_ref().unwrap();
        let (ws, node, key, value) = (edit.ws.clone(), edit.node.clone(), edit.key.clone(), edit.draft.clone());
        if close {
            ed.modal_edit = None;
        }
        if let Some(n) = ed.project.as_mut().and_then(|p| p.workspaces.get_mut(&ws)).and_then(|w| w.nodes.get_mut(&node)) {
            n.values.insert(key, value);
            ed.edited = true;
        }
    } else if cancel {
        ed.modal_edit = None;
    }
}

/// Builder | JSON, then the title, custom id and parts, or the JSON. `values`
/// (from earlier blocks) are listed at the end, when given.
fn form(ui: &mut Ui, theme: &Theme, edit: &mut ModalEdit, values: Option<&crate::editor::values::Scope>) {
    if let Some(json) = json_view::switch(ui, theme, edit.json.is_some()) {
        if json {
            edit.json = Some(JsonText::new(&edit.draft));
        } else if edit.json.as_ref().is_none_or(|j| j.error.is_none()) {
            edit.json = None;
        }
    }
    ui.add_space(8.0);
    if let Some(jt) = &mut edit.json {
        let help = t!("The form as Discord's JSON: { title, custom_id, components }. Edit it, or paste a modal (from the Discord docs); it applies as soon as it is valid.");
        if let Some(v) = json_view::editor(ui, theme, ui.scope_id().with("modal-json"), jt, help) {
            match from_json(&v) {
                Ok(f) => edit.draft = f,
                Err(e) => jt.error = Some(e),
            }
        }
        return;
    }
    let ids = answer_ids(&edit.draft);
    ui.label(RichText::new(tf!("{count} of {max} parts", count = parts(&edit.draft).len(), max = MAX_PARTS)).size(12.0).color(theme.text_muted));
    for p in problems(&edit.draft).iter().take(4) {
        ui.label(RichText::new(format!("• {p}")).size(12.0).color(theme.warning));
    }
    ui.add_space(6.0);
    // A fixed id, so the boxes keep theirs (and focus) when a problem line comes or goes.
    let form_id = ui.scope_id().with("modal-form-scope");
    ui.scope_builder(egui::UiBuilder::new().scope_id(form_id), |ui| {
        ScrollArea::vertical().id_salt("modal-form").max_height(dialog::remaining_height(ui, 300.0)).auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width() - 12.0);
            Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(9).inner_margin(Margin::same(10)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                line(ui, &mut edit.draft, "title", t!("Up to 45 characters"), t!("Title"));
                line(ui, &mut edit.draft, "custom_id", t!("Modal Submitted matches this"), t!("Custom id"));
            });
            ui.add_space(6.0);
            parts_editor(ui, theme, list_mut(&mut edit.draft, "components"));
            if !ids.is_empty() {
                ui.add_space(8.0);
                answers_hint(ui, theme, &ids);
            }
            if let Some(values) = values {
                ui.add_space(12.0);
                kit::section(ui, theme, t!("Values from earlier blocks"));
                crate::editor::values::list(ui, theme, values);
            }
        });
    });
}

/// The answer ids, for Modal Submitted › Input ids, with a button that copies them.
fn answers_hint(ui: &mut Ui, theme: &Theme, ids: &[String]) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(tf!("Modal Submitted gives the answers as: {ids}", ids = ids.join(", "))).size(12.0).color(theme.text_muted));
        if kit::button(ui, theme, t!("Copy for Input ids")).on_hover_text(t!("One per line, to paste into Modal Submitted › Input ids")).clicked() {
            ui.ctx().copy_text(ids.join("\n"));
        }
    });
}

/// The form's parts, with Up, Down, Remove and an Add menu.
fn parts_editor(ui: &mut Ui, theme: &Theme, list: &mut Vec<Value>) {
    let (mut remove, mut swap) = (None, None);
    let len = list.len();
    for (i, c) in list.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            card(ui, theme, &kind_name(c), i, len, &mut remove, &mut swap, true, |ui| body(ui, theme, c));
        });
    }
    if let Some((a, b)) = swap {
        list.swap(a, b);
    }
    if let Some(i) = remove {
        list.remove(i);
    }
    let full = list.len() >= MAX_PARTS;
    let resp = ui.add_enabled_ui(!full, |ui| kit::button(ui, theme, t!("+ Add to the form"))).inner;
    let resp = if full { resp.on_disabled_hover_text(tf!("A form has up to {max} parts.", max = MAX_PARTS)) } else { resp };
    egui::Popup::menu(&resp).id(ui.scope_id().with("add-menu")).show(|ui| {
        ui.set_min_width(260.0);
        for (what, label) in ADDABLE {
            if kit::menu_item(ui, crate::i18n::t(label), None, true).clicked() {
                let n = list.len() + 1;
                list.push(new_part(what, n));
            }
        }
    });
}

/// The controls for one part.
fn body(ui: &mut Ui, theme: &Theme, c: &mut Value) {
    if ty(c) == kind::TEXT {
        text_area(ui, c, "content", t!("Markdown: # Heading, **bold**, -# small text, [link](https://…)"));
        return;
    }
    if ty(c) != kind::LABEL {
        ui.label(RichText::new(t!("Edit this part as JSON.")).size(12.0).color(theme.text_muted));
        return;
    }
    line(ui, c, "label", t!("Up to 45 characters"), t!("Question"));
    line(ui, c, "description", t!("Optional: smaller text under the question"), t!("Hint"));
    if !c.get("component").is_some_and(Value::is_object) {
        c["component"] = json!({ "type": kind::TEXT_INPUT, "custom_id": "answer", "style": 1 });
    }
    let input = &mut c["component"];
    ui.add_space(4.0);
    match ty(input) {
        kind::TEXT_INPUT => text_input_editor(ui, theme, input),
        kind::FILE_UPLOAD => {
            line(ui, input, "custom_id", t!("The answer's name in Modal Submitted"), t!("Answer id"));
            ui.horizontal(|ui| {
                ui.set_min_height(kit::input_height(ui));
                label_column(ui, egui::Label::new(RichText::new(t!("How many files")).size(12.5)));
                number(ui, input, "min_values", t!("At least"));
                number(ui, input, "max_values", t!("At most"));
            });
            required(ui, theme, input);
        }
        t if t == kind::STRING_SELECT || kind::PICKERS.contains(&t) => {
            select_editor(ui, theme, input, true);
            required(ui, theme, input);
        }
        _ => {
            ui.label(RichText::new(t!("Edit this part as JSON.")).size(12.0).color(theme.text_muted));
        }
    }
}

/// Required, which Discord takes as true when it is left out.
fn required(ui: &mut Ui, theme: &Theme, input: &mut Value) {
    ui.horizontal(|ui| {
        let mut on = input.get("required").and_then(Value::as_bool).unwrap_or(true);
        let name = ui.label(RichText::new(t!("Must be answered")).color(theme.text)).id;
        if kit::toggle(ui, &mut on, theme).labelled_by(name).changed() {
            input["required"] = json!(on);
        }
    });
}

fn text_input_editor(ui: &mut Ui, theme: &Theme, input: &mut Value) {
    line(ui, input, "custom_id", t!("The answer's name in Modal Submitted"), t!("Answer id"));
    let style = input.get("style").and_then(Value::as_u64).unwrap_or(1);
    ui.horizontal(|ui| {
        ui.set_min_height(kit::input_height(ui));
        let name = label_column(ui, egui::Label::new(RichText::new(t!("Box")).size(12.5)));
        let current = STYLES.iter().find(|s| s.0 == style).map(|s| crate::i18n::t(s.1)).unwrap_or(t!("One line"));
        let mut picked = style;
        kit::dropdown(ui, ui.scope_id().with("style"), 180.0, current, |ui| {
            for (s, l) in STYLES {
                ui.selectable_value(&mut picked, s, crate::i18n::t(l));
            }
        })
        .labelled_by(name);
        if picked != style {
            input["style"] = json!(picked);
        }
    });
    line(ui, input, "placeholder", t!("Gray hint text in the empty box"), t!("Placeholder"));
    line(ui, input, "value", t!("Text already in the box"), t!("Starts with"));
    ui.horizontal(|ui| {
        ui.set_min_height(kit::input_height(ui));
        label_column(ui, egui::Label::new(RichText::new(t!("Characters")).size(12.5)));
        number(ui, input, "min_length", t!("At least"));
        number(ui, input, "max_length", t!("At most"));
    });
    required(ui, theme, input);
}

// ---- The preview ------------------------------------------------------------

/// The form as Discord shows it: a dialog with the title, the questions and
/// texts, and Cancel and Submit. `width` is the dialog's.
pub fn preview(ui: &mut Ui, form: &Value, width: f32) {
    Frame::new().fill(discord::CHAT).stroke(Stroke::new(1.0, discord::BORDER)).corner_radius(10).inner_margin(Margin::same(16)).show(ui, |ui| {
        let w = width - 34.0;
        ui.set_width(w);
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal(|ui| {
            let title = match text_of(form, "title") {
                "" => t!("Form").to_string(),
                t => t.to_string(),
            };
            ui.label(RichText::new(title).size(18.0).strong().color(discord::STRONG));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Discord's close cross, drawn: the fonts have no ✕.
                let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), egui::Sense::hover());
                let stroke = Stroke::new(1.6, discord::MUTED);
                ui.painter().line_segment([r.left_top(), r.right_bottom()], stroke);
                ui.painter().line_segment([r.right_top(), r.left_bottom()], stroke);
            });
        });
        ui.add_space(6.0);
        parts_preview(ui, form, w);
        ui.add_space(8.0);
        // A row of its own height: a right-to-left layout alone takes all the
        // height left, and the buttons would stretch to fill it.
        ui.allocate_ui_with_layout(vec2(w, 34.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
            Frame::new().fill(discord::button(1)).corner_radius(4).inner_margin(Margin::symmetric(16, 7)).show(ui, |ui| {
                ui.label(RichText::new(t!("Submit")).size(13.5).color(Color32::WHITE));
            });
            ui.add_space(8.0);
            ui.label(RichText::new(t!("Cancel")).size(13.5).color(discord::TEXT));
        });
    });
}

/// The form's parts as Discord shows them, for the preview inside a block.
pub fn block_preview(ui: &mut Ui, form: &Value, width: f32) {
    let width = width.min(440.0);
    Frame::new().fill(discord::CHAT).corner_radius(6).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_width(width - 20.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.label(RichText::new(text_of(form, "title")).size(14.5).strong().color(discord::STRONG));
        if parts(form).is_empty() {
            ui.label(RichText::new(t!("Empty: add questions in the builder")).size(12.0).color(discord::MUTED));
        }
        parts_preview(ui, form, width - 20.0);
    });
}

fn parts_preview(ui: &mut Ui, form: &Value, width: f32) {
    for c in parts(form) {
        match ty(c) {
            kind::TEXT => markdown(ui, text_of(c, "content"), width),
            kind::LABEL => {
                let input = c.get("component").unwrap_or(&Value::Null);
                let star = if ty(input) != 0 && input.get("required").and_then(Value::as_bool).unwrap_or(true) { " *" } else { "" };
                ui.add_space(2.0);
                ui.label(RichText::new(format!("{}{star}", text_of(c, "label"))).size(14.0).strong().color(discord::STRONG));
                let hint = text_of(c, "description");
                if !hint.is_empty() {
                    ui.label(RichText::new(hint).size(12.5).color(discord::MUTED));
                }
                input_preview(ui, input, width);
            }
            _ => {
                ui.label(RichText::new(kind_name(c)).size(12.0).italics().color(discord::MUTED));
            }
        }
    }
}

fn input_preview(ui: &mut Ui, input: &Value, width: f32) {
    match ty(input) {
        kind::TEXT_INPUT => {
            let tall = input.get("style").and_then(Value::as_u64) == Some(2);
            let (text, color) = match (text_of(input, "value"), text_of(input, "placeholder")) {
                (v, _) if !v.is_empty() => (v, discord::TEXT),
                (_, p) => (p, discord::MUTED),
            };
            Frame::new().fill(discord::DARK).stroke(Stroke::new(1.0, discord::BORDER)).corner_radius(4).inner_margin(Margin::symmetric(10, 9)).show(ui, |ui| {
                ui.set_width(width - 22.0);
                ui.set_min_height(if tall { 64.0 } else { 0.0 });
                ui.label(RichText::new(text).size(13.5).color(color));
            });
        }
        kind::FILE_UPLOAD => {
            let (r, _) = ui.allocate_exact_size(vec2(width, 58.0), egui::Sense::hover());
            ui.painter().rect_filled(r, 6.0, discord::DARK);
            let dash = Stroke::new(1.0, discord::BORDER);
            for (a, b) in [(r.left_top(), r.right_top()), (r.right_top(), r.right_bottom()), (r.right_bottom(), r.left_bottom()), (r.left_bottom(), r.left_top())] {
                ui.painter().add(egui::Shape::dashed_line(&[a, b], dash, 5.0, 4.0));
            }
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, t!("Drag files here, or click to choose"), egui::FontId::proportional(12.5), discord::MUTED);
        }
        _ if is_select(input) => {
            let placeholder = match text_of(input, "placeholder") {
                "" => t!("Make a selection").to_string(),
                p => p.to_string(),
            };
            Frame::new().fill(discord::DARK).stroke(Stroke::new(1.0, discord::BORDER)).corner_radius(4).inner_margin(Margin::symmetric(10, 9)).show(ui, |ui| {
                ui.set_width(width - 22.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(placeholder).size(13.5).color(discord::MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(10.0, 6.0), egui::Sense::hover());
                        ui.painter().add(egui::Shape::convex_polygon(vec![r.left_top(), r.right_top(), r.center_bottom()], discord::TEXT, Stroke::NONE));
                    });
                });
            });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms_are_read_and_checked() {
        let modal = json!({ "title": "Apply", "custom_id": "apply:${user.id}", "components": [
            { "type": 10, "content": "## Staff application" },
            { "type": 18, "label": "Your age", "description": "In years", "component": { "type": 4, "custom_id": "age", "style": 1, "max_length": 3 } },
            { "type": 18, "label": "Why us?", "component": { "type": 4, "custom_id": "why", "style": 2 } },
            { "type": 18, "label": "Team", "component": { "type": 3, "custom_id": "team", "options": [{ "label": "Mods", "value": "mods" }] } },
            { "type": 18, "label": "Proof", "component": { "type": 19, "custom_id": "proof", "min_values": 1, "max_values": 2 } }
        ] });
        let form = from_json(&modal).unwrap();
        assert_eq!(form, modal, "nothing is lost on the way");
        assert!(problems(&form).is_empty(), "{:?}", problems(&form));
        assert_eq!(answer_ids(&form), ["age", "why", "team", "proof"]);
    }

    #[test]
    fn text_inputs_in_rows_become_questions() {
        let old = json!({ "title": "T", "custom_id": "f", "components": [{ "type": 1, "components": [{ "type": 4, "custom_id": "name", "label": "Name", "style": 1 }] }] });
        let form = from_json(&old).unwrap();
        assert_eq!(parts(&form)[0], json!({ "type": 18, "label": "Name", "component": { "type": 4, "custom_id": "name", "style": 1 } }));
        let bare = from_json(&json!([{ "type": 10, "content": "x" }])).unwrap();
        assert_eq!((text_of(&bare, "title"), text_of(&bare, "custom_id")), ("Tell us more", "my-form"), "a list alone gets a title and id");
        assert!(from_json(&json!([{ "content": "no type" }])).is_err());
        assert!(from_json(&json!("text")).is_err());
    }

    #[test]
    fn problems_name_what_discord_refuses() {
        let form = json!({ "title": "", "custom_id": "f", "components": [
            { "type": 18, "label": "A", "component": { "type": 4, "custom_id": "same" } },
            { "type": 18, "label": "B", "component": { "type": 4, "custom_id": "same", "min_length": 10, "max_length": 5 } },
            { "type": 18, "label": "C", "component": { "type": 3, "custom_id": "c", "options": [] } },
            { "type": 10, "content": " " },
            { "type": 18, "label": "D", "component": { "type": 19, "custom_id": "d", "max_values": 11 } },
            { "type": 18, "label": "E", "component": { "type": 4, "custom_id": "e" } }
        ] });
        let p = problems(&form).join("\n");
        for want in ["title", "used twice", "least number", "1 to 25", "text is empty", "1 to 10 files", "Discord allows 5"] {
            assert!(p.contains(want), "{want}: {p}");
        }
        assert!(problems(&json!({ "title": "T", "custom_id": "f", "components": [{ "type": 10, "content": "x" }] })).iter().any(|p| p.contains("at least one question")));
    }
}
