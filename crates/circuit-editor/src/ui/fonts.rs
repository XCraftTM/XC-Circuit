//! Fonts beyond egui's own: a bold Ubuntu (egui writes in Ubuntu Light), for
//! bold text in message previews, the Console's font: egui's monospaced
//! one, or one installed on the computer, and the computer's colour emoji.
//! Ubuntu Font Licence: assets/fonts/LICENCE.txt.

use std::sync::{Arc, OnceLock};

use eframe::egui::text::LayoutJob;
use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId};

const BOLD: &str = "bold";
const CONSOLE: &str = "console";
const COLOR_EMOJI: &str = "color-emoji";

/// Adds the bold family to egui's fonts, with egui's own after it for the
/// characters it lacks (emoji and symbols), and the Console's with egui's
/// monospaced font. (The app itself uses [`install_with`]; tests use this.)
#[cfg(test)]
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions(None, None).0);
}

/// [`install`], with `console_font` (an installed font family, by name) for
/// the Console. Says what went wrong when it can't be loaded; the Console
/// then keeps egui's monospaced font.
pub fn install_with(ctx: &egui::Context, console_font: Option<&str>) -> Result<(), String> {
    let (fonts, result) = definitions(console_font, color_emoji());
    ctx.set_fonts(fonts);
    result
}

/// egui's fonts with ours: the colour emoji (when given) before egui's
/// black-and-white ones in every family, the bold family and the Console's.
fn definitions(console_font: Option<&str>, emoji: Option<Arc<FontData>>) -> (FontDefinitions, Result<(), String>) {
    let mut fonts = FontDefinitions::default();
    if let Some(emoji) = emoji {
        fonts.font_data.insert(COLOR_EMOJI.into(), emoji);
        for family in fonts.families.values_mut() {
            let at = family.iter().position(|f| f == "NotoEmoji-Regular").unwrap_or(family.len());
            family.insert(at, COLOR_EMOJI.into());
        }
    }
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
    (fonts, result)
}

/// The computer's colour emoji font (Segoe UI Emoji on Windows, Noto Color
/// Emoji on Linux, Apple Color Emoji on a Mac), so 😀 🎫 ✅ look as they do
/// in Discord, with only its colour emoji in it: see [`only_emoji`]. Read
/// once; None when there is none.
fn color_emoji() -> Option<Arc<FontData>> {
    static FONT: OnceLock<Option<Arc<FontData>>> = OnceLock::new();
    FONT.get_or_init(|| {
        let mut collection = fontique::Collection::new(fontique::CollectionOptions { shared: false, system_fonts: true });
        ["Segoe UI Emoji", "Noto Color Emoji", "Apple Color Emoji"].into_iter().find_map(|name| {
            let font = collection.family_by_name(name)?.default_font()?.clone();
            let blob = font.load(None)?;
            let (data, index) = only_emoji(blob.data(), font.index())?;
            let mut data = FontData::from_owned(data);
            data.index = index;
            Some(Arc::new(data))
        })
    })
    .clone()
}

/// Whether `c` stays text when it comes without the emoji variation
/// selector, as it does in the interface: Unicode's Emoji_Presentation=No
/// symbols that the editor draws in its own colours (arrows, ▶ ⏹, ⚠, ✔,
/// ™ …). Emoji drawn as emoji by default (😀, ✅, ⭐ …) are false.
fn stays_text(c: u32) -> bool {
    // Every Emoji_Presentation=Yes character below U+1F000 (from there on
    // all are drawn as emoji).
    const EMOJI: &[(u32, u32)] = &[
        (0x231A, 0x231B),
        (0x23E9, 0x23EC),
        (0x23F0, 0x23F0),
        (0x23F3, 0x23F3),
        (0x25FD, 0x25FE),
        (0x2614, 0x2615),
        (0x2648, 0x2653),
        (0x267F, 0x267F),
        (0x2693, 0x2693),
        (0x26A1, 0x26A1),
        (0x26AA, 0x26AB),
        (0x26BD, 0x26BE),
        (0x26C4, 0x26C5),
        (0x26CE, 0x26CE),
        (0x26D4, 0x26D4),
        (0x26EA, 0x26EA),
        (0x26F2, 0x26F3),
        (0x26F5, 0x26F5),
        (0x26FA, 0x26FA),
        (0x26FD, 0x26FD),
        (0x2705, 0x2705),
        (0x270A, 0x270B),
        (0x2728, 0x2728),
        (0x274C, 0x274C),
        (0x274E, 0x274E),
        (0x2753, 0x2755),
        (0x2757, 0x2757),
        (0x2795, 0x2797),
        (0x27B0, 0x27B0),
        (0x27BF, 0x27BF),
        (0x2B1B, 0x2B1C),
        (0x2B50, 0x2B50),
        (0x2B55, 0x2B55),
    ];
    if c >= 0x1F000 || EMOJI.iter().any(|&(a, b)| (a..=b).contains(&c)) {
        return false;
    }
    // Letters and punctuation (© ® ™ ‼ ℹ), arrows, technical and media
    // symbols (⌨ ⏹), geometric shapes (▶ ◀), more arrows, CJK signs, and
    // the few misc symbols and dingbats used as plain marks (⚠ ☑ ✔ ✖ ➡).
    matches!(c, 0..=0x24FF | 0x25A0..=0x25FF | 0x2900..=0x2BFF | 0x3000..=0x33FF | 0x26A0 | 0x2611 | 0x2714 | 0x2716 | 0x27A1)
}

