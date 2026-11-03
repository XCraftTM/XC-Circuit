//! The Builder | JSON switch the builders share, and the JSON text editor
//! behind it: edits and pasted JSON apply as soon as they are valid.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Align2, Color32, FontId, RichText, Stroke, Ui, vec2};
use serde_json::Value;

use crate::dialog;
use crate::theme::Theme;

/// The JSON a builder shows while switched to JSON, and why it doesn't apply.
#[derive(Clone, Debug, Default)]
pub struct JsonText {
    pub text: String,
    pub error: Option<String>,
    /// The line the JSON stops making sense on, marked by its number.
    pub error_line: Option<usize>,
}

impl JsonText {
    pub fn new(v: &Value) -> JsonText {
        JsonText { text: serde_json::to_string_pretty(v).unwrap_or_default(), error: None, error_line: None }
    }
}

/// Colors for the parts of JSON, from the theme's pin colors (darker on light themes).
struct Ink {
    key: Color32,
    string: Color32,
    number: Color32,
    literal: Color32,
    punct: Color32,
    text: Color32,
}

impl Ink {
    fn new(theme: &Theme) -> Ink {
        let pin = |key: &str, default: Color32| {
            let c = theme.pin_colors.get(key).copied().unwrap_or(default);
            if theme.dark { c } else { Color32::from_rgb((c.r() as f32 * 0.6) as u8, (c.g() as f32 * 0.6) as u8, (c.b() as f32 * 0.6) as u8) }
        };
        Ink {
            key: pin("list", Color32::from_rgb(0xB9, 0x8C, 0xFF)),
            string: pin("text", Color32::from_rgb(0xF5, 0xC4, 0x51)),
            number: pin("number", Color32::from_rgb(0x4F, 0xB3, 0xFF)),
            literal: pin("boolean", Color32::from_rgb(0xFF, 0x6F, 0x91)),
            punct: theme.text_muted,
            text: theme.text,
        }
    }
}

/// Where the string starting at `start` (a quote) ends: after its closing
/// quote, or at the end of the line when it has none.
fn string_end(text: &str, start: usize) -> usize {
    let mut escaped = false;
    for (i, c) in text[start + 1..].char_indices() {
        let at = start + 1 + i;
        match c {
            '\\' if !escaped => escaped = true,
            '"' if !escaped => return at + 1,
            '\n' => return at,
            _ => escaped = false,
        }
    }
    text.len()
}

/// JSON text in colors: keys, texts, numbers, true/false/null and punctuation.
fn highlight(text: &str, font: &FontId, ink: &Ink) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut i = 0;
    while let Some(c) = text[i..].chars().next() {
        let rest = &text[i..];
        let run = |stop: &dyn Fn(char) -> bool| i + rest.find(stop).unwrap_or(rest.len());
        let (end, color) = match c {
            '"' => {
                let end = string_end(text, i);
                (end, if text[end..].trim_start().starts_with(':') { ink.key } else { ink.string })
            }
            '-' | '0'..='9' => (run(&|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))), ink.number),
            c if c.is_alphabetic() => {
                let end = run(&|c: char| !c.is_alphanumeric());
                (end, if matches!(&text[i..end], "true" | "false" | "null") { ink.literal } else { ink.text })
            }
            '{' | '}' | '[' | ']' | ':' | ',' => (i + 1, ink.punct),
            _ => (run(&|c: char| c == '"' || c == '-' || c.is_alphanumeric() || "{}[]:,".contains(c)).max(i + c.len_utf8()), ink.text),
        };
        job.append(&text[i..end], 0.0, TextFormat::simple(font.clone(), color));
        i = end;
    }
    job
}

/// The Builder | JSON switch. Returns the side clicked this frame, if it is
/// not the one showing.
pub fn switch(ui: &mut Ui, theme: &Theme, json: bool) -> Option<bool> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let h = crate::inputs::input_height(ui);
        for (is_json, label) in [(false, t!("Builder")), (true, "JSON")] {
            let on = is_json == json;
            let ink = if on { if theme.dark { Color32::from_rgb(0, 24, 28) } else { Color32::WHITE } } else { theme.text };
            let corner = if is_json { egui::CornerRadius { nw: 0, sw: 0, ne: 7, se: 7 } } else { egui::CornerRadius { nw: 7, sw: 7, ne: 0, se: 0 } };
            let b = egui::Button::new(RichText::new(label).size(13.0).color(ink)).corner_radius(corner).min_size(vec2(82.0, h));
            let fill = if on { theme.accent } else { theme.panel };
            if dialog::styled_button(ui, theme, b, fill, Stroke::new(1.0, if on { theme.accent } else { theme.node_border })).clicked() && !on {
                picked = Some(is_json);
            }
        }
    });
    picked
}

