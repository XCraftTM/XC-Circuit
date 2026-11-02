//! Values a block can use in its texts: what the event and the blocks before
//! it on its action path made, found by following the Action wires back.
//!
//! Each is written as a placeholder: `{user}` for an output of the event,
//! `{#4.message}` for the Message output of block #4, `{name}` for a variable
//! set earlier in the run, and a property after a dot: `{user.username}`.
//!
//! Text boxes offer them while typing after `{`, in a menu on right-click and
//! behind the `{ }` button in their corner (in any text box drawn while a
//! block's values are in scope, see [`set_scope`]); builders also list them.
//! A value picked or clicked goes where the text cursor was.

use std::collections::HashSet;
use std::sync::Arc;

use circuit_core::graph::ACTION_IN;
use circuit_core::{BlockKind, BlockShape, Workspace};
use circuit_plugins::Registry;
use eframe::egui::{self, Color32, Id, Key, Modifiers, RichText, Ui, vec2};

use crate::theme::Theme;

/// One value a text can use.
#[derive(Clone, Debug, PartialEq)]
pub struct Avail {
    /// What goes between the braces: `user.username`, `#4.message`.
    pub token: String,
    /// What it is: "Member › Display name".
    pub label: String,
    /// Where it comes from: the block's name and number.
    pub source: String,
    /// Its type's name, such as Text or Member.
    pub kind: String,
}

/// The values block `node` can use, the event's first, then the blocks on the
/// way to it in the order they run, then run variables. `shape_of` gives a
/// placed block's shape.
pub fn available(ws: &Workspace, node: &str, reg: &Registry, shape_of: &mut dyn FnMut(&str) -> Option<BlockShape>) -> Vec<Avail> {
    // A value block (no Action wire into it) runs for the blocks it feeds,
    // through other value blocks if need be.
    let is_value = |id: &str| ws.nodes.get(id).and_then(|n| reg.block(&n.block)).is_some_and(|d| d.kind == BlockKind::Value);
    let mut start = Vec::new();
    let mut feeds = vec![node.to_string()];
    let mut fed: HashSet<String> = HashSet::new();
    while let Some(cur) = feeds.pop() {
        if !fed.insert(cur.clone()) {
            continue;
        }
        if is_value(&cur) {
            feeds.extend(ws.links.iter().filter(|l| l.from.node == cur && l.to.pin != ACTION_IN).map(|l| l.to.node.clone()));
        } else {
            start.push(cur);
        }
    }
    // Back along the Action wires; a Try Zone's outputs come after its blocks.
    let mut seen: HashSet<String> = start.iter().cloned().collect();
    let mut order: Vec<String> = Vec::new();
    let mut queue = start.clone();
    while let Some(cur) = queue.pop() {
        let mut before: Vec<String> = ws.links.iter().filter(|l| l.to.node == cur && l.to.pin == ACTION_IN).map(|l| l.from.node.clone()).collect();
        // Blocks in the same Try Zone as an entry point ran before it too: the
        // zone's own action input runs the first of them.
        if let Some(zone) = ws.nodes.get(&cur).and_then(|n| n.zone.clone()) {
            before.extend(ws.links.iter().filter(|l| l.to.node == zone && l.to.pin == ACTION_IN).map(|l| l.from.node.clone()));
        }
        for b in before {
            let inner: Vec<String> = if ws.zones.contains_key(&b) { ws.nodes.iter().filter(|(_, n)| n.zone.as_ref() == Some(&b)).map(|(id, _)| id.clone()).collect() } else { vec![b] };
            for id in inner {
                if seen.insert(id.clone()) {
                    order.push(id.clone());
                    queue.push(id);
                }
            }
        }
    }
    order.retain(|id| !start.contains(id) && id != node);
    order.reverse();

    let mut events = Vec::new();
    let mut steps = Vec::new();
    let mut vars = Vec::new();
    for id in &order {
        let Some(n) = ws.nodes.get(id) else { continue };
        let Some(def) = reg.block(&n.block) else { continue };
        let source = format!("{} · #{id}", def.name);
        if n.block_id() == "core.set_variable" {
            let name = n.values.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let run_scope = n.values.get("scope").and_then(|v| v.as_str()).is_none_or(|s| s == "run");
            if run_scope && !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') {
                vars.push(Avail { token: name.into(), label: name.into(), source: source.clone(), kind: t!("Variable").into() });
            }
        }
        let Some(shape) = shape_of(id) else { continue };
        let target = if def.kind == BlockKind::Event { &mut events } else { &mut steps };
        for (key, o) in &shape.outputs {
            let token = if def.kind == BlockKind::Event { key.clone() } else { format!("#{id}.{key}") };
            let label = crate::i18n::label(&o.label(key)).into_owned();
            let type_id = o.type_name.trim_end_matches('?');
            let kind = reg.types.get(type_id).map(|t| crate::i18n::label(&t.name).into_owned()).unwrap_or_else(|| crate::i18n::label(&circuit_core::block::title_case(type_id)).into_owned());
            target.push(Avail { token: token.clone(), label: label.clone(), source: source.clone(), kind });
            for p in reg.types.get(type_id).map(|t| t.properties.as_slice()).unwrap_or_default() {
                let pkind = crate::i18n::label(&circuit_core::block::title_case(&p.ty)).into_owned();
                target.push(Avail { token: format!("{token}.{}", p.path), label: format!("{label} › {}", crate::i18n::label(&p.name)), source: source.clone(), kind: pkind });
            }
        }
    }
    events.extend(steps);
    events.extend(vars);
    events
}

