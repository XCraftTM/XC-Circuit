//! Fonts beyond egui's own: a bold Ubuntu (egui writes in Ubuntu Light), for
//! bold text in message previews, and the Console's font: egui's monospaced
//! one, or one installed on the computer. Ubuntu Font Licence:
//! assets/fonts/LICENCE.txt.

use std::sync::Arc;

use eframe::egui::text::LayoutJob;
use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId};

const BOLD: &str = "bold";
const CONSOLE: &str = "console";

/// Adds the bold family to egui's fonts, with egui's own after it for the
/// characters it lacks (emoji and symbols), and the Console's with egui's
/// monospaced font. (The app itself uses [`install_with`]; tests use this.)
#[cfg(test)]
pub fn install(ctx: &egui::Context) {
    let _ = install_with(ctx, None);
}

/// [`install`], with `console_font` (an installed font family, by name) for
/// the Console. Says what went wrong when it can't be loaded; the Console
/// then keeps egui's monospaced font.
pub fn install_with(ctx: &egui::Context, console_font: Option<&str>) -> Result<(), String> {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("Ubuntu-Bold".into(), Arc::new(FontData::from_static(include_bytes!("../../assets/fonts/Ubuntu-Bold.ttf"))));
    let mut bold = vec!["Ubuntu-Bold".to_string()];
    bold.extend(fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    fonts.families.insert(FontFamily::Name(BOLD.into()), bold);
    // The Console's font first, then egui's monospaced one (and its
    // fallbacks) for the characters it lacks.
    let mut console = fonts.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    let mut result = Ok(());
    if let Some(name) = console_font {
        match load_installed(name) {
            Ok(data) => {
                let key = format!("installed:{name}");
                fonts.font_data.insert(key.clone(), Arc::new(data));
                console.insert(0, key);
            }
            Err(e) => result = Err(e),
        }
    }
    fonts.families.insert(FontFamily::Name(CONSOLE.into()), console);
    ctx.set_fonts(fonts);
    result
}

/// The regular face of an installed font family.
fn load_installed(name: &str) -> Result<FontData, String> {
    let mut collection = fontique::Collection::new(fontique::CollectionOptions { shared: false, system_fonts: true });
    let family = collection.family_by_name(name).ok_or_else(|| format!("there's no font called {name} on this computer"))?;
    let font = family.default_font().ok_or_else(|| format!("{name} has no font to use"))?;
    let blob = font.load(None).ok_or_else(|| format!("{name} couldn't be read"))?;
    let mut data = FontData::from_owned(blob.data().to_vec());
    data.index = font.index();
    Ok(data)
}

/// The fonts installed on this computer, by name, and whether each is
/// monospaced (every character as wide). It reads every family, which takes
/// a moment: not on the thread that draws.
pub fn installed_families() -> Vec<(String, bool)> {
    use skrifa::raw::TableProvider;
    let mut collection = fontique::Collection::new(fontique::CollectionOptions { shared: false, system_fonts: true });
    let names: Vec<String> = collection.family_names().map(String::from).collect();
    let mut out: Vec<(String, bool)> = names
        .into_iter()
        .map(|name| {
            let mono = collection
                .family_by_name(&name)
                .and_then(|f| f.default_font().cloned())
                .and_then(|f| f.load(None).map(|blob| (blob, f.index())))
                .is_some_and(|(blob, index)| skrifa::FontRef::from_index(blob.data(), index).ok().and_then(|f| f.post().ok()).is_some_and(|p| p.is_fixed_pitch() != 0));
            (name, mono)
        })
        .collect();
    out.sort_by_key(|(n, _)| n.to_lowercase());
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

/// The Console's text at `size`: its own font once installed, else egui's
/// monospaced one (contexts made without [`install`], such as tests').
pub fn console(ctx: &egui::Context, size: f32) -> FontId {
    let family = FontFamily::Name(CONSOLE.into());
    if ctx.fonts(|f| f.definitions().families.contains_key(&family)) { FontId::new(size, family) } else { FontId::monospace(size) }
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
