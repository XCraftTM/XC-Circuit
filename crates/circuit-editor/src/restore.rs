//! The Backups tab: restore the project, one workspace, or the bot's saved
//! data from a backup.

use std::path::{Path, PathBuf};

use circuit_core::backup::{Change, Snapshot, compare};
use eframe::egui::{self, Align, Layout, RichText, ScrollArea, Sense, Stroke, Ui, vec2};

use crate::editor::{Editor, Level, ago, local_time};
use crate::theme::{Theme, with_alpha};

/// Per workspace in a backup: key, name and how it differs from now.
type Comparison = Vec<(String, String, Change)>;

#[derive(Default)]
pub struct RestoreView {
    /// The snapshot opened to see what differs, and its comparison.
    open: Option<(PathBuf, Result<Comparison, String>)>,
    /// A restore waiting for "Yes".
    confirm: Option<Confirm>,
    /// The backups shown, read from disk again after a save or restore, and
    /// every few seconds (the bot makes its own when it starts).
    lists: Option<Lists>,
}

struct Lists {
    root: PathBuf,
    saves: u64,
    at: std::time::Instant,
    project: Vec<Snapshot>,
    bot: Vec<BotBackup>,
}

/// How long the lists are shown before they are read again.
const LISTS_FOR: std::time::Duration = std::time::Duration::from_secs(3);

impl RestoreView {
    fn lists(&mut self, ed: &Editor, root: &Path) -> &Lists {
        let fresh = self.lists.as_ref().is_some_and(|l| l.root == root && l.saves == ed.saves && l.at.elapsed() < LISTS_FOR);
        if !fresh {
            self.lists = Some(Lists {
                root: root.to_path_buf(),
                saves: ed.saves,
                at: std::time::Instant::now(),
                project: ed.backups.list(root),
                bot: bot_backups(&bot_data(root).1),
            });
        }
        self.lists.as_ref().expect("just read")
    }
}

#[derive(Clone, PartialEq)]
enum Confirm {
    Project(PathBuf),
    BotData(PathBuf),
}

/// The bot's saved data and its backups (made by the bot when it starts).
fn bot_data(root: &Path) -> (PathBuf, PathBuf) {
    let data = root.join(".circuit").join("data");
    (data.join("store.json"), data.join("backups"))
}

#[derive(Clone)]
struct BotBackup {
    path: PathBuf,
    name: String,
    at: u64,
    size: u64,
}

fn bot_backups(dir: &Path) -> Vec<BotBackup> {
    let mut list: Vec<BotBackup> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !(name.starts_with("store-") && name.ends_with(".json")) {
                return None;
            }
            let meta = e.metadata().ok()?;
            let at = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as u64;
            Some(BotBackup { path: e.path(), name, at, size: meta.len() })
        })
        .collect();
    list.sort_by(|a, b| b.name.cmp(&a.name));
    list
}