// ---- Text boxes -------------------------------------------------------------

const SCOPE: &str = "values-scope";
const LAST_TEXT: &str = "values-last-text";
const PENDING: &str = "values-pending";

/// The values text boxes offer from now on, until set to `None`.
pub fn set_scope(ctx: &egui::Context, values: Option<Arc<Vec<Avail>>>) {
    ctx.data_mut(|d| match values {
        Some(v) => {
            d.insert_temp(Id::new(SCOPE), v);
        }
        None => d.remove::<Arc<Vec<Avail>>>(Id::new(SCOPE)),
    });
}

fn scope(ctx: &egui::Context) -> Option<Arc<Vec<Avail>>> {
    ctx.data(|d| d.get_temp::<Arc<Vec<Avail>>>(Id::new(SCOPE)))
}

/// Puts `{token}` where the text cursor was in the text box used last.
pub fn insert(ctx: &egui::Context, token: &str) {
    ctx.data_mut(|d| d.insert_temp(Id::new(PENDING), token.to_string()));
}

/// Whether text box `id` has its values menu open, so the values stay in
/// scope while the pointer is in the menu.
pub fn menu_open(ctx: &egui::Context, id: Id) -> bool {
    ctx.data(|d| d.get_temp::<bool>(id.with("values-menu-open"))).unwrap_or(false)
}

/// The values, a submenu per block they come from. Returns the one picked.
fn menu(ui: &mut Ui, values: &[Avail]) -> Option<String> {
    let mut picked = None;
    ui.set_min_width(200.0);
    if values.is_empty() {
        ui.label(RichText::new(t!("No values yet: connect this block's Action input to an event or other blocks.")).size(12.0));
        return None;
    }
    let mut sources: Vec<&str> = Vec::new();
    for v in values {
        if !sources.contains(&v.source.as_str()) {
            sources.push(&v.source);
        }
    }
    for source in sources {
        ui.menu_button(source, |ui| {
            egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                for v in values.iter().filter(|v| v.source == source) {
                    let text = RichText::new(format!("{}   {{{}}}", v.label, v.token)).size(12.5);
                    if ui.button(text).on_hover_text(&v.kind).clicked() {
                        picked = Some(v.token.clone());
                        ui.close();
                    }
                }
            });
        });
    }
    picked
}

