//! The `layout` field type: a message built from Discord's Components V2
//! (containers, text, sections, images, files, separators, buttons and select
//! menus), and the Layout Builder that designs it.
//!
//! A layout is kept as Discord's own JSON, `{ "components": [...] }`, and the
//! builder edits that JSON in place: anything Discord supports survives, even
//! settings the builder has no control for (they can be edited as JSON).

use std::path::Path;

use eframe::egui::{self, Color32, CornerRadius, FontId, Frame, Margin, RichText, ScrollArea, Stroke, Ui, vec2};
use serde_json::{Value, json};

use crate::ui::dialog::{self, Part};
use crate::editor::Editor;
use crate::i18n::n;
use crate::builders::json_view::{self, JsonText};
use crate::ui::theme::Theme;

/// Discord's numbers for component types.
mod kind {
    pub const ROW: u64 = 1;
    pub const BUTTON: u64 = 2;
    pub const STRING_SELECT: u64 = 3;
    pub const SECTION: u64 = 9;
    pub const TEXT: u64 = 10;
    pub const THUMBNAIL: u64 = 11;
    pub const GALLERY: u64 = 12;
    pub const FILE: u64 = 13;
    pub const SEPARATOR: u64 = 14;
    pub const CONTAINER: u64 = 17;
    /// User, role, user-or-role and channel menus.
    pub const PICKERS: [u64; 4] = [5, 6, 7, 8];
}

/// The message flag that switches a message to Components V2.
pub const IS_COMPONENTS_V2: u64 = 1 << 15;
/// Discord's limits for one message.
const MAX_COMPONENTS: usize = 40;
const MAX_TEXT: usize = 4000;

/// What can be added to a message or a container.
const ADDABLE: [(&str, &str); 8] = [
    ("text", n("Text")),
    ("section", n("Section: text with a picture or button")),
    ("container", n("Container")),
    ("gallery", n("Media gallery")),
    ("file", n("File")),
    ("separator", n("Separator")),
    ("buttons", n("Row of buttons")),
    ("select", n("Select menu")),
];

const BUTTON_STYLES: [(u64, &str); 6] = [(1, n("Blurple")), (2, n("Grey")), (3, n("Green")), (4, n("Red")), (5, n("Link (opens a web page)")), (6, n("Premium (buys a SKU)"))];

const SELECT_KINDS: [(u64, &str); 5] = [(3, n("Options you list")), (5, n("Users")), (6, n("Roles")), (7, n("Users and roles")), (8, n("Channels"))];

/// The channel kinds a channel menu can offer, with Discord's numbers.
const CHANNEL_TYPES: [(u64, &str); 8] = [(0, n("Text")), (2, n("Voice")), (4, n("Category")), (5, n("Announcement")), (11, n("Public thread")), (12, n("Private thread")), (13, n("Stage")), (15, n("Forum"))];

// ---- The JSON ---------------------------------------------------------------

fn ty(v: &Value) -> u64 {
    v.get("type").and_then(Value::as_u64).unwrap_or(0)
}

fn text_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn list_of<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default()
}

/// The list under `key`, made if missing.
fn list_mut<'a>(v: &'a mut Value, key: &str) -> &'a mut Vec<Value> {
    if !v.get(key).is_some_and(Value::is_array) {
        v[key] = json!([]);
    }
    v[key].as_array_mut().unwrap()
}

/// The object under `key` (such as `media`), made if missing.
fn object_mut<'a>(v: &'a mut Value, key: &str) -> &'a mut Value {
    if !v.get(key).is_some_and(Value::is_object) {
        v[key] = json!({});
    }
    &mut v[key]
}

/// A layout's top-level components.
pub fn components(layout: &Value) -> &[Value] {
    list_of(layout, "components")
}

fn is_select(c: &Value) -> bool {
    ty(c) == kind::STRING_SELECT || kind::PICKERS.contains(&ty(c))
}

/// The layout as the JSON of a whole message, as Discord's docs show it.
pub fn to_json(layout: &Value) -> Value {
    json!({ "flags": IS_COMPONENTS_V2, "components": components(layout) })
}

/// A layout from pasted JSON: a message with `components`, or the list alone.
pub fn from_json(v: &Value) -> Result<Value, String> {
    let list = match v {
        Value::Array(list) => list.clone(),
        Value::Object(m) => {
            let has = |k: &str| m.get(k).is_some_and(|x| x.as_str().is_some_and(|s| !s.is_empty()) || x.as_array().is_some_and(|a| !a.is_empty()));
            if has("content") || has("embeds") {
                return Err(t!("Components V2 messages have no content or embeds: put text in Text components").into());
            }
            m.get("components").and_then(Value::as_array).cloned().ok_or_else(|| t!("a layout needs \"components\": [ … ]").to_string())?
        }
        _ => return Err(t!("paste a message with \"components\", or a list of components").into()),
    };
    if let Some(bad) = list.iter().find(|c| ty(c) == 0) {
        return Err(tf!("every component needs a \"type\": {json}", json = bad.to_string().chars().take(60).collect::<String>()));
    }
    Ok(json!({ "components": list }))
}

/// How many components Discord counts: every one, nested ones included.
fn count(list: &[Value]) -> usize {
    list.iter().map(|c| 1 + count(list_of(c, "components")) + usize::from(c.get("accessory").is_some())).sum()
}

/// The length of all text in Text components.
fn text_len(list: &[Value]) -> usize {
    list.iter().map(|c| text_of(c, "content").chars().count() + text_len(list_of(c, "components"))).sum()
}

/// Everything Discord would refuse the message for.
pub fn problems(layout: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let list = components(layout);
    if list.is_empty() {
        out.push(t!("Add at least one component.").to_string());
    }
    let n = count(list);
    if n > MAX_COMPONENTS {
        out.push(tf!("{count} components: Discord allows {max}.", count = n, max = MAX_COMPONENTS));
    }
    let chars = text_len(list);
    if chars > MAX_TEXT {
        out.push(tf!("{count} characters of text: Discord allows {max}.", count = chars, max = MAX_TEXT));
    }
    let mut ids: Vec<String> = Vec::new();
    check_list(list, &mut out, &mut ids);
    out.dedup();
    out
}

