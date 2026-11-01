#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bot;
mod canvas;
mod dialog;
mod editor;
mod embed;
mod history;
mod native;
mod palette;
mod panels;
mod picker;
mod plugin_manager;
mod slash;
mod stored;
mod theme;
mod titlebar;
mod watch;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("XC™ Circuit")
            .with_app_id("xc-circuit")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 600.0])
            // XC Circuit draws its own title bar; View › Use system title bar turns the system one back on.
            .with_decorations(false)
            .with_icon(app_icon(64)),
        ..Default::default()
    };
    eframe::run_native("XC Circuit", options, Box::new(|cc| Ok(Box::new(app::CircuitApp::new(cc)))))
}

/// Draws the XC monogram (the C as a wire with two pins, an X inside) into an RGBA icon.
fn app_icon(size: u32) -> egui::IconData {
    let s = size as f32 / 200.0;
    let aqua = [0x00, 0xE5, 0xFF];
    let ink = [0xE4, 0xF4, 0xF7];
    let bg = [0x0A, 0x16, 0x1B];
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            // Sample in the 200×200 design space.
            let (px, py) = ((x as f32 + 0.5) / s, (y as f32 + 0.5) / s);
            let inside_tile = {
                let (cx, cy) = (px.clamp(50.0, 150.0), py.clamp(50.0, 150.0));
                ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() <= 46.0
            };
            if !inside_tile {
                continue;
            }
            let mut color = bg;
            let (dx, dy) = (px - 100.0, py - 100.0);
            let r = (dx * dx + dy * dy).sqrt();
            let angle = dy.atan2(dx).to_degrees();
            if (r - 54.0).abs() <= 8.0 && !(-45.0..=45.0).contains(&angle) {
                color = aqua;
            }
            let on_x = |ax: f32, ay: f32, bx: f32, by: f32| {
                let (vx, vy) = (bx - ax, by - ay);
                let t = (((px - ax) * vx + (py - ay) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
                ((px - ax - t * vx).powi(2) + (py - ay - t * vy).powi(2)).sqrt() <= 6.5
            };
            if on_x(82.0, 82.0, 118.0, 118.0) || on_x(118.0, 82.0, 82.0, 118.0) {
                color = ink;
            }
            for (cx, cy) in [(138.2, 61.8), (138.2, 138.2)] {
                let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                if d <= 16.5 {
                    color = if d >= 9.5 { aqua } else { [0x07, 0x10, 0x14] };
                }
            }
            let i = ((y * size + x) * 4) as usize;
            rgba[i..i + 3].copy_from_slice(&color);
            rgba[i + 3] = 255;
        }
    }
    egui::IconData { rgba, width: size, height: size }
}
