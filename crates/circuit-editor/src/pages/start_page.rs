//! The start page, while no project is open: make or open one, reopen a
//! recent one, or look at the example.

use std::path::{Path, PathBuf};

use circuit_plugins::Registry;
use eframe::egui::{self, Align, Color32, CursorIcon, Layout, RichText, Sense, Stroke, Ui, vec2};

use crate::ui::dialog;
use crate::ui::theme::{Theme, with_alpha};

/// What was picked on the page.
pub enum Pick {
    New,
    Open,
    Recent(PathBuf),
    Forget(String),
    Example,
}

/// A recent project as listed: read once from its circuit.json.
struct Entry {
    path: String,
    name: String,
    /// The kind of project, such as Discord Bot.
    kind: String,
    exists: bool,
}

/// The listed projects, read again only when the recent list changes.
#[derive(Default)]
pub struct StartPage {
    read_for: Vec<String>,
    entries: Vec<Entry>,
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

fn read_entry(path: &str, reg: &Registry) -> Entry {
    let folder = Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string());
    let meta: Option<serde_json::Value> = std::fs::read_to_string(Path::new(path).join(circuit_core::project::PROJECT_FILE)).ok().and_then(|t| serde_json::from_str(&t).ok());
    let text = |k: &str| meta.as_ref().and_then(|m| m.get(k)).and_then(|v| v.as_str()).map(str::to_string);
    let kind = text("mode").map(|m| reg.mode(&m).map(|d| crate::i18n::tr(&d.name).into_owned()).unwrap_or(m)).unwrap_or_default();
    Entry { path: path.to_string(), name: text("name").filter(|n| !n.trim().is_empty()).unwrap_or(folder), kind, exists: meta.is_some() }
}

pub fn show(ui: &mut Ui, theme: &Theme, state: &mut StartPage, recent: &[String], reg: &Registry, has_example: bool) -> Option<Pick> {
    state.refresh(recent, reg);
    let mut pick = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let width = ui.available_width().min(820.0);
        let side = ((ui.available_width() - width) / 2.0).max(16.0);
        ui.add_space(48.0);
        ui.horizontal(|ui| {
            ui.add_space(side);
            ui.vertical(|ui| {
                ui.set_width(width);
                // Who we are.
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new(egui::include_image!("../../assets/icon.svg")).fit_to_exact_size(vec2(44.0, 44.0)));
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new("XC™ Circuit").size(24.0).color(theme.text));
                        ui.label(RichText::new(t!("Build bots by connecting blocks.")).size(13.0).color(theme.text_muted));
                    });
                });
                ui.add_space(28.0);
                ui.horizontal_top(|ui| {
                    // What to do.
                    ui.vertical(|ui| {
                        ui.set_width(220.0);
                        ui.spacing_mut().item_spacing.y = 8.0;
                        dialog::section(ui, theme, t!("Start"));
                        let wide = |ui: &mut Ui, primary: bool, text: &str| {
                            ui.allocate_ui_with_layout(vec2(220.0, crate::ui::inputs::input_height(ui)), Layout::top_down_justified(Align::Min), |ui| {
                                if primary { dialog::primary_button(ui, theme, text) } else { dialog::secondary_button(ui, theme, text) }
                            })
                            .inner
                        };
                        if wide(ui, true, t!("New project…")).clicked() {
                            pick = Some(Pick::New);
                        }
                        if wide(ui, false, t!("Open project…")).clicked() {
                            pick = Some(Pick::Open);
                        }
                        if has_example && wide(ui, false, t!("Open example: DemoBot")).clicked() {
                            pick = Some(Pick::Example);
                        }
                    });
                    ui.add_space(36.0);
                    // What was open lately.
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width());
                        dialog::section(ui, theme, t!("Recent projects"));
                        if state.entries.is_empty() {
                            ui.add_space(6.0);
                            ui.label(RichText::new(t!("Projects you open show up here.")).size(12.5).color(theme.text_muted));
                        }
                        for (i, e) in state.entries.iter().enumerate() {
                            if let Some(p) = recent_row(ui, theme, i, e) {
                                pick = Some(p);
                            }
                        }
                    });
                });
            });
        });
    });
    pick
}

/// One recent project: click to open; × takes it off the list.
fn recent_row(ui: &mut Ui, theme: &Theme, i: usize, e: &Entry) -> Option<Pick> {
    let mut pick = None;
    let h = 52.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let remove = egui::Rect::from_center_size(rect.right_center() - vec2(22.0, 0.0), vec2(26.0, 26.0));
    let x = ui.interact(remove, ui.id().with(("recent-forget", i)), Sense::click()).on_hover_text(t!("Remove from the list"));
    let hovered = resp.hovered() || x.hovered();
    if hovered {
        ui.painter().rect_filled(rect, 8.0, theme.panel_header);
    }
    let dim = |c: Color32| if e.exists { c } else { with_alpha(c, 110) };
    let p = ui.painter();
    p.text(rect.left_top() + vec2(12.0, 9.0), egui::Align2::LEFT_TOP, &e.name, egui::FontId::proportional(14.5), dim(theme.text));
    let sub = if e.exists {
        if e.kind.is_empty() { e.path.clone() } else { format!("{} · {}", e.kind, e.path) }
    } else {
        tf!("{path} (moved or deleted)", path = e.path)
    };
    let clip = rect.shrink2(vec2(12.0, 0.0)).with_max_x(remove.left() - 6.0);
    p.with_clip_rect(clip).text(rect.left_bottom() + vec2(12.0, -10.0), egui::Align2::LEFT_BOTTOM, sub, egui::FontId::proportional(11.5), dim(theme.text_muted));
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