fn check_list(list: &[Value], out: &mut Vec<String>, ids: &mut Vec<String>) {
    for c in list {
        let children = list_of(c, "components");
        match ty(c) {
            kind::SECTION => {
                if children.is_empty() || children.len() > 3 {
                    out.push(t!("A section has 1 to 3 texts.").into());
                }
                if c.get("accessory").is_none() {
                    out.push(t!("A section needs a picture or a button next to its text.").into());
                }
            }
            kind::GALLERY => {
                let items = list_of(c, "items").len();
                if items == 0 || items > 10 {
                    out.push(t!("A media gallery has 1 to 10 pictures.").into());
                }
            }
            kind::ROW => {
                let selects = children.iter().filter(|x| is_select(x)).count();
                if children.is_empty() || children.len() > 5 || (selects > 0 && children.len() > 1) {
                    out.push(t!("A row has 1 to 5 buttons, or one select menu.").into());
                }
            }
            kind::TEXT if text_of(c, "content").trim().is_empty() => out.push(t!("A text is empty.").into()),
            _ => {}
        }
        if ty(c) == kind::BUTTON || is_select(c) {
            check_interactive(c, out, ids);
        }
        if let Some(a) = c.get("accessory")
            && ty(a) == kind::BUTTON
        {
            check_interactive(a, out, ids);
        }
        check_list(children, out, ids);
    }
}

fn check_interactive(c: &Value, out: &mut Vec<String>, ids: &mut Vec<String>) {
    let style = c.get("style").and_then(Value::as_u64).unwrap_or(1);
    if ty(c) == kind::BUTTON && style == 5 {
        if text_of(c, "url").trim().is_empty() {
            out.push(t!("A link button needs a web address.").into());
        }
    } else if ty(c) == kind::BUTTON && style == 6 {
        if text_of(c, "sku_id").trim().is_empty() {
            out.push(t!("A premium button needs a SKU id.").into());
        }
    } else {
        let id = text_of(c, "custom_id").trim();
        if id.is_empty() || id.chars().count() > 100 {
            out.push(t!("Buttons and menus need a custom id of up to 100 characters.").into());
        } else if ids.iter().any(|x| x == id) {
            out.push(tf!("The custom id “{id}” is used twice.", id = id));
        } else {
            ids.push(id.to_string());
        }
    }
    if ty(c) == kind::BUTTON && text_of(c, "label").is_empty() && c.get("emoji").is_none() {
        out.push(t!("A button needs a label or an emoji.").into());
    }
    if ty(c) == kind::STRING_SELECT {
        let options = list_of(c, "options").len();
        if options == 0 || options > 25 {
            out.push(t!("A select menu has 1 to 25 options.").into());
        }
    }
}

fn kind_name(c: &Value) -> String {
    match ty(c) {
        kind::ROW if list_of(c, "components").first().is_some_and(is_select) => t!("Select menu").into(),
        kind::ROW => t!("Row of buttons").into(),
        kind::SECTION => t!("Section").into(),
        kind::TEXT => t!("Text").into(),
        kind::GALLERY => t!("Media gallery").into(),
        kind::FILE => t!("File").into(),
        kind::SEPARATOR => t!("Separator").into(),
        kind::CONTAINER => t!("Container").into(),
        kind::BUTTON => t!("Button").into(),
        n => tf!("Component type {number}", number = n),
    }
}

fn short(text: &str, max: usize) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or_default().trim_start_matches(['#', '-', '>', ' ']);
    if line.chars().count() > max { format!("{}…", line.chars().take(max).collect::<String>()) } else { line.to_string() }
}

/// One line per top-level component, for the block on the canvas.
pub fn summary(layout: &Value) -> Vec<String> {
    components(layout)
        .iter()
        .map(|c| {
            let children = list_of(c, "components");
            match ty(c) {
                kind::TEXT => format!("{}: {}", kind_name(c), short(text_of(c, "content"), 26)),
                kind::SECTION => format!("{}: {}", kind_name(c), short(children.first().map(|t| text_of(t, "content")).unwrap_or_default(), 24)),
                kind::CONTAINER => tf!("Container · {count} parts", count = children.len()),
                kind::GALLERY => tf!("Media gallery · {count} pictures", count = list_of(c, "items").len()),
                kind::ROW if !children.first().is_some_and(is_select) => {
                    format!("{}: {}", kind_name(c), children.iter().map(|b| text_of(b, "label")).collect::<Vec<_>>().join(", "))
                }
                _ => kind_name(c),
            }
        })
        .collect()
}

/// A new component of the kind picked in an Add menu. `n` keeps custom ids apart.
fn new_component(what: &str, n: usize) -> Value {
    match what {
        "text" => json!({ "type": kind::TEXT, "content": "Some **text**" }),
        "section" => json!({ "type": kind::SECTION, "components": [{ "type": kind::TEXT, "content": "Text next to a picture" }], "accessory": { "type": kind::THUMBNAIL, "media": { "url": "${avatar}" } } }),
        "container" => json!({ "type": kind::CONTAINER, "accent_color": 0x00D8F0, "components": [{ "type": kind::TEXT, "content": "## A container" }] }),
        "gallery" => json!({ "type": kind::GALLERY, "items": [{ "media": { "url": "https://" } }] }),
        "file" => json!({ "type": kind::FILE, "file": { "url": "attachment://file.txt" } }),
        "separator" => json!({ "type": kind::SEPARATOR, "divider": true, "spacing": 1 }),
        "buttons" => json!({ "type": kind::ROW, "components": [new_button(n)] }),
        _ => json!({ "type": kind::ROW, "components": [{ "type": kind::STRING_SELECT, "custom_id": format!("menu-{n}"), "placeholder": "Pick one", "options": [{ "label": "First", "value": "first" }] }] }),
    }
}

fn new_button(n: usize) -> Value {
    json!({ "type": kind::BUTTON, "style": 1, "label": "Button", "custom_id": format!("button-{n}") })
}

/// An emoji as typed: 👍, or <:name:id> (<a:name:id> when animated).
fn emoji_text(v: Option<&Value>) -> String {
    match v {
        Some(e) if e.get("id").is_some_and(|i| !i.is_null()) => {
            let a = if e.get("animated").and_then(Value::as_bool).unwrap_or(false) { "a" } else { "" };
            format!("<{a}:{}:{}>", text_of(e, "name"), e["id"].as_str().map(str::to_string).unwrap_or_else(|| e["id"].to_string()))
        }
        Some(e) => text_of(e, "name").to_string(),
        None => String::new(),
    }
}