fn size_text(bytes: u64) -> String {
    match bytes {
        0..1_024 => format!("{bytes} B"),
        1_024..1_048_576 => format!("{:.1} KB", bytes as f64 / 1_024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

/// Copies a bot data backup over store.json, keeping the current file as a
/// backup first.
fn restore_bot_data(store: &Path, backups: &Path, from: &Path) -> anyhow::Result<()> {
    let text = std::fs::read_to_string(from)?;
    serde_json::from_str::<serde_json::Value>(&text).map_err(|e| anyhow::anyhow!("that backup isn't valid JSON: {e}"))?;
    if let Ok(current) = std::fs::read_to_string(store)
        && serde_json::from_str::<serde_json::Value>(&current).is_ok()
    {
        std::fs::create_dir_all(backups)?;
        let stamp = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S-%3fZ");
        std::fs::write(backups.join(format!("store-{stamp}-before-restore.json")), current)?;
    }
    let tmp = store.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, store)?;
    Ok(())
}

pub fn ui(ui: &mut Ui, ed: &mut Editor, view: &mut RestoreView, bot_running: bool) {
    let theme = ed.theme.clone();
    ui.painter().rect_filled(ui.max_rect(), 0.0, theme.panel);
    let Some(root) = ed.project.as_ref().map(|p| p.root.clone()) else {
        ui.label(t!("Open a project to see its backups."));
        return;
    };
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin { left: 32, right: 32, top: 24, bottom: 32 }).show(ui, |ui| {
            ui.set_max_width(760.0);
            ui.label(RichText::new(t!("Backups")).size(22.0).strong().color(theme.text));
            ui.add_space(4.0);
            let note = if ed.prefs.backups {
                format!("A copy of the project is kept each time you save: the newest {}, plus one a day for {} days. Change this in Settings › Backups.", ed.prefs.backup_keep, ed.prefs.backup_days)
            } else {
                t!("Backups on save are turned off in Settings › Backups.").to_string()
            };
            ui.label(RichText::new(note).size(13.0).color(theme.text_muted));
            ui.add_space(16.0);
            project_backups(ui, ed, view, &theme, &root);
            ui.add_space(22.0);
            bot_data_backups(ui, ed, view, &theme, &root, bot_running);
        });
    });
}

fn heading(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.label(RichText::new(text.to_uppercase()).size(11.0).strong().color(theme.text_muted));
    ui.add_space(6.0);
}

fn project_backups(ui: &mut Ui, ed: &mut Editor, view: &mut RestoreView, theme: &Theme, root: &Path) {
    heading(ui, theme, "Project");
    let list = view.lists(ed, root).project.clone();
    if list.is_empty() {
        ui.label(RichText::new(t!("No backups yet. One is made the next time you save.")).color(theme.text_muted));
        return;
    }
    let mut restore_project: Option<Snapshot> = None;
    let mut restore_ws: Option<(Snapshot, String)> = None;
    for (i, snap) in list.iter().enumerate() {
        let open = view.open.as_ref().is_some_and(|(d, _)| *d == snap.dir);
        let frame = egui::Frame::new()
            .fill(if open { theme.panel_header } else { theme.panel })
            .stroke(Stroke::new(1.0, if open { with_alpha(theme.accent, 160) } else { theme.line }))
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(14, 10));
        let resp = frame
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(local_time(snap.info.saved_at)).size(14.0).color(theme.text));
                        let mut line = format!("{} · {} · {} workspaces · {}", ago(snap.info.saved_at), snap.info.reason, snap.info.workspaces.len(), size_text(snap.size));
                        if i == 0 {
                            line.push_str(t!(" · newest"));
                        }
                        ui.label(RichText::new(line).size(12.0).color(theme.text_muted));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(if open { "Hide" } else { "Compare" }).size(12.0).color(theme.accent));
                    });
                });
                if open && let Some((_, cmp)) = &view.open {
                    ui.add_space(8.0);
                    match cmp {
                        Err(e) => {
                            ui.label(RichText::new(format!("This backup can't be read: {e}")).color(theme.error));
                        }
                        Ok(rows) => {
                            for (key, name, change) in rows {
                                ui.horizontal(|ui| {
                                    let (text, color) = match change {
                                        Change::Same => (t!("same as now"), theme.text_muted),
                                        Change::Changed => (t!("changed since"), theme.warning),
                                        Change::OnlyInBackup => (t!("deleted since"), theme.error),
                                        Change::OnlyNow => (t!("added since"), theme.action),
                                    };
                                    ui.label(RichText::new(name).size(13.0).color(theme.text));
                                    crate::dialog::pill(ui, text, color);
                                    if matches!(change, Change::Changed | Change::OnlyInBackup) {
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            if crate::dialog::button(ui, theme, t!("Restore this workspace")).on_hover_text(t!("Brings it back as an unsaved change")).clicked() {
                                                restore_ws = Some((snap.clone(), key.clone()));
                                            }
                                        });
                                    }
                                });
                            }
                            ui.add_space(8.0);
                            if view.confirm == Some(Confirm::Project(snap.dir.clone())) {
                                ui.label(RichText::new(t!("Put the whole project back as it was then? Your current version is backed up first, so you can come back to it.")).size(12.5).color(theme.text));
                                ui.horizontal(|ui| {
                                    if crate::dialog::primary_button(ui, theme, t!("Yes, restore")).clicked() {
                                        restore_project = Some(snap.clone());
                                    }
                                    if crate::dialog::secondary_button(ui, theme, t!("Cancel")).clicked() {
                                        view.confirm = None;
                                    }
                                });
                            } else if crate::dialog::primary_button(ui, theme, t!("Restore the whole project")).clicked() {
                                view.confirm = Some(Confirm::Project(snap.dir.clone()));
                            }
                        }
                    }
                }
            })
            .response;
        // Clicking the summary opens or closes it.
        let summary = egui::Rect::from_min_size(resp.rect.min, vec2(resp.rect.width(), 48.0));
        if ui.interact(summary, ui.id().with(("snap", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            if open {
                view.open = None;
            } else {
                let cmp = match (ed.backups.load(snap), &ed.project) {
                    (Ok((_, ws)), Some(p)) => Ok(compare(&ws, &p.workspaces)),
                    (Err(e), _) => Err(format!("{e:#}")),
                    (_, None) => Err("no project".into()),
                };
                view.open = Some((snap.dir.clone(), cmp));
            }
            view.confirm = None;
        }
        ui.add_space(6.0);
    }
    if restore_project.is_some() || restore_ws.is_some() {
        view.lists = None;
    }
    if let Some(snap) = restore_project {
        view.confirm = None;
        view.open = None;
        if let Err(e) = ed.restore_project(&snap) {
            ed.log(Level::Error, format!("Couldn't restore: {e:#}"));
        }
    }
    if let Some((snap, key)) = restore_ws {
        if let Err(e) = ed.restore_workspace(&snap, &key) {
            ed.log(Level::Error, format!("Couldn't restore the workspace: {e:#}"));
        }
        view.open = None;
    }
}

