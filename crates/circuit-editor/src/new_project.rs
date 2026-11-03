//! File › New Project, laid out like an IDE's: the kinds of project (build
//! modes, which plugins add) on the left; on the right its name, where it goes
//! and what it starts from.

use std::path::{Path, PathBuf};

use circuit_core::Project;
use circuit_plugins::Registry;
use eframe::egui::{self, Align, CursorIcon, Id, Layout, Margin, RichText, Sense, Stroke, Ui, vec2};

use crate::dialog;
use crate::editor::file_uri;
use crate::theme::{Theme, with_alpha};

/// The window while it is open.
pub struct NewProject {
    pub mode: String,
    /// The starter: 0 = empty, then this kind's templates.
    pub picked: usize,
    pub name: String,
    pub location: String,
}

/// What the window asked for when it closed.
pub enum Outcome {
    Create { dir: PathBuf, name: String, mode: String, template: Option<PathBuf> },
    /// "More via plugins…": the plugin manager, to add other kinds.
    MorePlugins,
    Cancel,
}

/// A starter project to copy: a folder with `template.json` and the project.
pub struct Template {
    pub dir: PathBuf,
    pub name: String,
    pub description: String,
    /// The kind of project it is: `template.json`'s `mode`, else the project's.
    pub mode: String,
}

impl Template {
    fn read(dir: &Path) -> Option<Template> {
        let info: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("template.json")).ok()?).ok()?;
        let text = |k: &str| info.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let mode = match text("mode") {
            m if !m.is_empty() => m,
            _ => Project::load(dir).ok()?.meta.mode,
        };
        Some(Template { dir: dir.to_path_buf(), name: text("name"), description: text("description"), mode })
    }
}

/// The starters next to the examples and in plugins' `templates` folders.
pub fn templates(examples: Option<&Path>, reg: &Registry) -> Vec<Template> {
    let dirs = examples.map(|e| e.join("templates")).into_iter().chain(reg.plugins.iter().map(|p| p.dir.join("templates")));
    let mut list: Vec<Template> = dirs.flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten()).filter_map(|e| Template::read(&e.path())).collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

/// Characters Windows doesn't allow in a folder name.
const NOT_IN_NAMES: &str = "<>:\"/\\|?*";

impl NewProject {
    pub fn new(reg: &Registry, location: PathBuf) -> NewProject {
        let mode = reg.modes.first().map(|m| m.id.clone()).unwrap_or_default();
        // "New Project", or "New Project 2" … when that folder is taken.
        let name = (1..).map(|i| if i == 1 { "New Project".to_string() } else { format!("New Project {i}") }).find(|n| !location.join(n).exists()).expect("unbounded");
        NewProject { mode, picked: 0, name, location: location.display().to_string() }
    }

    /// The folder the project goes in, or why it can't be made.
    fn target(&self) -> Result<PathBuf, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(t!("Give the project a name.").into());
        }
        if name.chars().any(|c| NOT_IN_NAMES.contains(c)) || name.ends_with('.') {
            return Err(tf!("A name can't contain {chars} or end with a dot.", chars = "< > : \" / \\ | ? *"));
        }
        if self.location.trim().is_empty() {
            return Err(t!("Choose where it goes.").into());
        }
        let dir = Path::new(self.location.trim()).join(name);
        if std::fs::read_dir(&dir).is_ok_and(|mut d| d.next().is_some()) {
            return Err(t!("That folder already exists and isn't empty.").into());
        }
        Ok(dir)
    }
}

