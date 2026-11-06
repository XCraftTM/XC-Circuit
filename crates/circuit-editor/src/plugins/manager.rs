//! Plugins › Manage Plugins: every installed plugin, what it adds and how
//! its editor script runs.

use eframe::egui::{self, Color32, Frame, Margin, RichText};

use crate::editor::Editor;
use crate::plugins::host::{PluginHost, Status, status_text};
use crate::ui::dialog::{self, Part};
use crate::ui::kit;
use crate::ui::theme::Theme;

/// A plugin's state in a few words, and its colour.
fn status_of(theme: &Theme, lp: &circuit_plugins::registry::LoadedPlugin, host: &PluginHost) -> (&'static str, Color32) {
    let m = &lp.manifest;
    match host.plugins.iter().find(|p| p.id == m.id).map(|p| &p.status) {
        Some(Status::Loaded) => (t!("Script · running"), theme.action),
        Some(_) => (t!("Script · error"), theme.error),
        // Compiled Rust plugins: XC Circuit runs only scripts now.
        None if m.native.is_some() => (t!("Native · not supported"), theme.warning),
        None => (t!("Blocks & config"), theme.text_muted),
    }
}

/// The Plugins tab. Returns what its buttons asked for.
pub fn tab(ui: &mut egui::Ui, theme: &Theme, ed: &Editor, host: &PluginHost) -> Option<SideAsk> {
    let mut ask = None;
    dialog::tab(
        ui,
        theme,
        dialog::Spec { id: "plugins", title: t!("Plugins"), subtitle: Some(t!("Every installed plugin and how it runs")), icon: None, width: 560.0 },
        |ui, part| match part {
            Part::Body => {
                kit::section(ui, theme, t!("Installed"));
                for lp in &ed.reg.plugins {
                    let m = &lp.manifest;
                    let (status, color) = status_of(theme, lp, host);
                    Frame::new().fill(theme.panel_header).corner_radius(8).inner_margin(Margin::symmetric(12, 9)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&m.name).size(13.5).strong().color(theme.text));
                            ui.label(RichText::new(format!("v{} · {}", m.version, m.author)).size(11.0).color(theme.text_muted));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| kit::pill(ui, status, color));
                        });
                        if !m.description.is_empty() {
                            ui.label(RichText::new(&m.description).size(12.0).color(theme.text_muted));
                        }
                        if let Some(p) = host.plugins.iter().find(|p| p.id == m.id)
                            && p.status != Status::Loaded
                        {
                            ui.label(RichText::new(status_text(&p.status)).size(11.5).color(theme.warning));
                        } else if m.native.is_some() && m.editor.is_none() {
                            ui.label(RichText::new(t!("A compiled (native) plugin: XC Circuit runs plugins written in JavaScript only. Its blocks and settings still load.")).size(11.5).color(theme.warning));
                        }
                    });
                    ui.add_space(6.0);
                }
            }
            Part::Footer => {
                if kit::secondary_button(ui, theme, t!("Open folder")).on_hover_text(t!("Your own plugins go here")).clicked() {
                    ask = Some(SideAsk::OpenFolder);
                }
                if kit::secondary_button(ui, theme, t!("Reload")).on_hover_text(t!("Load plugin files again")).clicked() {
                    ask = Some(SideAsk::Reload);
                }
                if kit::primary_button(ui, theme, t!("Install…")).on_hover_text(t!("Install a plugin from a .zip file (or drop the file on the window)")).clicked() {
                    ask = Some(SideAsk::Install);
                }
            }
        },
    );
    ask
}

/// What the Plugins view of the side bar asks the app for.
pub enum SideAsk {
    /// Open the Plugins tab.
    Manage,
    Reload,
    OpenFolder,
    /// Install a plugin from a .zip file.
    Install,
}

/// The Plugins view of the side bar: every loaded plugin, with how it runs.
pub fn side(ui: &mut egui::Ui, theme: &Theme, ed: &Editor, host: &PluginHost) -> Option<SideAsk> {
    let mut ask = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        Frame::new().inner_margin(Margin::symmetric(12, 4)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if kit::button(ui, theme, t!("Manage…")).on_hover_text(t!("Every installed plugin and how it runs")).clicked() {
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
                let (status, color) = status_of(theme, lp, host);
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
