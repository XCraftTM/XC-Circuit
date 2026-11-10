//! Windows of their own: what is set up once and closed again (exporting the
//! bot, its remote, importing and exporting workspaces …) opens in a real
//! window with its own frame, beside the app's, not as a tab in it.

use super::*;

impl Tab {
    /// How big its window starts, for the ones that open in a window of
    /// their own. Git, Host and Live session only do while they are neither
    /// an icon in the activity bar nor a tab.
    pub(super) fn own_window(&self) -> Option<egui::Vec2> {
        Some(match self {
            Tab::Export => egui::vec2(580.0, 640.0),
            Tab::Hosting => egui::vec2(700.0, 780.0),
            Tab::Git | Tab::Host | Tab::Live => egui::vec2(700.0, 740.0),
            Tab::WorkspaceExport | Tab::WorkspaceImport => egui::vec2(680.0, 700.0),
            _ => return None,
        })
    }
}

/// Shows each of `tabs` in a window of its own, with the app's title bar
/// (or the system's, when that is turned on). Returns the ones that were
/// closed. `places`: where each first went, so it stays put while open.
pub(super) fn show(ctx: &egui::Context, theme: &crate::ui::theme::Theme, viewer: &mut Viewer, tabs: &[Tab], places: &mut std::collections::HashMap<String, (egui::Pos2, egui::Vec2)>) -> Vec<Tab> {
    let keys: Vec<String> = tabs.iter().map(super::chrome::window_key).collect();
    places.retain(|k, _| keys.contains(k));
    let mut closed = Vec::new();
    for (tab, key) in tabs.iter().zip(keys) {
        let Some(size) = tab.own_window() else { continue };
        let place = *places.entry(key.clone()).or_insert_with(|| dialog::centered_now(ctx, size));
        let title = viewer.title(tab);
        let icon = viewer.icon(tab);
        let system_frame = viewer.system_titlebar();
        let asked = dialog::own_window(ctx, theme, dialog::Own { key: &key, title: &title, icon: icon.as_deref(), place, system_frame }, |ui| {
            crate::app::watchdog::mark(format!("in the {title} window"));
            viewer.in_window = true;
            viewer.ui(ui, tab);
            viewer.in_window = false;
        });
        if asked {
            closed.push(tab.clone());
        }
    }
    closed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_of_its_own_sits_over_the_app_and_fits_the_screen() {
        let app = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(1400.0, 900.0));
        let (pos, size) = dialog::centered(Some(app), Some(egui::vec2(1920.0, 1080.0)), egui::vec2(700.0, 780.0));
        assert_eq!(size, egui::vec2(700.0, 780.0));
        assert_eq!(pos, egui::pos2(450.0, 160.0));
        // A small screen: it gets shorter instead of hanging off it.
        let (_, size) = dialog::centered(Some(app), Some(egui::vec2(1280.0, 720.0)), egui::vec2(700.0, 780.0));
        assert_eq!(size, egui::vec2(700.0, 600.0));
        // What is set up once opens this way; what is worked in stays a tab.
        assert!(Tab::Export.own_window().is_some() && Tab::Hosting.own_window().is_some() && Tab::WorkspaceImport.own_window().is_some());
        assert!(Tab::EmbedBuilder.own_window().is_none() && Tab::BotSettings.own_window().is_none() && Tab::Console.own_window().is_none());
    }
}