/// `font` (the face at `index`, for a collection) with a character map of
/// only its colour emoji, and the face's index in what comes back. egui
/// takes a character from the first font that has it, so the plain symbols
/// in an emoji font (and the ones that [`stays_text`]) are left to egui's
/// own fonts, in the text's colour. The new map is added at the end of the
/// file and the face pointed to it. None when the font has no colour emoji.
fn only_emoji(font: &[u8], index: u32) -> Option<(Vec<u8>, u32)> {
    use skrifa::MetadataProvider;
    use skrifa::raw::TableProvider;
    let face = skrifa::FontRef::from_index(font, index).ok()?;
    if face.colr().is_err() && face.cbdt().is_err() && face.sbix().is_err() {
        return None;
    }
    let colr = face.color_glyphs();
    let strikes = face.bitmap_strikes();
    let colour = |g| colr.get(g).is_some() || strikes.glyph_for_size(skrifa::instance::Size::new(64.0), g).is_some();
    let mut map: Vec<(u32, u32)> = face.charmap().mappings().filter(|&(c, g)| !stays_text(c) && colour(g)).map(|(c, g)| (c, g.to_u32())).collect();
    if map.is_empty() {
        return None;
    }
    map.sort_unstable();
    // Runs of characters with glyphs one after another: format 12's groups.
    let mut groups: Vec<(u32, u32, u32)> = Vec::new();
    for (c, g) in map {
        match groups.last_mut() {
            Some((start, end, first)) if c == *end + 1 && g == *first + (c - *start) => *end = c,
            _ => groups.push((c, c, g)),
        }
    }
    let mut cmap = Vec::new();
    cmap.extend(0u16.to_be_bytes()); // version
    cmap.extend(1u16.to_be_bytes()); // one subtable:
    cmap.extend(3u16.to_be_bytes()); // Windows,
    cmap.extend(10u16.to_be_bytes()); // full Unicode,
    cmap.extend(12u32.to_be_bytes()); // right after this.
    cmap.extend(12u16.to_be_bytes()); // Format 12
    cmap.extend(0u16.to_be_bytes());
    cmap.extend((16 + 12 * groups.len() as u32).to_be_bytes());
    cmap.extend(0u32.to_be_bytes()); // language
    cmap.extend((groups.len() as u32).to_be_bytes());
    for (start, end, glyph) in groups {
        cmap.extend(start.to_be_bytes());
        cmap.extend(end.to_be_bytes());
        cmap.extend(glyph.to_be_bytes());
    }

    // The face's table directory: at the start, or where a collection says.
    let u32_at = |at: usize| Some(u32::from_be_bytes(font.get(at..at + 4)?.try_into().ok()?));
    let dir = if font.starts_with(b"ttcf") { u32_at(12 + 4 * index as usize)? as usize } else { 0 };
    let tables = u16::from_be_bytes(font.get(dir + 4..dir + 6)?.try_into().ok()?) as usize;
    let record = (0..tables).map(|i| dir + 12 + 16 * i).find(|&r| font.get(r..r + 4) == Some(b"cmap"))?;
    let mut out = font.to_vec();
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
    let at = out.len() as u32;
    out.extend(&cmap);
    out[record + 8..record + 12].copy_from_slice(&at.to_be_bytes());
    out[record + 12..record + 16].copy_from_slice(&(cmap.len() as u32).to_be_bytes());
    Some((out, index))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_colour_emoji_come_from_the_emoji_font() {
        // Symbols the interface draws in its own colours stay text …
        for c in ['▶', '⚠', '✔', '⏹', '™', '©', '→', '#', '1'] {
            assert!(stays_text(c as u32), "{c}");
        }
        // … and emoji are emoji.
        for c in ['😀', '🎫', '✅', '⭐', '❤', '☀', '🔒'] {
            assert!(!stays_text(c as u32), "{c}");
        }
        // With the computer's emoji font (every Windows has one), its map
        // has the emoji and none of those symbols.
        let started = std::time::Instant::now();
        let Some(font) = color_emoji() else {
            if cfg!(windows) {
                panic!("Windows has Segoe UI Emoji");
            }
            return;
        };
        eprintln!("emoji font read in {:?}", started.elapsed());
        let face = skrifa::FontRef::from_index((*font.font).as_ref(), font.index).unwrap();
        use skrifa::MetadataProvider;
        let map = face.charmap();
        for c in ['😀', '🎫', '✅', '🔒'] {
            assert!(map.map(c).is_some(), "{c} is in the emoji font");
        }
        for c in ['▶', '⚠', '✔', '™', 'A', '1', '✓'] {
            assert!(map.map(c).is_none(), "{c} is left to the other fonts");
        }
    }
}