/// Whether a text box was used yet, so [`insert`] has somewhere to go.
pub fn has_target(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Id>(Id::new(LAST_TEXT)).is_some())
}

#[derive(Clone, Copy, PartialEq, Default)]
enum KeyAction {
    #[default]
    Down,
    Up,
    Pick,
    Close,
}

/// Before a text box with id `id`: while its list of values is open, the
/// arrow keys, Enter, Tab and Escape go to the list.
pub fn before(ui: &Ui, id: Id) {
    let open = ui.ctx().data(|d| d.get_temp::<usize>(id.with("values-selected"))).is_some();
    if !open || scope(ui.ctx()).is_none() || !ui.ctx().memory(|m| m.has_focus(id)) {
        return;
    }
    let action = ui.ctx().input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
            Some(KeyAction::Down)
        } else if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            Some(KeyAction::Up)
        } else if i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::Tab) {
            Some(KeyAction::Pick)
        } else if i.consume_key(Modifiers::NONE, Key::Escape) {
            Some(KeyAction::Close)
        } else {
            None
        }
    });
    if let Some(a) = action {
        ui.ctx().data_mut(|d| d.insert_temp(id.with("values-key"), a));
    }
}

/// The text cursor of text box `id`, as a character index.
fn cursor(ctx: &egui::Context, id: Id, text: &str) -> usize {
    egui::text_edit::TextEditState::load(ctx, id).and_then(|s| s.cursor.char_range()).map(|r| r.primary.index.into()).unwrap_or_else(|| text.chars().count())
}

fn set_cursor(ctx: &egui::Context, id: Id, at: usize) {
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(at))));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// Replaces the characters `from..to` of `text` with `{token}` and puts the
/// cursor after it. A `}` already right after the cursor is reused.
fn put(ctx: &egui::Context, id: Id, text: &mut String, from: usize, to: usize, token: &str) {
    let chars: Vec<char> = text.chars().collect();
    let closed = chars.get(to) == Some(&'}');
    let piece = if closed { format!("{{{token}") } else { format!("{{{token}}}") };
    let mut out: String = chars[..from].iter().collect();
    out.push_str(&piece);
    out.extend(&chars[to..]);
    *text = out;
    set_cursor(ctx, id, from + piece.chars().count() + usize::from(closed));
}

