//! The Workspaces panel: the project's workspaces as a tree with folders (in
//! folders), and its slash commands. Drag a workspace onto a folder to move it
//! there, or above or below another workspace to put it there (a line shows
//! where); drag a folder onto the middle of another to put it in there, or
//! onto its top or bottom edge to put it above or below. Double-click a name
//! to rename it. Each workspace shows its problems, whether its blocks just
//! ran (or failed), and whether it's turned off.

use std::collections::HashMap;
use std::path::PathBuf;

use circuit_core::Workspace;
use eframe::egui::{self, Align2, Color32, CursorIcon, FontId, Id, Key, Rect, Response, RichText, ScrollArea, Sense, Stroke, Ui, pos2, vec2};

use crate::ui::kit;
use crate::editor::folders;
use crate::editor::{Action, Editor, file_uri};
use crate::ui::theme::{Theme, with_alpha};

const ROW_H: f32 = 26.0;
const ROW_TOUCH_H: f32 = 40.0;
const INDENT: f32 = 16.0;
const ICON: f32 = 14.0;
const COLLAPSED_KEY: &str = "sidebar-collapsed";
/// Collapse keys that aren't folder names.
const PROJECT_ROW: &str = "\u{0}project";
const SLASH_ROW: &str = "\u{0}slash";

#[derive(Clone, PartialEq)]
enum Target {
    Workspace(String),
    Folder(String),
    /// A folder of slash commands.
    SlashFolder(String),
}

/// A name being edited in place.
#[derive(Clone)]
struct Renaming {
    target: Target,
    text: String,
    fresh: bool,
}

enum Do {
    Open(String),
    StartRename(Target, String),
    Rename(Target, String),
    Move(String, Option<String>),
    /// A workspace dropped next to another: its new folder, the other one,
    /// and whether it goes before (true) or after it.
    Place(String, Option<String>, String, bool),
    /// A folder dropped before (true) or after another folder.
    PlaceFolder(String, String, bool),
    /// A folder dropped into another one (or onto the project: to the top).
    NestFolder(String, Option<String>),
    /// Turns a workspace on (true) or off.
    SetEnabled(String, bool),
    DeleteWorkspace(String),
    /// Ask before deleting these workspaces.
    AskDelete(Vec<String>),
    /// A click on a workspace: it alone, Ctrl (add or take it out) or Shift (from the last one clicked).
    Select(String, Pick),
    Duplicate(String),
    NewWorkspace(Option<String>),
    /// A new folder, in a folder or at the top.
    NewFolder(Option<String>),
    DeleteFolder(String),
    Toggle(String),
    /// Opens a closed folder (something dragged was held over it).
    Expand(String),
    SlashCommands,
    /// Slash Commands, opened on a command (its first word).
    SlashOpen(String),
    /// A command moved into a folder (or none), before (true) or after
    /// another command, or where it was.
    SlashPlace(String, Option<String>, Option<(String, bool)>),
    /// Turns a command on (true) or off.
    SlashEnable(String, bool),
    SlashNewFolder(Option<String>),
    SlashDeleteFolder(String),
    /// A command folder dropped before (true) or after another.
    SlashPlaceFolder(String, String, bool),
    /// A command folder dropped into another one (or onto Slash commands: to the top).
    SlashNestFolder(String, Option<String>),
    Export(crate::editor::exchange::Preselect),
    Import,
}

/// What a dragged folder carries (a dragged workspace carries its key).
#[derive(Clone)]
struct FolderDrag(String);

/// What a dragged slash command carries: its first word.
#[derive(Clone)]
struct SlashDrag(String);

/// What a dragged folder of slash commands carries.
#[derive(Clone)]
struct SlashFolderDrag(String);

/// The key a folder of slash commands is opened and closed by (apart from workspace folders).
fn slash_folder_key(folder: &str) -> String {
    format!("\u{0}slash:{folder}")
}

/// Whether something dropped on `rect` goes before it: the pointer is in its upper half.
fn drops_before(ui: &Ui, rect: Rect) -> bool {
    ui.ctx().pointer_hover_pos().is_some_and(|p| p.y < rect.center().y)
}

/// The line that shows where a dragged row will land: above or below `rect`,
/// starting where that row's content starts (`depth`), so it's clear which
/// folder it lands in.
fn insertion_line(ui: &Ui, theme: &Theme, rect: Rect, before: bool, depth: usize) {
    let y = if before { rect.top() + 1.0 } else { rect.bottom() - 1.0 };
    let left = rect.left() + 10.0 + depth as f32 * INDENT;
    ui.painter().hline(left..=rect.right() - 8.0, y, Stroke::new(2.0, theme.accent));
    ui.painter().circle_stroke(pos2(left, y), 3.5, Stroke::new(2.0, theme.accent));
}

/// A row something dragged goes into (a folder, or the project): filled and outlined.
fn into_target(ui: &Ui, theme: &Theme, rect: Rect) {
    ui.painter().rect_filled(rect, 4.0, with_alpha(theme.accent, 55));
    ui.painter().rect_stroke(rect.shrink(1.0), 4.0, Stroke::new(1.5, theme.accent), egui::StrokeKind::Inside);
}

/// What letting go would do, shown beside the pointer (see `drag_preview`);
/// `ok` false when it can't go there.
#[derive(Clone)]
struct DropHint {
    text: String,
    ok: bool,
}

const HINT_KEY: &str = "sidebar-drop-hint";

fn hint(ui: &Ui, text: impl Into<String>, ok: bool) {
    ui.ctx().data_mut(|d| d.insert_temp(Id::unique(HINT_KEY), DropHint { text: text.into(), ok }));
}

/// Whether something has been held over closed folder row `resp` (opened
/// and closed by `key`) for a moment: then it opens, to drop into a folder inside.
fn held_over(ui: &Ui, resp: &Response, key: &str, open: bool) -> bool {
    let id = Id::unique("sidebar-held-over");
    if open || !egui::DragAndDrop::has_any_payload(ui.ctx()) || !resp.contains_pointer() {
        if ui.ctx().data(|d| d.get_temp::<(String, f64)>(id)).is_some_and(|(k, _)| k == key) {
            ui.ctx().data_mut(|d| d.remove::<(String, f64)>(id));
        }
        return false;
    }
    let now = ui.input(|i| i.time);
    let since = match ui.ctx().data(|d| d.get_temp::<(String, f64)>(id)) {
        Some((k, t)) if k == key => t,
        _ => {
            ui.ctx().data_mut(|d| d.insert_temp(id, (key.to_string(), now)));
            now
        }
    };
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
    now - since > 0.7
}

/// Where a folder dropped on another folder's row goes.
#[derive(Clone, Copy, PartialEq)]
enum Zone {
    Before,
    Into,
    After,
}

/// Where a folder dropped on folder row `rect` goes: above it (the row's top
/// third), below it (its bottom third), or into it.
fn folder_zone(ui: &Ui, rect: Rect) -> Zone {
    let y = ui.ctx().pointer_hover_pos().map_or(rect.center().y, |p| p.y);
    let third = rect.height() / 3.0;
    if y < rect.top() + third {
        Zone::Before
    } else if y > rect.bottom() - third {
        Zone::After
    } else {
        Zone::Into
    }
}

/// A folder (dragged as `D`, which `path_of` gives the path of) over folder
/// row `resp` (at `path`): shows where it would land, and on release says
/// which folder and where. Not into itself or a folder inside it.
fn folder_drop<D: Send + Sync + 'static>(ui: &Ui, theme: &Theme, resp: &Response, path: &str, path_of: impl Fn(&D) -> &str) -> Option<(String, Zone)> {
    if let Some(dragged) = resp.dnd_hover_payload::<D>() {
        let dragged = path_of(&dragged);
        let name = folders::name(path);
        if folders::within(path, dragged) {
            if path != dragged {
                hint(ui, t!("Not into a folder inside itself"), false);
            }
        } else {
            match folder_zone(ui, resp.rect) {
                Zone::Into => {
                    into_target(ui, theme, resp.rect);
                    hint(ui, tf!("Into {folder}", folder = name), true);
                }
                zone => {
                    insertion_line(ui, theme, resp.rect, zone == Zone::Before, folders::depth(path) + 1);
                    let place = if zone == Zone::Before { tf!("Above {name}", name = name) } else { tf!("Below {name}", name = name) };
                    hint(ui, place, true);
                }
            }
        }
    }
    if egui::DragAndDrop::has_payload_of_type::<D>(ui.ctx())
        && let Some(dragged) = resp.dnd_release_payload::<D>()
        && !folders::within(path, path_of(&dragged))
    {
        return Some((path_of(&dragged).to_string(), folder_zone(ui, resp.rect)));
    }
    None
}

/// The tree below the project: each folder, then the folders in it, then
/// its workspaces (or commands).
enum Entry {
    Folder(String),
    Inside(String),
}

fn tree(all: &[String], parent: Option<&str>, out: &mut Vec<Entry>) {
    for f in folders::children(all, parent) {
        out.push(Entry::Folder(f.clone()));
        tree(all, Some(f), out);
        out.push(Entry::Inside(f.clone()));
    }
}

/// The folders `path` is in (and itself, with `inclusive`), outermost first.
fn around(path: &str, inclusive: bool) -> impl Iterator<Item = &str> {
    let ends = path.match_indices(folders::SEP).map(|(i, _)| i);
    ends.chain(inclusive.then_some(path.len())).map(move |i| &path[..i])
}

/// The project's workspace folders, every one, in the tree's order.
fn ws_folders(project: &circuit_core::Project) -> Vec<String> {
    folders::ordered(&project.meta.folders, project.workspaces.values().filter_map(|w| w.folder.as_deref()))
}

/// Icon files by name, looked up before the project is borrowed.
struct Icons {
    paths: std::collections::HashMap<&'static str, PathBuf>,
}

impl Icons {
    fn new(ed: &Editor) -> Icons {
        let names = ["workspace", "folder", "folder-plus", "plus", "slash", "more", "pencil", "copy", "eye", "eye-off", "package", "trash", "pointer", "alert"];
        Icons { paths: names.iter().filter_map(|n| Some((*n, ed.reg.icons.get(*n)?.clone()))).collect() }
    }

    fn paint(&self, ui: &Ui, name: &str, center: egui::Pos2, size: f32, tint: Color32) {
        if let Some(path) = self.paths.get(name) {
            egui::Image::from_uri(file_uri(path)).tint(tint).paint_at(ui, Rect::from_center_size(center, vec2(size, size)));
        }
    }
}

/// A menu entry with its icon.
fn item(ui: &mut Ui, theme: &Theme, icons: &Icons, icon: &str, text: impl Into<egui::WidgetText>, danger: bool) -> Response {
    let tint = if danger { theme.error } else { theme.text_muted };
    kit::menu_icon_item(ui, icons.paths.get(icon).map(|p| p.as_path()), tint, text)
}

