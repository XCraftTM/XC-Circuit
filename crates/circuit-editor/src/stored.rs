//! The Stored Data panel: what the bot saved with ctx.store (data/store.json).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Frame, Margin, RichText, ScrollArea, Ui};
use serde_json::{Map, Value};

use crate::editor::{Editor, Level};

#[derive(Default)]
pub struct StoredView {
    data: Map<String, Value>,
    loaded: Option<Instant>,
    filter: String,
    /// (scope, key) being edited, and its text.
    editing: Option<(String, String, String)>,
}

fn file(ed: &Editor) -> Option<PathBuf> {
    ed.project.as_ref().map(|p| p.root.join(".circuit").join("data").join("store.json"))
}

impl StoredView {
    fn reload(&mut self, ed: &Editor) {
        self.data = file(ed)
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        self.loaded = Some(Instant::now());
    }

    fn write(&self, ed: &mut Editor) {
        let Some(f) = file(ed) else { return };
        let _ = std::fs::create_dir_all(f.parent().unwrap_or(&f));
        // Through a temporary file, like the bot itself, so a crash can't leave half a file.
        let tmp = f.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(&Value::Object(self.data.clone())).unwrap_or_default();
        match std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &f)) {
            Ok(()) => ed.log(Level::Success, t!("Stored data saved")),
            Err(e) => ed.log(Level::Error, format!("Could not save stored data: {e}")),
        }
    }
}

pub fn panel(ui: &mut Ui, ed: &mut Editor, view: &mut StoredView, bot_running: bool) {
    let theme = ed.theme.clone();
    // While the bot runs it owns the file, so the panel only shows it and refreshes.
    let stale = view.loaded.is_none_or(|t| t.elapsed() > Duration::from_secs(if bot_running { 2 } else { 30 }));
    if stale && view.editing.is_none() {
        view.reload(ed);
    }
    if bot_running {
        ui.ctx().request_repaint_after(Duration::from_secs(2));
    }

    let mut changed = false;
    Frame::new().inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 90.0).max(80.0);
            ui.add(crate::inputs::TextLine::new(&mut view.filter).hint(t!("Filter keys")).width(w).compact());
            if crate::dialog::button(ui, &theme, t!("Refresh")).clicked() {
                view.reload(ed);
            }
        });
        let hint = if bot_running { t!("Bot running: read-only, refreshes every 2 s") } else { t!("Double-click a value to edit it. Changes apply on the next run.") };
        ui.add(egui::Label::new(RichText::new(hint).size(11.5).color(theme.text_muted)).wrap());
    });
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        Frame::new().inner_margin(Margin::symmetric(12, 4)).show(ui, |ui| {
            if view.data.is_empty() {
                ui.label(RichText::new(t!("Nothing stored yet. Blocks like Set Stored Value write here when the bot runs.")).color(theme.text_muted));
            }
            let filter = view.filter.to_lowercase();
            let scopes: Vec<String> = view.data.keys().cloned().collect();
            for scope in scopes {
                let Some(Value::Object(entries)) = view.data.get(&scope).cloned() else { continue };
                let keys: Vec<&String> = entries.keys().filter(|k| filter.is_empty() || k.to_lowercase().contains(&filter) || scope.contains(&filter)).collect();
                if keys.is_empty() {
                    continue;
                }
                egui::CollapsingHeader::new(RichText::new(format!("{scope}  ·  {}", keys.len())).monospace()).default_open(true).id_salt(&scope).show(ui, |ui| {
                    egui::Grid::new(("stored", &scope)).num_columns(3).spacing([16.0, 4.0]).striped(true).show(ui, |ui| {
                        for key in keys {
                            ui.label(RichText::new(key).monospace().color(theme.text));
                            let value = &entries[key];
                            let editing_this = view.editing.as_ref().is_some_and(|(s, k, _)| s == &scope && k == key);
                            if editing_this {
                                let text = &mut view.editing.as_mut().unwrap().2;
                                let r = ui.add(crate::inputs::TextLine::new(text).width(260.0).monospace().compact());
                                if r.lost_focus() {
                                    let (s, k, t) = view.editing.take().unwrap();
                                    let parsed = serde_json::from_str(&t).unwrap_or(Value::String(t));
                                    if let Some(Value::Object(m)) = view.data.get_mut(&s) {
                                        m.insert(k, parsed);
                                        changed = true;
                                    }
                                } else {
                                    r.request_focus();
                                }
                            } else {
                                let shown = value.to_string();
                                let short = if shown.chars().count() > 60 { format!("{}…", shown.chars().take(60).collect::<String>()) } else { shown.clone() };
                                let r = ui.add(egui::Label::new(RichText::new(short).monospace().color(theme.text_muted)).sense(egui::Sense::click()));
                                if r.double_clicked() && !bot_running {
                                    view.editing = Some((scope.clone(), key.clone(), shown));
                                }
                            }
                            if ui.add_enabled_ui(!bot_running, |ui| crate::dialog::danger_button(ui, &theme, t!("Delete"))).inner.clicked()
                                && let Some(Value::Object(m)) = view.data.get_mut(&scope)
                            {
                                m.shift_remove(key);
                                changed = true;
                            }
                            ui.end_row();
                        }
                    });
                });
            }
        });
    });
    if changed {
        view.data.retain(|_, v| v.as_object().is_none_or(|m| !m.is_empty()));
        view.write(ed);
    }
}
