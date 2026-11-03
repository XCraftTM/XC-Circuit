#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod i18n;
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
mod history;
mod inputs;
mod json_view;
mod layout;
mod locales;
mod native;
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
mod stored;
mod theme;
mod titlebar;
mod tour;
mod values;
mod ui_strings;
mod watch;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result {
    #[cfg_attr(not(windows), expect(unused_mut))]
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
    // wgpu also tries OpenGL by default, and NVIDIA's OpenGL driver opens
    // screen-sized helper windows for that, which flash white at startup.
    // DirectX 12 and Vulkan cover every Windows XC Circuit runs on; the
    // WGPU_BACKEND variable still chooses another.
    #[cfg(windows)]
    if std::env::var_os("WGPU_BACKEND").is_none()
        && let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup
    {
        setup.instance_descriptor.backends = eframe::wgpu::Backends::PRIMARY;
    }
    eframe::run_native("XC Circuit", options, Box::new(|cc| Ok(Box::new(app::CircuitApp::new(cc)))))
}

/// Draws the XC monogram (the C as a wire with two pins, an X inside) into an RGBA icon.
fn app_icon(size: u32) -> egui::IconData {
    egui::IconData { rgba: icon::icon_rgba(size), width: size, height: size }
}