/// A small rounded label drawn right-aligned at `right`; gives where it starts.
fn pill(ui: &Ui, right: f32, y: f32, text: &str, fill: Color32, ink: Color32) -> f32 {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(10.5), ink);
    let size = vec2((galley.size().x + 10.0).max(18.0), 16.0);
    let rect = Rect::from_min_size(pos2(right - size.x, y - size.y / 2.0), size);
    ui.painter().rect_filled(rect, 8.0, fill);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, ink);
    rect.left()
}

/// A problem count: a warning triangle and the number on an amber pill. Returns its left edge.
fn problems_pill(ui: &Ui, icons: &Icons, theme: &Theme, right: f32, y: f32, count: usize) -> f32 {
    let ink = Color32::from_rgb(0x1E, 0x1F, 0x22);
    let galley = ui.painter().layout_no_wrap(count.to_string(), FontId::proportional(10.5), ink);
    let size = vec2(galley.size().x + 24.0, 16.0);
    let rect = Rect::from_min_size(pos2(right - size.x, y - size.y / 2.0), size);
    ui.painter().rect_filled(rect, 8.0, theme.warning);
    icons.paint(ui, "alert", pos2(rect.left() + 10.0, y), 11.0, ink);
    ui.painter().galley(pos2(rect.left() + 17.0, y - galley.size().y / 2.0), galley, ink);
    rect.left()
}

/// How a click on a workspace changes the selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pick {
    One,
    Toggle,
    Range,
}

/// Workspaces in the order the list showed them last, for Shift-click.
const ORDER_KEY: &str = "sidebar-ws-order";

/// "Delete them?" for the workspaces Delete (or the menu) picked.
pub fn delete_dialog(ctx: &egui::Context, ed: &mut Editor) {
    let Some(keys) = ed.ask_delete_workspaces.clone() else { return };
    let names: Vec<String> = keys.iter().filter_map(|k| ed.project.as_ref()?.workspaces.get(k).map(|w| w.name.clone())).collect();
    if names.is_empty() {
        ed.ask_delete_workspaces = None;
        return;
    }
    let theme = ed.theme.clone();
    let title = match names.len() {
        1 => tf!("Delete {name}?", name = names[0]),
        n => tf!("Delete {count} workspaces?", count = n),
    };
    let (mut delete, mut cancel) = (false, false);
    let close = crate::ui::dialog::show(ctx, &theme, crate::ui::dialog::Spec { id: "delete-workspaces", title: &title, subtitle: None, icon: None, width: 440.0 }, |ui, part| match part {
        crate::ui::dialog::Part::Body => {
            if names.len() > 1 {
                ui.label(RichText::new(names.join(", ")).color(theme.text));
                ui.add_space(4.0);
            }
            ui.label(RichText::new(t!("Their blocks and notes go with them. Until you save, closing the project without saving brings them back; backups keep earlier versions too.")).color(theme.text_muted));
        }
        crate::ui::dialog::Part::Footer => {
            delete = kit::danger_button(ui, &theme, t!("Delete")).clicked() || ui.input(|i| i.key_pressed(Key::Enter));
            cancel = kit::secondary_button(ui, &theme, t!("Cancel")).clicked();
        }
    });
    if delete {
        ed.ask_delete_workspaces = None;
        apply(ed, ctx, keys.into_iter().map(Do::DeleteWorkspace).collect(), &mut None);
    } else if cancel || close {
        ed.ask_delete_workspaces = None;
    }
}

/// What the list shows about a workspace besides its name.
#[derive(Default, Clone, Copy)]
struct Marks {
    problems: usize,
    /// Its blocks ran in the last two seconds.
    live: bool,
    /// One of them failed in the last 30 seconds.
    failed: bool,
    off: bool,
}

impl Marks {
    /// The room they take at the row's right end, before the ⋯ button.
    fn width(&self) -> f32 {
        (if self.off { 30.0 } else { 0.0 }) + (if self.problems > 0 { 38.0 } else { 0.0 }) + (if self.live || self.failed { 14.0 } else { 0.0 })
    }

    /// Paints them, right to left from `right`.
    fn paint(&self, ui: &Ui, theme: &Theme, icons: &Icons, right: f32, y: f32) {
        let mut x = right;
        if self.live || self.failed {
            let color = if self.failed { theme.error } else { theme.action };
            ui.painter().circle_filled(pos2(x - 5.0, y), 4.0, color);
            if self.live {
                ui.painter().circle_stroke(pos2(x - 5.0, y), 6.5, Stroke::new(1.0, with_alpha(color, 110)));
            }
            x -= 14.0;
        }
        if self.problems > 0 {
            x = problems_pill(ui, icons, theme, x, y, self.problems) - 4.0;
        }
        if self.off {
            pill(ui, x, y, t!("off"), theme.field, theme.text_muted);
        }
    }
}

/// The arrow that shows whether a folder is open.
fn chevron(ui: &Ui, center: egui::Pos2, open: bool, color: Color32) {
    let s = 3.5;
    let pts = if open { [center + vec2(-s, -s * 0.5), center + vec2(0.0, s * 0.5), center + vec2(s, -s * 0.5)] } else { [center + vec2(-s * 0.5, -s), center + vec2(s * 0.5, 0.0), center + vec2(-s * 0.5, s)] };
    ui.painter().line(pts.to_vec(), Stroke::new(1.5, color));
}

/// One tree row. Returns its response (click, double-click, drag, drop, context menu).
#[allow(clippy::too_many_arguments)]
fn row(ui: &mut Ui, theme: &Theme, icons: &Icons, id: Id, depth: usize, open: Option<bool>, icon: Option<&str>, label: &str, active: bool, strong: bool, right: f32, dim: bool) -> (Response, Rect) {
    let h = if theme.touch { ROW_TOUCH_H } else { ROW_H };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    match open {
        // A folder (or the project): open or closed.
        Some(open) => {
            resp.widget_info(|| egui::WidgetInfo { selected: Some(active), ..egui::WidgetInfo::labeled(egui::Role::TreeItem, true, label) });
            crate::ui::kit::accesskit_expanded(&resp, open);
        }
        None => crate::ui::kit::describe_selected(&resp, egui::Role::TreeItem, active, label),
    }
    let being_dragged = resp.dragged() && egui::DragAndDrop::has_any_payload(ui.ctx());
    if active {
        ui.painter().rect_filled(rect, 0.0, with_alpha(theme.accent, 38));
        ui.painter().rect_filled(Rect::from_min_size(rect.min, vec2(2.0, rect.height())), 0.0, theme.accent);
    } else if resp.hovered() && !egui::DragAndDrop::has_any_payload(ui.ctx()) {
        ui.painter().rect_filled(rect, 0.0, theme.panel_header);
    }
    let mut x = rect.left() + 10.0 + depth as f32 * INDENT;
    if let Some(o) = open {
        chevron(ui, pos2(x + 4.0, rect.center().y), o, theme.text_muted);
    }
    if depth > 0 || open.is_some() {
        x += 14.0;
    }
    if let Some(name) = icon {
        icons.paint(ui, name, pos2(x + ICON / 2.0, rect.center().y), ICON, if active { theme.accent } else { theme.text_muted });
        x += ICON + 7.0;
    }
    let font = if strong { FontId::proportional(11.5) } else { FontId::proportional(13.0) };
    let color = if strong || dim { theme.text_muted } else { theme.text };
    let text = if strong { label.to_uppercase() } else { label.to_string() };
    let galley = ui.painter().layout_no_wrap(text, font, color);
    // Long names stop before the marks and buttons on the right.
    let clip = Rect::from_min_max(pos2(x, rect.top()), pos2((rect.right() - 4.0 - right).max(x), rect.bottom()));
    ui.painter().with_clip_rect(clip).galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, color);
    // The row being dragged stays in its place, faded, with a dashed outline.
    if being_dragged {
        ui.painter().rect_filled(rect, 0.0, with_alpha(theme.panel, 170));
        let r = rect.shrink2(vec2(4.0, 2.0));
        let dash = Stroke::new(1.0, with_alpha(theme.accent, 160));
        for (a, b) in [(r.left_top(), r.right_top()), (r.left_bottom(), r.right_bottom()), (r.left_top(), r.left_bottom()), (r.right_top(), r.right_bottom())] {
            ui.painter().extend(egui::Shape::dashed_line(&[a, b], dash, 4.0, 3.0));
        }
    }
    (resp, Rect::from_min_max(pos2(x - 3.0, rect.top() + 2.0), pos2(rect.right() - 6.0, rect.bottom() - 2.0)))
}

/// A small icon button drawn over the right end of a row.
fn row_button(ui: &mut Ui, theme: &Theme, icons: &Icons, rect: Rect, id: Id, icon: &str, tip: &str) -> egui::Response {
    let resp = ui.interact(rect, id, Sense::click());
    crate::ui::kit::describe(&resp, egui::Role::Button, tip);
    let resp = resp.on_hover_text(tip).on_hover_cursor(CursorIcon::PointingHand);
    if resp.hovered() || egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp)) {
        ui.painter().rect_filled(rect, 4.0, theme.field);
    }
    icons.paint(ui, icon, rect.center(), 14.0, if resp.hovered() { theme.text } else { theme.text_muted });
    resp
}

/// The ⋯ button on the right of a row, which opens the row's menu (the same
/// one right-click opens).
fn more_button(ui: &mut Ui, theme: &Theme, icons: &Icons, row: Rect, id: Id) -> egui::Response {
    let size = if theme.touch { vec2(36.0, 34.0) } else { vec2(24.0, 22.0) };
    row_button(ui, theme, icons, Rect::from_center_size(pos2(row.right() - size.x / 2.0 - 4.0, row.center().y), size), id, "more", t!("More"))
}

/// The name editor shown in place of a row's label.
fn rename_box(ui: &mut Ui, area: Rect, state: &mut Renaming) -> Option<bool> {
    let id = Id::unique("sidebar-rename");
    let resp = ui.put(area, egui::TextEdit::singleline(&mut state.text).id(id).margin(egui::Margin::symmetric(4, 2))).accessible_name(t!("Rename"));
    if state.fresh {
        resp.request_focus();
        state.fresh = false;
        return None;
    }
    if ui.input(|i| i.key_pressed(Key::Escape)) {
        return Some(false);
    }
    if resp.lost_focus() {
        return Some(true);
    }
    None
}

