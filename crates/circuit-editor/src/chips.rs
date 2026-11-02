//! Preview texts with each `${…}` shown as a chip, the way Discord shows a
//! mention: what goes there is only known when the bot runs, so the chip says
//! what it will be.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Color32, FontId};

/// Discord's mention colors.
const CHIP_FILL: Color32 = Color32::from_rgba_premultiplied(26, 30, 73, 77);
const CHIP_TEXT: Color32 = Color32::from_rgb(0xC9, 0xCD, 0xFB);

/// `text` laid out in `font` and `color`, wrapped at `wrap`, with each `${…}`
/// as a chip showing the expression.
pub fn job(text: &str, font: FontId, color: Color32, wrap: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let chars: Vec<char> = text.chars().collect();
    let plain = TextFormat::simple(font.clone(), color);
    let chip = TextFormat { font_id: font, color: CHIP_TEXT, background: CHIP_FILL, ..Default::default() };
    let mut from = 0;
    for (start, end) in circuit_core::template::expressions(text) {
        job.append(&chars[from..start - 2].iter().collect::<String>(), 0.0, plain.clone());
        let code = chars[start..end].iter().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
        job.append(&format!(" {code} "), 0.0, chip.clone());
        from = end + 1;
    }
    job.append(&chars[from.min(chars.len())..].iter().collect::<String>(), 0.0, plain);
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions_become_chips() {
        let job = job("Hello ${#1.user.username}! ${ xp  * 2 } XP", FontId::proportional(14.0), Color32::WHITE, 300.0);
        let parts: Vec<(&str, bool)> = job
            .sections
            .iter()
            .map(|s| {
                let (start, end): (usize, usize) = (s.byte_range.start.into(), s.byte_range.end.into());
                (&job.text[start..end], s.format.background != Color32::TRANSPARENT)
            })
            .collect();
        assert_eq!(parts, [("Hello ", false), (" #1.user.username ", true), ("! ", false), (" xp * 2 ", true), (" XP", false)]);
    }
}