fn parse_emoji(text: &str) -> Option<Value> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(inner) = t.strip_prefix('<').and_then(|t| t.strip_suffix('>')) {
        let parts: Vec<&str> = inner.split(':').collect();
        if parts.len() == 3 {
            return Some(json!({ "id": parts[2], "name": parts[1], "animated": parts[0] == "a" }));
        }
    }
    Some(json!({ "name": t }))
}

// ---- The Layout Builder -----------------------------------------------------

/// An open Layout Builder: which field it edits and the draft.
pub struct LayoutEdit {
    pub ws: String,
    pub node: String,
    pub key: String,
    pub draft: Value,
    /// The draft as JSON, while switched to JSON.
    pub json: Option<JsonText>,
}

impl LayoutEdit {
    pub fn new(ws: &str, node: &str, key: &str, value: &Value) -> LayoutEdit {
        let draft = from_json(value).unwrap_or_else(|_| json!({ "components": [] }));
        LayoutEdit { ws: ws.into(), node: node.into(), key: key.into(), draft, json: None }
    }
}

pub fn open(ed: &mut Editor, ws: &str, node: &str, key: &str) {
    let value = ed.workspace(ws).and_then(|w| w.nodes.get(node)).and_then(|n| n.values.get(key).cloned()).unwrap_or(Value::Null);
    ed.layout_edit = Some(LayoutEdit::new(ws, node, key, &value));
}

/// The Layout Builder tab.
pub fn tab(ui: &mut Ui, ed: &mut Editor) {
    let Some((ws, node)) = ed.layout_edit.as_ref().map(|e| (e.ws.clone(), e.node.clone())) else { return };
    // Texts offer the values of earlier blocks after `{`, and list them on the right.
    let values = std::sync::Arc::new(ed.values_for(&ws, &node));
    crate::editor::values::set_scope(ui.ctx(), Some(values.clone()));
    let bot = ed.bot_profile.shown.clone();
    let Some(edit) = &mut ed.layout_edit else { return };
    let theme = ed.theme.clone();
    let icon = ed.reg.icons.get("layers").cloned();
    let (mut apply, mut close, mut cancel) = (false, false, false);
    dialog::tab(
        ui,
        &theme,
        dialog::Spec {
            id: "layout-builder",
            title: t!("Layout Builder"),
            subtitle: Some(t!("Components V2: the whole message is built from parts. ${…} in any text holds JavaScript: type ${ for values from earlier blocks")),
            icon: icon.as_deref(),
            width: 1220.0,
        },
        |ui, part| match part {
            Part::Body => {
                ui.horizontal_top(|ui| {
                    let h = dialog::remaining_height(ui, 420.0);
                    ui.allocate_ui_with_layout(vec2(620.0, h), egui::Layout::top_down(egui::Align::Min), |ui| form(ui, &theme, edit));
                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        dialog::section(ui, &theme, t!("Preview"));
                        let h = (dialog::remaining_height(ui, 300.0) * 0.55).max(200.0);
                        ScrollArea::vertical().id_salt("layout-preview").max_height(h).auto_shrink([false, true]).show(ui, |ui| preview(ui, &theme, &edit.draft, bot.as_ref()));
                        ui.add_space(10.0);
                        dialog::section(ui, &theme, t!("Values from earlier blocks"));
                        ScrollArea::vertical().id_salt("layout-values").max_height(dialog::remaining_height(ui, 140.0)).auto_shrink([false, true]).show(ui, |ui| crate::editor::values::list(ui, &theme, &values));
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
    crate::editor::values::set_scope(ui.ctx(), None);
    if apply {
        let edit = ed.layout_edit.as_ref().unwrap();
        let (ws, node, key, value) = (edit.ws.clone(), edit.node.clone(), edit.key.clone(), edit.draft.clone());
        if close {
            ed.layout_edit = None;
        }
        if let Some(n) = ed.project.as_mut().and_then(|p| p.workspaces.get_mut(&ws)).and_then(|w| w.nodes.get_mut(&node)) {
            n.values.insert(key, value);
            ed.edited = true;
        }
    } else if cancel {
        ed.layout_edit = None;
    }
}

fn form(ui: &mut Ui, theme: &Theme, edit: &mut LayoutEdit) {
    if let Some(json) = json_view::switch(ui, theme, edit.json.is_some()) {
        if json {
            edit.json = Some(JsonText::new(&to_json(&edit.draft)));
        } else if edit.json.as_ref().is_none_or(|j| j.error.is_none()) {
            edit.json = None;
        }
    }
    ui.add_space(8.0);
    if let Some(jt) = &mut edit.json {
        let help = t!("The message as Discord's JSON. Edit it, or paste a Components V2 message or a list of components (from the Discord docs or a message designer); it applies as soon as it is valid.");
        if let Some(v) = json_view::editor(ui, theme, ui.id().with("layout-json"), jt, help) {
            match from_json(&v) {
                Ok(l) => edit.draft = l,
                Err(e) => jt.error = Some(e),
            }
        }
        return;
    }
    let list = components(&edit.draft);
    ui.label(
        RichText::new(tf!("{count} of {max} components · {chars} of {max_chars} characters of text", count = count(list), max = MAX_COMPONENTS, chars = text_len(list), max_chars = MAX_TEXT))
            .size(12.0)
            .color(theme.text_muted),
    );
    for p in problems(&edit.draft).iter().take(4) {
        ui.label(RichText::new(format!("• {p}")).size(12.0).color(theme.warning));
    }
    ui.add_space(6.0);
    // A fixed id, so the boxes in the form keep theirs (and the one being
    // typed in keeps focus) when a problem line above appears or goes.
    let form_id = ui.id().with("layout-form-scope");
    ui.scope_builder(egui::UiBuilder::new().id(form_id), |ui| {
        ScrollArea::vertical().id_salt("layout-form").max_height(dialog::remaining_height(ui, 300.0)).auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width() - 12.0);
            let next = count(components(&edit.draft)) + 1;
            list_editor(ui, theme, list_mut(&mut edit.draft, "components"), true, next);
        });
    });
}

/// The components of a message (`top`) or a container, with Up, Down, Remove and an Add menu.
fn list_editor(ui: &mut Ui, theme: &Theme, list: &mut Vec<Value>, top: bool, next: usize) {
    let (mut remove, mut swap) = (None, None);
    let len = list.len();
    for (i, c) in list.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            card(ui, theme, &kind_name(c), i, len, &mut remove, &mut swap, top, |ui| body(ui, theme, c, next));
        });
    }
    if let Some((a, b)) = swap {
        list.swap(a, b);
    }
    if let Some(i) = remove {
        list.remove(i);
    }
    let resp = dialog::button(ui, theme, if top { t!("+ Add to the message") } else { t!("+ Add to the container") });
    egui::Popup::menu(&resp).id(ui.id().with("add-menu")).show(|ui| {
        ui.set_min_width(260.0);
        for (what, label) in ADDABLE {
            if what == "container" && !top {
                continue;
            }
            if ui.button(crate::i18n::t(label)).clicked() {
                list.push(new_component(what, next));
            }
        }
    });
}