/// After a text box: shows the values matching what was typed after `{`, puts
/// in the one picked, and puts in a value clicked in a values list. Returns
/// whether `text` changed.
pub fn after(ui: &mut Ui, resp: &egui::Response, text: &mut String) -> bool {
    let ctx = ui.ctx().clone();
    let id = resp.id;
    let Some(values) = scope(&ctx) else { return false };
    let mut changed = false;

    // Right-click, or the { } button in the box's top-right corner (shown while
    // the pointer is over the box or it has the text cursor): pick a value from
    // a menu, and it goes in at the text cursor.
    let mut picked = None;
    resp.context_menu(|ui| picked = menu(ui, &values));
    let menu_id = id.with("values-menu");
    let visible = resp.rect.intersect(ui.clip_rect());
    let menu_was_open = egui::Popup::is_id_open(&ctx, menu_id);
    if ui.rect_contains_pointer(visible) || resp.has_focus() || menu_was_open {
        let side = 22.0;
        let top = if visible.height() < side * 2.0 + 8.0 { visible.center().y - side / 2.0 } else { visible.top() + 4.0 };
        let r = egui::Rect::from_min_size(egui::pos2(visible.right() - side - 4.0, top), vec2(side, side));
        let b = ui.put(r, egui::Button::new(RichText::new("{}").monospace().size(11.0)).wrap_mode(egui::TextWrapMode::Extend).corner_radius(5)).on_hover_text(t!("Insert a value from earlier blocks (or right-click, or type {)"));
        egui::Popup::menu(&b).id(menu_id).show(|ui| {
            if let Some(token) = menu(ui, &values) {
                picked = Some(token);
            }
        });
    }
    let open = resp.context_menu_opened() || egui::Popup::is_id_open(&ctx, menu_id);
    ctx.data_mut(|d| d.insert_temp(id.with("values-menu-open"), open));
    if let Some(token) = picked {
        let at = cursor(&ctx, id, text);
        put(&ctx, id, text, at, at, &token);
        return true;
    }
    if resp.has_focus() {
        ctx.data_mut(|d| d.insert_temp(Id::new(LAST_TEXT), id));
    }
    // A value clicked in a list goes where this box's cursor was.
    if ctx.data(|d| d.get_temp::<Id>(Id::new(LAST_TEXT))) == Some(id)
        && let Some(token) = ctx.data_mut(|d| d.remove_temp::<String>(Id::new(PENDING)))
    {
        let at = cursor(&ctx, id, text);
        put(&ctx, id, text, at, at, &token);
        return true;
    }
    let selected_id = id.with("values-selected");
    let key = ctx.data_mut(|d| d.remove_temp::<KeyAction>(id.with("values-key")));
    if !resp.has_focus() {
        ctx.data_mut(|d| d.remove::<usize>(selected_id));
        return false;
    }
    // The name being typed after an open brace, up to the cursor.
    let at = cursor(&ctx, id, text);
    let chars: Vec<char> = text.chars().collect();
    let mut start = at.min(chars.len());
    while start > 0 && (chars[start - 1].is_alphanumeric() || matches!(chars[start - 1], '_' | '$' | '.' | '#')) {
        start -= 1;
    }
    let dismissed = ctx.data(|d| d.get_temp::<usize>(id.with("values-dismissed")));
    if start == 0 || chars[start - 1] != '{' || dismissed == Some(start) {
        ctx.data_mut(|d| d.remove::<usize>(selected_id));
        return false;
    }
    let typed: String = chars[start..at.min(chars.len())].iter().collect::<String>().to_lowercase();
    // Best first: names that start with what was typed, then names with a part
    // that does, then names and descriptions that contain it.
    let rank = |v: &Avail| {
        let token = v.token.to_lowercase();
        if token.starts_with(&typed) {
            Some(0)
        } else if token.split('.').any(|part| part.starts_with(&typed)) {
            Some(1)
        } else if token.contains(&typed) || v.label.to_lowercase().contains(&typed) {
            Some(2)
        } else {
            None
        }
    };
    let mut ranked: Vec<(usize, &Avail)> = values.iter().filter_map(|v| rank(v).map(|r| (r, v))).collect();
    ranked.sort_by_key(|(r, _)| *r);
    let matches: Vec<&Avail> = ranked.into_iter().map(|(_, v)| v).take(60).collect();
    if matches.is_empty() {
        ctx.data_mut(|d| d.remove::<usize>(selected_id));
        return false;
    }
    let mut selected = ctx.data(|d| d.get_temp::<usize>(selected_id)).unwrap_or(0).min(matches.len() - 1);
    let mut pick = None;
    match key {
        Some(KeyAction::Down) => selected = (selected + 1) % matches.len(),
        Some(KeyAction::Up) => selected = (selected + matches.len() - 1) % matches.len(),
        Some(KeyAction::Pick) => pick = Some(selected),
        Some(KeyAction::Close) => {
            ctx.data_mut(|d| {
                d.insert_temp(id.with("values-dismissed"), start);
                d.remove::<usize>(selected_id);
            });
            return false;
        }
        None => {}
    }

    // The list, under the box (on the canvas the box is in a zoomed layer).
    let rect = ctx.layer_transform_to_global(resp.layer_id).map_or(resp.rect, |t| t * resp.rect);
    egui::Area::new(id.with("values-list")).order(egui::Order::Foreground).fixed_pos(rect.left_bottom() + vec2(0.0, 4.0)).show(&ctx, |ui| {
        let v = ui.visuals().clone();
        egui::Frame::new().fill(v.window_fill).stroke(v.window_stroke).corner_radius(8).inner_margin(6).shadow(v.popup_shadow).show(ui, |ui| {
            ui.set_width(rect.width().clamp(300.0, 460.0));
            ui.label(RichText::new(t!("Values from earlier blocks · arrow keys and Enter pick one")).size(11.0).color(v.weak_text_color()));
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for (i, value) in matches.iter().enumerate() {
                    let row = value_row(ui, value, i == selected);
                    if i == selected && key.is_some() {
                        row.scroll_to_me(None);
                    }
                    if row.clicked() {
                        pick = Some(i);
                    }
                }
            });
        });
    });
    match pick {
        Some(i) => {
            put(&ctx, id, text, start - 1, at, &matches[i].token);
            ctx.data_mut(|d| d.remove::<usize>(selected_id));
            changed = true;
        }
        None => {
            ctx.data_mut(|d| d.insert_temp(selected_id, selected));
        }
    }
    changed
}

