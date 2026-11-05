//! Values a block can use in its texts: what the event and the blocks before
//! it on its action path made, found by following the Action wires back.
//!
//! Texts use them in `${…}`, which holds JavaScript: `${#1.user}` is the User
//! output of block #1, `${name}` a variable set earlier in the run, and
//! members follow a dot: `${#1.user.username}`, `${#1.user.displayAvatarURL()}`. The members come from
//! class catalogs made from the libraries' type definitions (the Discord
//! plugin's is made from discord.js).
//!
//! Text boxes offer them inside `${…}` and on right-click (in any text box drawn
//! while a block's values are in scope, see [`set_scope`]): a list of the
//! blocks before this one that slides to a block's values and on to their
//! members, or after a dot the members of what is before it. Builders list
//! the values too. A value picked or clicked goes where the text cursor was.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use circuit_core::graph::ACTION_IN;
use circuit_core::{BlockKind, BlockShape, Workspace};
use circuit_plugins::{ClassDef, Member, Registry};
use eframe::egui::{self, Color32, Id, Key, Modifiers, RichText, Ui, vec2};

use crate::ui::theme::Theme;

/// The blocks that run before `node` on its action path, first to last
/// (for a value block: before the blocks it feeds).
pub fn upstream(ws: &Workspace, node: &str, reg: &Registry) -> Vec<String> {
    upstream_with(ws, node, reg, &Wiring::new(ws))
}

/// A workspace's wires by block, so walks back along them don't go through
/// all of them at each step. Made once and reused while the wires stay.
#[derive(Default)]
pub struct Wiring {
    action_into: HashMap<String, Vec<String>>,
    data_from: HashMap<String, Vec<String>>,
    zone_blocks: HashMap<String, Vec<String>>,
}

impl Wiring {
    pub fn new(ws: &Workspace) -> Wiring {
        let mut w = Wiring::default();
        for l in &ws.links {
            if l.to.pin == ACTION_IN {
                w.action_into.entry(l.to.node.clone()).or_default().push(l.from.node.clone());
            } else {
                w.data_from.entry(l.from.node.clone()).or_default().push(l.to.node.clone());
            }
        }
        for (id, n) in &ws.nodes {
            if let Some(z) = &n.zone {
                w.zone_blocks.entry(z.clone()).or_default().push(id.clone());
            }
        }
        w
    }
}

/// `upstream` with the wires already indexed.
pub fn upstream_with(ws: &Workspace, node: &str, reg: &Registry, wiring: &Wiring) -> Vec<String> {
    // A value block (no Action wire into it) runs for the blocks it feeds,
    // through other value blocks if need be.
    let is_value = |id: &str| ws.nodes.get(id).and_then(|n| reg.block(&n.block)).is_some_and(|d| d.kind == BlockKind::Value);
    let Wiring { action_into, data_from, zone_blocks } = wiring;
    // Names are borrowed from the wiring while walking; copied once at the end.
    let mut start: Vec<&str> = Vec::new();
    let mut feeds: Vec<&str> = vec![node];
    let mut fed: HashSet<&str> = HashSet::new();
    while let Some(cur) = feeds.pop() {
        if !fed.insert(cur) {
            continue;
        }
        if is_value(cur) {
            feeds.extend(data_from.get(cur).into_iter().flatten().map(String::as_str));
        } else {
            start.push(cur);
        }
    }
    // Back along the Action wires; a Try Zone's outputs come after its blocks.
    let mut seen: HashSet<&str> = start.iter().copied().collect();
    let mut order: Vec<&str> = Vec::new();
    let mut queue = start.clone();
    while let Some(cur) = queue.pop() {
        let mut before: Vec<&str> = action_into.get(cur).into_iter().flatten().map(String::as_str).collect();
        // Blocks in the same Try Zone as an entry point ran before it too: the
        // zone's own action input runs the first of them.
        if let Some(zone) = ws.nodes.get(cur).and_then(|n| n.zone.as_deref()) {
            before.extend(action_into.get(zone).into_iter().flatten().map(String::as_str));
        }
        for b in before {
            let inner: Vec<&str> = if ws.zones.contains_key(b) { zone_blocks.get(b).into_iter().flatten().map(String::as_str).collect() } else { vec![b] };
            for id in inner {
                if seen.insert(id) {
                    order.push(id);
                    queue.push(id);
                }
            }
        }
    }
    order.retain(|id| !start.contains(id) && *id != node);
    order.reverse();
    order.into_iter().map(str::to_string).collect()
}