/// A framed card with a title and Up, Down and Remove buttons.
#[allow(clippy::too_many_arguments)]
fn card(ui: &mut Ui, theme: &Theme, title: &str, i: usize, len: usize, remove: &mut Option<usize>, swap: &mut Option<(usize, usize)>, top: bool, add: impl FnOnce(&mut Ui)) {
    let fill = if top { theme.panel_header } else { theme.panel };
    Frame::new().fill(fill).stroke(Stroke::new(1.0, theme.line)).corner_radius(9).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.set_min_height(crate::ui::inputs::input_height(ui));
            ui.label(RichText::new(title).strong().color(theme.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if dialog::danger_button(ui, theme, t!("Remove")).clicked() {
                    *remove = Some(i);
                }
                if ui.add_enabled_ui(i + 1 < len, |ui| dialog::button(ui, theme, t!("Down"))).inner.clicked() {
                    *swap = Some((i, i + 1));
                }
                if ui.add_enabled_ui(i > 0, |ui| dialog::button(ui, theme, t!("Up"))).inner.clicked() {
                    *swap = Some((i, i - 1));
                }
            });
        });
        ui.add_space(4.0);
        add(ui);
    });
    ui.add_space(6.0);
}

/// The controls for one component.
fn body(ui: &mut Ui, theme: &Theme, c: &mut Value, next: usize) {
    match ty(c) {
        kind::TEXT => text_area(ui, c, "content", t!("Markdown: # Heading, **bold**, -# small text, [link](https://…)")),
        kind::SECTION => {
            let texts = list_mut(c, "components");
            let mut remove = None;
            let many = texts.len() > 1;
            for (i, t) in texts.iter_mut().enumerate() {
                ui.push_id(("text", i), |ui| {
                    ui.horizontal_top(|ui| {
                        let w = ui.available_width() - if many { 46.0 } else { 0.0 };
                        ui.allocate_ui(vec2(w, 10.0), |ui| text_area(ui, t, "content", t!("Text")));
                        if many && dialog::danger_button(ui, theme, "×").on_hover_text(t!("Remove this text")).clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = remove {
                texts.remove(i);
            }
            if texts.len() < 3 && dialog::button(ui, theme, t!("+ Add text")).clicked() {
                texts.push(json!({ "type": kind::TEXT, "content": "More text" }));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.set_min_height(crate::ui::inputs::input_height(ui));
                ui.label(RichText::new(t!("Next to the text")).color(theme.text));
                let is_button = c.get("accessory").is_some_and(|a| ty(a) == kind::BUTTON);
                let current = if is_button { t!("A button") } else { t!("A picture") };
                crate::ui::widgets::dropdown(ui, ui.id().with("accessory"), 140.0, current, |ui| {
                    if ui.selectable_label(!is_button, t!("A picture")).clicked() && is_button {
                        c["accessory"] = json!({ "type": kind::THUMBNAIL, "media": { "url": "${avatar}" } });
                    }
                    if ui.selectable_label(is_button, t!("A button")).clicked() && !is_button {
                        c["accessory"] = new_button(next);
                    }
                });
            });
            let accessory = object_mut(c, "accessory");
            if ty(accessory) == kind::BUTTON {
                button_editor(ui, theme, accessory);
            } else {
                if ty(accessory) == 0 {
                    accessory["type"] = json!(kind::THUMBNAIL);
                }
                media_editor(ui, theme, accessory, true);
            }
        }
        kind::CONTAINER => {
            ui.horizontal(|ui| {
                ui.set_min_height(crate::ui::inputs::input_height(ui));
                let mut on = c.get("accent_color").is_some_and(|v| !v.is_null());
                ui.label(RichText::new(t!("Accent color")).color(theme.text));
                if crate::ui::widgets::toggle(ui, &mut on, theme).changed() {
                    if on {
                        c["accent_color"] = json!(0x00D8F0);
                    } else if let Some(m) = c.as_object_mut() {
                        m.remove("accent_color");
                    }
                }
                if let Some(n) = c.get("accent_color").and_then(Value::as_u64) {
                    let mut color = Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8);
                    if crate::builders::color::color_button(ui, ui.id().with("accent"), &mut color, theme, None) {
                        c["accent_color"] = json!(((color.r() as u32) << 16) | ((color.g() as u32) << 8) | color.b() as u32);
                    }
                }
                ui.add_space(12.0);
                flag(ui, theme, c, "spoiler", t!("Spoiler"));
            });
            ui.add_space(4.0);
            list_editor(ui, theme, list_mut(c, "components"), false, next);
        }
        kind::GALLERY => {
            let items = list_mut(c, "items");
            let mut remove = None;
            for (i, item) in items.iter_mut().enumerate() {
                ui.push_id(("item", i), |ui| {
                    Frame::new().stroke(Stroke::new(1.0, theme.line)).corner_radius(7).inner_margin(8).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        media_editor(ui, theme, item, true);
                        if dialog::danger_button(ui, theme, t!("Remove picture")).clicked() {
                            remove = Some(i);
                        }
                    });
                });
                ui.add_space(4.0);
            }
            if let Some(i) = remove {
                items.remove(i);
            }
            if items.len() < 10 && dialog::button(ui, theme, t!("+ Add picture")).clicked() {
                items.push(json!({ "media": { "url": "https://" } }));
            }
        }
        kind::FILE => {
            let file = object_mut(c, "file");
            line(ui, file, "url", t!("attachment://report.pdf, a web address or a file path"), t!("File"));
            ui.horizontal(|ui| flag(ui, theme, c, "spoiler", t!("Spoiler")));
            ui.label(RichText::new(t!("Web addresses and file paths are attached when the message is sent.")).size(11.5).color(theme.text_muted));
        }
        kind::SEPARATOR => {
            ui.horizontal(|ui| {
                ui.set_min_height(crate::ui::inputs::input_height(ui));
                let mut divider = c.get("divider").and_then(Value::as_bool).unwrap_or(true);
                ui.label(RichText::new(t!("Line")).color(theme.text));
                if crate::ui::widgets::toggle(ui, &mut divider, theme).changed() {
                    c["divider"] = json!(divider);
                }
                ui.add_space(12.0);
                ui.label(RichText::new(t!("Space")).color(theme.text));
                let mut spacing = c.get("spacing").and_then(Value::as_u64).unwrap_or(1);
                let labels = [t!("Small"), t!("Large")];
                crate::ui::widgets::dropdown(ui, ui.id().with("spacing"), 110.0, labels[(spacing == 2) as usize], |ui| {
                    ui.selectable_value(&mut spacing, 1, labels[0]);
                    ui.selectable_value(&mut spacing, 2, labels[1]);
                });
                c["spacing"] = json!(spacing);
            });
        }
        kind::ROW => {
            let children = list_mut(c, "components");
            if children.first().is_some_and(is_select) {
                select_editor(ui, theme, &mut children[0]);
                return;
            }
            let mut remove = None;
            for (i, b) in children.iter_mut().enumerate() {
                ui.push_id(("button", i), |ui| {
                    Frame::new().stroke(Stroke::new(1.0, theme.line)).corner_radius(7).inner_margin(8).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        button_editor(ui, theme, b);
                        if dialog::danger_button(ui, theme, t!("Remove button")).clicked() {
                            remove = Some(i);
                        }
                    });
                });
                ui.add_space(4.0);
            }
            if let Some(i) = remove {
                children.remove(i);
            }
            if children.len() < 5 && dialog::button(ui, theme, t!("+ Add button")).clicked() {
                children.push(new_button(next + children.len()));
            }
        }
        _ => {
            ui.label(RichText::new(t!("The builder has no controls for this one: edit it in JSON.")).size(12.0).color(theme.text_muted));
        }
    }
}

