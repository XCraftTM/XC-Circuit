//! Plugins › Manage Plugins: every loaded plugin, and the switch and
//! approvals for native Rust plugins. Changes wait for Save.

use eframe::egui::{self, Frame, Margin, RichText};

use crate::ui::kit;
use crate::ui::dialog::{self, Part};
use crate::editor::Editor;
use crate::plugins::native::{Kind, NativeHost, Status, status_text};
use crate::ui::theme::Theme;

/// The changes made in the Plugins tab, applied when it is saved.
pub struct Draft {
    pub allow: bool,
    /// Native plugins to approve, by id.
    pub trust: Vec<String>,
}

impl Draft {
    pub fn new(native: &NativeHost) -> Self {
        Draft { allow: native.allow, trust: Vec::new() }
    }
}

pub enum Ask {
    /// Apply the changes; true also closes the tab.
    Save(bool),
    Cancel,
}

/// The Plugins tab.
pub fn tab(ui: &mut egui::Ui, theme: &Theme, ed: &Editor, native: &NativeHost, draft: &mut Draft) -> Option<Ask> {
    let mut ask = None;
    let changed = draft.allow != native.allow || !draft.trust.is_empty();
    dialog::tab(
        ui,
        theme,
        dialog::Spec { id: "plugins", title: t!("Plugins"), subtitle: Some(t!("What's loaded, and which native plugins may run")), icon: None, width: 560.0 },
        |ui, part| match part {
            Part::Body => {
                kit::toggle_row(ui, theme, &mut draft.allow, t!("Allow native plugins"), t!("Native plugins are compiled Rust code. They run with the same access to your computer as XC Circuit, and a broken one can crash the app. Only turn this on for plugins you trust."));
                ui.add_space(10.0);
                kit::section(ui, theme, t!("Installed"));
                for lp in &ed.reg.plugins {
                    let m = &lp.manifest;
                    let np = native.plugins.iter().find(|p| p.id == m.id);
                    let approved = draft.trust.contains(&m.id);
                    Frame::new().fill(theme.panel_header).corner_radius(8).inner_margin(Margin::symmetric(12, 9)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&m.name).size(13.5).strong().color(theme.text));
                            ui.label(RichText::new(format!("v{} · {}", m.version, m.author)).size(11.0).color(theme.text_muted));
                            let script = np.is_some_and(|p| p.kind == Kind::Script);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match np.map(|p| &p.status) {
                                None => kit::pill(ui, t!("Blocks & config"), theme.text_muted),
                                Some(Status::Loaded) if script => kit::pill(ui, t!("Script · running"), theme.action),
                                Some(_) if script => kit::pill(ui, t!("Script · error"), theme.error),
                                Some(Status::Loaded) => kit::pill(ui, t!("Native · running"), theme.action),
                                Some(Status::NeedsApproval) if approved => {
                                    if kit::secondary_button(ui, theme, t!("Undo")).clicked() {
                                        draft.trust.retain(|id| *id != m.id);
                                    }
                                    kit::pill(ui, t!("Allowed on save"), theme.action);
                                }
                                Some(Status::NeedsApproval) => {
                                    if kit::primary_button(ui, theme, t!("Allow")).on_hover_text(lp.dir.display().to_string()).clicked() {
                                        draft.trust.push(m.id.clone());
                                    }
                                    kit::pill(ui, "Native", theme.warning);
                                }
                                Some(Status::Off) => kit::pill(ui, t!("Native · off"), theme.text_muted),
                                Some(_) => kit::pill(ui, t!("Native · not running"), theme.error),
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
            }
            Part::Footer => {
                if kit::primary_button(ui, theme, t!("Save and close")).clicked() {
                    ask = Some(Ask::Save(true));
                }
                if kit::secondary_button(ui, theme, t!("Save")).clicked() {
                    ask = Some(Ask::Save(false));
                }
                if kit::secondary_button(ui, theme, t!("Cancel")).clicked() {
                    ask = Some(Ask::Cancel);
                }
                if changed {
                    ui.label(RichText::new(t!("Unsaved changes")).size(12.0).color(theme.warning));
                }
            }
        },
    );
    ask
}

/// What the Plugins view of the side bar asks the app for.
pub enum SideAsk {
    /// Open the Plugins tab, to allow native plugins.
    Manage,
    Reload,
    OpenFolder,
    /// Install a plugin from a .zip file.
    Install,
}

/// The Plugins view of the side bar: every loaded plugin, with how it runs.
pub fn side(ui: &mut egui::Ui, theme: &Theme, ed: &Editor, native: &NativeHost) -> Option<SideAsk> {
    let mut ask = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        Frame::new().inner_margin(Margin::symmetric(12, 4)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if kit::button(ui, theme, t!("Manage…")).on_hover_text(t!("Allow native plugins")).clicked() {
                    ask = Some(SideAsk::Manage);
                }
                if kit::button(ui, theme, t!("Install…")).on_hover_text(t!("Install a plugin from a .zip file (or drop the file on the window)")).clicked() {
                    ask = Some(SideAsk::Install);
                }
                if kit::button(ui, theme, t!("Reload")).on_hover_text(t!("Load plugin files again")).clicked() {
                    ask = Some(SideAsk::Reload);
                }
                if kit::button(ui, theme, t!("Open folder")).on_hover_text(t!("Your own plugins go here")).clicked() {
                    ask = Some(SideAsk::OpenFolder);
                }
            });
            ui.add_space(8.0);
            for lp in &ed.reg.plugins {
                let m = &lp.manifest;
                let np = native.plugins.iter().find(|p| p.id == m.id);
                let script = np.is_some_and(|p| p.kind == Kind::Script);
                let (status, color) = match np.map(|p| &p.status) {
                    None => (t!("Blocks & config"), theme.text_muted),
                    Some(Status::Loaded) if script => (t!("Script · running"), theme.action),
                    Some(_) if script => (t!("Script · error"), theme.error),
                    Some(Status::Loaded) => (t!("Native · running"), theme.action),
                    Some(Status::NeedsApproval) => (t!("Native · needs your OK"), theme.warning),
                    Some(Status::Off) => (t!("Native · off"), theme.text_muted),
                    Some(_) => (t!("Native · not running"), theme.error),
                };
                ui.label(RichText::new(&m.name).size(13.0).strong().color(theme.text));
                ui.label(RichText::new(format!("v{} · {}", m.version, m.author)).size(11.0).color(theme.text_muted));
                ui.label(RichText::new(status).size(11.0).color(color));
                if !m.description.is_empty() {
                    kit::hint(ui, theme, &m.description);
                }
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
            }
        });
    });
    ask
}