/// Names a block gets from earlier blocks without an input: the event's
/// outputs and run variables. `shape_of` gives a placed block's shape.
pub fn known_names(ws: &Workspace, upstream: &[String], reg: &Registry, shape_of: &mut dyn FnMut(&str) -> Option<Arc<BlockShape>>) -> HashSet<String> {
    let mut out = HashSet::new();
    for id in upstream {
        let Some(n) = ws.nodes.get(id) else { continue };
        let Some(def) = reg.block(&n.block) else { continue };
        if def.kind == BlockKind::Event {
            if let Some(s) = shape_of(id) {
                out.extend(s.outputs.iter().map(|(k, _)| k.clone()));
            }
        } else if n.block_id() == "core.set_variable" && n.values.get("scope").and_then(|v| v.as_str()).is_none_or(|s| s == "run") {
            out.extend(n.values.get("name").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(str::to_string));
        }
    }
    out
}

/// One value a text can use.
#[derive(Clone, Debug, PartialEq)]
pub struct Avail {
    /// What goes between the braces: `#4.message`, or a variable's name.
    pub token: String,
    /// Also understood before a dot: an event output's own name (`user`).
    pub alias: Option<String>,
    /// What it is: "Message".
    pub label: String,
    /// Its type's name, such as Text or Member.
    pub kind: String,
    /// The class of its value in the catalogs, for what can follow a dot.
    pub class: Option<String>,
}

/// The values one block gives (or the run's variables).
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    /// The block's name and number: "Slash Command · #1".
    pub title: String,
    pub values: Vec<Avail>,
}

/// What texts on one block can use: the values of the blocks before it, and
/// the class catalogs for what follows a dot.
#[derive(Clone, Default)]
pub struct Scope {
    pub sources: Vec<Source>,
    pub classes: Arc<HashMap<String, ClassDef>>,
}

impl Scope {
    fn members(&self, class: &str) -> &[Member] {
        self.classes.get(class).map(|c| c.members.as_slice()).unwrap_or_default()
    }

    fn values(&self) -> impl Iterator<Item = &Avail> {
        self.sources.iter().flat_map(|s| s.values.iter())
    }
}

/// The values block `node` can use, a [`Source`] per block before it on its
/// action path (the event first), then run variables. Every block value is
/// written `{#id.key}`. `shape_of` gives a placed block's shape.
pub fn available(ws: &Workspace, node: &str, reg: &Registry, shape_of: &mut dyn FnMut(&str) -> Option<Arc<BlockShape>>) -> Scope {
    let order = upstream(ws, node, reg);
    let mut sources = Vec::new();
    let mut vars = Vec::new();
    for id in &order {
        let Some(n) = ws.nodes.get(id) else { continue };
        let Some(def) = reg.block(&n.block) else { continue };
        if n.block_id() == "core.set_variable" {
            let name = n.values.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let run_scope = n.values.get("scope").and_then(|v| v.as_str()).is_none_or(|s| s == "run");
            if run_scope && !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') {
                vars.push(Avail { token: name.into(), alias: None, label: name.into(), kind: t!("Variable").into(), class: None });
            }
        }
        let Some(shape) = shape_of(id) else { continue };
        let values: Vec<Avail> = shape
            .outputs
            .iter()
            .map(|(key, o)| {
                let type_id = o.type_name.trim_end_matches('?');
                Avail {
                    token: format!("#{id}.{key}"),
                    alias: (def.kind == BlockKind::Event).then(|| key.clone()),
                    label: o.label(key),
                    kind: reg.types.get(type_id).map(|t| t.name.clone()).unwrap_or_else(|| circuit_core::block::title_case(type_id)),
                    class: reg.type_classes.get(type_id).cloned(),
                }
            })
            .collect();
        if !values.is_empty() {
            sources.push(Source { title: format!("{} · #{id}", def.name), values });
        }
    }
    if !vars.is_empty() {
        sources.push(Source { title: t!("Variables").into(), values: vars });
    }
    Scope { sources, classes: reg.classes.clone() }
}

// ---- Text boxes -------------------------------------------------------------

const SCOPE: &str = "values-scope";
const LAST_TEXT: &str = "values-last-text";
const PENDING: &str = "values-pending";
const OPEN_LIST: &str = "values-open-list";
/// The height of one row of the list.
const ROW_H: f32 = 26.0;
/// Rows shown before the list scrolls.
const ROWS: usize = 9;

/// Where the list of values was drawn last (screen), for tests.
#[cfg(test)]
pub fn open_list_rect(ctx: &egui::Context) -> Option<egui::Rect> {
    ctx.data(|d| d.get_temp::<egui::Rect>(Id::new(OPEN_LIST)))
}

/// The values text boxes offer from now on, until set to `None`.
pub fn set_scope(ctx: &egui::Context, scope: Option<Arc<Scope>>) {
    ctx.data_mut(|d| match scope {
        Some(v) => {
            d.insert_temp(Id::new(SCOPE), v);
        }
        None => d.remove::<Arc<Scope>>(Id::new(SCOPE)),
    });
}

fn scope(ctx: &egui::Context) -> Option<Arc<Scope>> {
    ctx.data(|d| d.get_temp::<Arc<Scope>>(Id::new(SCOPE)))
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

/// Whether a text box was used yet, so [`insert`] has somewhere to go.
pub fn has_target(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Id>(Id::new(LAST_TEXT)).is_some())
}