/// The name in front of a row's control, in a column of its own.
fn label_column(ui: &mut Ui, label: egui::Label) {
    let h = crate::ui::inputs::input_height(ui);
    ui.allocate_ui_with_layout(vec2(110.0, h), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_width(110.0);
        ui.add(label.truncate());
    });
}

/// A text box for a string under `key`; an empty one removes the key.
fn line(ui: &mut Ui, v: &mut Value, key: &str, hint: &str, label: &str) {
    ui.horizontal(|ui| {
        ui.set_min_height(crate::ui::inputs::input_height(ui));
        label_column(ui, egui::Label::new(RichText::new(label).size(12.5)));
        let mut text = text_of(v, key).to_string();
        let w = ui.available_width();
        if ui.add(crate::ui::inputs::TextLine::new(&mut text).hint(hint).width(w)).changed() {
            set_text(v, key, text);
        }
    });
}

fn set_text(v: &mut Value, key: &str, text: String) {
    if text.is_empty() {
        if let Some(m) = v.as_object_mut() {
            m.remove(key);
        }
    } else {
        v[key] = json!(text);
    }
}

fn text_area(ui: &mut Ui, v: &mut Value, key: &str, hint: &str) {
    let mut text = text_of(v, key).to_string();
    if ui.add(crate::ui::inputs::TextArea::new(ui.id().with(key), &mut text).hint(hint).rows(3)).changed() {
        v[key] = json!(text);
    }
}

/// A switch for a yes/no setting that is left out when off.
fn flag(ui: &mut Ui, theme: &Theme, v: &mut Value, key: &str, label: &str) {
    let mut on = v.get(key).and_then(Value::as_bool).unwrap_or(false);
    ui.label(RichText::new(label).color(theme.text));
    if crate::ui::widgets::toggle(ui, &mut on, theme).changed() {
        if on {
            v[key] = json!(true);
        } else if let Some(m) = v.as_object_mut() {
            m.remove(key);
        }
    }
}

/// A picture: its address, a description for screen readers and Spoiler.
fn media_editor(ui: &mut Ui, theme: &Theme, v: &mut Value, spoiler: bool) {
    let media = object_mut(v, "media");
    line(ui, media, "url", t!("https://…, ${avatar} or a file path"), t!("Picture"));
    line(ui, v, "description", t!("What it shows, for screen readers"), t!("Description"));
    if spoiler {
        ui.horizontal(|ui| flag(ui, theme, v, "spoiler", t!("Spoiler")));
    }
}

fn button_editor(ui: &mut Ui, theme: &Theme, b: &mut Value) {
    let style = b.get("style").and_then(Value::as_u64).unwrap_or(1);
    ui.horizontal(|ui| {
        ui.set_min_height(crate::ui::inputs::input_height(ui));
        label_column(ui, egui::Label::new(RichText::new(t!("Style")).size(12.5)));
        let current = BUTTON_STYLES.iter().find(|s| s.0 == style).map(|s| crate::i18n::t(s.1)).unwrap_or(t!("Blurple"));
        let mut picked = style;
        crate::ui::widgets::dropdown(ui, ui.id().with("style"), 210.0, current, |ui| {
            for (s, l) in BUTTON_STYLES {
                ui.selectable_value(&mut picked, s, crate::i18n::t(l));
            }
        });
        if picked != style {
            b["style"] = json!(picked);
            let m = b.as_object_mut().unwrap();
            // Each kind of button has its own target: an id, a web page or a SKU.
            match picked {
                5 => {
                    m.remove("custom_id");
                    m.remove("sku_id");
                    m.entry("url").or_insert(json!("https://"));
                }
                6 => {
                    m.remove("custom_id");
                    m.remove("url");
                    m.remove("label");
                    m.entry("sku_id").or_insert(json!(""));
                }
                _ => {
                    m.remove("url");
                    m.remove("sku_id");
                    m.entry("custom_id").or_insert(json!("button"));
                }
            }
        }
        ui.add_space(8.0);
        flag(ui, theme, b, "disabled", t!("Disabled"));
    });
    if picked_style(b) != 6 {
        line(ui, b, "label", t!("Up to 80 characters"), t!("Label"));
        emoji_line(ui, b);
    }
    match picked_style(b) {
        5 => line(ui, b, "url", "https://…", t!("Web address")),
        6 => line(ui, b, "sku_id", "1234567890", t!("SKU id")),
        _ => line(ui, b, "custom_id", t!("Button Pressed matches this"), t!("Custom id")),
    }
}