/// The Workspaces panel. `problems` counts the Problems panel's entries per workspace.
pub fn panel(ui: &mut Ui, ed: &mut Editor, problems: &HashMap<&str, usize>) {
    let theme = ed.theme.clone();
    let icons = Icons::new(ed);
    let Some(project) = ed.project.as_ref() else {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
            ui.label(RichText::new(t!("No project open")).color(theme.text_muted));
            ui.label(t!("Use File › Open Project or File › New Project."));
        });
        return;
    };
    let ctx = ui.ctx().clone();
    let collapsed: Vec<String> = ctx.data_mut(|d| d.get_persisted::<Vec<String>>(Id::unique(COLLAPSED_KEY)).unwrap_or_default());
    let is_open = |key: &str| !collapsed.iter().any(|c| c == key);
    let mut renaming: Option<Renaming> = ctx.data(|d| d.get_temp(Id::unique("sidebar-renaming")));
    let mut todo: Vec<Do> = Vec::new();
    // Selected workspaces (Ctrl- and Shift-click), and the order rows are drawn in, for Shift.
    ed.ws_list_frame = ctx.cumulative_frame_nr();
    let selected = ed.ws_selection.clone();
    let order = std::cell::RefCell::new(Vec::<String>::new());
    // Each workspace's events, for its hover text: counted again only after
    // a change, not for every block of every workspace each frame.
    let events_id = Id::unique("sidebar-event-counts");
    let events_kept = std::cell::RefCell::new(ctx.data_mut(|d| std::mem::take(d.get_temp_mut_or_default::<HashMap<String, (u64, usize)>>(events_id))));
    let events_stamp = |ws: &circuit_core::Workspace| {
        let mut h = crate::editor::Fingerprint::default();
        std::hash::Hash::hash(&(ed.history.revision(), ed.reloads, ws.nodes.len()), &mut h);
        std::hash::Hasher::finish(&h)
    };
    let list_area = ui.max_rect();
    ctx.data_mut(|d| d.remove::<DropHint>(Id::unique(HINT_KEY)));

    // Folders in the tree's order, with any a workspace names that the list lacks.
    let all_folders = ws_folders(project);
    let active = ed.active_workspace.clone();
    // What the running bot did lately, per workspace: when its blocks last ran, and whether one failed lately.
    let mut ran: HashMap<&str, (std::time::Instant, bool, Option<&str>)> = HashMap::new();
    for ((ws, _), a) in &ed.activity {
        let failed = a.error.is_some() && a.at.elapsed() < crate::bot::TIME_SHOWN;
        let e = ran.entry(ws.as_str()).or_insert((a.at, false, None));
        if a.at > e.0 {
            e.0 = a.at;
        }
        if failed {
            e.1 = true;
            e.2 = a.error.as_deref();
        }
    }
    if ran.values().any(|(at, _, _)| at.elapsed() < crate::bot::GLOW) {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
    }
    let reg = &ed.reg;

    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add_space(4.0);

        // The project: collapses everything; dropping a workspace here takes it out of its folder.
        let (resp, _) = row(ui, &theme, &icons, Id::unique("sidebar-project"), 0, Some(is_open(PROJECT_ROW)), None, &project.meta.name, false, true, 54.0, false);
        let buttons = resp.rect;
        if row_button(ui, &theme, &icons, Rect::from_center_size(pos2(buttons.right() - 16.0, buttons.center().y), vec2(22.0, 22.0)), Id::unique("sidebar-new-folder"), "folder-plus", t!("New folder")).clicked() {
            todo.push(Do::NewFolder(None));
        } else if row_button(ui, &theme, &icons, Rect::from_center_size(pos2(buttons.right() - 40.0, buttons.center().y), vec2(22.0, 22.0)), Id::unique("sidebar-new-ws"), "plus", t!("New workspace")).clicked() {
            todo.push(Do::NewWorkspace(None));
        } else if resp.clicked() {
            todo.push(Do::Toggle(PROJECT_ROW.into()));
        }
        if resp.dnd_hover_payload::<String>().is_some() {
            into_target(ui, &theme, resp.rect);
            hint(ui, t!("Out of its folder, to the top"), true);
        } else if resp.dnd_hover_payload::<FolderDrag>().is_some() {
            into_target(ui, &theme, resp.rect);
            hint(ui, t!("To the top"), true);
        }
        if egui::DragAndDrop::has_payload_of_type::<String>(ui.ctx())
            && let Some(key) = resp.dnd_release_payload::<String>()
        {
            todo.push(Do::Move((*key).clone(), None));
        }
        // A folder dropped here goes to the top.
        if egui::DragAndDrop::has_payload_of_type::<FolderDrag>(ui.ctx())
            && let Some(dragged) = resp.dnd_release_payload::<FolderDrag>()
        {
            todo.push(Do::NestFolder(dragged.0.clone(), None));
        }
        resp.context_menu(|ui| {
            ui.set_min_width(200.0);
            if item(ui, &theme, &icons, "plus", t!("New workspace"), false).clicked() {
                todo.push(Do::NewWorkspace(None));
                ui.close();
            }
            if item(ui, &theme, &icons, "folder-plus", t!("New folder"), false).clicked() {
                todo.push(Do::NewFolder(None));
                ui.close();
            }
            ui.separator();
            if item(ui, &theme, &icons, "folder", t!("Import workspaces…"), false).clicked() {
                todo.push(Do::Import);
                ui.close();
            }
            if ui.add_enabled_ui(!project.workspaces.is_empty(), |ui| item(ui, &theme, &icons, "package", t!("Export all workspaces…"), false)).inner.clicked() {
                todo.push(Do::Export(crate::editor::exchange::Preselect::All));
                ui.close();
            }
        });

        if is_open(PROJECT_ROW) {
            let workspace_row = |ui: &mut Ui, key: &String, ws: &Workspace, depth: usize, todo: &mut Vec<Do>, renaming: &mut Option<Renaming>| {
                let target = Target::Workspace(key.clone());
                let editing = renaming.as_ref().is_some_and(|r| r.target == target);
                let activity = ran.get(key.as_str());
                let marks = Marks {
                    problems: problems.get(key.as_str()).copied().unwrap_or(0),
                    live: activity.is_some_and(|(at, _, _)| at.elapsed() < crate::bot::GLOW),
                    failed: activity.is_some_and(|(_, failed, _)| *failed),
                    off: ws.disabled,
                };
                let label = if editing { "" } else { ws.name.as_str() };
                let room = 32.0 + marks.width();
                order.borrow_mut().push(key.clone());
                let lit = active.as_deref() == Some(key.as_str()) || selected.contains(key);
                let (resp, label_area) = row(ui, &theme, &icons, Id::unique(("sidebar-ws", key)), depth, None, Some("workspace"), label, lit, false, room, ws.disabled);
                if !editing {
                    marks.paint(ui, &theme, &icons, resp.rect.right() - 34.0, resp.rect.center().y);
                }
                // On hover: what's in it, its problems, what the bot did with it.
                let stamp = events_stamp(ws);
                let events = match events_kept.borrow().get(key.as_str()) {
                    Some((s, n)) if *s == stamp => Some(*n),
                    _ => None,
                };
                let events = events.unwrap_or_else(|| {
                    let n = ws.nodes.values().filter(|n| reg.block(&n.block).is_some_and(|d| d.kind == circuit_core::BlockKind::Event)).count();
                    events_kept.borrow_mut().insert(key.clone(), (stamp, n));
                    n
                });
                let blocks = match ws.nodes.len() {
                    1 => t!("1 block").to_string(),
                    n => tf!("{count} blocks", count = n),
                };
                let events = match events {
                    1 => t!("1 event").to_string(),
                    n => tf!("{count} events", count = n),
                };
                let mut tip = vec![format!("{blocks} · {events}")];
                match marks.problems {
                    0 => {}
                    1 => tip.push(t!("1 problem: see the Problems panel").to_string()),
                    n => tip.push(tf!("{count} problems: see the Problems panel", count = n)),
                }
                if let Some((at, _, error)) = activity {
                    tip.push(match error {
                        Some(e) => tf!("A block failed {ago}: {error}", ago = crate::bot::ago(at.elapsed()), error = e),
                        None => tf!("Its blocks ran {ago}", ago = crate::bot::ago(at.elapsed())),
                    });
                }
                if ws.disabled {
                    tip.push(t!("Turned off: its blocks don't run. Right-click › Turn on.").to_string());
                }
                let resp = resp.on_hover_text(tip.join("\n"));
                if editing && let Some(r) = renaming.as_mut() {
                    match rename_box(ui, label_area, r) {
                        Some(true) => todo.push(Do::Rename(target.clone(), r.text.clone())),
                        Some(false) => *renaming = None,
                        None => {}
                    }
                    return;
                }
                resp.dnd_set_drag_payload(key.clone());
                // Another workspace dropped here goes above or below this one, in its folder.
                if let Some(dragged) = resp.dnd_hover_payload::<String>()
                    && *dragged != *key
                {
                    let before = drops_before(ui, resp.rect);
                    insertion_line(ui, &theme, resp.rect, before, depth);
                    let place = if before { tf!("Above {name}", name = ws.name) } else { tf!("Below {name}", name = ws.name) };
                    // Said where it lands when that's another folder.
                    let from = project.workspaces.get(&*dragged).and_then(|w| w.folder.clone());
                    let text = match &ws.folder {
                        _ if from == ws.folder => place,
                        Some(f) => tf!("{place}, in {folder}", place = place, folder = folders::shown(f)),
                        None => tf!("{place}, out of its folder", place = place),
                    };
                    hint(ui, text, true);
                }
                if egui::DragAndDrop::has_payload_of_type::<String>(ui.ctx())
                    && let Some(dragged) = resp.dnd_release_payload::<String>()
                    && *dragged != *key
                {
                    todo.push(Do::Place((*dragged).clone(), ws.folder.clone(), key.clone(), drops_before(ui, resp.rect)));
                }
                let more = more_button(ui, &theme, &icons, resp.rect, Id::unique(("sidebar-ws-more", key)));
                if resp.double_clicked() {
                    todo.push(Do::StartRename(target.clone(), ws.name.clone()));
                } else if resp.clicked() {
                    // Ctrl (or Cmd) adds it to the selection or takes it out, Shift
                    // selects from the last one clicked; a plain click opens it.
                    let mods = ui.input(|i| i.modifiers);
                    if mods.command || mods.ctrl {
                        todo.push(Do::Select(key.clone(), Pick::Toggle));
                    } else if mods.shift {
                        todo.push(Do::Select(key.clone(), Pick::Range));
                    } else {
                        todo.push(Do::Select(key.clone(), Pick::One));
                        todo.push(Do::Open(key.clone()));
                    }
                }
                let mut menu = |ui: &mut Ui| {
                    ui.set_min_width(200.0);
                    if item(ui, &theme, &icons, "pointer", t!("Open"), false).clicked() {
                        todo.push(Do::Open(key.clone()));
                        ui.close();
                    }
                    if item(ui, &theme, &icons, "pencil", t!("Rename"), false).on_hover_text(t!("Or double-click its name")).clicked() {
                        todo.push(Do::StartRename(target.clone(), ws.name.clone()));
                        ui.close();
                    }
                    if item(ui, &theme, &icons, "copy", t!("Duplicate"), false).on_hover_text(t!("A copy with all its blocks, turned off so it doesn't answer twice")).clicked() {
                        todo.push(Do::Duplicate(key.clone()));
                        ui.close();
                    }
                    ui.separator();
                    let (switch, tip, icon) = if ws.disabled {
                        (t!("Turn on"), t!("Its blocks run again when the bot runs."), "eye")
                    } else {
                        (t!("Turn off"), t!("The bot leaves it out: its events don't fire and its blocks don't run, until you turn it on again."), "eye-off")
                    };
                    if item(ui, &theme, &icons, icon, switch, false).on_hover_text(tip).clicked() {
                        todo.push(Do::SetEnabled(key.clone(), ws.disabled));
                        ui.close();
                    }
                    let folder_icon = icons.paths.get("folder").map(|p| egui::Image::from_uri(file_uri(p)).fit_to_exact_size(vec2(14.0, 14.0)).tint(theme.text_muted));
                    let moves = |ui: &mut Ui| {
                        if ws.folder.is_some() && kit::menu_item(ui, t!("No folder"), None, true).clicked() {
                            todo.push(Do::Move(key.clone(), None));
                            ui.close();
                        }
                        for f in &all_folders {
                            if ws.folder.as_ref() != Some(f) && kit::menu_item(ui, folders::shown(f), None, true).clicked() {
                                todo.push(Do::Move(key.clone(), Some(f.clone())));
                                ui.close();
                            }
                        }
                        if all_folders.is_empty() {
                            ui.label(RichText::new(t!("No folders yet")).color(theme.text_muted));
                        }
                    };
                    match folder_icon {
                        Some(image) => {
                            ui.menu_image_text_button(image, t!("Move to"), moves);
                        }
                        None => {
                            ui.menu_button(t!("Move to"), moves);
                        }
                    }
                    if item(ui, &theme, &icons, "package", t!("Export…"), false).clicked() {
                        todo.push(Do::Export(crate::editor::exchange::Preselect::Workspace(key.clone())));
                        ui.close();
                    }
                    ui.separator();
                    if selected.len() > 1 && selected.contains(key) {
                        if item(ui, &theme, &icons, "trash", RichText::new(tf!("Delete {count} workspaces", count = selected.len())).color(theme.error), true).clicked() {
                            todo.push(Do::AskDelete(selected.clone()));
                            ui.close();
                        }
                    } else if item(ui, &theme, &icons, "trash", RichText::new(t!("Delete workspace")).color(theme.error), true).clicked() {
                        todo.push(Do::DeleteWorkspace(key.clone()));
                        ui.close();
                    }
                };
                resp.context_menu(|ui| menu(ui));
                egui::Popup::menu(&more).show(|ui| menu(ui));
            };

            let mut entries = Vec::new();
            tree(&all_folders, None, &mut entries);
            for entry in &entries {
                let folder = match entry {
                    // A folder's workspaces, after the folders in it: hidden in a closed one.
                    Entry::Inside(folder) => {
                        if around(folder, true).all(&is_open) {
                            let depth = folders::depth(folder) + 2;
                            for (key, ws) in project.workspaces.iter().filter(|(_, w)| w.folder.as_ref() == Some(folder)) {
                                workspace_row(ui, key, ws, depth, &mut todo, &mut renaming);
                            }
                        }
                        continue;
                    }
                    Entry::Folder(folder) => folder,
                };
                if !around(folder, false).all(&is_open) {
                    continue;
                }
                let target = Target::Folder(folder.clone());
                let editing = renaming.as_ref().is_some_and(|r| r.target == target);
                let open = is_open(folder);
                // Every workspace in it, also in the folders inside.
                let inside: Vec<&String> = project.workspaces.iter().filter(|(_, w)| w.folder.as_deref().is_some_and(|f| folders::within(f, folder))).map(|(k, _)| k).collect();
                let folder_problems: usize = inside.iter().map(|k| problems.get(k.as_str()).copied().unwrap_or(0)).sum();
                let own = folders::name(folder);
                let (resp, label_area) = row(ui, &theme, &icons, Id::unique(("sidebar-folder", folder)), folders::depth(folder) + 1, Some(open), Some("folder"), if editing { "" } else { own }, false, false, 60.0, false);
                if !editing {
                    // How many workspaces it holds, and (closed) their problems.
                    let y = resp.rect.center().y;
                    let mut x = resp.rect.right() - 34.0;
                    if !open && folder_problems > 0 {
                        x = problems_pill(ui, &icons, &theme, x, y, folder_problems) - 4.0;
                    }
                    ui.painter().text(pos2(x, y), Align2::RIGHT_CENTER, inside.len().to_string(), FontId::proportional(11.0), theme.text_muted);
                }
                if editing && let Some(r) = renaming.as_mut() {
                    match rename_box(ui, label_area, r) {
                        Some(true) => todo.push(Do::Rename(target.clone(), r.text.clone())),
                        Some(false) => renaming = None,
                        None => {}
                    }
                } else {
                    let more = more_button(ui, &theme, &icons, resp.rect, Id::unique(("sidebar-folder-more", folder)));
                    if resp.double_clicked() {
                        todo.push(Do::StartRename(target.clone(), own.to_string()));
                    } else if resp.clicked() {
                        todo.push(Do::Toggle(folder.clone()));
                    }
                    if resp.dnd_hover_payload::<String>().is_some() {
                        into_target(ui, &theme, resp.rect);
                        hint(ui, tf!("Into {folder}", folder = own), true);
                    }
                    if held_over(ui, &resp, folder, open) {
                        todo.push(Do::Expand(folder.clone()));
                    }
                    // egui's dnd_release_payload takes whatever is dragged, even of
                    // another type: ask for the one that's there.
                    if egui::DragAndDrop::has_payload_of_type::<String>(ui.ctx())
                        && let Some(key) = resp.dnd_release_payload::<String>()
                    {
                        todo.push(Do::Move((*key).clone(), Some(folder.clone())));
                    }
                    // Another folder dropped on its middle goes in it; on its top
                    // or bottom edge, above or below it.
                    resp.dnd_set_drag_payload(FolderDrag(folder.clone()));
                    match folder_drop::<FolderDrag>(ui, &theme, &resp, folder, |d| &d.0) {
                        Some((dragged, Zone::Into)) => todo.push(Do::NestFolder(dragged, Some(folder.clone()))),
                        Some((dragged, zone)) => todo.push(Do::PlaceFolder(dragged, folder.clone(), zone == Zone::Before)),
                        None => {}
                    }
                    let mut menu = |ui: &mut Ui| {
                        ui.set_min_width(200.0);
                        if item(ui, &theme, &icons, "plus", t!("New workspace here"), false).clicked() {
                            todo.push(Do::NewWorkspace(Some(folder.clone())));
                            ui.close();
                        }
                        if item(ui, &theme, &icons, "folder-plus", t!("New folder here"), false).clicked() {
                            todo.push(Do::NewFolder(Some(folder.clone())));
                            ui.close();
                        }
                        if item(ui, &theme, &icons, "pencil", t!("Rename"), false).on_hover_text(t!("Or double-click its name")).clicked() {
                            todo.push(Do::StartRename(target.clone(), own.to_string()));
                            ui.close();
                        }
                        let filled = !inside.is_empty();
                        if ui.add_enabled_ui(filled, |ui| item(ui, &theme, &icons, "package", t!("Export folder…"), false)).inner.clicked() {
                            todo.push(Do::Export(crate::editor::exchange::Preselect::Folder(folder.clone())));
                            ui.close();
                        }
                        ui.separator();
                        if item(ui, &theme, &icons, "trash", RichText::new(t!("Delete folder")).color(theme.error), true).on_hover_text(t!("What's in it moves up a level; no workspace is deleted")).clicked() {
                            todo.push(Do::DeleteFolder(folder.clone()));
                            ui.close();
                        }
                    };
                    resp.context_menu(|ui| menu(ui));
                    egui::Popup::menu(&more).show(|ui| menu(ui));
                }
            }
            for (key, ws) in project.workspaces.iter().filter(|(_, w)| w.folder.is_none()) {
                workspace_row(ui, key, ws, 1, &mut todo, &mut renaming);
            }
            if project.workspaces.is_empty() {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(26.0);
                    ui.label(RichText::new(t!("No workspaces yet. Click + above.")).color(theme.text_muted).size(12.0));
                });
            }
        }

        // Slash commands, for modes that have them: in folders and the order
        // they're dragged into, each turned on or off.
        let commands = crate::builders::slash::commands_of(project);
        let roots = crate::builders::slash::roots(&commands);
        if !roots.is_empty() || ed.has_slash_commands() {
            let slash_folders = crate::builders::slash::folders_of(project, &commands);
            ui.add_space(10.0);
            let line = ui.available_rect_before_wrap();
            ui.painter().hline(line.x_range(), line.top(), Stroke::new(1.0, theme.line));
            ui.add_space(4.0);
            let open = is_open(SLASH_ROW);
            let (resp, _) = row(ui, &theme, &icons, Id::unique("sidebar-slash"), 0, Some(open), None, t!("Slash commands"), false, true, 54.0, false);
            ui.painter().text(pos2(resp.rect.right() - 34.0, resp.rect.center().y), Align2::RIGHT_CENTER, roots.len().to_string(), FontId::proportional(11.5), theme.text_muted);
            if row_button(ui, &theme, &icons, Rect::from_center_size(pos2(resp.rect.right() - 16.0, resp.rect.center().y), vec2(22.0, 22.0)), Id::unique("sidebar-slash-folder"), "folder-plus", t!("New folder")).clicked() {
                todo.push(Do::SlashNewFolder(None));
            } else if resp.clicked() {
                todo.push(Do::Toggle(SLASH_ROW.into()));
            }
            if resp.dnd_hover_payload::<SlashDrag>().is_some() {
                into_target(ui, &theme, resp.rect);
                hint(ui, t!("Out of its folder, to the top"), true);
            } else if resp.dnd_hover_payload::<SlashFolderDrag>().is_some() {
                into_target(ui, &theme, resp.rect);
                hint(ui, t!("To the top"), true);
            }
            // A command dropped here leaves its folder.
            if egui::DragAndDrop::has_payload_of_type::<SlashDrag>(ui.ctx())
                && let Some(dragged) = resp.dnd_release_payload::<SlashDrag>()
            {
                todo.push(Do::SlashPlace(dragged.0.clone(), None, None));
            }
            // A folder dropped here goes to the top.
            if egui::DragAndDrop::has_payload_of_type::<SlashFolderDrag>(ui.ctx())
                && let Some(dragged) = resp.dnd_release_payload::<SlashFolderDrag>()
            {
                todo.push(Do::SlashNestFolder(dragged.0.clone(), None));
            }
            resp.context_menu(|ui| {
                ui.set_min_width(200.0);
                if item(ui, &theme, &icons, "folder-plus", t!("New folder"), false).clicked() {
                    todo.push(Do::SlashNewFolder(None));
                    ui.close();
                }
                if item(ui, &theme, &icons, "slash", t!("Edit commands…"), false).clicked() {
                    todo.push(Do::SlashCommands);
                    ui.close();
                }
            });

            let first_of = |root: &str| commands.iter().find(|c| c.name.split_whitespace().next() == Some(root));
            let command_row = |ui: &mut Ui, root: &String, depth: usize, todo: &mut Vec<Do>| {
                let Some(first) = first_of(root) else { return };
                let (off, folder) = (first.disabled, first.folder.clone());
                let marks = Marks { off, ..Default::default() };
                let (resp, _) = row(ui, &theme, &icons, Id::unique(("sidebar-cmd", root)), depth, None, Some("slash"), &format!("/{root}"), false, false, 32.0 + marks.width(), off);
                marks.paint(ui, &theme, &icons, resp.rect.right() - 34.0, resp.rect.center().y);
                let subs: Vec<&str> = commands.iter().filter(|c| c.name.split_whitespace().next() == Some(root.as_str()) && c.name != *root).map(|c| c.name.as_str()).collect();
                let mut tip = vec![first.description.clone()];
                if !subs.is_empty() {
                    tip.push(tf!("Subcommands: {list}", list = subs.join(", ")));
                }
                if off {
                    tip.push(t!("Turned off: it isn't registered with Discord, so nobody can use it. Right-click › Turn on.").to_string());
                }
                let resp = resp.on_hover_text(tip.join("\n"));
                resp.dnd_set_drag_payload(SlashDrag(root.clone()));
                if let Some(dragged) = resp.dnd_hover_payload::<SlashDrag>()
                    && dragged.0 != *root
                {
                    let before = drops_before(ui, resp.rect);
                    insertion_line(ui, &theme, resp.rect, before, depth);
                    let name = format!("/{root}");
                    let place = if before { tf!("Above {name}", name = name) } else { tf!("Below {name}", name = name) };
                    let from = first_of(&dragged.0).and_then(|c| c.folder.clone());
                    let text = match &folder {
                        _ if from == folder => place,
                        Some(f) => tf!("{place}, in {folder}", place = place, folder = folders::shown(f)),
                        None => tf!("{place}, out of its folder", place = place),
                    };
                    hint(ui, text, true);
                }
                if egui::DragAndDrop::has_payload_of_type::<SlashDrag>(ui.ctx())
                    && let Some(dragged) = resp.dnd_release_payload::<SlashDrag>()
                    && dragged.0 != *root
                {
                    todo.push(Do::SlashPlace(dragged.0.clone(), folder.clone(), Some((root.clone(), drops_before(ui, resp.rect)))));
                }
                let more = more_button(ui, &theme, &icons, resp.rect, Id::unique(("sidebar-cmd-more", root)));
                if resp.clicked() {
                    todo.push(Do::SlashOpen(root.clone()));
                }
                let mut menu = |ui: &mut Ui| {
                    ui.set_min_width(200.0);
                    if item(ui, &theme, &icons, "pencil", t!("Edit"), false).clicked() {
                        todo.push(Do::SlashOpen(root.clone()));
                        ui.close();
                    }
                    let (switch, tip, icon) = if off {
                        (t!("Turn on"), t!("It's registered with Discord again when the bot starts."), "eye")
                    } else {
                        (t!("Turn off"), t!("The bot doesn't register it with Discord, so nobody can use it, until you turn it on again."), "eye-off")
                    };
                    if item(ui, &theme, &icons, icon, switch, false).on_hover_text(tip).clicked() {
                        todo.push(Do::SlashEnable(root.clone(), off));
                        ui.close();
                    }
                    let moves = |ui: &mut Ui| {
                        if folder.is_some() && kit::menu_item(ui, t!("No folder"), None, true).clicked() {
                            todo.push(Do::SlashPlace(root.clone(), None, None));
                            ui.close();
                        }
                        for f in &slash_folders {
                            if folder.as_ref() != Some(f) && kit::menu_item(ui, folders::shown(f), None, true).clicked() {
                                todo.push(Do::SlashPlace(root.clone(), Some(f.clone()), None));
                                ui.close();
                            }
                        }
                        if slash_folders.is_empty() {
                            ui.label(RichText::new(t!("No folders yet")).color(theme.text_muted));
                        }
                    };
                    ui.menu_button(t!("Move to"), moves);
                };
                resp.context_menu(|ui| menu(ui));
                egui::Popup::menu(&more).show(|ui| menu(ui));
            };

            if open {
                let in_folder = |root: &String, f: &str| first_of(root).is_some_and(|c| c.folder.as_deref().is_some_and(|cf| folders::within(cf, f)));
                let mut entries = Vec::new();
                tree(&slash_folders, None, &mut entries);
                for entry in &entries {
                    let folder = match entry {
                        Entry::Inside(folder) => {
                            if around(folder, true).all(|f| is_open(&slash_folder_key(f))) {
                                let depth = folders::depth(folder) + 2;
                                for root in roots.iter().filter(|r| first_of(r).is_some_and(|c| c.folder.as_ref() == Some(folder))) {
                                    command_row(ui, root, depth, &mut todo);
                                }
                            }
                            continue;
                        }
                        Entry::Folder(folder) => folder,
                    };
                    if !around(folder, false).all(|f| is_open(&slash_folder_key(f))) {
                        continue;
                    }
                    let target = Target::SlashFolder(folder.clone());
                    let editing = renaming.as_ref().is_some_and(|r| r.target == target);
                    let key = slash_folder_key(folder);
                    let folder_open = is_open(&key);
                    let inside = roots.iter().filter(|r| in_folder(r, folder)).count();
                    let own = folders::name(folder);
                    let (resp, label_area) = row(ui, &theme, &icons, Id::unique(("sidebar-slash-folder", folder)), folders::depth(folder) + 1, Some(folder_open), Some("folder"), if editing { "" } else { own }, false, false, 60.0, false);
                    if !editing {
                        ui.painter().text(pos2(resp.rect.right() - 34.0, resp.rect.center().y), Align2::RIGHT_CENTER, inside.to_string(), FontId::proportional(11.0), theme.text_muted);
                    }
                    if editing && let Some(r) = renaming.as_mut() {
                        match rename_box(ui, label_area, r) {
                            Some(true) => todo.push(Do::Rename(target.clone(), r.text.clone())),
                            Some(false) => renaming = None,
                            None => {}
                        }
                    } else {
                        let more = more_button(ui, &theme, &icons, resp.rect, Id::unique(("sidebar-slash-folder-more", folder)));
                        if resp.double_clicked() {
                            todo.push(Do::StartRename(target.clone(), own.to_string()));
                        } else if resp.clicked() {
                            todo.push(Do::Toggle(key.clone()));
                        }
                        if resp.dnd_hover_payload::<SlashDrag>().is_some() {
                            into_target(ui, &theme, resp.rect);
                            hint(ui, tf!("Into {folder}", folder = own), true);
                        }
                        if held_over(ui, &resp, &key, folder_open) {
                            todo.push(Do::Expand(key.clone()));
                        }
                        if egui::DragAndDrop::has_payload_of_type::<SlashDrag>(ui.ctx())
                            && let Some(dragged) = resp.dnd_release_payload::<SlashDrag>()
                        {
                            todo.push(Do::SlashPlace(dragged.0.clone(), Some(folder.clone()), None));
                        }
                        resp.dnd_set_drag_payload(SlashFolderDrag(folder.clone()));
                        match folder_drop::<SlashFolderDrag>(ui, &theme, &resp, folder, |d| &d.0) {
                            Some((dragged, Zone::Into)) => todo.push(Do::SlashNestFolder(dragged, Some(folder.clone()))),
                            Some((dragged, zone)) => todo.push(Do::SlashPlaceFolder(dragged, folder.clone(), zone == Zone::Before)),
                            None => {}
                        }
                        let mut menu = |ui: &mut Ui| {
                            ui.set_min_width(200.0);
                            if item(ui, &theme, &icons, "folder-plus", t!("New folder here"), false).clicked() {
                                todo.push(Do::SlashNewFolder(Some(folder.clone())));
                                ui.close();
                            }
                            if item(ui, &theme, &icons, "pencil", t!("Rename"), false).on_hover_text(t!("Or double-click its name")).clicked() {
                                todo.push(Do::StartRename(target.clone(), own.to_string()));
                                ui.close();
                            }
                            ui.separator();
                            if item(ui, &theme, &icons, "trash", RichText::new(t!("Delete folder")).color(theme.error), true).on_hover_text(t!("What's in it moves up a level; no command is deleted")).clicked() {
                                todo.push(Do::SlashDeleteFolder(folder.clone()));
                                ui.close();
                            }
                        };
                        resp.context_menu(|ui| menu(ui));
                        egui::Popup::menu(&more).show(|ui| menu(ui));
                    }
                }
                for root in roots.iter().filter(|r| first_of(r).is_some_and(|c| c.folder.is_none())) {
                    command_row(ui, root, 1, &mut todo);
                }
                let (resp, _) = row(ui, &theme, &icons, Id::unique("sidebar-cmd-new"), 1, None, Some("plus"), t!("Edit commands…"), false, false, 0.0, false);
                if resp.clicked() {
                    todo.push(Do::SlashCommands);
                }
            }
        }
        ui.add_space(8.0);
        // Held near the top or bottom edge while dragging, the list scrolls.
        if egui::DragAndDrop::has_any_payload(ui.ctx())
            && let Some(p) = ui.ctx().pointer_hover_pos()
        {
            let clip = ui.clip_rect();
            let edge = 30.0;
            if clip.x_range().contains(p.x) {
                if (clip.top() - 12.0..clip.top() + edge).contains(&p.y) {
                    ui.scroll_with_delta(vec2(0.0, 8.0));
                    ui.ctx().request_repaint();
                } else if (clip.bottom() - edge..clip.bottom() + 12.0).contains(&p.y) {
                    ui.scroll_with_delta(vec2(0.0, -8.0));
                    ui.ctx().request_repaint();
                }
            }
        }
    });
    drag_preview(&ctx, &theme, &icons, project);

    ctx.data_mut(|d| d.insert_temp(Id::unique(ORDER_KEY), order.into_inner()));
    // A click anywhere else, or Escape, ends the selection, so Delete deletes blocks again.
    let elsewhere = ui.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !list_area.contains(p)));
    let escape = renaming.is_none() && ui.input(|i| i.key_pressed(Key::Escape));
    if (elsewhere || escape) && ed.ask_delete_workspaces.is_none() {
        ed.ws_selection.clear();
    }
    let events_kept = events_kept.into_inner();
    ctx.data_mut(|d| d.insert_temp(events_id, events_kept));
    apply(ed, &ctx, todo, &mut renaming);
    ctx.data_mut(|d| match renaming {
        Some(r) => {
            d.insert_temp(Id::unique("sidebar-renaming"), r);
        }
        None => d.remove::<Renaming>(Id::unique("sidebar-renaming")),
    });
}