/// The JSON as editable text, with Format and Copy, filling the room left in
/// the dialog tab. Returns the parsed value after an edit that parses; a parse
/// error is kept in `jt.error` and shown.
pub fn editor(ui: &mut Ui, theme: &Theme, id: egui::Id, jt: &mut JsonText, help: &str) -> Option<Value> {
    let mut parsed = None;
    ui.label(RichText::new(help).size(12.0).color(theme.text_muted));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if dialog::button(ui, theme, t!("Format")).on_hover_text(t!("Indent the JSON neatly")).clicked()
            && let Ok(v) = serde_json::from_str::<Value>(&jt.text)
        {
            jt.text = serde_json::to_string_pretty(&v).unwrap_or_default();
        }
        if dialog::button(ui, theme, t!("Copy")).clicked() {
            ui.ctx().copy_text(jt.text.clone());
        }
        if dialog::button(ui, theme, t!("Clear")).on_hover_text(t!("Empty the box, to paste other JSON in")).clicked() {
            jt.text.clear();
        }
        match &jt.error {
            Some(e) => ui.label(RichText::new(e.as_str()).size(12.0).color(theme.warning)),
            None => ui.label(RichText::new(t!("Applied")).size(12.0).color(theme.action)),
        };
    });
    ui.add_space(6.0);
    let before = jt.text.clone();
    // Less the frame's margins and line.
    let height = crate::dialog::remaining_height(ui, 220.0) - 18.0;
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let ink = Ink::new(theme);
    let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, _wrap: f32| ui.fonts_mut(|f| f.layout_job(highlight(text.as_str(), &font, &ink)));
    // Room for the line numbers, like a code editor.
    let lines = jt.text.split('\n').count();
    let digit = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
    let gutter = lines.to_string().len().max(2) as f32 * digit + 16.0;
    egui::Frame::new().fill(theme.field).stroke(Stroke::new(1.0, if jt.error.is_some() { theme.warning } else { theme.node_border })).corner_radius(7).inner_margin(8).show(ui, |ui| {
        egui::ScrollArea::both().id_salt(id.with("scroll")).max_height(height).auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let left = ui.cursor().left();
                ui.add_space(gutter);
                let out = egui::TextEdit::multiline(&mut jt.text)
                    .id(id)
                    .code_editor()
                    .frame(egui::Frame::NONE)
                    .desired_width(f32::INFINITY)
                    .min_size(vec2(0.0, height - 4.0))
                    .layouter(&mut layouter)
                    .show(ui);
                // A number at the start of each line (rows don't wrap), the
                // line with a JSON mistake marked.
                let painter = ui.painter();
                let mut line = 1;
                let mut starts_line = true;
                for row in &out.galley.rows {
                    if starts_line {
                        let rect = row.rect().translate(out.galley_pos.to_vec2());
                        let wrong = jt.error_line == Some(line);
                        if wrong {
                            let band = egui::Rect::from_x_y_ranges(left..=out.response.rect.right(), rect.y_range());
                            painter.rect_filled(band, 2.0, theme.warning.gamma_multiply(0.14));
                        }
                        let color = if wrong { theme.warning } else { theme.text_muted.gamma_multiply(0.75) };
                        painter.text(egui::pos2(left + gutter - 10.0, rect.center().y), Align2::RIGHT_CENTER, line.to_string(), font.clone(), color);
                        line += 1;
                    }
                    starts_line = row.ends_with_newline;
                }
            });
        });
    });
    if jt.text != before {
        match serde_json::from_str::<Value>(&jt.text) {
            Ok(v) => {
                jt.error = None;
                jt.error_line = None;
                parsed = Some(v);
            }
            Err(e) => {
                jt.error = Some(tf!("Line {line}, column {column}: {problem}", line = e.line(), column = e.column(), problem = e.to_string().split(" at line").next().unwrap_or_default()));
                jt.error_line = Some(e.line());
            }
        }
    }
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_each_part() {
        let ink = Ink { key: Color32::RED, string: Color32::YELLOW, number: Color32::BLUE, literal: Color32::GREEN, punct: Color32::GRAY, text: Color32::WHITE };
        let text = "{\n  \"title\": \"a \\\"b\\\" c\",\n  \"n\": -1.5e3, \"ok\": true, \"x\": null,\n  \"cut";
        let job = highlight(text, &FontId::monospace(12.0), &ink);
        let parts: Vec<(&str, Color32)> = job.sections.iter().map(|s| (&job.text[s.byte_range.start.0..s.byte_range.end.0], s.format.color)).filter(|(t, _)| !t.trim().is_empty()).collect();
        assert_eq!(job.text, text, "nothing is lost");
        assert_eq!(
            parts,
            [
                ("{", Color32::GRAY),
                ("\"title\"", Color32::RED),
                (":", Color32::GRAY),
                ("\"a \\\"b\\\" c\"", Color32::YELLOW),
                (",", Color32::GRAY),
                ("\"n\"", Color32::RED),
                (":", Color32::GRAY),
                ("-1.5e3", Color32::BLUE),
                (",", Color32::GRAY),
                ("\"ok\"", Color32::RED),
                (":", Color32::GRAY),
                ("true", Color32::GREEN),
                (",", Color32::GRAY),
                ("\"x\"", Color32::RED),
                (":", Color32::GRAY),
                ("null", Color32::GREEN),
                (",", Color32::GRAY),
                ("\"cut", Color32::YELLOW),
            ]
        );
    }
}