fn picked_style(b: &Value) -> u64 {
    b.get("style").and_then(Value::as_u64).unwrap_or(1)
}

fn emoji_line(ui: &mut Ui, v: &mut Value) {
    ui.horizontal(|ui| {
        ui.set_min_height(crate::ui::inputs::input_height(ui));
        label_column(ui, egui::Label::new(RichText::new(t!("Emoji")).size(12.5)));
        let mut text = emoji_text(v.get("emoji"));
        let w = ui.available_width();
        if ui.add(crate::ui::inputs::TextLine::new(&mut text).hint("👍 or <:name:id>").width(w)).changed() {
            match parse_emoji(&text) {
                Some(e) => v["emoji"] = e,
                None => {
                    v.as_object_mut().unwrap().remove("emoji");
                }
            }
        }
    });
}

fn number(ui: &mut Ui, v: &mut Value, key: &str, label: &str) {
    let mut text = v.get(key).and_then(Value::as_u64).map(|n| n.to_string()).unwrap_or_default();
    ui.label(RichText::new(label).size(12.5));
    if ui.add(crate::ui::inputs::TextLine::new(&mut text).hint("1").width(60.0)).changed() {
        match text.trim().parse::<u64>() {
            Ok(n) => v[key] = json!(n),
            Err(_) => {
                v.as_object_mut().unwrap().remove(key);
            }
        }
    }
}

fn select_editor(ui: &mut Ui, theme: &Theme, sel: &mut Value) {
    let current = ty(sel);
    ui.horizontal(|ui| {
        ui.set_min_height(crate::ui::inputs::input_height(ui));
        label_column(ui, egui::Label::new(RichText::new(t!("Choose from")).size(12.5)));
        let label = SELECT_KINDS.iter().find(|k| k.0 == current).map(|k| crate::i18n::t(k.1)).unwrap_or_default();
        let mut picked = current;
        crate::ui::widgets::dropdown(ui, ui.id().with("select-kind"), 180.0, label, |ui| {
            for (k, l) in SELECT_KINDS {
                ui.selectable_value(&mut picked, k, crate::i18n::t(l));
            }
        });
        if picked != current {
            sel["type"] = json!(picked);
            let m = sel.as_object_mut().unwrap();
            m.remove("default_values");
            if picked == kind::STRING_SELECT {
                m.entry("options").or_insert(json!([{ "label": "First", "value": "first" }]));
            } else {
                m.remove("options");
            }
            if picked != 8 {
                m.remove("channel_types");
            }
        }
        ui.add_space(8.0);
        flag(ui, theme, sel, "disabled", t!("Disabled"));
    });
    line(ui, sel, "custom_id", t!("Select Menu Used matches this"), t!("Custom id"));
    line(ui, sel, "placeholder", t!("Shown before anything is picked"), t!("Placeholder"));
    ui.horizontal(|ui| {
        ui.set_min_height(crate::ui::inputs::input_height(ui));
        label_column(ui, egui::Label::new(RichText::new(t!("How many")).size(12.5)));
        number(ui, sel, "min_values", t!("At least"));
        number(ui, sel, "max_values", t!("At most"));
    });
    match ty(sel) {
        kind::STRING_SELECT => {
            let options = list_mut(sel, "options");
            let mut remove = None;
            for (i, o) in options.iter_mut().enumerate() {
                ui.push_id(("option", i), |ui| {
                    Frame::new().stroke(Stroke::new(1.0, theme.line)).corner_radius(7).inner_margin(8).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        line(ui, o, "label", t!("What people see"), t!("Label"));
                        line(ui, o, "value", t!("What the bot gets"), t!("Value"));
                        line(ui, o, "description", t!("Optional, under the label"), t!("Description"));
                        emoji_line(ui, o);
                        ui.horizontal(|ui| {
                            flag(ui, theme, o, "default", t!("Picked at first"));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if dialog::danger_button(ui, theme, t!("Remove option")).clicked() {
                                    remove = Some(i);
                                }
                            });
                        });
                    });
                });
                ui.add_space(4.0);
            }
            if let Some(i) = remove {
                options.remove(i);
            }
            if options.len() < 25 && dialog::button(ui, theme, t!("+ Add option")).clicked() {
                let n = options.len() + 1;
                options.push(json!({ "label": format!("Option {n}"), "value": format!("option{n}") }));
            }
        }
        t => {
            if t == 8 {
                ui.horizontal(|ui| {
                    ui.set_min_height(crate::ui::inputs::input_height(ui));
                    label_column(ui, egui::Label::new(RichText::new(t!("Channel kinds")).size(12.5)));
                    let types = list_mut(sel, "channel_types");
                    let mut picked: Vec<u64> = types.iter().filter_map(Value::as_u64).collect();
                    if crate::ui::widgets::multi_select(ui, ui.id().with("channel-kinds"), 220.0, &CHANNEL_TYPES, &mut picked, t!("Every kind")) {
                        *types = picked.into_iter().map(|n| json!(n)).collect();
                    }
                });
            }
            if t != 7 {
                // Picked at first: ids of users, roles or channels.
                let what = match t {
                    5 => "user",
                    6 => "role",
                    _ => "channel",
                };
                let mut ids = list_of(sel, "default_values").iter().map(|d| text_of(d, "id").to_string()).collect::<Vec<_>>().join(", ");
                ui.horizontal(|ui| {
                    ui.set_min_height(crate::ui::inputs::input_height(ui));
                    label_column(ui, egui::Label::new(RichText::new(t!("Picked at first")).size(12.5)));
                    let w = ui.available_width();
                    if ui.add(crate::ui::inputs::TextLine::new(&mut ids).hint(t!("Ids, separated by commas")).width(w)).changed() {
                        let list: Vec<Value> = ids.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|id| json!({ "id": id, "type": what })).collect();
                        if list.is_empty() {
                            sel.as_object_mut().unwrap().remove("default_values");
                        } else {
                            sel["default_values"] = Value::Array(list);
                        }
                    }
                });
            }
        }
    }
}

