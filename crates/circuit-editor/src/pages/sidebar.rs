//! The Workspaces panel: the project's workspaces as a tree with folders, and
//! its slash commands. Drag a workspace onto a folder to move it there, or
//! above or below another workspace to put it there (a line shows where);
//! drag folders to order them too. Double-click a name to rename it. Each workspace shows its problems,
//! whether its blocks just ran (or failed), and whether it's turned off.

use std::collections::HashMap;
use std::path::PathBuf;

use circuit_core::Workspace;
use eframe::egui::{self, Align2, Color32, CursorIcon, FontId, Id, Key, Rect, Response, RichText, ScrollArea, Sense, Stroke, Ui, pos2, vec2};
use serde_json::Value;

use crate::ui::kit;
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
    /// Turns a workspace on (true) or off.
    SetEnabled(String, bool),
    DeleteWorkspace(String),
    Duplicate(String),
    NewWorkspace(Option<String>),
    NewFolder,
    DeleteFolder(String),
    Toggle(String),
    SlashCommands,
    Export(crate::editor::exchange::Preselect),
    Import,
}

/// What a dragged folder carries (a dragged workspace carries its key).
#[derive(Clone)]
struct FolderDrag(String);

/// Whether something dropped on `rect` goes before it: the pointer is in its upper half.
fn drops_before(ui: &Ui, rect: Rect) -> bool {
    ui.ctx().pointer_hover_pos().is_some_and(|p| p.y < rect.center().y)
}

/// The line that shows where a dragged row will land: above or below `rect`.
fn insertion_line(ui: &Ui, theme: &Theme, rect: Rect, before: bool) {
    let y = if before { rect.top() } else { rect.bottom() };
    let left = rect.left() + 8.0;
    ui.painter().hline(left..=rect.right() - 8.0, y, Stroke::new(2.0, theme.accent));
    ui.painter().circle_filled(pos2(left, y), 3.5, theme.accent);
}

/// Icon files by name, looked up before the project is borrowed.
struct Icons {
    paths: std::collections::HashMap<&'static str, PathBuf>,
}

impl Icons {
    fn new(ed: &Editor) -> Icons {
        let names = ["workspace", "folder", "folder-plus", "plus", "slash", "more", "pencil", "copy", "eye", "eye-off", "package", "trash", "pointer"];
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
        (if self.off { 30.0 } else { 0.0 }) + (if self.problems > 0 { 26.0 } else { 0.0 }) + (if self.live || self.failed { 14.0 } else { 0.0 })
    }