/// Where new projects go when none went anywhere yet: next to the open
/// project, else in Documents.
pub fn default_location(open_project: Option<&Path>) -> PathBuf {
    if let Some(parent) = open_project.and_then(Path::parent) {
        return parent.to_path_buf();
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    let documents = home.join("Documents");
    if documents.is_dir() { documents } else { home }
}

const SIDEBAR_W: f32 = 200.0;
const HEIGHT: f32 = 600.0;

pub fn show(ctx: &egui::Context, theme: &Theme, state: &mut NewProject, reg: &Registry, templates: &[Template]) -> Option<Outcome> {
    let mut outcome = None;
    let target = state.target();
    let close = dialog::show(
        ctx,
        theme,
        dialog::Spec { id: "new-project", title: t!("New project"), subtitle: None, icon: None, width: 820.0 },
        |ui, part| match part {
            dialog::Part::Body => {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if sidebar(ui, theme, state, reg) {
                        outcome = Some(Outcome::MorePlugins);
                    }
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width());
                        ui.set_min_height(HEIGHT);
                        if form(ui, theme, state, reg, templates, &target) {
                            outcome = create(state, templates, &target);
                        }
                    });
                });
            }
            dialog::Part::Footer => {
                ui.add_enabled_ui(target.is_ok(), |ui| {
                    if dialog::primary_button(ui, theme, t!("Create")).clicked() {
                        outcome = create(state, templates, &target);
                    }
                });
                if dialog::secondary_button(ui, theme, t!("Cancel")).clicked() {
                    outcome = Some(Outcome::Cancel);
                }
            }
        },
    );
    if close && outcome.is_none() {
        outcome = Some(Outcome::Cancel);
    }
    outcome
}

fn create(state: &NewProject, templates: &[Template], target: &Result<PathBuf, String>) -> Option<Outcome> {
    let dir = target.as_ref().ok()?.clone();
    let template = state.picked.checked_sub(1).and_then(|i| templates.iter().filter(|t| t.mode == state.mode).nth(i)).map(|t| t.dir.clone());
    Some(Outcome::Create { dir, name: state.name.trim().to_string(), mode: state.mode.clone(), template })
}

/// The kinds of project, one row each. Returns true for "More via plugins…".
fn sidebar(ui: &mut Ui, theme: &Theme, state: &mut NewProject, reg: &Registry) -> bool {
    let mut more = false;
    egui::Frame::new().fill(theme.panel_header).corner_radius(8).inner_margin(Margin::same(8)).show(ui, |ui| ui.vertical(|ui| {
        let width = SIDEBAR_W - 16.0;
        ui.set_width(width);
        let top = ui.cursor().top();
        ui.spacing_mut().item_spacing.y = 2.0;
        dialog::section(ui, theme, t!("Type"));
        for mode in &reg.modes {
            let on = state.mode == mode.id;
            let (rect, resp) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click());
            if on {
                ui.painter().rect_filled(rect, 6.0, with_alpha(theme.accent, 40));
            } else if resp.hovered() {
                ui.painter().rect_filled(rect, 6.0, theme.field);
            }
            let icon_rect = egui::Rect::from_center_size(rect.left_center() + vec2(18.0, 0.0), vec2(16.0, 16.0));
            if let Some(icon) = mode.icon.as_ref().and_then(|i| reg.icons.get(i)) {
                egui::Image::from_uri(file_uri(icon)).tint(if on { theme.accent } else { theme.text_muted }).paint_at(ui, icon_rect);
            }
            ui.painter().text(rect.left_center() + vec2(36.0, 0.0), egui::Align2::LEFT_CENTER, crate::i18n::tr(&mode.name), egui::FontId::proportional(13.5), theme.text);
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() && !on {
                state.mode = mode.id.clone();
                state.picked = 0;
            }
        }
        // At the bottom, like an IDE's.
        ui.add_space((top + HEIGHT - 16.0 - 20.0 - ui.cursor().top()).max(8.0));
        more = ui.link(RichText::new(t!("More via plugins…")).size(12.5).color(theme.accent)).on_hover_text(t!("Plugins can add other kinds of projects")).clicked();
    }));
    more
}