// ---- The preview ------------------------------------------------------------

/// Discord's own colors, so the preview looks like the real thing in any theme.
mod discord {
    use eframe::egui::Color32;
    pub const CHAT: Color32 = Color32::from_rgb(0x31, 0x33, 0x38);
    pub const CONTAINER: Color32 = Color32::from_rgb(0x2B, 0x2D, 0x31);
    pub const BORDER: Color32 = Color32::from_rgb(0x3F, 0x41, 0x47);
    pub const DARK: Color32 = Color32::from_rgb(0x1E, 0x1F, 0x22);
    pub const TEXT: Color32 = Color32::from_rgb(0xDB, 0xDE, 0xE1);
    pub const STRONG: Color32 = Color32::from_rgb(0xF2, 0xF3, 0xF5);
    pub const MUTED: Color32 = Color32::from_rgb(0x94, 0x9B, 0xA4);
    pub const LINK: Color32 = Color32::from_rgb(0x00, 0xA8, 0xFC);
    pub fn button(style: u64) -> Color32 {
        match style {
            1 | 6 => Color32::from_rgb(0x58, 0x65, 0xF2),
            3 => Color32::from_rgb(0x24, 0x80, 0x46),
            4 => Color32::from_rgb(0xDA, 0x37, 0x3C),
            _ => Color32::from_rgb(0x4E, 0x50, 0x58),
        }
    }
}

/// The message as Discord shows it, with the bot's name above it.
pub fn preview(ui: &mut Ui, theme: &Theme, layout: &Value, bot: Option<&crate::bot::profile::BotProfile>) {
    Frame::new().fill(discord::CHAT).corner_radius(8).inner_margin(Margin::same(14)).show(ui, |ui| {
        ui.set_width(ui.available_width().min(520.0));
        ui.horizontal_top(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), egui::Sense::hover());
            crate::bot::profile::paint_avatar(ui, r, bot, theme.accent);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(crate::bot::profile::name(bot)).size(14.5).strong().color(discord::STRONG));
                    ui.label(RichText::new("APP · Today at 12:00").size(11.0).color(discord::MUTED));
                });
                let w = ui.available_width();
                preview_list(ui, components(layout), w);
            });
        });
    });
}

fn preview_list(ui: &mut Ui, list: &[Value], width: f32) {
    for c in list {
        preview_one(ui, c, width);
    }
}

fn preview_one(ui: &mut Ui, c: &Value, width: f32) {
    let children = list_of(c, "components");
    match ty(c) {
        kind::TEXT => markdown(ui, text_of(c, "content"), width),
        kind::SECTION => {
            ui.horizontal_top(|ui| {
                let side = 84.0;
                ui.allocate_ui_with_layout(vec2(width - side - 12.0, 10.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    // The text takes the width, so the picture sits at the right edge.
                    ui.set_min_width(width - side - 12.0);
                    for t in children {
                        markdown(ui, text_of(t, "content"), width - side - 12.0);
                    }
                });
                if let Some(a) = c.get("accessory") {
                    if ty(a) == kind::BUTTON {
                        preview_button(ui, a);
                    } else {
                        picture(ui, text_of(a.get("media").unwrap_or(&Value::Null), "url"), vec2(side, side), a.get("spoiler").and_then(Value::as_bool).unwrap_or(false));
                    }
                }
            });
        }
        kind::CONTAINER => {
            let accent = c.get("accent_color").and_then(Value::as_u64).map(|n| Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8));
            let frame = Frame::new()
                .fill(discord::CONTAINER)
                .stroke(Stroke::new(1.0, discord::BORDER))
                .corner_radius(8)
                .inner_margin(Margin { left: if accent.is_some() { 18 } else { 14 }, right: 14, top: 12, bottom: 12 })
                .show(ui, |ui| {
                    ui.set_width(width - 32.0);
                    ui.spacing_mut().item_spacing.y = 8.0;
                    preview_list(ui, children, width - 32.0);
                });
            let r = frame.response.rect;
            if let Some(color) = accent {
                let bar = egui::Rect::from_min_max(r.left_top(), egui::pos2(r.left() + 4.0, r.bottom()));
                ui.painter().rect_filled(bar, CornerRadius { nw: 8, sw: 8, ne: 0, se: 0 }, color);
            }
            if c.get("spoiler").and_then(Value::as_bool).unwrap_or(false) {
                spoiler_cover(ui, r);
            }
        }
        kind::GALLERY => {
            let items = list_of(c, "items");
            let per_row = match items.len() {
                1 => 1,
                2 | 4 => 2,
                _ => 3,
            };
            let gap = 4.0;
            let w = ((width - gap * (per_row as f32 - 1.0)) / per_row as f32).floor();
            let h = if per_row == 1 { 220.0 } else { w.min(160.0) };
            for row in items.chunks(per_row) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for item in row {
                        picture(ui, text_of(item.get("media").unwrap_or(&Value::Null), "url"), vec2(w, h), item.get("spoiler").and_then(Value::as_bool).unwrap_or(false));
                    }
                });
            }
        }
        kind::FILE => {
            let url = text_of(c.get("file").unwrap_or(&Value::Null), "url");
            Frame::new().fill(discord::CONTAINER).stroke(Stroke::new(1.0, discord::BORDER)).corner_radius(8).inner_margin(Margin::same(12)).show(ui, |ui| {
                ui.set_width(width.min(400.0) - 26.0);
                ui.label(RichText::new(file_name(url)).size(14.0).color(discord::LINK));
                ui.label(RichText::new(t!("File")).size(11.5).color(discord::MUTED));
            });
        }
        kind::SEPARATOR => {
            let space = if c.get("spacing").and_then(Value::as_u64) == Some(2) { 16.0 } else { 6.0 };
            ui.add_space(space);
            if c.get("divider").and_then(Value::as_bool).unwrap_or(true) {
                let (r, _) = ui.allocate_exact_size(vec2(width, 1.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 0.0, discord::BORDER);
            }
            ui.add_space(space);
        }
        kind::ROW => {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                for x in children {
                    if is_select(x) {
                        preview_select(ui, x, width);
                    } else {
                        preview_button(ui, x);
                    }
                }
            });
        }
        _ => {
            ui.label(RichText::new(kind_name(c)).size(12.0).italics().color(discord::MUTED));
        }
    }
}