/// One value: its placeholder, what it is and where it comes from. Colors come
/// from the theme's egui visuals, so text boxes anywhere can show the list.
fn value_row(ui: &mut Ui, v: &Avail, selected: bool) -> egui::Response {
    let visuals = ui.visuals().clone();
    let accent = visuals.selection.stroke.color;
    let fill = if selected { crate::theme::with_alpha(accent, 40) } else { Color32::TRANSPARENT };
    let frame = egui::Frame::new().fill(fill).corner_radius(5).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{{{}}}", v.token)).monospace().size(12.0).color(accent));
            ui.label(RichText::new(&v.label).size(12.0).color(visuals.text_color()));
        });
        ui.label(RichText::new(format!("{} · {}", v.source, v.kind)).size(10.5).color(visuals.weak_text_color()));
    });
    let resp = ui.interact(frame.response.rect, ui.id().with(("value-row", &v.token)), egui::Sense::click());
    if resp.hovered() {
        ctx_cursor(ui);
    }
    resp
}

fn ctx_cursor(ui: &Ui) {
    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
}

/// The values list of a builder: every value grouped by where it comes from;
/// clicking one puts it where the text cursor was.
pub fn list(ui: &mut Ui, theme: &Theme, values: &[Avail]) {
    if values.is_empty() {
        ui.label(RichText::new(t!("Nothing comes before this block yet: connect its Action input to an event or other blocks, and their values show up here.")).size(12.0).color(theme.text_muted));
        return;
    }
    let tip = if has_target(ui.ctx()) { t!("Click a value to put it where the text cursor is, or type { in any text.") } else { t!("Click into a text, then a value to put it there. Or type { in any text.") };
    ui.label(RichText::new(tip).size(12.0).color(theme.text_muted));
    let mut source = "";
    for v in values {
        if v.source != source {
            source = &v.source;
            ui.add_space(4.0);
            ui.label(RichText::new(source).size(11.5).strong().color(theme.text_muted));
        }
        if value_row(ui, v, false).clicked() {
            insert(ui.ctx(), &v.token);
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn values_come_from_the_action_path() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = crate::editor::Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        // Send Message (#4) in /rank: after the Slash Command (#1) and Build Embed (#3).
        let values = ed.values_for("rank-command", "4");
        let tokens: Vec<&str> = values.iter().map(|v| v.token.as_str()).collect();
        assert!(tokens.contains(&"user"), "event outputs by name: {tokens:?}");
        assert!(tokens.contains(&"user.username"), "with their properties: {tokens:?}");
        assert!(tokens.contains(&"#3.embed"), "earlier blocks by number: {tokens:?}");
        assert!(tokens.iter().position(|t| *t == "user") < tokens.iter().position(|t| *t == "#3.embed"), "the event first");
        assert!(!tokens.iter().any(|t| t.starts_with("#4.") || t.starts_with("#5.")), "not itself or later blocks: {tokens:?}");
        // Get Stored Value (#2) is a value block: it gets what the block it feeds gets.
        let fed = ed.values_for("rank-command", "2");
        assert!(fed.iter().any(|v| v.token == "user"), "{fed:?}");
    }
}
