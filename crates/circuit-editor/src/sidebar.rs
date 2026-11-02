//! The Workspaces panel: the project's workspaces as a tree with folders, and
//! its slash commands. Drag a workspace onto a folder to move it there;
//! double-click a name to rename it.

use std::path::PathBuf;

use circuit_core::Workspace;
use eframe::egui::{self, Align2, Color32, CursorIcon, FontId, Id, Key, Rect, Response, RichText, ScrollArea, Sense, Stroke, Ui, pos2, vec2};
use serde_json::Value;

use crate::editor::{Action, Editor, file_uri};
use crate::theme::{Theme, with_alpha};

const ROW_H: f32 = 26.0;
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
    DeleteWorkspace(String),
    NewWorkspace(Option<String>),
    NewFolder,
    DeleteFolder(String),
    Toggle(String),
    SlashCommands,
}

/// Icon files by name, looked up before the project is borrowed.
struct Icons {
    paths: std::collections::HashMap<&'static str, PathBuf>,
}

impl Icons {
    fn new(ed: &Editor) -> Icons {
        let names = ["workspace", "folder", "folder-plus", "plus", "slash"];
        Icons { paths: names.iter().filter_map(|n| Some((*n, ed.reg.icons.get(*n)?.clone()))).collect() }
    }

    fn paint(&self, ui: &Ui, name: &str, center: egui::Pos2, size: f32, tint: Color32) {
        if let Some(path) = self.paths.get(name) {
            egui::Image::from_uri(file_uri(path)).tint(tint).paint_at(ui, Rect::from_center_size(center, vec2(size, size)));
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
fn row(ui: &mut Ui, theme: &Theme, icons: &Icons, id: Id, depth: usize, open: Option<bool>, icon: Option<&str>, label: &str, active: bool, strong: bool) -> (Response, Rect) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let drop_target = resp.dnd_hover_payload::<String>().is_some();
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
    let color = if strong { theme.text_muted } else { theme.text };
    let text = if strong { label.to_uppercase() } else { label.to_string() };
    let galley = ui.painter().layout_no_wrap(text, font, color);
    let clip = Rect::from_min_max(pos2(x, rect.top()), pos2(rect.right() - 4.0, rect.bottom()));
    ui.painter().with_clip_rect(clip).galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, color);
    (resp, Rect::from_min_max(pos2(x - 3.0, rect.top() + 2.0), pos2(rect.right() - 6.0, rect.bottom() - 2.0)))
}

/// A small icon button drawn over the right end of a row.
fn row_button(ui: &mut Ui, theme: &Theme, icons: &Icons, rect: Rect, id: Id, icon: &str, tip: &str) -> bool {
    let resp = ui.interact(rect, id, Sense::click()).on_hover_text(tip).on_hover_cursor(CursorIcon::PointingHand);
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, theme.field);
    }
    icons.paint(ui, icon, rect.center(), 14.0, if resp.hovered() { theme.text } else { theme.text_muted });
    resp.clicked()
}

