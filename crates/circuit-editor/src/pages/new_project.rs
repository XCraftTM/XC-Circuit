//! File › New Project, laid out like an IDE's: the kinds of project (build
//! modes, which plugins add) on the left; on the right its name, where it goes
//! and what it starts from.

use std::path::{Path, PathBuf};

use circuit_core::Project;
use circuit_plugins::Registry;
use eframe::egui::{self, CursorIcon, Margin, RichText, Sense, Stroke, Ui, vec2};

use crate::ui::kit;
use crate::i18n::n;
use crate::ui::dialog;
use crate::editor::file_uri;
use crate::ui::theme::{Theme, with_alpha};

/// The window while it is open.
#[derive(Default)]
pub struct NewProject {
    pub mode: String,
    /// The starter: 0 = empty, then this kind's templates.
    pub picked: usize,
    pub name: String,
    pub location: String,
    /// Kept in a Git repository: the project goes into the app's own folder
    /// for those, and Share › Git… opens once it is made.
    pub in_git: bool,
    /// A folder just chosen with Browse…, to be remembered for next time
    /// even if this project isn't made.
    pub browsed: Option<String>,
    /// The starters, read from the disk once when the window opens (they
    /// were read again for every frame it was drawn).
    pub starters: Option<Vec<Template>>,
    /// The last look at the folder the project goes in: for which name and
    /// location, when, and what it found.
    checked: Option<(String, String, std::time::Instant, Result<PathBuf, String>)>,
    /// Where its window first went on the screen, and how big.
    place: Option<(egui::Pos2, egui::Vec2)>,
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
    /// Its group in the gallery (`template.json`'s `category`): Basics,
    /// Community, Moderation, Notifications … Without one: More.
    pub category: String,
    /// An icon of the icon set (`template.json`'s `icon`).
    pub icon: String,
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
        let category = Some(text("category")).filter(|c| !c.is_empty()).unwrap_or_else(|| "More".into());
        let icon = Some(text("icon")).filter(|c| !c.is_empty()).unwrap_or_else(|| "workspace".into());
        Some(Template { dir: dir.to_path_buf(), name: text("name"), description: text("description"), category, icon, mode })
    }
}

/// The starters next to the examples and in plugins' `templates` folders.
pub fn templates(examples: Option<&Path>, reg: &Registry) -> Vec<Template> {
    let dirs = examples.map(|e| e.join("templates")).into_iter().chain(reg.plugins.iter().map(|p| p.dir.join("templates")));
    let mut list: Vec<Template> = dirs.flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten()).filter_map(|e| Template::read(&e.path())).collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

/// The gallery's groups, in this order; others come after them, then More.
const CATEGORIES: [&str; 5] = [n("Basics"), n("Community"), n("Moderation"), n("Notifications"), n("Games & servers")];

/// Characters Windows doesn't allow in a folder name.
const NOT_IN_NAMES: &str = "<>:\"/\\|?*";

impl NewProject {
    pub fn new(reg: &Registry, location: PathBuf) -> NewProject {
        let mode = reg.modes.first().map(|m| m.id.clone()).unwrap_or_default();
        // "New Project", or "New Project 2" … when that folder is taken.
        let name = (1..).map(|i| if i == 1 { "New Project".to_string() } else { format!("New Project {i}") }).find(|n| !location.join(n).exists()).expect("unbounded");
        NewProject { mode, picked: 0, name, location: location.display().to_string(), ..Default::default() }
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
        let own = crate::bot::remotes_dir();
        if self.in_git {
            let _ = std::fs::create_dir_all(&own);
        } else if self.location.trim().is_empty() {
            return Err(t!("Choose where it goes.").into());
        }
        let location = if self.in_git { own.as_path() } else { Path::new(self.location.trim()) };
        if !location.is_dir() {
            return Err(if location.exists() { t!("That's a file, not a folder.") } else { t!("That folder isn't there. Is its drive connected?") }.into());
        }
        let dir = location.join(name);
        if std::fs::read_dir(&dir).is_ok_and(|mut d| d.next().is_some()) {
            return Err(t!("That folder already exists and isn't empty.").into());
        }
        Ok(dir)
    }

    /// [`Self::target`], looked up on the disk when the name or the location
    /// changes and about once a second, not for every frame: a location on a
    /// slow or missing network drive would hold the window up while typing.
    /// For the docs' picture of the window: the folder shown there (someone
    /// else's Documents) counts as free, as it would be on their computer.
    #[cfg(test)]
    pub fn as_if_the_folder_is_free(&mut self) {
        let target = PathBuf::from(&self.location).join(&self.name);
        self.checked = Some((self.name.clone(), self.location.clone(), std::time::Instant::now(), Ok(target)));
    }

