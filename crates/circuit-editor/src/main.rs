#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod i18n;
mod graphics;
mod icon;
mod app;
mod bot;
mod bot_profile;
mod canvas;
mod chips;
mod color;
mod dialog;
mod editor;
mod embed;
mod exchange;
mod fonts;
mod history;
mod inputs;
mod install;
mod json_view;
mod layout;
mod markdown;
mod locales;
mod native;
mod script;
mod new_project;
mod palette;
mod panels;
mod picker;
mod settings;
mod setup;
mod prefs;
mod restore;
mod reveal;
mod sidebar;
mod plugin_manager;
mod slash;
mod start_page;
mod stored;
mod theme;
mod titlebar;
mod updater;
mod tour;
mod values;
mod ui_strings;
mod watch;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result {
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("XC™ Circuit")
            .with_app_id("xc-circuit")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 600.0])
            // XC Circuit draws its own title bar; View › Use system title bar turns the system one back on.
            .with_decorations(false)
            .with_icon(app_icon(64)),
        window_builder: Some(Box::new(reveal::defer_maximize)),
        ..Default::default()
    };
    // The best graphics first; when they can't start, the next, more widely
    // supported ones (see graphics.rs).
    let stage = graphics::stage();
    graphics::configure(&mut options, stage);
    let result = eframe::run_native("XC Circuit", options, Box::new(|cc| Ok(Box::new(app::CircuitApp::new(cc)))));
    if let Err(e) = &result
        && graphics::is_graphics_error(e)
        && let Some(code) = graphics::restart_with_next(stage, e)
    {
        std::process::exit(code);
    }
    result
}

/// Draws the XC monogram (the C as a wire with two pins, an X inside) into an RGBA icon.
fn app_icon(size: u32) -> egui::IconData {
    egui::IconData { rgba: icon::icon_rgba(size), width: size, height: size }
}
