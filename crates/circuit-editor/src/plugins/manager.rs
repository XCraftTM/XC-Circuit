//! The Plugins view of the side bar: every installed plugin, what it adds and
//! how its editor script runs; turning plugins on and off, and removing them.
//! The plugins that come with the app can't be turned off or removed, except
//! optional ones: removing those hides them, and Add back brings them back.

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
    /// Remove a plugin (its id): its folder is deleted (an optional one that
    /// comes with the app is left out instead).
    Remove(String),
    /// Bring back an optional plugin that comes with the app (its id).
    AddBack(String),
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

/// Whether a plugin is among those the search asks for: every word typed
/// is in its name, its description, its author or its id.
fn found(search: &str, m: &circuit_plugins::manifest::PluginManifest) -> bool {
    let hay = format!("{} {} {} {}", m.name, m.description, m.author, m.id).to_lowercase();
    search.to_lowercase().split_whitespace().all(|w| hay.contains(w))
}

/// The Plugins view of the side bar: a search, then the plugins you added
/// and the ones that come with the app, each a row with its picture, name,
/// what it does, who made it and a gear for what can be done with it.
/// Returns what was asked for.
pub fn side(ui: &mut egui::Ui, theme: &Theme, ed: &Editor, host: &PluginHost) -> Option<SideAsk> {
    use crate::ui::side;
    let mut ask = None;
    // The plugin whose removal was asked for: it waits for a second "Remove".
    let confirm_id = egui::Id::unique("plugin-remove");
    let mut confirm: Option<String> = ui.ctx().data(|d| d.get_temp(confirm_id));
    let search_id = egui::Id::unique("plugin-search");
    let mut search: String = ui.ctx().data(|d| d.get_temp(search_id)).unwrap_or_default();
    side::search(ui, theme, &mut search, t!("Search plugins"));
    ui.ctx().data_mut(|d| d.insert_temp(search_id, search.clone()));
    let icons = &ed.reg.icons;
    let removed = |id: &str| ed.prefs.removed_plugins.iter().any(|p| p == id);
    // By name: turning one off or on doesn't move it.
    let mut all: Vec<_> = ed.reg.plugins.iter().map(|p| (p, true)).chain(ed.reg.disabled.iter().filter(|p| !removed(&p.manifest.id)).map(|p| (p, false))).filter(|(p, _)| found(&search, &p.manifest)).collect();
    all.sort_by_key(|(p, _)| p.manifest.name.to_lowercase());
    let (built_in, added): (Vec<_>, Vec<_>) = all.into_iter().partition(|(p, _)| ed.bundled_plugins.contains(&p.manifest.id));
    let gone: Vec<&(String, String)> = ed.optional_plugins.iter().filter(|(id, _)| removed(id)).collect();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut rows = |ui: &mut egui::Ui, list: &[(&circuit_plugins::registry::LoadedPlugin, bool)], built_in: bool| {
            for (lp, on) in list {
                let m = &lp.manifest;
                let (status, color) = status_of(theme, lp, host, *on);
                let (rect, resp) = side::row(ui, theme, 62.0, false);
                kit::describe(&resp, egui::Role::Button, &m.name);
                let picture = egui::Rect::from_min_size(rect.min + egui::vec2(12.0, 11.0), egui::vec2(40.0, 40.0));
                let own_icon = m.icon.as_ref().map(|i| lp.dir.join(i)).filter(|p| p.is_file());
                let letters: String = m.name.chars().filter(|c| c.is_alphanumeric()).take(2).collect::<String>().to_uppercase();
                side::tile(ui, picture, if *on { theme.accent } else { theme.text_muted }, own_icon.as_deref().or(icons.get("plug").map(|p| p.as_path())), &letters);
                let x = picture.right() + 10.0;
                let width = rect.right() - x - 34.0;
                let ink = if *on { theme.text } else { theme.text_muted };
                let p = ui.painter();
                p.text(egui::pos2(x, rect.top() + 9.0), egui::Align2::LEFT_TOP, side::fitted(ui, &m.name, egui::FontId::proportional(13.0), width), egui::FontId::proportional(13.0), ink);
                p.text(egui::pos2(x, rect.top() + 27.0), egui::Align2::LEFT_TOP, side::fitted(ui, &m.description, egui::FontId::proportional(11.5), width + 26.0), egui::FontId::proportional(11.5), theme.text_muted);
                let by = if m.author.is_empty() { format!("v{}", m.version) } else { format!("{} · v{}", m.author, m.version) };
                let by_w = p.text(egui::pos2(x, rect.top() + 43.0), egui::Align2::LEFT_TOP, &by, egui::FontId::proportional(11.0), theme.text_muted).width();
                p.text(egui::pos2(x + by_w + 8.0, rect.top() + 43.0), egui::Align2::LEFT_TOP, side::fitted(ui, status, egui::FontId::proportional(11.0), (width - by_w - 8.0).max(0.0)), egui::FontId::proportional(11.0), color);
                // The whole of it, held over the row.
                let script = host.plugins.iter().find(|p| p.id == m.id).filter(|p| p.status != Status::Loaded && *on).map(|p| status_text(&p.status));
                let resp = resp.on_hover_ui(|ui| {
                    ui.set_max_width(360.0);
                    ui.label(RichText::new(format!("{}  v{}", m.name, m.version)).strong());
                    if !m.description.is_empty() {
                        ui.label(&m.description);
                    }
                    ui.label(RichText::new(status).color(color));
                    if let Some(why) = &script {
                        ui.label(RichText::new(why).color(theme.warning));
                    } else if m.native.is_some() && m.editor.is_none() && *on {
                        ui.label(RichText::new(t!("A compiled (native) plugin: XC Circuit runs plugins written in JavaScript only. Its blocks and settings still load.")).color(theme.warning));
                    }
                    ui.label(RichText::new(lp.dir.display().to_string()).size(11.0).color(theme.text_muted));
                });
                // The gear: what can be done with it. Clicking the row opens it too.
                let gear_rect = egui::Rect::from_min_size(egui::pos2(rect.right() - 30.0, rect.top() + 6.0), egui::vec2(22.0, 22.0));
                let mut gear_ui = ui.new_child(egui::UiBuilder::new().id_salt(("plugin-gear", &m.id)).max_rect(gear_rect));
                let gear = side::action(&mut gear_ui, theme, icons, "settings", t!("Manage this plugin"));
                let menu = |ui: &mut egui::Ui, ask: &mut Option<SideAsk>, confirm: &mut Option<String>| {
                    if built_in {
                        ui.label(RichText::new(t!("Comes with the app")).size(11.5).color(theme.text_muted));
                    } else {
                        let label = if *on { t!("Turn off") } else { t!("Turn on") };
                        if kit::menu_item(ui, label, None, true).clicked() {
                            *ask = Some(SideAsk::Enable(m.id.clone(), !*on));
                            ui.close();
                        }
                        if kit::menu_item(ui, t!("Remove…"), None, true).clicked() {
                            *confirm = Some(m.id.clone());
                            ui.close();
                        }
                    }
                    if kit::menu_item(ui, t!("Show its folder"), None, true).clicked() {
                        crate::editor::show_in_folder(&lp.dir);
                        ui.close();
                    }
                };
                egui::Popup::menu(&gear).id(egui::Id::unique(("plugin-menu", &m.id))).show(|ui| menu(ui, &mut ask, &mut confirm));
                resp.context_menu(|ui| menu(ui, &mut ask, &mut confirm));
                if confirm.as_deref() == Some(m.id.as_str()) && !built_in {
                    Frame::new().inner_margin(Margin::symmetric(14, 6)).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        let optional = ed.optional_plugins.iter().any(|(p, _)| *p == m.id);
                        let question = if optional { tf!("Remove {name}? It comes with the app, so Add back at the end of this list brings it back.", name = m.name) } else { tf!("Remove {name}? Its folder is deleted: {folder}", name = m.name, folder = lp.dir.display()) };
                        ui.add(egui::Label::new(RichText::new(question).size(11.5).color(theme.warning)).wrap());
                        ui.horizontal(|ui| {
                            if kit::danger_button(ui, theme, t!("Remove")).clicked() {
                                ask = Some(SideAsk::Remove(m.id.clone()));
                                confirm = None;
                            }
                            if kit::secondary_button(ui, theme, t!("Cancel")).clicked() {
                                confirm = None;
                            }
                        });
                    });
                }
            }
        };
        side::section(ui, theme, "plugins-added", t!("Installed"), Some(added.len()), |_| {}, |ui| {
            if added.is_empty() {
                side::note(ui, theme, if search.trim().is_empty() { t!("None yet. Install one from a .zip file with + above, or drop the file on the window.") } else { t!("No plugin of that name.") });
            }
            rows(ui, &added, false);
        });
        side::section(ui, theme, "plugins-built-in", t!("Built in"), Some(built_in.len()), |_| {}, |ui| rows(ui, &built_in, true));
        if !gone.is_empty() {
            side::section(ui, theme, "plugins-removed", t!("Removed"), Some(gone.len()), |_| {}, |ui| {
                for (id, name) in &gone {
                    Frame::new().inner_margin(Margin::symmetric(14, 3)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(name).size(12.5).color(theme.text_muted));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if kit::button(ui, theme, t!("Add back")).on_hover_text(t!("It comes with the app: turn it on again")).clicked() {
                                    ask = Some(SideAsk::AddBack(id.clone()));
                                }
                            });
                        });
                    });
                }
            });
        }
    });
    ui.ctx().data_mut(|d| match confirm {
        Some(id) => {
            d.insert_temp(confirm_id, id);
        }
        None => d.remove::<String>(confirm_id),
    });
    ask
}
