//! Plugins › Manage Plugins: every loaded plugin, and the switch and
//! approvals for native Rust plugins.

use eframe::egui::{self, Frame, Margin, RichText};

use crate::dialog::{self, Part};
use crate::editor::Editor;
use crate::native::{NativeHost, Status, status_text};
use crate::theme::Theme;

pub enum Ask {
    /// The "Allow native plugins" switch changed.
    SetAllow(bool),
    Trust(String),
    Close,
}

pub fn window(ctx: &egui::Context, theme: &Theme, ed: &Editor, native: &NativeHost) -> Option<Ask> {
    let mut ask = None;
    let mut allow = native.allow;
    let close = dialog::window(
        ctx,
        theme,
        dialog::Spec { id: "plugins", title: "Plugins", subtitle: Some("What's loaded, and which native plugins may run"), icon: None, width: 560.0 },
        |ui, part| match part {
            Part::Body => {
                dialog::toggle_row(ui, theme, &mut allow, "Allow native plugins", "Native plugins are compiled Rust code. They run with the same access to your computer as XC Circuit, and a broken one can crash the app. Only turn this on for plugins you trust.");
                if allow != native.allow {
                    ask = Some(Ask::SetAllow(allow));
                }
                ui.add_space(10.0);
                dialog::section(ui, theme, "Installed");
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for lp in &ed.reg.plugins {
                        let m = &lp.manifest;
                        let np = native.plugins.iter().find(|p| p.id == m.id);
                        Frame::new().fill(theme.panel_header).corner_radius(8).inner_margin(Margin::symmetric(12, 9)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&m.name).size(13.5).strong().color(theme.text));
                                ui.label(RichText::new(format!("v{} · {}", m.version, m.author)).size(11.0).color(theme.text_muted));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match np.map(|p| &p.status) {
                                    None => dialog::pill(ui, "Blocks & config", theme.text_muted),
                                    Some(Status::Loaded) => dialog::pill(ui, "Native · running", theme.action),
                                    Some(Status::NeedsApproval) => {
                                        if dialog::primary_button(ui, theme, "Allow").on_hover_text(lp.dir.display().to_string()).clicked() {
                                            ask = Some(Ask::Trust(m.id.clone()));
                                        }
                                        dialog::pill(ui, "Native", theme.warning);
                                    }
                                    Some(Status::Off) => dialog::pill(ui, "Native · off", theme.text_muted),
                                    Some(_) => dialog::pill(ui, "Native · not running", theme.error),
                                });
                            });
                            if !m.description.is_empty() {
                                ui.label(RichText::new(&m.description).size(12.0).color(theme.text_muted));
                            }
                            if let Some(p) = np
                                && !matches!(p.status, Status::Loaded | Status::Off)
                            {
                                let color = if p.status == Status::NeedsApproval { theme.text_muted } else { theme.warning };
                                ui.label(RichText::new(status_text(&p.status)).size(11.5).color(color));
                                if p.status == Status::NeedsApproval {
                                    ui.label(RichText::new(lp.dir.display().to_string()).size(11.0).monospace().color(theme.text_muted));
                                }
                            }
                        });
                        ui.add_space(6.0);
                    }
                });
            }
            Part::Footer => {
                if dialog::primary_button(ui, theme, "Done").clicked() {
                    ask = Some(Ask::Close);
                }
            }
        },
    );
    if close && ask.is_none() {
        ask = Some(Ask::Close);
    }
    ask
}