/// Name, location and starter. Returns true when a starter was double-clicked.
fn form(ui: &mut Ui, theme: &Theme, state: &mut NewProject, reg: &Registry, templates: &[Template], target: &Result<PathBuf, String>) -> bool {
    let mut go = false;
    if let Some(mode) = reg.mode(&state.mode) {
        ui.label(RichText::new(crate::i18n::tr(&mode.name)).size(16.0).strong().color(theme.text));
        ui.label(RichText::new(crate::i18n::tr(&mode.description)).size(12.0).color(theme.text_muted));
        ui.add_space(8.0);
    }
    dialog::field(ui, theme, t!("Name"), None, |ui| dialog::text_input(ui, &mut state.name, "", false));
    dialog::field(ui, theme, t!("Location"), None, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.allocate_ui(vec2(ui.available_width() - 96.0, crate::inputs::input_height(ui)), |ui| dialog::text_input(ui, &mut state.location, "", false));
            if dialog::secondary_button(ui, theme, t!("Browse…")).clicked() {
                let mut picker = rfd::FileDialog::new().set_title(t!("Where the project goes"));
                if Path::new(&state.location).is_dir() {
                    picker = picker.set_directory(&state.location);
                }
                if let Some(dir) = picker.pick_folder() {
                    state.location = dir.display().to_string();
                }
            }
        });
    });
    match target {
        Ok(dir) => ui.label(RichText::new(tf!("Created in {path}", path = dir.display())).size(11.5).color(theme.text_muted)),
        Err(why) => ui.label(RichText::new(why).size(11.5).color(theme.warning)),
    };
    ui.add_space(10.0);

    dialog::section(ui, theme, t!("Start from"));
    let empty = (t!("Empty project").to_string(), t!("One empty workspace.").to_string());
    let starters = templates.iter().filter(|t| t.mode == state.mode).map(|t| (crate::i18n::tr(&t.name).into_owned(), crate::i18n::tr(&t.description).into_owned()));
    let rows: Vec<(String, String)> = std::iter::once(empty).chain(starters).collect();
    egui::ScrollArea::vertical().id_salt("new-project-starters").max_height(ui.available_height().max(160.0)).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        for (i, (name, description)) in rows.iter().enumerate() {
            let on = state.picked == i;
            let r = egui::Frame::new()
                .fill(if on { with_alpha(theme.accent, 30) } else { theme.panel_header })
                .stroke(Stroke::new(1.0, if on { theme.accent } else { theme.line }))
                .corner_radius(8)
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width() - 4.0);
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new(name).size(13.5).strong().color(theme.text));
                        ui.label(RichText::new(description).size(12.0).color(theme.text_muted));
                    });
                });
            let click = ui.interact(r.response.rect, Id::new(("new-project-starter", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            if click.clicked() {
                state.picked = i;
            }
            go |= click.double_clicked() && target.is_ok();
        }
    });
    go
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starters_know_their_kind() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let reg = crate::editor::Editor::new(vec![root.join("plugins")], None).unwrap().reg;
        let list = templates(Some(&root.join("examples")), &reg);
        assert!(list.iter().all(|t| reg.mode(&t.mode).is_some() && !t.name.is_empty()), "every starter is for an installed kind");
        let count = |mode: &str| list.iter().filter(|t| t.mode == mode).count();
        assert!(count("discord-bot") >= 5 && count("fluxer-bot") >= 2, "{} Discord, {} Fluxer", count("discord-bot"), count("fluxer-bot"));
    }

    #[test]
    fn checks_the_name_and_folder() {
        let dir = std::env::temp_dir().join(format!("xc-new-project-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Taken")).unwrap();
        std::fs::write(dir.join("Taken").join("x.txt"), "").unwrap();
        std::fs::create_dir_all(dir.join("Empty")).unwrap();
        let mut p = NewProject { mode: "discord-bot".into(), picked: 0, name: "My Bot".into(), location: dir.display().to_string() };
        assert_eq!(p.target(), Ok(dir.join("My Bot")));
        p.name = "Empty".into();
        assert!(p.target().is_ok(), "an empty folder is fine");
        p.name = "Taken".into();
        assert!(p.target().is_err(), "a folder with files is not");
        for bad in ["", "  ", "a/b", "what?", "dot."] {
            p.name = bad.into();
            assert!(p.target().is_err(), "{bad:?}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