/// What's being dragged, beside the pointer: its icon and name, and what
/// letting go would do (red when it can't go there). The cursor grabs it.
fn drag_preview(ctx: &egui::Context, theme: &Theme, icons: &Icons, project: &circuit_core::Project) {
    let dragged: Option<(&str, String)> = if let Some(key) = egui::DragAndDrop::payload::<String>(ctx) {
        Some(("workspace", project.workspaces.get(&*key).map(|w| w.name.clone()).unwrap_or_else(|| (*key).clone())))
    } else if let Some(f) = egui::DragAndDrop::payload::<FolderDrag>(ctx) {
        Some(("folder", folders::name(&f.0).to_string()))
    } else if let Some(c) = egui::DragAndDrop::payload::<SlashDrag>(ctx) {
        Some(("slash", format!("/{}", c.0)))
    } else {
        egui::DragAndDrop::payload::<SlashFolderDrag>(ctx).map(|f| ("folder", folders::name(&f.0).to_string()))
    };
    let (Some((icon, name)), Some(at)) = (dragged, ctx.pointer_hover_pos()) else { return };
    if !ctx.input(|i| i.pointer.any_down()) {
        return;
    }
    let said: Option<DropHint> = ctx.data(|d| d.get_temp(Id::unique(HINT_KEY)));
    ctx.set_cursor_icon(if said.as_ref().is_some_and(|h| !h.ok) { CursorIcon::NotAllowed } else { CursorIcon::Grabbing });
    egui::Area::new(Id::unique("sidebar-drag-preview")).order(egui::Order::Tooltip).fixed_pos(at + vec2(16.0, 10.0)).interactable(false).show(ctx, |ui| {
        egui::Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.accent)).corner_radius(6).inner_margin(egui::Margin::symmetric(10, 6)).shadow(ctx.global_style().visuals.popup_shadow).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(ICON, ICON), Sense::hover());
                icons.paint(ui, icon, r.center(), ICON, theme.accent);
                ui.label(RichText::new(name).size(13.0).strong().color(theme.text));
            });
            let (text, color) = match &said {
                Some(h) => (h.text.clone(), if h.ok { theme.accent } else { theme.error }),
                None => (t!("Drop it on a folder, or above or below a row").to_string(), theme.text_muted),
            };
            ui.label(RichText::new(text).size(11.5).color(color));
        });
    });
}

