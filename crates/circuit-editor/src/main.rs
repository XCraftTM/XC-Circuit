#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod i18n;
mod graphics;
mod icon;
mod app;
#[cfg(test)]
mod bench;
mod bench_data;
mod bot;
mod bot_profile;
mod canvas;
mod chips;
mod color;
mod crash;
mod dialog;
mod editor;
mod embed;
mod exchange;
mod fonts;
mod history;
mod inputs;
mod install;
mod keymap;
mod json_view;
mod layout;
mod markdown;
mod locales;
mod native;
mod script;
mod sound;
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
    // `xc-circuit --export-icon <folder>`: the icon as PNGs and an .ico, for packaging.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--export-icon") {
        let dir = std::path::PathBuf::from(args.get(2).map(String::as_str).unwrap_or("."));
        if let Err(e) = export_icon(&dir) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    // --fps [results.json]: measure real frame rates (see app/fps.rs).
    let fps = args.get(1).map(String::as_str) == Some("--fps");
    if fps {
        let out = std::path::PathBuf::from(args.get(2).map(String::as_str).unwrap_or("fps-results.json"));
        let out = std::path::absolute(&out).unwrap_or(out);
        if let Err(e) = app::fps::prepare(&out) {
            eprintln!("fps: couldn't prepare the project: {e}");
            std::process::exit(1);
        }
    }
    crash::install();
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
    if fps {
        // As fast as it can draw, at a fixed size.
        options.wgpu_options.surface.present_mode = eframe::wgpu::PresentMode::AutoNoVsync;
        options.window_builder = None;
    }
    let result = eframe::run_native("XC Circuit", options, Box::new(|cc| Ok(Box::new(app::CircuitApp::new(cc)))));
    if let Err(e) = &result
        && graphics::is_graphics_error(e)
        && let Some(code) = graphics::restart_with_next(stage, e)
    {
        std::process::exit(code);
    }
    result
}

/// Writes the icon as `xc-circuit-<size>.png` (16 to 512) and `xc-circuit.ico`.
fn export_icon(dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let png = |size: u32| -> Result<Vec<u8>, String> {
        let img = image::RgbaImage::from_raw(size, size, icon::icon_rgba(size)).ok_or("icon size")?;
        let mut bytes = std::io::Cursor::new(Vec::new());
        img.write_to(&mut bytes, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        Ok(bytes.into_inner())
    };
    for size in [16, 24, 32, 48, 64, 128, 256, 512] {
        std::fs::write(dir.join(format!("xc-circuit-{size}.png")), png(size)?).map_err(|e| e.to_string())?;
    }
    // An .ico holding PNG images, for the Windows installer.
    let sizes = [16u32, 24, 32, 48, 64, 128, 256];
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| png(s)).collect::<Result<_, _>>()?;
    let mut ico = vec![0, 0, 1, 0];
    ico.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&size, data) in sizes.iter().zip(&images) {
        let side = if size >= 256 { 0 } else { size as u8 };
        ico.extend_from_slice(&[side, side, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(data.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for data in images {
        ico.extend_from_slice(&data);
    }
    std::fs::write(dir.join("xc-circuit.ico"), ico).map_err(|e| e.to_string())
}

/// Draws the XC monogram (the C as a wire with two pins, an X inside) into an RGBA icon.
fn app_icon(size: u32) -> egui::IconData {
    egui::IconData { rgba: icon::icon_rgba(size), width: size, height: size }
}