/// The right-click menu: a submenu per block, and in it a submenu per value
/// that has members (two levels deep). Returns the token picked.
fn menu(ui: &mut Ui, scope: &Scope) -> Option<String> {
    let mut picked = None;
    ui.set_min_width(200.0);
    if scope.sources.is_empty() {
        ui.label(RichText::new(t!("No values yet: connect this block's Action input to an event or other blocks.")).size(12.0));
        return None;
    }
    for source in &scope.sources {
        ui.menu_button(&source.title, |ui| {
            for v in &source.values {
                let text = format!("{}   {{{}}}", v.label, v.token);
                match &v.class {
                    Some(class) if !scope.members(class).is_empty() => {
                        ui.menu_button(text, |ui| member_menu(ui, scope, class, &v.token, &v.label, 1, &mut picked));
                    }
                    _ => {
                        if ui.button(text).on_hover_text(&v.kind).clicked() {
                            picked = Some(v.token.clone());
                            ui.close();
                        }
                    }
                }
            }
        });
    }
    picked
}

/// A value itself, then its members; members with members of their own get a
/// submenu while `depth` allows.
fn member_menu(ui: &mut Ui, scope: &Scope, class: &str, token: &str, label: &str, depth: usize, picked: &mut Option<String>) {
    if ui.button(RichText::new(format!("{label}   {{{token}}}")).strong()).clicked() {
        *picked = Some(token.to_string());
        ui.close();
    }
    ui.separator();
    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
        for m in scope.members(class) {
            let path = format!("{token}.{}", m.name);
            let text = format!("{}   {}", m.name, kind_text(m));
            match &m.class {
                Some(c) if depth < 2 && !scope.members(c).is_empty() => {
                    ui.menu_button(text, |ui| member_menu(ui, scope, c, &path, &m.name, depth + 1, picked));
                }
                _ => {
                    let b = ui.button(text);
                    let b = if let Some(doc) = &m.doc { b.on_hover_text(doc) } else { b };
                    if b.clicked() {
                        *picked = Some(path);
                        ui.close();
                    }
                }
            }
        }
    });
}

/// A member's type as shown: `Text`, `Date?` (can be empty), `User`.
fn kind_text(m: &Member) -> String {
    format!("{}{}", m.ty, if m.optional { "?" } else { "" })
}

#[derive(Clone, Copy, PartialEq, Default)]
enum KeyAction {
    #[default]
    Down,
    Up,
    Pick,
    Close,
    Back,
    Into,
}

