//! The Plugins view of the side bar: every installed plugin, what it adds and
//! how its editor script runs; turning plugins on and off, and removing them.
//! The plugins that come with the app can't be turned off or removed.

use eframe::egui::{self, Color32, Frame, Margin, RichText};

use crate::editor::Editor;
use crate::plugins::host::{PluginHost, Status, status_text};
use crate::ui::kit;
use crate::ui::theme::Theme;

/// What the Plugins view asks the app for.
pub enum SideAsk {
    Reload,
    OpenFolder,
    /// Install a plugin from a .zip file.
    Install,
    /// Turn a plugin (its id) on or off.
    Enable(String, bool),
    /// Remove a plugin (its id): its folder is deleted.
    Remove(String),
}

/// A plugin's state in a few words, and its colour.
fn status_of(theme: &Theme, lp: &circuit_plugins::registry::LoadedPlugin, host: &PluginHost, on: bool) -> (&'static str, Color32) {
    let m = &lp.manifest;
    if !on {
        return (t!("Turned off"), theme.text_muted);
    }
    match host.plugins.iter().find(|p| p.id == m.id).map(|p| &p.status) {
        Some(Status::Loaded) => (t!("Script · running"), theme.action),
        Some(_) => (t!("Script · error"), theme.error),
        // Compiled Rust plugins: XC Circuit runs only scripts now.
        None if m.native.is_some() => (t!("Native · not supported"), theme.warning),
        None => (t!("Blocks & config"), theme.text_muted),
    }
}

/// The Plugins view of the side bar: every plugin, turned on or off, with
/// how it runs. Returns what its buttons asked for.
pub fn side(ui: &mut egui::Ui, theme: &Theme, ed: &Editor, host: &PluginHost) -> Option<SideAsk> {
    let mut ask = None;
    // The plugin whose trash can was clicked: removing it waits for a second "Remove".
    let confirm_id = egui::Id::unique("plugin-remove");
    let mut confirm: Option<String> = ui.ctx().data(|d| d.get_temp(confirm_id));
    let trash = ed.reg.icons.get("trash").cloned();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        Frame::new().inner_margin(Margin::symmetric(12, 4)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
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
            // By name: turning one off or on doesn't move it.
            let mut all: Vec<_> = ed.reg.plugins.iter().map(|p| (p, true)).chain(ed.reg.disabled.iter().map(|p| (p, false))).collect();
            all.sort_by_key(|(p, _)| p.manifest.name.to_lowercase());
            for (lp, on) in all {
                let m = &lp.manifest;
                let builtin = ed.bundled_plugins.contains(&m.id);
                let (status, color) = status_of(theme, lp, host, on);
                Frame::new().fill(theme.panel_header).corner_radius(8).inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let name = RichText::new(&m.name).size(13.0).strong().color(if on { theme.text } else { theme.text_muted });
                        ui.add(egui::Label::new(name).truncate());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if builtin {
                                kit::pill(ui, t!("Built in"), theme.text_muted);
                                return;
                            }
                            if kit::row_icon(ui, theme, trash.as_deref(), "×", t!("Remove this plugin")).clicked() {
                                confirm = Some(m.id.clone());
                            }
                            let mut now = on;
                            if kit::toggle(ui, &mut now, theme).on_hover_text(t!("Turn this plugin on or off")).changed() {
                                ask = Some(SideAsk::Enable(m.id.clone(), now));
                            }
                        });
                    });
                    ui.label(RichText::new(format!("v{} · {}", m.version, m.author)).size(11.0).color(theme.text_muted));
                    ui.label(RichText::new(status).size(11.0).color(color));
                    if let Some(p) = host.plugins.iter().find(|p| p.id == m.id)
                        && p.status != Status::Loaded
                        && on
                    {
                        ui.add(egui::Label::new(RichText::new(status_text(&p.status)).size(11.0).color(theme.warning)).wrap());
                    } else if m.native.is_some() && m.editor.is_none() && on {
                        kit::hint(ui, theme, t!("A compiled (native) plugin: XC Circuit runs plugins written in JavaScript only. Its blocks and settings still load."));
                    }
                    if !m.description.is_empty() {
                        kit::hint(ui, theme, &m.description);
                    }
                    if confirm.as_deref() == Some(m.id.as_str()) && !builtin {
                        ui.add_space(4.0);
                        ui.add(egui::Label::new(RichText::new(tf!("Remove {name}? Its folder is deleted: {folder}", name = m.name, folder = lp.dir.display())).size(11.5).color(theme.warning)).wrap());
                        ui.horizontal(|ui| {
                            if kit::danger_button(ui, theme, t!("Remove")).clicked() {
                                ask = Some(SideAsk::Remove(m.id.clone()));
                                confirm = None;
                            }
                            if kit::secondary_button(ui, theme, t!("Cancel")).clicked() {
                                confirm = None;
                            }
                        });
                    }
                });
                ui.add_space(6.0);
            }
        });
    });
    ui.ctx().data_mut(|d| match confirm {
        Some(id) => {
            d.insert_temp(confirm_id, id);
        }
        None => d.remove::<String>(confirm_id),
    });
    ask
}