fn unique(base: &str, taken: impl Fn(&str) -> bool) -> String {
    (1..).map(|i| if i == 1 { base.to_string() } else { format!("{base} {i}") }).find(|n| !taken(n)).expect("unbounded")
}

fn apply(ed: &mut Editor, ctx: &egui::Context, todo: Vec<Do>, renaming: &mut Option<Renaming>) {
    for action in todo {
        let Some(project) = ed.project.as_mut() else { return };
        match action {
            Do::Open(key) => ed.actions.push(Action::OpenWorkspace(key)),
            Do::StartRename(target, text) => *renaming = Some(Renaming { target, text, fresh: true }),
            Do::Rename(target, text) => {
                *renaming = None;
                let name = text.trim().to_string();
                if name.is_empty() {
                    continue;
                }
                match target {
                    Target::Workspace(key) => {
                        if let Some(ws) = project.workspaces.get_mut(&key)
                            && ws.name != name
                        {
                            ws.name = name;
                            ed.edited = true;
                        }
                    }
                    Target::Folder(old) => {
                        // The name typed is the folder's own; it stays where it is.
                        let new = folders::child(folders::parent(&old), &name);
                        let mut list = ws_folders(project);
                        if folders::move_folder(&mut list, project.workspaces.values_mut().map(|w| &mut w.folder), &old, &new) {
                            project.meta.folders = list;
                            ed.edited = true;
                        }
                    }
                    Target::SlashFolder(old) => {
                        let new = folders::child(folders::parent(&old), &name);
                        let mut commands = crate::builders::slash::commands_of(project);
                        let mut list = crate::builders::slash::folders_of(project, &commands);
                        if folders::move_folder(&mut list, commands.iter_mut().map(|c| &mut c.folder), &old, &new) {
                            ed.edited |= crate::builders::slash::save_to(project, &commands, &list);
                        }
                    }
                }
            }
            Do::Move(key, folder) => {
                if let Some(ws) = project.workspaces.get_mut(&key)
                    && ws.folder != folder
                {
                    ws.folder = folder.clone();
                    if let Some(f) = folder {
                        if !project.meta.folders.contains(&f) {
                            project.meta.folders.push(f.clone());
                        }
                        uncollapse(ctx, &f);
                    }
                    ed.edited = true;
                }
            }
            Do::Place(key, folder, next_to, before) => {
                if key != next_to
                    && let Some(mut ws) = project.workspaces.shift_remove(&key)
                {
                    ws.folder = folder.clone();
                    let at = project.workspaces.get_index_of(&next_to).map_or(project.workspaces.len(), |i| if before { i } else { i + 1 });
                    project.workspaces.shift_insert(at, key, ws);
                    if let Some(f) = folder {
                        if !project.meta.folders.contains(&f) {
                            project.meta.folders.push(f.clone());
                        }
                        uncollapse(ctx, &f);
                    }
                    ed.edited = true;
                }
            }
            Do::PlaceFolder(name, next_to, before) => {
                let mut list = ws_folders(project);
                if let Some(now) = folders::place_folder(&mut list, project.workspaces.values_mut().map(|w| &mut w.folder), &name, &next_to, before) {
                    project.meta.folders = list;
                    if let Some(p) = folders::parent(&now) {
                        uncollapse(ctx, p);
                    }
                    ed.edited = true;
                }
            }
            Do::NestFolder(name, into) => {
                let mut list = ws_folders(project);
                if folders::nest_folder(&mut list, project.workspaces.values_mut().map(|w| &mut w.folder), &name, into.as_deref()).is_some() {
                    project.meta.folders = list;
                    if let Some(f) = &into {
                        uncollapse(ctx, f);
                    }
                    ed.edited = true;
                }
            }
            Do::SetEnabled(key, on) => {
                if let Some(ws) = project.workspaces.get_mut(&key)
                    && ws.disabled == on
                {
                    ws.disabled = !on;
                    ed.edited = true;
                }
            }
            Do::Duplicate(key) => {
                let Some(ws) = project.workspaces.get(&key) else { continue };
                let name = unique(&tf!("{name} copy", name = ws.name), |n| project.workspaces.values().any(|w| w.name == n));
                // Turned off, so the copy's events don't answer a second time.
                let copy = Workspace { name: name.clone(), disabled: true, ..ws.clone() };
                let new_key = project.new_workspace_key(&name);
                project.workspaces.insert(new_key.clone(), copy);
                ed.actions.push(Action::OpenWorkspace(new_key));
                ed.log(crate::editor::Level::Info, tf!("{name} is a copy of {original}, turned off so it doesn't answer twice. Right-click › Turn on when it's ready.", name = name, original = ws_name(&key, ed)));
                ed.edited = true;
            }
            Do::AskDelete(keys) => ed.ask_delete_workspaces = Some(keys),
            Do::Select(key, pick) => {
                let picked = &mut ed.ws_selection;
                match pick {
                    Pick::One => *picked = vec![key.clone()],
                    Pick::Toggle => match picked.iter().position(|k| *k == key) {
                        Some(i) => {
                            picked.remove(i);
                        }
                        None => picked.push(key.clone()),
                    },
                    Pick::Range => {
                        let order: Vec<String> = ctx.data(|d| d.get_temp(Id::unique(ORDER_KEY))).unwrap_or_default();
                        let from = ed.ws_anchor.as_ref().and_then(|a| order.iter().position(|k| k == a));
                        let to = order.iter().position(|k| *k == key);
                        *picked = match (from, to) {
                            (Some(a), Some(b)) => order[a.min(b)..=a.max(b)].to_vec(),
                            _ => vec![key.clone()],
                        };
                    }
                }
                // Shift keeps counting from the same one.
                if pick != Pick::Range {
                    ed.ws_anchor = Some(key);
                }
            }
            Do::DeleteWorkspace(key) => {
                ed.ws_selection.retain(|k| *k != key);
                // The file goes on save, so Don't save brings the workspace back.
                project.remove_workspace(&key);
                ed.actions.push(Action::CloseWorkspaceTabs(key));
                ed.edited = true;
            }
            Do::NewWorkspace(folder) => {
                let name = unique("Workspace", |n| project.workspaces.values().any(|w| w.name == n));
                let key = project.new_workspace_key(&name);
                project.workspaces.insert(key.clone(), Workspace { folder: folder.clone(), ..Workspace::new(name.clone()) });
                if let Some(f) = &folder {
                    uncollapse(ctx, f);
                }
                uncollapse(ctx, PROJECT_ROW);
                ed.actions.push(Action::OpenWorkspace(key.clone()));
                *renaming = Some(Renaming { target: Target::Workspace(key), text: name, fresh: true });
                ed.edited = true;
            }
            Do::NewFolder(parent) => {
                let mut list = ws_folders(project);
                let name = folders::free_name(&list, parent.as_deref(), t!("New folder"));
                let path = folders::child(parent.as_deref(), &name);
                list.push(path.clone());
                project.meta.folders = list;
                uncollapse(ctx, PROJECT_ROW);
                if let Some(p) = &parent {
                    uncollapse(ctx, p);
                }
                *renaming = Some(Renaming { target: Target::Folder(path), text: name, fresh: true });
                ed.edited = true;
            }
            Do::DeleteFolder(name) => {
                let mut list = ws_folders(project);
                folders::delete_folder(&mut list, project.workspaces.values_mut().map(|w| &mut w.folder), &name);
                project.meta.folders = list;
                ed.edited = true;
            }
            Do::Expand(key) => uncollapse(ctx, &key),
            Do::Toggle(key) => ctx.data_mut(|d| {
                let list = d.get_persisted_mut_or_default::<Vec<String>>(Id::unique(COLLAPSED_KEY));
                match list.iter().position(|c| *c == key) {
                    Some(i) => {
                        list.remove(i);
                    }
                    None => list.push(key),
                }
            }),
            Do::SlashCommands => crate::builders::slash::open(ed),
            Do::SlashOpen(root) => crate::builders::slash::open_at(ed, &root),
            Do::SlashPlace(root, folder, next_to) => {
                let mut commands = crate::builders::slash::commands_of(project);
                let mut folders = crate::builders::slash::folders_of(project, &commands);
                if let Some(f) = &folder
                    && !folders.contains(f)
                {
                    folders.push(f.clone());
                }
                crate::builders::slash::place(&mut commands, &root, folder, next_to.as_ref().map(|(o, b)| (o.as_str(), *b)));
                ed.edited |= crate::builders::slash::save_to(project, &commands, &folders);
            }
            Do::SlashEnable(root, on) => {
                let mut commands = crate::builders::slash::commands_of(project);
                let folders = crate::builders::slash::folders_of(project, &commands);
                for c in commands.iter_mut().filter(|c| c.name.split_whitespace().next() == Some(root.as_str())) {
                    c.disabled = !on;
                }
                ed.edited |= crate::builders::slash::save_to(project, &commands, &folders);
            }
            Do::SlashNewFolder(parent) => {
                let commands = crate::builders::slash::commands_of(project);
                let mut list = crate::builders::slash::folders_of(project, &commands);
                let name = folders::free_name(&list, parent.as_deref(), t!("New folder"));
                let path = folders::child(parent.as_deref(), &name);
                list.push(path.clone());
                ed.edited |= crate::builders::slash::save_to(project, &commands, &list);
                uncollapse(ctx, SLASH_ROW);
                if let Some(p) = &parent {
                    uncollapse(ctx, &slash_folder_key(p));
                }
                *renaming = Some(Renaming { target: Target::SlashFolder(path), text: name, fresh: true });
            }
            Do::SlashDeleteFolder(name) => {
                let mut commands = crate::builders::slash::commands_of(project);
                let mut list = crate::builders::slash::folders_of(project, &commands);
                folders::delete_folder(&mut list, commands.iter_mut().map(|c| &mut c.folder), &name);
                ed.edited |= crate::builders::slash::save_to(project, &commands, &list);
            }
            Do::SlashPlaceFolder(name, next_to, before) => {
                let mut commands = crate::builders::slash::commands_of(project);
                let mut list = crate::builders::slash::folders_of(project, &commands);
                if let Some(now) = folders::place_folder(&mut list, commands.iter_mut().map(|c| &mut c.folder), &name, &next_to, before) {
                    ed.edited |= crate::builders::slash::save_to(project, &commands, &list);
                    if let Some(p) = folders::parent(&now) {
                        uncollapse(ctx, &slash_folder_key(p));
                    }
                }
            }
            Do::SlashNestFolder(name, into) => {
                let mut commands = crate::builders::slash::commands_of(project);
                let mut list = crate::builders::slash::folders_of(project, &commands);
                if folders::nest_folder(&mut list, commands.iter_mut().map(|c| &mut c.folder), &name, into.as_deref()).is_some() {
                    ed.edited |= crate::builders::slash::save_to(project, &commands, &list);
                    if let Some(f) = &into {
                        uncollapse(ctx, &slash_folder_key(f));
                    }
                }
            }
            Do::Export(preselect) => crate::editor::exchange::open_export(ed, preselect),
            Do::Import => crate::editor::exchange::pick_import(ed),
        }
    }
}

