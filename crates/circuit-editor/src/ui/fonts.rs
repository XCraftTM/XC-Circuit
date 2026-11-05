//! Fonts beyond egui's own: a bold Ubuntu (egui writes in Ubuntu Light), for
//! bold text in message previews. Ubuntu Font Licence: assets/fonts/LICENCE.txt.

use std::sync::Arc;

use eframe::egui::text::LayoutJob;
use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId};

const BOLD: &str = "bold";

/// Adds the bold family to egui's fonts, with egui's own after it for the
/// characters it lacks (emoji and symbols).
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("Ubuntu-Bold".into(), Arc::new(FontData::from_static(include_bytes!("../../assets/fonts/Ubuntu-Bold.ttf"))));
    let mut bold = vec!["Ubuntu-Bold".to_string()];
    bold.extend(fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    fonts.families.insert(FontFamily::Name(BOLD.into()), bold);
    ctx.set_fonts(fonts);
}

/// Bold text at `size` (see `usable`).
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD.into()))
}

/// `job`, with its bold text in the regular font where the bold one isn't in
/// use (yet): egui stops at a font family it doesn't know. Contexts made
/// without `install` (tests') have only egui's fonts, and installed fonts
/// work from the next frame on.
pub fn usable(ctx: &egui::Context, mut job: LayoutJob) -> LayoutJob {
    let bold = FontFamily::Name(BOLD.into());
    if !ctx.fonts(|f| f.definitions().families.contains_key(&bold)) {
        for s in &mut job.sections {
            if s.format.font_id.family == bold {
                s.format.font_id.family = FontFamily::Proportional;
            }
        }
    }
    job
}