    /// Paints them, right to left from `right`.
    fn paint(&self, ui: &Ui, theme: &Theme, right: f32, y: f32) {
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
            x = pill(ui, x, y, &self.problems.to_string(), theme.warning, Color32::from_rgb(0x1E, 0x1F, 0x22)) - 4.0;
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
    // A workspace dragged over a folder or the project goes into it (the row
    // fills); over another workspace it goes next to it (a line shows where).
    let drop_target = resp.dnd_hover_payload::<String>().is_some() && icon != Some("workspace");
    if active {
        ui.painter().rect_filled(rect, 0.0, with_alpha(theme.accent, 38));
        ui.painter().rect_filled(Rect::from_min_size(rect.min, vec2(2.0, rect.height())), 0.0, theme.accent);
    } else if drop_target {
        ui.painter().rect_filled(rect, 0.0, with_alpha(theme.accent, 60));
    } else if resp.hovered() {
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
pub fn panel(ui: &mut Ui, ed: &mut Editor, problems: &HashMap<String, usize>) {
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

    // Folders in their saved order, plus any a workspace names that the list lacks.
    let mut folders = project.meta.folders.clone();
    for ws in project.workspaces.values() {
        if let Some(f) = &ws.folder
            && !folders.contains(f)
        {
            folders.push(f.clone());
        }
    }
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
            todo.push(Do::NewFolder);
        } else if row_button(ui, &theme, &icons, Rect::from_center_size(pos2(buttons.right() - 40.0, buttons.center().y), vec2(22.0, 22.0)), Id::unique("sidebar-new-ws"), "plus", t!("New workspace")).clicked() {
            todo.push(Do::NewWorkspace(None));
        } else if resp.clicked() {
            todo.push(Do::Toggle(PROJECT_ROW.into()));
        }
        if egui::DragAndDrop::has_payload_of_type::<String>(ui.ctx())
            && let Some(key) = resp.dnd_release_payload::<String>()
        {
            todo.push(Do::Move((*key).clone(), None));
        }
        resp.context_menu(|ui| {
            ui.set_min_width(200.0);
            if item(ui, &theme, &icons, "plus", t!("New workspace"), false).clicked() {
                todo.push(Do::NewWorkspace(None));
                ui.close();
            }
            if item(ui, &theme, &icons, "folder-plus", t!("New folder"), false).clicked() {
                todo.push(Do::NewFolder);
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
                    problems: problems.get(key).copied().unwrap_or(0),
                    live: activity.is_some_and(|(at, _, _)| at.elapsed() < crate::bot::GLOW),
                    failed: activity.is_some_and(|(_, failed, _)| *failed),
                    off: ws.disabled,
                };
                let label = if editing { "" } else { ws.name.as_str() };
                let room = 32.0 + marks.width();
                let (resp, label_area) = row(ui, &theme, &icons, Id::unique(("sidebar-ws", key)), depth, None, Some("workspace"), label, active.as_deref() == Some(key.as_str()), false, room, ws.disabled);
                if !editing {
                    marks.paint(ui, &theme, resp.rect.right() - 34.0, resp.rect.center().y);
                }
                // On hover: what's in it, its problems, what the bot did with it.
                let events = ws.nodes.values().filter(|n| reg.block(&n.block).is_some_and(|d| d.kind == circuit_core::BlockKind::Event)).count();
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
                    insertion_line(ui, &theme, resp.rect, drops_before(ui, resp.rect));
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
                    todo.push(Do::Open(key.clone()));
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
                        for f in &folders {
                            if ws.folder.as_ref() != Some(f) && kit::menu_item(ui, f, None, true).clicked() {
                                todo.push(Do::Move(key.clone(), Some(f.clone())));
                                ui.close();
                            }
                        }
                        if folders.iter().all(|f| ws.folder.as_ref() == Some(f)) && ws.folder.is_none() {
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
                    if item(ui, &theme, &icons, "trash", RichText::new(t!("Delete workspace")).color(theme.error), true).clicked() {
                        todo.push(Do::DeleteWorkspace(key.clone()));
                        ui.close();
                    }
                };
                resp.context_menu(|ui| menu(ui));
                egui::Popup::menu(&more).show(|ui| menu(ui));
            };

            for folder in &folders {
                let target = Target::Folder(folder.clone());
                let editing = renaming.as_ref().is_some_and(|r| r.target == target);
                let open = is_open(folder);
                let inside: Vec<&String> = project.workspaces.iter().filter(|(_, w)| w.folder.as_ref() == Some(folder)).map(|(k, _)| k).collect();
                let folder_problems: usize = inside.iter().map(|k| problems.get(*k).copied().unwrap_or(0)).sum();
                let (resp, label_area) = row(ui, &theme, &icons, Id::unique(("sidebar-folder", folder)), 1, Some(open), Some("folder"), if editing { "" } else { folder }, false, false, 60.0, false);
                if !editing {
                    // How many workspaces it holds, and (closed) their problems.
                    let y = resp.rect.center().y;
                    let mut x = resp.rect.right() - 34.0;
                    if !open && folder_problems > 0 {
                        x = pill(ui, x, y, &folder_problems.to_string(), theme.warning, Color32::from_rgb(0x1E, 0x1F, 0x22)) - 4.0;
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
                        todo.push(Do::StartRename(target.clone(), folder.clone()));
                    } else if resp.clicked() {
                        todo.push(Do::Toggle(folder.clone()));
                    }
                    // egui's dnd_release_payload takes whatever is dragged, even of
                    // another type: ask for the one that's there.
                    if egui::DragAndDrop::has_payload_of_type::<String>(ui.ctx())
                        && let Some(key) = resp.dnd_release_payload::<String>()
                    {
                        todo.push(Do::Move((*key).clone(), Some(folder.clone())));
                    }
                    // Folders are ordered by dragging one above or below another.
                    resp.dnd_set_drag_payload(FolderDrag(folder.clone()));
                    if let Some(dragged) = resp.dnd_hover_payload::<FolderDrag>()
                        && dragged.0 != *folder
                    {
                        insertion_line(ui, &theme, resp.rect, drops_before(ui, resp.rect));
                    }
                    if egui::DragAndDrop::has_payload_of_type::<FolderDrag>(ui.ctx())
                        && let Some(dragged) = resp.dnd_release_payload::<FolderDrag>()
                        && dragged.0 != *folder
                    {
                        todo.push(Do::PlaceFolder(dragged.0.clone(), folder.clone(), drops_before(ui, resp.rect)));
                    }
                    let mut menu = |ui: &mut Ui| {
                        ui.set_min_width(200.0);
                        if item(ui, &theme, &icons, "plus", t!("New workspace here"), false).clicked() {
                            todo.push(Do::NewWorkspace(Some(folder.clone())));
                            ui.close();
                        }
                        if item(ui, &theme, &icons, "pencil", t!("Rename"), false).on_hover_text(t!("Or double-click its name")).clicked() {
                            todo.push(Do::StartRename(target.clone(), folder.clone()));
                            ui.close();
                        }
                        let filled = !inside.is_empty();
                        if ui.add_enabled_ui(filled, |ui| item(ui, &theme, &icons, "package", t!("Export folder…"), false)).inner.clicked() {
                            todo.push(Do::Export(crate::editor::exchange::Preselect::Folder(folder.clone())));
                            ui.close();
                        }
                        ui.separator();
                        if item(ui, &theme, &icons, "trash", RichText::new(t!("Delete folder")).color(theme.error), true).on_hover_text(t!("Its workspaces move out of it; none are deleted")).clicked() {
                            todo.push(Do::DeleteFolder(folder.clone()));
                            ui.close();
                        }
                    };
                    resp.context_menu(|ui| menu(ui));
                    egui::Popup::menu(&more).show(|ui| menu(ui));
                }
                if open {
                    for (key, ws) in project.workspaces.iter().filter(|(_, w)| w.folder.as_ref() == Some(folder)) {
                        workspace_row(ui, key, ws, 2, &mut todo, &mut renaming);
                    }
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

        // Slash commands, for modes that have them.
        let commands: Vec<String> = project
            .meta
            .data
            .get(crate::builders::slash::DATA_KEY)
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(|c| c.get("name").and_then(Value::as_str).map(str::to_string)).collect())
            .unwrap_or_default();
        if !commands.is_empty() || ed.has_slash_commands() {
            ui.add_space(10.0);
            let line = ui.available_rect_before_wrap();
            ui.painter().hline(line.x_range(), line.top(), Stroke::new(1.0, theme.line));
            ui.add_space(4.0);
            let open = is_open(SLASH_ROW);
            let (resp, area) = row(ui, &theme, &icons, Id::unique("sidebar-slash"), 0, Some(open), None, t!("Slash commands"), false, true, 30.0, false);
            ui.painter().text(pos2(area.right() - 4.0, area.center().y), Align2::RIGHT_CENTER, commands.len().to_string(), FontId::proportional(11.5), theme.text_muted);
            if resp.clicked() {
                todo.push(Do::Toggle(SLASH_ROW.into()));
            }
            if open {
                for name in &commands {
                    let (resp, _) = row(ui, &theme, &icons, Id::unique(("sidebar-cmd", name)), 1, None, Some("slash"), name, false, false, 0.0, false);
                    if resp.on_hover_text(t!("Edit in the Slash Command Builder")).clicked() {
                        todo.push(Do::SlashCommands);
                    }
                }
                let (resp, _) = row(ui, &theme, &icons, Id::unique("sidebar-cmd-new"), 1, None, Some("plus"), t!("Edit commands…"), false, false, 0.0, false);
                if resp.clicked() {
                    todo.push(Do::SlashCommands);
                }
            }
        }
        ui.add_space(8.0);
    });

    apply(ed, &ctx, todo, &mut renaming);
    ctx.data_mut(|d| match renaming {
        Some(r) => {
            d.insert_temp(Id::unique("sidebar-renaming"), r);
        }
        None => d.remove::<Renaming>(Id::unique("sidebar-renaming")),
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
                    Target::Folder(old) if old != name => {
                        for ws in project.workspaces.values_mut().filter(|w| w.folder.as_ref() == Some(&old)) {
                            ws.folder = Some(name.clone());
                        }
                        project.meta.folders.retain(|f| *f != name);
                        match project.meta.folders.iter().position(|f| *f == old) {
                            Some(i) => project.meta.folders[i] = name,
                            None => project.meta.folders.push(name),
                        }
                        ed.edited = true;
                    }
                    Target::Folder(_) => {}
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
                // The list may lack folders only a workspace names: complete it first.
                for ws in project.workspaces.values() {
                    if let Some(f) = &ws.folder
                        && !project.meta.folders.contains(f)
                    {
                        project.meta.folders.push(f.clone());
                    }
                }
                let folders = &mut project.meta.folders;
                if name != next_to
                    && let Some(i) = folders.iter().position(|f| *f == name)
                {
                    folders.remove(i);
                    let at = folders.iter().position(|f| *f == next_to).map_or(folders.len(), |j| if before { j } else { j + 1 });
                    folders.insert(at, name);
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
            Do::DeleteWorkspace(key) => {
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
            Do::NewFolder => {
                let name = unique(t!("New folder"), |n| project.meta.folders.iter().any(|f| f == n));
                project.meta.folders.push(name.clone());
                uncollapse(ctx, PROJECT_ROW);
                *renaming = Some(Renaming { target: Target::Folder(name.clone()), text: name, fresh: true });
                ed.edited = true;
            }
            Do::DeleteFolder(name) => {
                project.meta.folders.retain(|f| *f != name);
                for ws in project.workspaces.values_mut().filter(|w| w.folder.as_ref() == Some(&name)) {
                    ws.folder = None;
                }
                ed.edited = true;
            }
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

    /// The middle of tree row `i` (0 is the project).
    fn row_at(i: usize) -> Pos2 {
        pos2(120.0, 8.0 + 4.0 + ROW_H * i as f32 + ROW_H / 2.0)
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
        problems.insert("welcome".to_string(), 2);
        crate::ui::fonts::install(&ctx);
        for _ in 0..2 {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(260.0, 600.0))), ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| panel(ui, &mut ed, &problems));
            out.textures_delta.clear();
        }
    }
}