/// Before a text box with id `id`: while its list of values is open, the
/// arrow keys, Enter, Tab and Escape go to the list (Left and Right too while
/// it lists blocks and their values).
pub fn before(ui: &Ui, id: Id) {
    let open = ui.ctx().data(|d| d.get_temp::<usize>(id.with("values-selected"))).is_some();
    if !open || scope(ui.ctx()).is_none() || !ui.ctx().memory(|m| m.has_focus(id)) {
        return;
    }
    let browsing = ui.ctx().data(|d| d.get_temp::<Nav>(id.with("values-nav"))).is_some_and(|n| n.browsing);
    let action = ui.ctx().input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
            Some(KeyAction::Down)
        } else if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            Some(KeyAction::Up)
        } else if i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::Tab) {
            Some(KeyAction::Pick)
        } else if i.consume_key(Modifiers::NONE, Key::Escape) {
            Some(KeyAction::Close)
        } else if browsing && i.consume_key(Modifiers::NONE, Key::ArrowLeft) {
            Some(KeyAction::Back)
        } else if browsing && i.consume_key(Modifiers::NONE, Key::ArrowRight) {
            Some(KeyAction::Into)
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

/// Puts `token` in place of the characters `from..to`, which are inside a
/// `${…}`. When that expression has no `}` yet, one is added and the cursor
/// goes after it; otherwise the cursor stays right after the token.
fn put(ctx: &egui::Context, id: Id, text: &mut String, from: usize, to: usize, token: &str) {
    let chars: Vec<char> = text.chars().collect();
    let closed = circuit_core::template::expression_at(&chars, from).is_some_and(|start| circuit_core::template::expression_end(&chars, start).is_some());
    let mut out: String = chars[..from].iter().collect();
    out.push_str(token);
    let mut cursor = from + token.chars().count();
    if !closed {
        out.push('}');
        cursor += 1;
    }
    out.extend(&chars[to..]);
    *text = out;
    set_cursor(ctx, id, cursor);
}

/// Puts a value in at the text cursor: the token alone inside a `${…}`,
/// otherwise as `${token}`.
fn put_at_cursor(ctx: &egui::Context, id: Id, text: &mut String, token: &str) {
    let at = cursor(ctx, id, text);
    let chars: Vec<char> = text.chars().collect();
    let at = at.min(chars.len());
    if circuit_core::template::expression_at(&chars, at).is_some() {
        put(ctx, id, text, at, at, token);
    } else {
        let piece = format!("${{{token}}}");
        let mut out: String = chars[..at].iter().collect();
        out.push_str(&piece);
        out.extend(&chars[at..]);
        *text = out;
        set_cursor(ctx, id, at + piece.chars().count());
    }
}

/// A step into the list of blocks: a block (by its place in the scope), or the
/// members of a class, reached through `token`.
#[derive(Clone, Debug, PartialEq)]
enum Step {
    Source(usize),
    Members { class: String, token: String, label: String },
}

/// Where the list is while it shows blocks and their values: the steps taken
/// (`steps[..level]`; later ones are kept while sliding back), for the brace
/// at `brace`.
#[derive(Clone, Debug, Default)]
struct Nav {
    steps: Vec<Step>,
    /// The row picked at each step, to come back to.
    picks: Vec<usize>,
    level: usize,
    brace: usize,
    browsing: bool,
}

/// One row of the list.
struct Row {
    /// The name, in code letters: `#1.user`, `username`.
    code: String,
    /// What it is.
    text: String,
    /// Its type, on the right.
    kind: String,
    /// What a pick puts between the braces.
    insert: Option<String>,
    /// Where the › leads.
    into: Option<Step>,
    doc: Option<String>,
}

/// The rows after `steps`: the blocks, a block's values, or a class's members.
fn rows_at(scope: &Scope, steps: &[Step]) -> Vec<Row> {
    match steps.last() {
        None => scope
            .sources
            .iter()
            .enumerate()
            .map(|(i, s)| Row { code: String::new(), text: s.title.clone(), kind: if s.values.len() == 1 { t!("1 value").to_string() } else { tf!("{count} values", count = s.values.len()) }, insert: None, into: Some(Step::Source(i)), doc: None })
            .collect(),
        Some(Step::Source(i)) => scope.sources.get(*i).map(|s| s.values.iter().map(|v| value_row(scope, v)).collect()).unwrap_or_default(),
        Some(Step::Members { class, token, .. }) => scope.members(class).iter().map(|m| member_row(scope, m, token)).collect(),
    }
}

fn value_row(scope: &Scope, v: &Avail) -> Row {
    let into = v.class.as_ref().filter(|c| !scope.members(c).is_empty()).map(|c| Step::Members { class: c.clone(), token: v.token.clone(), label: v.label.clone() });
    Row { code: v.token.clone(), text: v.label.clone(), kind: v.kind.clone(), insert: Some(v.token.clone()), into, doc: None }
}

fn member_row(scope: &Scope, m: &Member, prefix: &str) -> Row {
    // Methods go in called: ${#1.user.displayAvatarURL()}.
    let token = if m.kind == "method" { format!("{prefix}.{}()", m.name) } else { format!("{prefix}.{}", m.name) };
    let into = m.class.as_ref().filter(|c| !scope.members(c).is_empty()).map(|c| Step::Members { class: c.clone(), token: token.clone(), label: m.name.clone() });
    let code = if m.kind == "method" { format!("{}()", m.name) } else { m.name.clone() };
    Row { code, text: String::new(), kind: kind_text(m), insert: Some(token), into, doc: m.doc.clone() }
}

fn title_at(scope: &Scope, steps: &[Step]) -> String {
    match steps.last() {
        None => t!("Values from earlier blocks").into(),
        Some(Step::Source(i)) => scope.sources.get(*i).map(|s| s.title.clone()).unwrap_or_default(),
        Some(Step::Members { token, class, .. }) => format!("{token} · {class}"),
    }
}

/// What was typed after the brace decides the list.
enum Mode {
    /// Nothing yet: the blocks, sliding to their values and members.
    Browse,
    /// `{#1.user.na`: the members of what is before the last dot.
    Members { class: String, prefix: String, typed: String },
    /// Anything else: the values whose name or description has it.
    Search(String),
}

/// What the text after the brace means: a value (by its token, or an event
/// output by its own name) and then members, with what is typed after the last dot.
fn mode(scope: &Scope, typed: &str) -> Mode {
    if typed.is_empty() {
        return Mode::Browse;
    }
    let lower = typed.to_lowercase();
    // The longest value name the text continues with a dot.
    let base = scope
        .values()
        .flat_map(|v| std::iter::once((v.token.clone(), v)).chain(v.alias.clone().map(|a| (a, v))))
        .filter(|(name, v)| v.class.is_some() && lower.starts_with(&format!("{}.", name.to_lowercase())))
        .max_by_key(|(name, _)| name.len());
    let Some((name, v)) = base else { return Mode::Search(lower) };
    // Follow the members named after it, up to the last dot.
    let mut class = v.class.clone().unwrap();
    let mut prefix = v.token.clone();
    let rest = &typed[name.len() + 1..];
    let mut parts: Vec<&str> = rest.split('.').collect();
    let last = parts.pop().unwrap_or_default().to_lowercase();
    for part in parts {
        let Some(m) = scope.members(&class).iter().find(|m| m.name.eq_ignore_ascii_case(part)) else { return Mode::Search(lower) };
        let Some(next) = &m.class else { return Mode::Search(lower) };
        prefix = format!("{prefix}.{}", m.name);
        class = next.clone();
    }
    Mode::Members { class, prefix, typed: last }
}

/// After a text box: shows the list for what was typed after `{` (the blocks
/// before this one, their values and members), puts in the one picked, and
/// puts in a value clicked in a builder's list or the right-click menu.
/// Returns whether `text` changed.
pub fn after(ui: &mut Ui, resp: &egui::Response, text: &mut String) -> bool {
    let ctx = ui.ctx().clone();
    let id = resp.id;
    let Some(scope) = scope(&ctx) else { return false };

    // Right-click: pick a value from a menu, and it goes in at the text cursor.
    let mut picked = None;
    resp.context_menu(|ui| picked = menu(ui, &scope));
    let open = resp.context_menu_opened();
    ctx.data_mut(|d| d.insert_temp(id.with("values-menu-open"), open));
    if let Some(token) = picked {
        put_at_cursor(&ctx, id, text, &token);
        return true;
    }
    if resp.has_focus() {
        ctx.data_mut(|d| d.insert_temp(Id::new(LAST_TEXT), id));
    }
    // A value clicked in a list goes where this box's cursor was.
    if ctx.data(|d| d.get_temp::<Id>(Id::new(LAST_TEXT))) == Some(id)
        && let Some(token) = ctx.data_mut(|d| d.remove_temp::<String>(Id::new(PENDING)))
    {
        put_at_cursor(&ctx, id, text, &token);
        return true;
    }
    let selected_id = id.with("values-selected");
    let list_id = id.with("values-list-rect");
    let nav_id = id.with("values-nav");
    let key = ctx.data_mut(|d| d.remove_temp::<KeyAction>(id.with("values-key")));
    let close = |ctx: &egui::Context| {
        ctx.data_mut(|d| {
            d.remove::<usize>(selected_id);
            d.remove::<egui::Rect>(list_id);
            d.remove::<Nav>(nav_id);
        })
    };
    // Pressing on the list takes the text cursor from the box: the list stays
    // while the pointer is on it, so the click picks (and puts the cursor back).
    let was_open = ctx.data(|d| d.get_temp::<usize>(selected_id)).is_some();
    let on_list = ctx.data(|d| d.get_temp::<egui::Rect>(list_id)).is_some_and(|r| ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| r.contains(p)));
    if !resp.has_focus() && !(was_open && on_list) {
        close(&ctx);
        return false;
    }
    // The name being typed in a ${…} expression, up to the cursor.
    let chars: Vec<char> = text.chars().collect();
    let at = cursor(&ctx, id, text).min(chars.len());
    let Some(expr) = circuit_core::template::expression_at(&chars, at) else {
        close(&ctx);
        return false;
    };
    let mut start = at;
    while start > expr && (chars[start - 1].is_alphanumeric() || matches!(chars[start - 1], '_' | '$' | '.' | '#')) {
        start -= 1;
    }
    // Right after `${` the list shows the blocks; elsewhere in an expression it
    // shows once a name is being typed.
    let dismissed = ctx.data(|d| d.get_temp::<usize>(id.with("values-dismissed")));
    if dismissed == Some(start) || (start == at && start != expr) {
        close(&ctx);
        return false;
    }
    let typed: String = chars[start..at.min(chars.len())].iter().collect();
    let mode = mode(&scope, &typed);
    let mut nav = ctx.data(|d| d.get_temp::<Nav>(nav_id)).filter(|n| n.brace == start).unwrap_or(Nav { brace: start, ..Default::default() });
    nav.browsing = matches!(mode, Mode::Browse);
    let rows: Vec<Row> = match &mode {
        Mode::Browse => rows_at(&scope, &nav.steps[..nav.level]),
        Mode::Members { class, prefix, typed } => {
            let mut list: Vec<(u8, &Member)> = scope
                .members(class)
                .iter()
                .filter_map(|m| {
                    let name = m.name.to_lowercase();
                    if name.starts_with(typed.as_str()) {
                        Some((0, m))
                    } else if name.contains(typed.as_str()) {
                        Some((1, m))
                    } else {
                        None
                    }
                })
                .collect();
            list.sort_by_key(|(r, _)| *r);
            list.into_iter().map(|(_, m)| Row { into: None, ..member_row(&scope, m, prefix) }).collect()
        }
        Mode::Search(typed) => {
            // Best first: names that start with what was typed, then names with
            // a part that does, then names and descriptions that contain it.
            let rank = |v: &Avail| {
                let names: Vec<String> = std::iter::once(v.token.to_lowercase()).chain(v.alias.as_ref().map(|a| a.to_lowercase())).collect();
                if names.iter().any(|n| n.starts_with(typed.as_str())) {
                    Some(0)
                } else if names.iter().any(|n| n.split('.').any(|p| p.starts_with(typed.as_str()))) {
                    Some(1)
                } else if names.iter().any(|n| n.contains(typed.as_str())) || v.label.to_lowercase().contains(typed.as_str()) {
                    Some(2)
                } else {
                    None
                }
            };
            let mut ranked: Vec<(u8, &Avail)> = scope.values().filter_map(|v| rank(v).map(|r| (r, v))).collect();
            ranked.sort_by_key(|(r, _)| *r);
            ranked.into_iter().take(60).map(|(_, v)| Row { into: None, ..value_row(&scope, v) }).collect()
        }
    };
    if rows.is_empty() && !(nav.browsing && nav.level > 0) {
        close(&ctx);
        return false;
    }

    let count = rows.len().max(1);
    let mut selected = ctx.data(|d| d.get_temp::<usize>(selected_id)).unwrap_or(0).min(count - 1);
    // What to do: put a token in, go into a step, or back.
    let mut insert: Option<String> = None;
    let mut into: Option<Step> = None;
    let mut back = false;
    let activate = |row: &Row, insert: &mut Option<String>, into: &mut Option<Step>| match (&row.insert, &row.into) {
        (Some(t), _) => *insert = Some(t.clone()),
        (None, Some(s)) => *into = Some(s.clone()),
        _ => {}
    };
    match key {
        Some(KeyAction::Down) => selected = (selected + 1) % count,
        Some(KeyAction::Up) => selected = (selected + count - 1) % count,
        Some(KeyAction::Pick) => {
            if let Some(r) = rows.get(selected) {
                activate(r, &mut insert, &mut into);
            }
        }
        Some(KeyAction::Into) => into = rows.get(selected).and_then(|r| r.into.clone()),
        Some(KeyAction::Back) => back = nav.level > 0,
        Some(KeyAction::Close) => {
            ctx.data_mut(|d| d.insert_temp(id.with("values-dismissed"), start));
            close(&ctx);
            return false;
        }
        None => {}
    }

    // The list, under the box (on the canvas the box is in a zoomed layer).
    let rect = ctx.layer_transform_to_global(resp.layer_id).map_or(resp.rect, |t| t * resp.rect);
    let width = rect.width().clamp(340.0, 480.0);
    // While browsing, the list slides sideways between the steps.
    let slide = if nav.browsing { ctx.animate_value_with_time(id.with("values-slide"), nav.level as f32, 0.18) } else { nav.level as f32 };
    let area = egui::Area::new(id.with("values-list")).order(egui::Order::Foreground).fixed_pos(rect.left_bottom() + vec2(0.0, 4.0)).show(&ctx, |ui| {
        let v = ui.visuals().clone();
        egui::Frame::new().fill(v.window_fill).stroke(v.window_stroke).corner_radius(8).inner_margin(6).shadow(v.popup_shadow).show(ui, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 2.0;
            // The heading: where the list is, and the way back.
            let heading = match &mode {
                Mode::Browse => title_at(&scope, &nav.steps[..nav.level]),
                Mode::Members { prefix, class, .. } => format!("{prefix} · {class}"),
                Mode::Search(_) => t!("Values from earlier blocks").into(),
            };
            let can_back = nav.browsing && nav.level > 0;
            let head = ui.horizontal(|ui| {
                ui.set_width(width);
                let text = if can_back { format!("‹  {heading}") } else { heading };
                ui.add(egui::Label::new(RichText::new(text).size(11.5).strong().color(v.weak_text_color())).truncate());
            });
            let head = ui.interact(head.response.rect, id.with("values-back"), if can_back { egui::Sense::click() } else { egui::Sense::hover() });
            if can_back && head.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if head.clicked() {
                back = true;
            }
            // The rows of the steps on screen: the current one, and while
            // sliding the one it comes from.
            let levels: Vec<usize> = if nav.browsing { [slide.floor() as usize, slide.ceil() as usize].into_iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect() } else { vec![nav.level] };
            let tallest = levels.iter().map(|l| if *l == nav.level { rows.len() } else { rows_at(&scope, &nav.steps[..(*l).min(nav.steps.len())]).len() }).max().unwrap_or(1);
            let h = tallest.clamp(1, ROWS) as f32 * (ROW_H + 2.0);
            let (area, _) = ui.allocate_exact_size(vec2(width, h), egui::Sense::hover());
            for level in levels {
                let offset = (level as f32 - slide) * width;
                let current = level == nav.level;
                let shown: Vec<Row> = if current { Vec::new() } else { rows_at(&scope, &nav.steps[..level.min(nav.steps.len())]) };
                let list = if current { &rows } else { &shown };
                let mut child = ui.new_child(egui::UiBuilder::new().id_salt(("values-level", level)).max_rect(area.translate(vec2(offset, 0.0))).layout(egui::Layout::top_down(egui::Align::Min)));
                child.set_clip_rect(area.intersect(ui.clip_rect()));
                egui::ScrollArea::vertical().id_salt(("values-scroll", level, nav.steps.get(level.saturating_sub(1)).map(|s| format!("{s:?}")))).max_height(h).auto_shrink([false, false]).show(&mut child, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (i, row) in list.iter().enumerate() {
                        let (resp, go_into) = row_ui(ui, row, current && i == selected);
                        if current {
                            if i == selected && key.is_some() {
                                resp.scroll_to_me(None);
                            }
                            if go_into {
                                into = row.into.clone();
                            } else if resp.clicked() {
                                activate(row, &mut insert, &mut into);
                            }
                        }
                    }
                    if list.is_empty() {
                        ui.label(RichText::new(t!("Nothing here")).size(12.0).color(ui.visuals().weak_text_color()));
                    }
                });
            }
            if matches!(mode, Mode::Browse) || matches!(mode, Mode::Search(_)) {
                ui.label(RichText::new(t!("Enter puts it in · › goes into it · type . after a value for its members")).size(10.5).color(v.weak_text_color()));
            }
        });
    });
    ctx.data_mut(|d| {
        d.insert_temp(list_id, area.response.rect);
        d.insert_temp(Id::new(OPEN_LIST), area.response.rect);
    });
    if let Some(token) = insert {
        put(&ctx, id, text, start, at, &token);
        close(&ctx);
        return true;
    }
    if let Some(step) = into {
        // Going into a value from a search or member list starts browsing there.
        nav.steps.truncate(nav.level);
        nav.steps.push(step);
        nav.picks.truncate(nav.level);
        nav.picks.push(selected);
        nav.level += 1;
        selected = 0;
        if !nav.browsing {
            // Browsing needs the brace with nothing after it.
            put_browse(&ctx, id, text, start, at);
        }
        ctx.request_repaint();
    } else if back {
        nav.level -= 1;
        selected = nav.picks.get(nav.level).copied().unwrap_or(0);
        ctx.request_repaint();
    }
    ctx.data_mut(|d| {
        d.insert_temp(selected_id, selected);
        d.insert_temp(nav_id, nav);
    });
    false
}