/// A text component's markdown, as Discord shows it (see `markdown`).
fn markdown(ui: &mut Ui, text: &str, width: f32) {
    let m = crate::builders::markdown::layout(ui.painter(), text, crate::builders::markdown::Kind::Full, 14.0, discord::TEXT, width);
    let (rect, _) = ui.allocate_exact_size(vec2(width, m.size().y), egui::Sense::hover());
    m.paint(ui.painter(), rect.min);
}

fn file_name(url: &str) -> String {
    let name = url.trim_start_matches("attachment://").rsplit(['/', '\\']).next().unwrap_or(url);
    if name.is_empty() { "file".into() } else { name.into() }
}

/// A picture: shown when it is a file on this computer, else a frame with its name.
fn picture(ui: &mut Ui, url: &str, size: egui::Vec2, spoiler: bool) {
    let (r, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(r, 8.0, discord::DARK);
    let path = Path::new(url);
    if !url.is_empty() && !url.contains("://") && path.is_file() {
        egui::Image::from_uri(crate::editor::file_uri(path)).corner_radius(8).paint_at(ui, r);
    } else {
        let label = if url.contains("${") { url.to_string() } else { file_name(url) };
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(11.5), discord::MUTED);
    }
    if spoiler {
        spoiler_cover(ui, r);
    }
}

fn spoiler_cover(ui: &mut Ui, r: egui::Rect) {
    ui.painter().rect_filled(r, 8.0, Color32::from_rgba_unmultiplied(0x1E, 0x1F, 0x22, 235));
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, t!("SPOILER"), FontId::proportional(12.0), discord::STRONG);
}

fn preview_button(ui: &mut Ui, b: &Value) {
    let style = picked_style(b);
    let label = match (text_of(b, "label"), emoji_text(b.get("emoji"))) {
        (l, e) if !e.is_empty() && !e.starts_with('<') => format!("{e} {l}"),
        (l, _) if style == 6 => if l.is_empty() { t!("Buy").to_string() } else { l.to_string() },
        (l, _) => l.to_string(),
    };
    let disabled = b.get("disabled").and_then(Value::as_bool).unwrap_or(false);
    let fill = discord::button(style);
    let fill = if disabled { fill.gamma_multiply(0.5) } else { fill };
    Frame::new().fill(fill).corner_radius(4).inner_margin(Margin::symmetric(16, 7)).show(ui, |ui| {
        ui.label(RichText::new(label).size(13.5).color(Color32::WHITE));
    });
}

fn preview_select(ui: &mut Ui, s: &Value, width: f32) {
    let placeholder = match text_of(s, "placeholder") {
        "" => t!("Make a selection").to_string(),
        p => p.to_string(),
    };
    Frame::new().fill(discord::DARK).stroke(Stroke::new(1.0, discord::BORDER)).corner_radius(4).inner_margin(Margin::symmetric(12, 9)).show(ui, |ui| {
        ui.set_width(width.min(400.0) - 26.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(placeholder).size(13.5).color(discord::MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 6.0), egui::Sense::hover());
                ui.painter().add(egui::Shape::convex_polygon(vec![r.left_top(), r.right_top(), r.center_bottom()], discord::TEXT, Stroke::NONE));
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_layouts_are_read_and_checked() {
        let message = json!({ "flags": 32768, "components": [
            { "type": 17, "accent_color": 5793266, "components": [
                { "type": 10, "content": "## Hi ${user}" },
                { "type": 9, "components": [{ "type": 10, "content": "Level {level}" }], "accessory": { "type": 11, "media": { "url": "${avatar}" } } },
                { "type": 14 },
                { "type": 1, "components": [{ "type": 2, "style": 1, "label": "Claim", "custom_id": "claim" }, { "type": 2, "style": 5, "label": "Site", "url": "https://x.example" }] }
            ] },
            { "type": 12, "items": [{ "media": { "url": "https://x.example/a.png" } }] }
        ] });
        let layout = from_json(&message).unwrap();
        assert_eq!(components(&layout).len(), 2);
        // The container, its text, section (with its text and picture), separator
        // and row (with 2 buttons), and the gallery.
        assert_eq!(count(components(&layout)), 10);
        assert!(problems(&layout).is_empty(), "{:?}", problems(&layout));
        assert_eq!(to_json(&layout), message, "nothing is lost on the way back");
        assert_eq!(summary(&layout), ["Container · 4 parts", "Media gallery · 1 pictures"]);

        let lists_only = from_json(&json!([{ "type": 10, "content": "x" }])).unwrap();
        assert_eq!(components(&lists_only).len(), 1);
        assert!(from_json(&json!({ "content": "hi", "components": [] })).is_err(), "V2 messages have no content");
        assert!(from_json(&json!([{ "content": "no type" }])).is_err());
    }

    #[test]
    fn problems_name_what_discord_refuses() {
        let layout = json!({ "components": [
            { "type": 9, "components": [{ "type": 10, "content": "x" }] },
            { "type": 1, "components": [{ "type": 2, "style": 1, "label": "A", "custom_id": "same" }, { "type": 2, "style": 1, "label": "B", "custom_id": "same" }] },
            { "type": 1, "components": [{ "type": 2, "style": 5, "label": "Go" }] }
        ] });
        let p = problems(&layout).join("\n");
        assert!(p.contains("picture or a button"), "{p}");
        assert!(p.contains("used twice"), "{p}");
        assert!(p.contains("web address"), "{p}");
    }

    #[test]
    fn emojis_read_both_ways() {
        assert_eq!(parse_emoji("<a:party:123>").unwrap(), json!({ "id": "123", "name": "party", "animated": true }));
        assert_eq!(emoji_text(Some(&json!({ "id": "123", "name": "party", "animated": true }))), "<a:party:123>");
        assert_eq!(emoji_text(Some(&parse_emoji("👍").unwrap())), "👍");
        assert!(parse_emoji("  ").is_none());
    }
}