/// A workspace's name, by its key.
fn ws_name(key: &str, ed: &Editor) -> String {
    ed.project.as_ref().and_then(|p| p.workspaces.get(key)).map(|w| w.name.clone()).unwrap_or_else(|| key.to_string())
}

fn uncollapse(ctx: &egui::Context, key: &str) {
    ctx.data_mut(|d| d.get_persisted_mut_or_default::<Vec<String>>(Id::unique(COLLAPSED_KEY)).retain(|c| c != key));
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Event, Modifiers, PointerButton, Pos2};

    use super::*;

    /// Workspaces go above or below another by dragging, folders too; the order is saved.
    #[test]
    fn workspaces_and_folders_reorder_by_dragging() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("xc-reorder-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy_dir(&root.join("examples/DemoBot"), &dir);
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&dir);
        let ctx = egui::Context::default();
        frame(&ctx, &mut ed, vec![]);
        frame(&ctx, &mut ed, vec![]);
        let drag = |ctx: &egui::Context, ed: &mut Editor, from: Pos2, to: Pos2| {
            frame(ctx, ed, vec![Event::PointerMoved(from)]);
            frame(ctx, ed, vec![press(from, true)]);
            for i in 1..=8 {
                frame(ctx, ed, vec![Event::PointerMoved(from + (to - from) * (i as f32 / 8.0))]);
            }
            frame(ctx, ed, vec![press(to, false)]);
            frame(ctx, ed, vec![]);
        };
        let keys = |ed: &Editor| ed.project.as_ref().unwrap().workspaces.keys().cloned().collect::<Vec<_>>();
        assert_eq!(keys(&ed), ["rank-command", "startup", "welcome"]);

        // Rows: DemoBot, Commands, Rank command, Events, Startup, Welcome. Startup below Welcome.
        drag(&ctx, &mut ed, row_at(4), row_at(5) + vec2(0.0, ROW_H / 2.0 - 3.0));
        assert_eq!(keys(&ed), ["rank-command", "welcome", "startup"]);
        // Now …, Events, Welcome, Startup. The Events folder above Commands.
        drag(&ctx, &mut ed, row_at(3), row_at(1) - vec2(0.0, ROW_H / 2.0 - 3.0));
        assert_eq!(ed.project.as_ref().unwrap().meta.folders, ["Events", "Commands"]);

        // Saved, and the same when opened again.
        ed.project.as_mut().unwrap().save().unwrap();
        let again = circuit_core::Project::load(&dir).unwrap();
        assert_eq!(again.workspaces.keys().cloned().collect::<Vec<_>>(), ["rank-command", "welcome", "startup"]);
        assert_eq!(again.meta.folders, ["Events", "Commands"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ctrl- and Shift-click select workspaces; Delete asks, then deletes them; Escape lets go.
    #[test]
    fn workspaces_are_selected_and_deleted_together() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("xc-multi-delete-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy_dir(&root.join("examples/DemoBot"), &dir);
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&dir);
        let ctx = egui::Context::default();
        frame(&ctx, &mut ed, vec![]);
        frame(&ctx, &mut ed, vec![]);
        let click_with = |ctx: &egui::Context, ed: &mut Editor, p: Pos2, modifiers: Modifiers| {
            let run = |ctx: &egui::Context, ed: &mut Editor, mut events: Vec<Event>| {
                events.insert(0, Event::ModifiersChanged(modifiers));
                let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(260.0, 600.0))), events, ..Default::default() };
                let mut out = ctx.run_ui(raw, |ui| panel(ui, ed, &HashMap::new()));
                out.textures_delta.clear();
            };
            run(ctx, ed, vec![Event::PointerMoved(p)]);
            run(ctx, ed, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers }]);
            run(ctx, ed, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers }]);
            run(ctx, ed, vec![]);
        };
        // Rows: DemoBot, Commands, Rank command, Events, Startup, Welcome.
        click_with(&ctx, &mut ed, row_at(2), Modifiers::NONE);
        assert_eq!(ed.ws_selection, ["rank-command"]);
        click_with(&ctx, &mut ed, row_at(5), Modifiers::COMMAND);
        assert_eq!(ed.ws_selection, ["rank-command", "welcome"], "Ctrl adds");
        click_with(&ctx, &mut ed, row_at(5), Modifiers::COMMAND);
        assert_eq!(ed.ws_selection, ["rank-command"], "and takes out again");
        click_with(&ctx, &mut ed, row_at(4), Modifiers::SHIFT);
        assert_eq!(ed.ws_selection, ["startup", "welcome"], "Shift: from the last one clicked, as the list shows them");

        // Delete asks first; Enter (or the Delete button) deletes them.
        let picked = ed.ws_selection.clone();
        apply(&mut ed, &ctx, vec![Do::AskDelete(picked)], &mut None);
        let ask = |ctx: &egui::Context, ed: &mut Editor, events: Vec<Event>| {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))), events, ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| delete_dialog(ui.ctx(), ed));
            out.textures_delta.clear();
        };
        ask(&ctx, &mut ed, vec![]);
        assert_eq!(ed.project.as_ref().unwrap().workspaces.len(), 3, "nothing goes before the answer");
        ask(&ctx, &mut ed, vec![Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
        assert_eq!(ed.project.as_ref().unwrap().workspaces.keys().cloned().collect::<Vec<_>>(), ["rank-command"]);
        assert!(ed.ws_selection.is_empty() && ed.ask_delete_workspaces.is_none());

        // Escape lets go of a selection, so Delete deletes blocks again.
        click_with(&ctx, &mut ed, row_at(2), Modifiers::NONE);
        assert_eq!(ed.ws_selection, ["rank-command"]);
        frame(&ctx, &mut ed, vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
        assert!(ed.ws_selection.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(e.file_name());
            if e.path().is_dir() {
                copy_dir(&e.path(), &target);
            } else {
                std::fs::copy(e.path(), target).unwrap();
            }
        }
    }

    fn frame(ctx: &egui::Context, ed: &mut Editor, events: Vec<Event>) {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(260.0, 600.0))), events, ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| panel(ui, ed, &HashMap::new()));
        out.textures_delta.clear();
    }

    /// The middle of tree row `i` (0 is the project; the list starts 4 px down).
    fn row_at(i: usize) -> Pos2 {
        pos2(120.0, 4.0 + ROW_H * i as f32 + ROW_H / 2.0)
    }

    fn press(p: Pos2, down: bool) -> Event {
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: down, modifiers: Modifiers::NONE }
    }

    fn click(ctx: &egui::Context, ed: &mut Editor, p: Pos2) {
        frame(ctx, ed, vec![Event::PointerMoved(p)]);
        frame(ctx, ed, vec![press(p, true)]);
        frame(ctx, ed, vec![press(p, false)]);
        frame(ctx, ed, vec![]);
    }

    #[test]
    fn folders_drag_rename_and_collapse() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        let ctx = egui::Context::default();
        frame(&ctx, &mut ed, vec![]);
        frame(&ctx, &mut ed, vec![]);
        let folder = |ed: &Editor, key: &str| ed.project.as_ref().unwrap().workspaces[key].folder.clone();
        assert_eq!(folder(&ed, "welcome").as_deref(), Some("Events"));

        // Rows: DemoBot, Commands, Rank command, Events, Startup, Welcome.
        // Drag Welcome onto the Commands folder.
        let (from, to) = (row_at(5), row_at(1));
        frame(&ctx, &mut ed, vec![Event::PointerMoved(from)]);
        frame(&ctx, &mut ed, vec![press(from, true)]);
        for i in 1..=8 {
            frame(&ctx, &mut ed, vec![Event::PointerMoved(from + (to - from) * (i as f32 / 8.0))]);
        }
        frame(&ctx, &mut ed, vec![press(to, false)]);
        frame(&ctx, &mut ed, vec![]);
        assert_eq!(folder(&ed, "welcome").as_deref(), Some("Commands"), "dropped on the folder");

        // Now: DemoBot, Commands, Rank command, Welcome, Events, Startup. Double-click Startup to rename it.
        let startup = row_at(5);
        frame(&ctx, &mut ed, vec![Event::PointerMoved(startup)]);
        frame(&ctx, &mut ed, vec![press(startup, true), press(startup, false)]);
        frame(&ctx, &mut ed, vec![press(startup, true), press(startup, false)]);
        frame(&ctx, &mut ed, vec![]);
        frame(&ctx, &mut ed, vec![Event::Key { key: Key::A, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND }]);
        frame(&ctx, &mut ed, vec![Event::Text("Boot".into())]);
        frame(&ctx, &mut ed, vec![Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
        frame(&ctx, &mut ed, vec![]);
        assert_eq!(ed.project.as_ref().unwrap().workspaces["startup"].name, "Boot");

        // Collapsing Events hides Boot; a new folder is added and named in place.
        click(&ctx, &mut ed, row_at(4));
        let collapsed: Vec<String> = ctx.data_mut(|d| d.get_persisted::<Vec<String>>(Id::unique(COLLAPSED_KEY)).unwrap_or_default());
        assert!(collapsed.contains(&"Events".to_string()));
        click(&ctx, &mut ed, pos2(260.0 - 16.0, row_at(0).y));
        assert_eq!(ed.project.as_ref().unwrap().meta.folders, ["Commands", "Events", "New folder"]);
        assert!(ed.unsaved_now());
    }

    /// A folder dragged onto the middle of another goes in it, with its
    /// workspaces; onto the project, back to the top. Slash command folders
    /// nest the same way.
    #[test]
    fn folders_go_into_folders() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        let ctx = egui::Context::default();
        frame(&ctx, &mut ed, vec![]);
        frame(&ctx, &mut ed, vec![]);
        let drag = |ctx: &egui::Context, ed: &mut Editor, from: Pos2, to: Pos2| {
            frame(ctx, ed, vec![Event::PointerMoved(from)]);
            frame(ctx, ed, vec![press(from, true)]);
            for i in 1..=8 {
                frame(ctx, ed, vec![Event::PointerMoved(from + (to - from) * (i as f32 / 8.0))]);
            }
            frame(ctx, ed, vec![press(to, false)]);
            frame(ctx, ed, vec![]);
        };
        let folder = |ed: &Editor, key: &str| ed.project.as_ref().unwrap().workspaces[key].folder.clone();

        // Rows: DemoBot, Commands, Rank command, Events, Startup, Welcome. Events onto the middle of Commands.
        drag(&ctx, &mut ed, row_at(3), row_at(1));
        assert_eq!(folder(&ed, "welcome").as_deref(), Some("Commands/Events"), "its workspaces go along");
        assert_eq!(ws_folders(ed.project.as_ref().unwrap()), ["Commands", "Commands/Events"]);

        // A folder in it, made in place; deleting Commands lifts what's in it up a level.
        apply(&mut ed, &ctx, vec![Do::NewFolder(Some("Commands/Events".into()))], &mut None);
        assert!(ed.project.as_ref().unwrap().meta.folders.contains(&"Commands/Events/New folder".to_string()));
        apply(&mut ed, &ctx, vec![Do::Rename(Target::Folder("Commands/Events/New folder".into()), "Old/ones".into())], &mut None);
        assert!(ed.project.as_ref().unwrap().meta.folders.contains(&"Commands/Events/Old-ones".to_string()), "a / in a name doesn't make a folder");
        apply(&mut ed, &ctx, vec![Do::DeleteFolder("Commands".into())], &mut None);
        assert_eq!(ws_folders(ed.project.as_ref().unwrap()), ["Events", "Events/Old-ones"]);
        assert_eq!(folder(&ed, "welcome").as_deref(), Some("Events"));
        assert_eq!(folder(&ed, "rank-command"), None, "Commands' own workspace goes to the top");

        // Rows now: DemoBot, Events, Old-ones, Startup, Welcome, Rank command. Old-ones onto the project: to the top.
        frame(&ctx, &mut ed, vec![]);
        drag(&ctx, &mut ed, row_at(2), row_at(0));
        assert_eq!(ws_folders(ed.project.as_ref().unwrap()), ["Events", "Old-ones"]);

        // Slash command folders in folders.
        let commands = |ed: &Editor| crate::builders::slash::commands_of(ed.project.as_ref().unwrap());
        let first = crate::builders::slash::roots(&commands(&ed))[0].clone();
        apply(&mut ed, &ctx, vec![Do::SlashNewFolder(None)], &mut None);
        apply(&mut ed, &ctx, vec![Do::SlashNewFolder(Some("New folder".into()))], &mut None);
        apply(&mut ed, &ctx, vec![Do::SlashPlace(first.clone(), Some("New folder/New folder".into()), None)], &mut None);
        let folder_of = |ed: &Editor| commands(ed).iter().find(|c| c.name.split_whitespace().next() == Some(first.as_str())).and_then(|c| c.folder.clone());
        assert_eq!(folder_of(&ed).as_deref(), Some("New folder/New folder"));
        frame(&ctx, &mut ed, vec![]);
        apply(&mut ed, &ctx, vec![Do::SlashNestFolder("New folder/New folder".into(), None)], &mut None);
        assert_eq!(folder_of(&ed).as_deref(), Some("New folder 2"), "to the top, next to one of the same name");
        apply(&mut ed, &ctx, vec![Do::SlashDeleteFolder("New folder 2".into())], &mut None);
        assert_eq!(folder_of(&ed), None);
        // Drawing the nested lists doesn't fail.
        apply(&mut ed, &ctx, vec![Do::SlashNewFolder(Some("New folder".into()))], &mut None);
        frame(&ctx, &mut ed, vec![]);
    }

    /// While dragging, the preview says what letting go would do.
    #[test]
    fn dragging_says_where_it_goes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        let ctx = egui::Context::default();
        frame(&ctx, &mut ed, vec![]);
        frame(&ctx, &mut ed, vec![]);
        let said = |ctx: &egui::Context| ctx.data(|d| d.get_temp::<DropHint>(Id::unique(HINT_KEY))).map(|h| (h.text, h.ok));
        // Rows: DemoBot, Commands, Rank command, Events, Startup, Welcome. Welcome held over …
        let from = row_at(5);
        frame(&ctx, &mut ed, vec![Event::PointerMoved(from)]);
        frame(&ctx, &mut ed, vec![press(from, true)]);
        let mut over = |to: Pos2| {
            for i in 1..=4 {
                frame(&ctx, &mut ed, vec![Event::PointerMoved(from + (to - from) * (i as f32 / 4.0))]);
            }
            frame(&ctx, &mut ed, vec![]);
            said(&ctx)
        };
        assert_eq!(over(row_at(1)), Some(("Into Commands".to_string(), true)), "a folder");
        assert_eq!(over(row_at(2) - vec2(0.0, 6.0)), Some(("Above Rank command, in Commands".to_string(), true)), "a workspace in another folder");
        assert_eq!(over(row_at(0)), Some(("Out of its folder, to the top".to_string(), true)), "the project");
        frame(&ctx, &mut ed, vec![press(row_at(0), false)]);
        frame(&ctx, &mut ed, vec![]);
        assert_eq!(ed.project.as_ref().unwrap().workspaces["welcome"].folder, None);
        assert_eq!(said(&ctx), None, "nothing said once let go");

        // Held over a closed folder for a moment, it opens.
        apply(&mut ed, &ctx, vec![Do::Toggle("Events".into())], &mut None);
        let closed = |ctx: &egui::Context| ctx.data_mut(|d| d.get_persisted::<Vec<String>>(Id::unique(COLLAPSED_KEY)).unwrap_or_default()).contains(&"Events".to_string());
        assert!(closed(&ctx));
        // Rows: DemoBot, Commands, Rank command, Events (closed), Welcome.
        let (from, to) = (row_at(2), row_at(3));
        frame(&ctx, &mut ed, vec![Event::PointerMoved(from)]);
        frame(&ctx, &mut ed, vec![press(from, true)]);
        for i in 1..=4 {
            frame(&ctx, &mut ed, vec![Event::PointerMoved(from + (to - from) * (i as f32 / 4.0))]);
        }
        assert!(closed(&ctx), "not at once");
        for _ in 0..60 {
            frame(&ctx, &mut ed, vec![]);
        }
        assert!(!closed(&ctx), "open after a moment");
        frame(&ctx, &mut ed, vec![press(to, false)]);
    }

    /// Duplicate makes a copy with every block, turned off, and opens it;
    /// the list draws marks for problems, activity and turned-off workspaces.
    #[test]
    fn duplicates_and_marks() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        let ctx = egui::Context::default();
        let blocks = ed.project.as_ref().unwrap().workspaces["welcome"].nodes.len();
        apply(&mut ed, &ctx, vec![Do::Duplicate("welcome".into())], &mut None);
        let p = ed.project.as_ref().unwrap();
        let (key, copy) = p.workspaces.iter().find(|(_, w)| w.name == "Welcome copy").expect("a copy");
        assert!(copy.disabled, "turned off");
        assert_eq!(copy.nodes.len(), blocks);
        assert_eq!(copy.folder.as_deref(), Some("Events"), "in the same folder");
        assert!(ed.actions.iter().any(|a| matches!(a, Action::OpenWorkspace(k) if k == key)));
        apply(&mut ed, &ctx, vec![Do::Duplicate("welcome".into())], &mut None);
        assert!(ed.project.as_ref().unwrap().workspaces.values().any(|w| w.name == "Welcome copy 2"), "a second copy gets its own name");

        // Marks: room for each, and drawing them all doesn't fail.
        let all = Marks { problems: 3, live: true, failed: true, off: true };
        assert!(all.width() > Marks::default().width());
        assert_eq!(Marks::default().width(), 0.0);
        let node = ed.project.as_ref().unwrap().workspaces["welcome"].nodes.keys().next().unwrap().clone();
        ed.activity.insert(("welcome".into(), node), crate::bot::Activity { at: std::time::Instant::now(), error: Some("Missing Permissions".into()), ms: None });
        let mut problems = HashMap::new();
        problems.insert("welcome", 2);
        crate::ui::fonts::install(&ctx);
        for _ in 0..2 {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(260.0, 600.0))), ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| panel(ui, &mut ed, &problems));
            out.textures_delta.clear();
        }
    }
}