/// Clears what was typed after the brace, so the list browses from there.
fn put_browse(ctx: &egui::Context, id: Id, text: &mut String, start: usize, at: usize) {
    let chars: Vec<char> = text.chars().collect();
    let mut out: String = chars[..start].iter().collect();
    out.extend(&chars[at.min(chars.len())..]);
    *text = out;
    set_cursor(ctx, id, start);
}

/// One row: its name in code letters, what it is, its type on the right, and
/// a › to go into it. Returns the row's response and whether the › was clicked.
fn row_ui(ui: &mut Ui, row: &Row, selected: bool) -> (egui::Response, bool) {
    let visuals = ui.visuals().clone();
    let accent = visuals.selection.stroke.color;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), egui::Sense::click());
    let fill = if selected {
        crate::ui::theme::with_alpha(accent, 45)
    } else if resp.hovered() {
        crate::ui::theme::with_alpha(visuals.text_color(), 14)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 5.0, fill);
    let mid = rect.center().y;
    let mut x = rect.left() + 8.0;
    let painter = ui.painter().with_clip_rect(rect);
    if !row.code.is_empty() {
        let g = painter.layout_no_wrap(row.code.clone(), egui::FontId::monospace(12.0), accent);
        painter.galley(egui::pos2(x, mid - g.size().y / 2.0), g.clone(), accent);
        x += g.size().x + 10.0;
    }
    let arrow_w = if row.into.is_some() { 22.0 } else { 0.0 };
    let kind = painter.layout_no_wrap(row.kind.clone(), egui::FontId::proportional(11.5), visuals.weak_text_color());
    let kind_x = rect.right() - 8.0 - arrow_w - kind.size().x;
    painter.galley(egui::pos2(kind_x, mid - kind.size().y / 2.0), kind, visuals.weak_text_color());
    if !row.text.is_empty() && kind_x - x > 30.0 {
        let job = egui::text::LayoutJob::simple_singleline(row.text.clone(), egui::FontId::proportional(12.5), if row.code.is_empty() { visuals.text_color() } else { visuals.weak_text_color() });
        let mut job = job;
        job.wrap = egui::text::TextWrapping::truncate_at_width(kind_x - x - 8.0);
        let g = painter.layout_job(job);
        painter.galley(egui::pos2(x, mid - g.size().y / 2.0), g, visuals.text_color());
    }
    let mut go_into = false;
    if row.into.is_some() {
        let arrow = egui::Rect::from_min_size(egui::pos2(rect.right() - arrow_w - 4.0, rect.top()), vec2(arrow_w, ROW_H));
        let a = ui.interact(arrow, resp.id.with("into"), egui::Sense::click());
        let color = if a.hovered() { accent } else { visuals.weak_text_color() };
        painter.text(arrow.center(), egui::Align2::CENTER_CENTER, "›", egui::FontId::proportional(16.0), color);
        go_into = a.clicked();
        if a.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let resp = match &row.doc {
        Some(doc) => resp.on_hover_text(doc),
        None => resp,
    };
    (resp, go_into)
}

