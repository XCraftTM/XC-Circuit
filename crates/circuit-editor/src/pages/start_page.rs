//! The start window's Projects page, while no project is open: make, open
//! or get one, or reopen a recent one (searched by name or folder).

use std::path::{Path, PathBuf};

use circuit_plugins::Registry;
use eframe::egui::{self, Align, Color32, CursorIcon, Layout, RichText, Sense, Stroke, Ui, vec2};

use crate::ui::kit;
use crate::ui::theme::{Theme, with_alpha};

/// What was picked on the page.
pub enum Pick {
    New,
    Open,
    /// Get a project from a Git repository.
    Get,
    Recent(PathBuf),
    Forget(String),
}

/// A recent project as listed: read once from its project file.
struct Entry {
    path: String,
    name: String,
    /// The kind of project, such as Discord Bot.
    kind: String,
    /// The branch it is on, when it is kept in a Git repository.
    branch: Option<String>,
    exists: bool,
}

/// The listed projects, read again only when the recent list changes, and
/// what is typed into the search.
#[derive(Default)]
pub struct StartPage {
    read_for: Vec<String>,
    entries: Vec<Entry>,
    search: String,
}

impl StartPage {
    fn refresh(&mut self, recent: &[String], reg: &Registry) {
        if self.read_for == recent {
            return;
        }
        self.read_for = recent.to_vec();
        self.entries = recent.iter().map(|p| read_entry(p, reg)).collect();
    }
}

/// The branch a repository's `HEAD` file names (none for a commit on its own).
fn branch_in(head: &str) -> Option<String> {
    head.trim().strip_prefix("ref: refs/heads/").map(str::to_string).filter(|b| !b.is_empty())
}

fn read_entry(path: &str, reg: &Registry) -> Entry {
    let folder = Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string());
    let meta: Option<serde_json::Value> = [circuit_core::project::PROJECT_FILE, circuit_core::project::OLD_PROJECT_FILE].iter().find_map(|file| std::fs::read_to_string(Path::new(path).join(file)).ok()).and_then(|t| serde_json::from_str(&t).ok());
    let text = |k: &str| meta.as_ref().and_then(|m| m.get(k)).and_then(|v| v.as_str()).map(str::to_string);
    let kind = text("mode")
        .map(|m| match reg.mode(&m) {
            Some(d) => crate::i18n::tr(&d.name).into_owned(),
            None if m == crate::editor::NO_MODE => t!("Core blocks").to_string(),
            None => m,
        })
        .unwrap_or_default();
    let branch = std::fs::read_to_string(Path::new(path).join(".git").join("HEAD")).ok().and_then(|h| branch_in(&h));
    Entry { path: path.to_string(), name: text("name").filter(|n| !n.trim().is_empty()).unwrap_or(folder), kind, branch, exists: meta.is_some() }
}

/// Whether a project is among those the search asks for: every word typed
/// is in its name or its folder.
fn found(search: &str, name: &str, path: &str) -> bool {
    let (name, path) = (name.to_lowercase(), path.to_lowercase());
    search.to_lowercase().split_whitespace().all(|w| name.contains(w) || path.contains(w))
}

/// Up to two letters for a project's picture: of its first two words, or
/// its first two letters.
fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let letters: String = match words.as_slice() {
        [one] => one.chars().take(2).collect(),
        many => many.iter().take(2).filter_map(|w| w.chars().next()).collect(),
    };
    letters.to_uppercase()
}

/// A colour for a project's picture, the same each time for the same name.
fn tint(name: &str) -> Color32 {
    const TINTS: [Color32; 6] = [
        Color32::from_rgb(0x4F, 0x8C, 0xF7),
        Color32::from_rgb(0x2B, 0xB3, 0xA3),
        Color32::from_rgb(0x9B, 0x6D, 0xF2),
        Color32::from_rgb(0xE0, 0x7B, 0x39),
        Color32::from_rgb(0x4C, 0xAF, 0x6A),
        Color32::from_rgb(0xD6, 0x5C, 0x8A),
    ];
    let sum = name.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    TINTS[sum as usize % TINTS.len()]
}