/// The name editor shown in place of a row's label.
fn rename_box(ui: &mut Ui, area: Rect, state: &mut Renaming) -> Option<bool> {
    let id = Id::new("sidebar-rename");
    let resp = ui.put(area, egui::TextEdit::singleline(&mut state.text).id(id).margin(egui::Margin::symmetric(4, 2)));
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

pub fn panel(ui: &mut Ui, ed: &mut Editor) {
    let theme = ed.theme.clone();
    let icons = Icons::new(ed);
    let Some(project) = ed.project.as_ref() else {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
            ui.label(RichText::new("No project open").color(theme.text_muted));
            ui.label("Use File › Open Project or File › New Project.");
        });
        return;
    };
    let ctx = ui.ctx().clone();
    let collapsed: Vec<String> = ctx.data_mut(|d| d.get_persisted::<Vec<String>>(Id::new(COLLAPSED_KEY)).unwrap_or_default());
    let is_open = |key: &str| !collapsed.iter().any(|c| c == key);
    let mut renaming: Option<Renaming> = ctx.data(|d| d.get_temp(Id::new("sidebar-renaming")));
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

    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add_space(4.0);

        // The project: collapses everything; dropping a workspace here takes it out of its folder.
        let (resp, _) = row(ui, &theme, &icons, Id::new("sidebar-project"), 0, Some(is_open(PROJECT_ROW)), None, &project.meta.name, false, true);
        let buttons = resp.rect;
        if row_button(ui, &theme, &icons, Rect::from_center_size(pos2(buttons.right() - 16.0, buttons.center().y), vec2(22.0, 22.0)), Id::new("sidebar-new-folder"), "folder-plus", "New folder") {
            todo.push(Do::NewFolder);
        } else if row_button(ui, &theme, &icons, Rect::from_center_size(pos2(buttons.right() - 40.0, buttons.center().y), vec2(22.0, 22.0)), Id::new("sidebar-new-ws"), "plus", "New workspace") {
            todo.push(Do::NewWorkspace(None));
        } else if resp.clicked() {
            todo.push(Do::Toggle(PROJECT_ROW.into()));
        }
        if let Some(key) = resp.dnd_release_payload::<String>() {
            todo.push(Do::Move((*key).clone(), None));
        }

        if is_open(PROJECT_ROW) {
            let workspace_row = |ui: &mut Ui, key: &String, ws: &Workspace, depth: usize, todo: &mut Vec<Do>, renaming: &mut Option<Renaming>| {
                let target = Target::Workspace(key.clone());
                let editing = renaming.as_ref().is_some_and(|r| r.target == target);
                let label = if editing { "" } else { ws.name.as_str() };
                let (resp, label_area) = row(ui, &theme, &icons, Id::new(("sidebar-ws", key)), depth, None, Some("workspace"), label, active.as_deref() == Some(key.as_str()), false);
                if editing && let Some(r) = renaming.as_mut() {
                    match rename_box(ui, label_area, r) {
                        Some(true) => todo.push(Do::Rename(target.clone(), r.text.clone())),
                        Some(false) => *renaming = None,
                        None => {}
                    }
                    return;
                }
                resp.dnd_set_drag_payload(key.clone());
                if resp.double_clicked() {
                    todo.push(Do::StartRename(target.clone(), ws.name.clone()));
                } else if resp.clicked() {
                    todo.push(Do::Open(key.clone()));
                }
                resp.context_menu(|ui| {
                    if ui.button("Open").clicked() {
                        todo.push(Do::Open(key.clone()));
                        ui.close();
                    }
                    if ui.button("Rename").clicked() {
                        todo.push(Do::StartRename(target.clone(), ws.name.clone()));
                        ui.close();
                    }
                    ui.menu_button("Move to", |ui| {
                        if ws.folder.is_some() && ui.button("No folder").clicked() {
                            todo.push(Do::Move(key.clone(), None));
                            ui.close();
                        }
                        for f in &folders {
                            if ws.folder.as_ref() != Some(f) && ui.button(f).clicked() {
                                todo.push(Do::Move(key.clone(), Some(f.clone())));
                                ui.close();
                            }
                        }
                    });
                    ui.separator();
                    if ui.button(RichText::new("Delete workspace").color(theme.error)).clicked() {
                        todo.push(Do::DeleteWorkspace(key.clone()));
                        ui.close();
                    }
                });
            };

            for folder in &folders {
                let target = Target::Folder(folder.clone());
                let editing = renaming.as_ref().is_some_and(|r| r.target == target);
                let open = is_open(folder);
                let (resp, label_area) = row(ui, &theme, &icons, Id::new(("sidebar-folder", folder)), 1, Some(open), Some("folder"), if editing { "" } else { folder }, false, false);
                if editing && let Some(r) = renaming.as_mut() {
                    match rename_box(ui, label_area, r) {
                        Some(true) => todo.push(Do::Rename(target.clone(), r.text.clone())),
                        Some(false) => renaming = None,
                        None => {}
                    }
                } else {
                    if resp.double_clicked() {
                        todo.push(Do::StartRename(target.clone(), folder.clone()));
                    } else if resp.clicked() {
                        todo.push(Do::Toggle(folder.clone()));
                    }
                    if let Some(key) = resp.dnd_release_payload::<String>() {
                        todo.push(Do::Move((*key).clone(), Some(folder.clone())));
                    }
                    resp.context_menu(|ui| {
                        if ui.button("New workspace here").clicked() {
                            todo.push(Do::NewWorkspace(Some(folder.clone())));
                            ui.close();
                        }
                        if ui.button("Rename").clicked() {
                            todo.push(Do::StartRename(target.clone(), folder.clone()));
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Delete folder").on_hover_text("Its workspaces move out of it; none are deleted").clicked() {
                            todo.push(Do::DeleteFolder(folder.clone()));
                            ui.close();
                        }
                    });
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
                    ui.label(RichText::new("No workspaces yet. Click + above.").color(theme.text_muted).size(12.0));
                });
            }
        }

        // Slash commands, for modes that have them.
        let commands: Vec<String> = project
            .meta
            .data
            .get(crate::slash::DATA_KEY)
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(|c| c.get("name").and_then(Value::as_str).map(str::to_string)).collect())
            .unwrap_or_default();
        if !commands.is_empty() || project.meta.mode.contains("discord") {
            ui.add_space(10.0);
            let line = ui.available_rect_before_wrap();
            ui.painter().hline(line.x_range(), line.top(), Stroke::new(1.0, theme.line));
            ui.add_space(4.0);
            let open = is_open(SLASH_ROW);
            let (resp, area) = row(ui, &theme, &icons, Id::new("sidebar-slash"), 0, Some(open), None, "Slash commands", false, true);
            ui.painter().text(pos2(area.right() - 4.0, area.center().y), Align2::RIGHT_CENTER, commands.len().to_string(), FontId::proportional(11.5), theme.text_muted);
            if resp.clicked() {
                todo.push(Do::Toggle(SLASH_ROW.into()));
            }
            if open {
                for name in &commands {
                    let (resp, _) = row(ui, &theme, &icons, Id::new(("sidebar-cmd", name)), 1, None, Some("slash"), name, false, false);
                    if resp.on_hover_text("Edit in the Slash Command Builder").clicked() {
                        todo.push(Do::SlashCommands);
                    }
                }
                let (resp, _) = row(ui, &theme, &icons, Id::new("sidebar-cmd-new"), 1, None, Some("plus"), "Edit commands…", false, false);
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
            d.insert_temp(Id::new("sidebar-renaming"), r);
        }
        None => d.remove::<Renaming>(Id::new("sidebar-renaming")),
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
                            ed.dirty = true;
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
                        ed.dirty = true;
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
                    ed.dirty = true;
                }
            }
            Do::DeleteWorkspace(key) => {
                // The file goes on save, so Don't save brings the workspace back.
                project.remove_workspace(&key);
                ed.actions.push(Action::CloseWorkspaceTabs(key));
                ed.dirty = true;
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
                ed.dirty = true;
            }
            Do::NewFolder => {
                let name = unique("New folder", |n| project.meta.folders.iter().any(|f| f == n));
                project.meta.folders.push(name.clone());
                uncollapse(ctx, PROJECT_ROW);
                *renaming = Some(Renaming { target: Target::Folder(name.clone()), text: name, fresh: true });
                ed.dirty = true;
            }
            Do::DeleteFolder(name) => {
                project.meta.folders.retain(|f| *f != name);
                for ws in project.workspaces.values_mut().filter(|w| w.folder.as_ref() == Some(&name)) {
                    ws.folder = None;
                }
                ed.dirty = true;
            }
            Do::Toggle(key) => ctx.data_mut(|d| {
                let list = d.get_persisted_mut_or_default::<Vec<String>>(Id::new(COLLAPSED_KEY));
                match list.iter().position(|c| *c == key) {
                    Some(i) => {
                        list.remove(i);
                    }
                    None => list.push(key),
                }
            }),
            Do::SlashCommands => crate::slash::open(ed),
        }
    }
}

fn uncollapse(ctx: &egui::Context, key: &str) {
    ctx.data_mut(|d| d.get_persisted_mut_or_default::<Vec<String>>(Id::new(COLLAPSED_KEY)).retain(|c| c != key));
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Event, Modifiers, PointerButton, Pos2};

    use super::*;

    fn frame(ctx: &egui::Context, ed: &mut Editor, events: Vec<Event>) {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(260.0, 600.0))), events, ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| panel(ui, ed));
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
        let collapsed: Vec<String> = ctx.data_mut(|d| d.get_persisted::<Vec<String>>(Id::new(COLLAPSED_KEY)).unwrap_or_default());
        assert!(collapsed.contains(&"Events".to_string()));
        click(&ctx, &mut ed, pos2(260.0 - 16.0, row_at(0).y));
        assert_eq!(ed.project.as_ref().unwrap().meta.folders, ["Commands", "Events", "New folder"]);
        assert!(ed.dirty);
    }
}