/// The values list of a builder: each block's values (type `.` after one in a
/// text for its members); clicking one puts it where the text cursor was.
pub fn list(ui: &mut Ui, theme: &Theme, scope: &Scope) {
    if scope.sources.is_empty() {
        ui.label(RichText::new(t!("Nothing comes before this block yet: connect its Action input to an event or other blocks, and their values show up here.")).size(12.0).color(theme.text_muted));
        return;
    }
    let tip = if has_target(ui.ctx()) { t!("Click a value to put it where the text cursor is, or type ${ in any text.") } else { t!("Click into a text, then a value to put it there. Or type ${ in any text.") };
    ui.label(RichText::new(tip).size(12.0).color(theme.text_muted));
    for source in &scope.sources {
        ui.add_space(4.0);
        ui.label(RichText::new(&source.title).size(11.5).strong().color(theme.text_muted));
        for v in &source.values {
            let row = Row { into: None, ..value_row(scope, v) };
            if row_ui(ui, &row, false).0.clicked() {
                insert(ui.ctx(), &v.token);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The {name} inputs a block's shape has, by name.
    fn inputs(ed: &mut crate::editor::Editor, ws_key: &str, node: &str) -> Vec<String> {
        let crate::editor::Editor { reg, engine, project, shapes, define_ctx, .. } = ed;
        let ws = &project.as_ref().unwrap().workspaces[ws_key];
        shapes.begin();
        let shape = crate::editor::node_shape(reg, engine, shapes, ws_key, ws, node, define_ctx).unwrap();
        shape.fields.clone().into_iter().filter(|(_, f)| f.label.as_deref().is_some_and(|l| l.starts_with("${"))).map(|(k, _)| k).collect()
    }

    #[test]
    fn names_from_earlier_blocks_need_no_input() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = crate::editor::Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        let set = |ed: &mut crate::editor::Editor, ws: &str, node: &str, key: &str, text: &str| {
            ed.project.as_mut().unwrap().workspaces.get_mut(ws).unwrap().nodes.get_mut(node).unwrap().values.insert(key.into(), text.into());
        };
        // After a Slash Command, user and interaction come from the event; score doesn't.
        set(&mut ed, "rank-command", "4", "text", "Hi ${user} ${interaction} ${score}");
        let send = inputs(&mut ed, "rank-command", "4");
        assert_eq!(send, ["score"]);
        // The same in an embed's texts; {xp} and {level} are wired, so they stay.
        let embed = inputs(&mut ed, "rank-command", "3");
        assert_eq!(embed, ["xp", "level"]);
        // After Member Joins there is no interaction, so it gets an input there.
        set(&mut ed, "welcome", "3", "message", "Welcome ${member} ${interaction}");
        let log = inputs(&mut ed, "welcome", "3");
        assert_eq!(log, ["interaction"]);
    }

    #[test]
    fn values_come_from_the_action_path() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = crate::editor::Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        // Send Message (#4) in /rank: after the Slash Command (#1) and Build Embed (#3).
        let scope = ed.values_for("rank-command", "4");
        let titles: Vec<&str> = scope.sources.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["Slash Command · #1", "Build Embed · #3"], "the event first, then the blocks on the way");
        let user = scope.values().find(|v| v.token == "#1.user").expect("event outputs by number");
        assert_eq!((user.alias.as_deref(), user.class.as_deref()), (Some("user"), Some("User")));
        assert!(scope.members("User").iter().any(|m| m.name == "username" && m.ty == "Text"), "members from discord.js");
        assert!(scope.values().any(|v| v.token == "#3.embed"));
        // Get Stored Value (#2) is a value block: it gets what the block it feeds gets.
        let fed = ed.values_for("rank-command", "2");
        assert!(fed.values().any(|v| v.token == "#1.user"));
    }

    #[test]
    fn typing_after_a_dot_lists_members() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = crate::editor::Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        let scope = ed.values_for("rank-command", "4");
        let members = |typed: &str| match mode(&scope, typed) {
            Mode::Members { class, prefix, typed } => (class, prefix, typed),
            _ => panic!("{typed}: no members"),
        };
        assert_eq!(members("#1.user.na"), ("User".into(), "#1.user".into(), "na".into()));
        // An event output's own name works too, and so do members of members.
        assert_eq!(members("member.roles.hi"), ("GuildMemberRoleManager".into(), "#1.member.roles".into(), "hi".into()));
        assert!(matches!(mode(&scope, ""), Mode::Browse));
        assert!(matches!(mode(&scope, "us"), Mode::Search(_)));
    }
}