fn bot_data_backups(ui: &mut Ui, ed: &mut Editor, view: &mut RestoreView, theme: &Theme, root: &Path, bot_running: bool) {
    heading(ui, theme, t!("The bot's saved data"));
    ui.label(
        RichText::new(t!("Made by the bot itself each time it starts (the newest 10 are kept). Exported bots do the same in their data/backups folder."))
            .size(12.5)
            .color(theme.text_muted),
    );
    ui.add_space(8.0);
    let (store, dir) = bot_data(root);
    let list = view.lists(ed, root).bot.clone();
    if list.is_empty() {
        ui.label(RichText::new(t!("No backups yet. One is made the next time the bot starts with saved data.")).color(theme.text_muted));
        return;
    }
    let mut restore: Option<PathBuf> = None;
    for b in &list {
        egui::Frame::new().fill(theme.panel).stroke(Stroke::new(1.0, theme.line)).corner_radius(8).inner_margin(egui::Margin::symmetric(14, 8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(local_time(b.at)).size(13.5).color(theme.text));
                    let before = if b.name.contains("before-restore") { t!(" · before a restore") } else { "" };
                    ui.label(RichText::new(format!("{} · {}{before}", ago(b.at), size_text(b.size))).size(12.0).color(theme.text_muted));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if view.confirm == Some(Confirm::BotData(b.path.clone())) {
                        if crate::dialog::button(ui, theme, t!("Cancel")).clicked() {
                            view.confirm = None;
                        }
                        if crate::dialog::button(ui, theme, RichText::new(t!("Yes, restore")).strong()).clicked() {
                            restore = Some(b.path.clone());
                        }
                    } else {
                        let r = ui.add_enabled_ui(!bot_running, |ui| crate::dialog::button(ui, theme, t!("Restore"))).inner;
                        let r = if bot_running { r.on_disabled_hover_text(t!("Stop the bot first")) } else { r };
                        if r.clicked() {
                            view.confirm = Some(Confirm::BotData(b.path.clone()));
                        }
                    }
                });
            });
        });
        ui.add_space(4.0);
    }
    if let Some(from) = restore {
        view.confirm = None;
        view.lists = None;
        match restore_bot_data(&store, &dir, &from) {
            Ok(()) => ed.log(Level::Success, t!("The bot's saved data was restored. It's used the next time the bot starts.")),
            Err(e) => ed.log(Level::Error, format!("Couldn't restore the bot's saved data: {e:#}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_the_project_or_one_workspace() {
        let base = std::env::temp_dir().join(format!("xc-restore-ed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("bot");
        circuit_core::Project::create(&root, "Bot", "none").unwrap();
        let plugins = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let mut ed = Editor::new(vec![plugins], None).unwrap();
        ed.backups = circuit_core::backup::Backups::new(base.join("backups"));
        ed.open_project(&root);
        let rename = |ed: &mut Editor, name: &str| {
            ed.project.as_mut().unwrap().workspaces.get_mut("main").unwrap().name = name.into();
        };
        rename(&mut ed, "First");
        ed.save();
        rename(&mut ed, "Second");
        ed.project.as_mut().unwrap().workspaces.insert("extra".into(), circuit_core::Workspace::new("Extra"));
        ed.save();
        let list = ed.backups.list(&root);
        assert_eq!(list.len(), 2);

        // The whole project, as it was at the first save.
        ed.restore_project(&list[1]).unwrap();
        let p = ed.project.as_ref().unwrap();
        assert_eq!(p.workspaces["main"].name, "First");
        assert!(!p.workspaces.contains_key("extra"));
        assert!(!p.workspace_path("extra").exists(), "saved: its file is gone");
        assert!(!ed.unsaved_now());
        // It had just been saved, so its backup already is the version from before.
        assert!(ed.backups.list(&root).iter().any(|s| s.dir == list[0].dir), "the version before restoring was kept");

        // One workspace from the second save, as an unsaved change.
        ed.restore_workspace(&list[0], "extra").unwrap();
        assert_eq!(ed.project.as_ref().unwrap().workspaces["extra"].name, "Extra");
        assert!(ed.unsaved_now());

        // Restoring over unsaved changes backs them up first.
        ed.restore_project(&list[1]).unwrap();
        let before = ed.backups.list(&root).into_iter().find(|s| s.info.reason == "Before restore").expect("unsaved changes were backed up");
        assert!(before.info.workspaces.iter().any(|(k, _)| k == "extra"));
    }

    #[test]
    fn bot_data_restore_keeps_the_current_file() {
        let dir = std::env::temp_dir().join(format!("xc-restore-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let backups = dir.join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        let store = dir.join("store.json");
        std::fs::write(&store, r#"{"now":1}"#).unwrap();
        let old = backups.join("store-2026-01-01T00-00-00-000Z.json");
        std::fs::write(&old, r#"{"then":1}"#).unwrap();
        restore_bot_data(&store, &backups, &old).unwrap();
        assert_eq!(std::fs::read_to_string(&store).unwrap(), r#"{"then":1}"#);
        let list = bot_backups(&backups);
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|b| b.name.contains("before-restore") && std::fs::read_to_string(&b.path).unwrap() == r#"{"now":1}"#));
        // A broken backup is refused and changes nothing.
        let bad = backups.join("store-2026-01-02T00-00-00-000Z.json");
        std::fs::write(&bad, "{ nope").unwrap();
        assert!(restore_bot_data(&store, &backups, &bad).is_err());
        assert_eq!(std::fs::read_to_string(&store).unwrap(), r#"{"then":1}"#);
    }
}