/// The page: a search and New Project, Open and Get from Git on top, the
/// recent projects under them.
pub fn show(ui: &mut Ui, theme: &Theme, state: &mut StartPage, recent: &[String], reg: &Registry) -> Option<Pick> {
    state.refresh(recent, reg);
    let mut pick = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if kit::secondary_button(ui, theme, t!("Get from Git")).on_hover_text(t!("Get a project from a Git repository")).clicked() {
                pick = Some(Pick::Get);
            }
            if kit::secondary_button(ui, theme, t!("Open")).on_hover_text(t!("Open a project's folder")).clicked() {
                pick = Some(Pick::Open);
            }
            if kit::primary_button(ui, theme, t!("New Project")).clicked() {
                pick = Some(Pick::New);
            }
            ui.add_sized([ui.available_width(), kit::input_height(ui)], egui::TextEdit::singleline(&mut state.search).hint_text(t!("Search projects")));
        });
    });
    ui.add_space(8.0);
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, theme.line));
    ui.add_space(8.0);
    egui::ScrollArea::vertical().id_salt("start-projects").auto_shrink([false, false]).show(ui, |ui| {
        let shown: Vec<usize> = (0..state.entries.len()).filter(|&i| found(&state.search, &state.entries[i].name, &state.entries[i].path)).collect();
        if state.entries.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(t!("Projects you open show up here.")).size(12.5).color(theme.text_muted));
        } else if shown.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(t!("No project of that name.")).size(12.5).color(theme.text_muted));
        }
        for i in shown {
            if let Some(p) = recent_row(ui, theme, i, &state.entries[i]) {
                pick = Some(p);
            }
        }
    });
    pick
}

/// One recent project: click to open; × takes it off the list.
fn recent_row(ui: &mut Ui, theme: &Theme, i: usize, e: &Entry) -> Option<Pick> {
    let mut pick = None;
    let h = 52.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    crate::ui::kit::describe(&resp, egui::Role::Button, &format!("{}, {}", e.name, e.path));
    let remove = egui::Rect::from_center_size(rect.right_center() - vec2(22.0, 0.0), vec2(26.0, 26.0));
    let x = ui.interact(remove, ui.scope_id().with(("recent-forget", i)), Sense::click());
    crate::ui::kit::describe(&x, egui::Role::Button, &format!("{}: {}", t!("Remove from the list"), e.name));
    let x = x.on_hover_text(t!("Remove from the list"));
    let hovered = resp.hovered() || x.hovered();
    if hovered {
        ui.painter().rect_filled(rect, 8.0, theme.panel_header);
    }
    let dim = |c: Color32| if e.exists { c } else { with_alpha(c, 110) };
    let p = ui.painter();
    let picture = egui::Rect::from_center_size(rect.left_center() + vec2(26.0, 0.0), vec2(28.0, 28.0));
    p.rect_filled(picture, 6.0, dim(if e.exists { tint(&e.name) } else { theme.text_muted }));
    p.text(picture.center(), egui::Align2::CENTER_CENTER, initials(&e.name), egui::FontId::proportional(11.5), Color32::WHITE);
    let x0 = picture.right() + 12.0;
    p.text(egui::pos2(x0, rect.top() + 9.0), egui::Align2::LEFT_TOP, &e.name, egui::FontId::proportional(14.0), dim(theme.text));
    let mut sub = if e.exists { e.path.clone() } else { tf!("{path} (moved or deleted)", path = e.path) };
    if e.exists && !e.kind.is_empty() {
        sub = format!("{} · {sub}", e.kind);
    }
    if let Some(branch) = e.branch.as_ref().filter(|_| e.exists) {
        sub = format!("{sub} · {branch}");
    }
    let clip = rect.with_min_x(x0).with_max_x(remove.left() - 6.0);
    p.with_clip_rect(clip).text(egui::pos2(x0, rect.bottom() - 10.0), egui::Align2::LEFT_BOTTOM, sub, egui::FontId::proportional(11.5), dim(theme.text_muted));
    if hovered || !e.exists {
        let c = if x.hovered() { theme.error } else { theme.text_muted };
        if x.hovered() {
            p.rect(remove, 6.0, with_alpha(theme.error, 30), Stroke::NONE, egui::StrokeKind::Inside);
        }
        p.text(remove.center(), egui::Align2::CENTER_CENTER, "×", egui::FontId::proportional(16.0), c);
    }
    if x.clicked() {
        pick = Some(Pick::Forget(e.path.clone()));
    } else if resp.clicked() && e.exists {
        pick = Some(Pick::Recent(PathBuf::from(&e.path)));
    }
    if e.exists {
        resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(&e.path);
    }
    pick
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_are_searched_and_pictured() {
        assert!(found("", "Demo Bot", "C:/bots/demo"));
        assert!(found("demo", "Demo Bot", "C:/x") && found("bots de", "Demo Bot", "C:/bots/demo"));
        assert!(!found("ticket", "Demo Bot", "C:/bots/demo"));
        assert_eq!(initials("Demo Bot"), "DB");
        assert_eq!(initials("tickets"), "TI");
        assert_eq!(initials("xc-circuit-sync-test"), "XC");
        assert_eq!(tint("Demo Bot"), tint("Demo Bot"));
        assert_eq!(branch_in("ref: refs/heads/main\n").as_deref(), Some("main"));
        assert_eq!(branch_in("0123abcd\n"), None);
    }
}