    fn target_now(&mut self) -> Result<PathBuf, String> {
        if let Some((name, location, at, found)) = &self.checked
            && *name == self.name
            && *location == self.location
            && at.elapsed() < std::time::Duration::from_secs(1)
        {
            return found.clone();
        }
        let found = self.target();
        self.checked = Some((self.name.clone(), self.location.clone(), std::time::Instant::now(), found.clone()));
        found
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

/// The New Project window: a window of its own beside the app's (or the
/// start window), with the kinds of project on the left and the form on
/// the right. `system_frame`: with the system's title bar.
pub fn show(ctx: &egui::Context, theme: &Theme, state: &mut NewProject, reg: &Registry, templates: &[Template], system_frame: bool) -> Option<Outcome> {
    let mut outcome = None;
    let place = *state.place.get_or_insert_with(|| dialog::centered_now(ctx, WINDOW));
    let icon = reg.icons.get("folder-plus").cloned();
    let close = dialog::own_window(ctx, theme, dialog::Own { key: "new-project", title: t!("New project"), icon: icon.as_deref(), place, system_frame }, |ui| outcome = content(ui, theme, state, reg, templates));
    if close && outcome.is_none() {
        outcome = Some(Outcome::Cancel);
    }
    outcome
}

/// How big the New Project window starts.
pub const WINDOW: egui::Vec2 = egui::vec2(880.0, 760.0);

/// What is in the New Project window, under its title bar: the kinds of
/// project, the form, and Create and Cancel at the bottom.
pub fn content(ui: &mut Ui, theme: &Theme, state: &mut NewProject, reg: &Registry, templates: &[Template]) -> Option<Outcome> {
    let mut outcome = None;
    let target = state.target_now();
    dialog::tab(
        ui,
        theme,
        dialog::Spec { id: "new-project", title: t!("New project"), subtitle: Some(t!("What kind of bot, its name, and where it is kept")), icon: None, width: 820.0 },
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
                    if kit::primary_button(ui, theme, t!("Create")).clicked() {
                        outcome = create(state, templates, &target);
                    }
                });
                if kit::secondary_button(ui, theme, t!("Cancel")).clicked() {
                    outcome = Some(Outcome::Cancel);
                }
            }
        },
    );
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
        kit::section(ui, theme, t!("Type"));
        for mode in &reg.modes {
            let on = state.mode == mode.id;
            let (rect, resp) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click());
            kit::describe_selected(&resp, egui::Role::RadioButton, on, &crate::i18n::tr(&mode.name));
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
    kit::field(ui, theme, t!("Name"), None, |ui| kit::text_input(ui, &mut state.name, "", false));
    ui.horizontal_wrapped(|ui| {
        for (git, label, tip) in [
            (false, t!("Only on this computer"), t!("A folder you choose. You can still put it in a Git repository later (Share › Git…).")),
            (true, t!("In a Git repository"), t!("For several computers, or to work on it with others: on GitHub, GitLab or Gitea. XC Circuit keeps the project's folder itself, and asks for the repository next.")),
        ] {
            if kit::radio(ui, theme, state.in_git == git, label).on_hover_text(tip).clicked() && state.in_git != git {
                state.in_git = git;
                state.checked = None;
            }
            ui.add_space(8.0);
        }
    });
    ui.add_space(4.0);
    if !state.in_git {
    kit::field(ui, theme, t!("Location"), None, |ui| {
        let mut browse = false;
        crate::ui::kit::field_with_button(ui, |ui| {
            kit::text_input(ui, &mut state.location, "", false);
        }, |ui| browse = kit::secondary_button(ui, theme, t!("Browse…")).clicked());
        if browse {
            let mut picker = rfd::FileDialog::new().set_title(t!("Where the project goes"));
            if Path::new(&state.location).is_dir() {
                picker = picker.set_directory(&state.location);
            }
            if let Some(dir) = picker.pick_folder() {
                state.location = dir.display().to_string();
                state.browsed = Some(state.location.clone());
            }
        }
    });
    }
    match target {
        Ok(dir) => kit::hint(ui, theme, tf!("Created in {path}", path = dir.display())),
        Err(why) => ui.label(RichText::new(why).size(11.5).color(theme.warning)),
    };
    ui.add_space(10.0);

    kit::section(ui, theme, t!("Start from"));
    // Cards in groups: Empty project first, then each category's templates.
    // `picked` counts as before: 0 is empty, then this kind's templates in name order.
    let mine: Vec<(usize, &Template)> = templates.iter().filter(|t| t.mode == state.mode).enumerate().map(|(i, t)| (i + 1, t)).collect();
    let mut groups: Vec<String> = CATEGORIES.iter().map(|c| c.to_string()).collect();
    for (_, t) in &mine {
        if !groups.contains(&t.category) {
            groups.insert(groups.len(), t.category.clone());
        }
    }
    groups.retain(|g| g != "More");
    groups.push("More".into());
    let picked_description = match state.picked {
        0 => t!("One empty workspace.").to_string(),
        i => mine.iter().find(|(j, _)| *j == i).map(|(_, t)| crate::i18n::tr(&t.description).into_owned()).unwrap_or_default(),
    };
    egui::ScrollArea::vertical().id_salt("new-project-starters").max_height((ui.ctx().content_rect().height() - 470.0).clamp(150.0, 330.0)).auto_shrink([false, true]).show(ui, |ui| {
        let width = ui.available_width() - 6.0;
        let cols = if width > 460.0 { 2 } else { 1 };
        let gap = 8.0;
        let card = vec2((width - gap * (cols - 1) as f32) / cols as f32, 58.0);
        let mut grid = |ui: &mut Ui, items: &[(usize, String, String, String)]| {
            for row in items.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (i, icon, name, description) in row {
                        let (rect, click) = ui.allocate_exact_size(card, Sense::click());
                        let on = state.picked == *i;
                        kit::describe_selected(&click, egui::Role::RadioButton, on, name);
                        // The title on one line, ending in … when it's longer (the
                        // tooltip has it whole), so it never runs into the description.
                        let left = rect.left() + 10.0 + 36.0 + 10.0;
                        let room = rect.right() - left - 10.0;
                        let mut job = egui::text::LayoutJob::simple(name.clone(), egui::FontId::proportional(13.5), theme.text, room);
                        job.wrap.max_rows = 1;
                        job.wrap.overflow_character = Some('…');
                        let title = ui.fonts_mut(|f| f.layout_job(job));
                        let tip = if title.elided { format!("{name}

{description}") } else { description.clone() };
                        let click = click.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip);
                        let fill = if on { with_alpha(theme.accent, 30) } else if click.hovered() { crate::ui::theme::mix(theme.panel_header, theme.text, 0.05) } else { theme.panel_header };
                        let painter = ui.painter();
                        painter.rect(rect, 8.0, fill, Stroke::new(1.0, if on { theme.accent } else { theme.line }), egui::StrokeKind::Inside);
                        let tile = egui::Rect::from_min_size(rect.min + vec2(10.0, 11.0), vec2(36.0, 36.0));
                        painter.rect_filled(tile, 8.0, with_alpha(theme.accent, if on { 60 } else { 28 }));
                        if let Some(path) = reg.icons.get(icon.as_str()) {
                            egui::Image::from_uri(file_uri(path)).tint(theme.accent).paint_at(ui, tile.shrink(9.0));
                        }
                        painter.galley(egui::pos2(left, rect.top() + 9.0), title, theme.text);
                        // Two lines at most, ending in … when there's more.
                        let mut job = egui::text::LayoutJob::simple(description.clone(), egui::FontId::proportional(11.5), theme.text_muted, room);
                        job.wrap.max_rows = 2;
                        job.wrap.overflow_character = Some('…');
                        let text = ui.fonts_mut(|f| f.layout_job(job));
                        ui.painter().galley(egui::pos2(left, rect.top() + 27.0), text, theme.text_muted);
                        if click.clicked() {
                            state.picked = *i;
                        }
                        go |= click.double_clicked() && target.is_ok();
                    }
                });
                ui.add_space(gap - ui.spacing().item_spacing.y);
            }
        };
        grid(ui, &[(0, "plus".into(), t!("Empty project").to_string(), t!("One empty workspace.").to_string())]);
        for group in &groups {
            let items: Vec<(usize, String, String, String)> = mine
                .iter()
                .filter(|(_, t)| t.category == *group)
                .map(|(i, t)| (*i, t.icon.clone(), crate::i18n::tr(&t.name).into_owned(), crate::i18n::tr(&t.description).into_owned()))
                .collect();
            if items.is_empty() {
                continue;
            }
            ui.add_space(4.0);
            ui.label(RichText::new(crate::i18n::tr(group).to_uppercase()).size(10.5).strong().color(theme.text_muted));
            ui.add_space(2.0);
            grid(ui, &items);
        }
    });
    // The picked one's whole description.
    ui.add_space(6.0);
    ui.label(RichText::new(picked_description).size(12.0).color(theme.text_muted));
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
        let mut p = NewProject { mode: "discord-bot".into(), picked: 0, name: "My Bot".into(), location: dir.display().to_string(), ..Default::default() };
        assert_eq!(p.target(), Ok(dir.join("My Bot")));
        // The window's own look is kept for a moment, and taken again when the name changes.
        assert_eq!(p.target_now(), Ok(dir.join("My Bot")));
        std::fs::create_dir_all(dir.join("My Bot")).unwrap();
        std::fs::write(dir.join("My Bot").join("x.txt"), "").unwrap();
        assert!(p.target().is_err() && p.target_now().is_ok(), "not looked up again within the second");
        p.name = "My Bot 2".into();
        assert_eq!(p.target_now(), Ok(dir.join("My Bot 2")));
        p.name = "Empty".into();
        assert!(p.target().is_ok(), "an empty folder is fine");
        p.name = "Taken".into();
        assert!(p.target().is_err(), "a folder with files is not");
        for bad in ["", "  ", "a/b", "what?", "dot."] {
            p.name = bad.into();
            assert!(p.target().is_err(), "{bad:?}");
        }
        p.name = "My Bot".into();
        p.location = dir.join("Missing").display().to_string();
        assert!(p.target().is_err(), "a folder that isn't there");
        p.location = dir.join("Taken").join("x.txt").display().to_string();
        assert!(p.target().is_err(), "a file");
        let _ = std::fs::remove_dir_all(dir);
    }
}
