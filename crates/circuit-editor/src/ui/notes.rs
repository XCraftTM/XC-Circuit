//! Release notes and other Markdown written for people, in the app's own
//! colors: # headings, - and 1. lists, **bold**, `code` and [links](https://…).
//! (Discord's own Markdown, for message previews, is in `builders::markdown`.)

use eframe::egui::{self, RichText, Ui};

use crate::ui::theme::Theme;

/// A piece of a line.
#[derive(Debug, PartialEq)]
enum Span<'a> {
    Text(&'a str),
    Bold(&'a str),
    Code(&'a str),
    Link(&'a str, &'a str),
}

/// Splits a line into its pieces. Unclosed marks are kept as text.
fn spans<'a>(line: &'a str) -> Vec<Span<'a>> {
    let mut out = Vec::new();
    let (mut rest, mut text_from) = (line, 0usize);
    let mut at = 0usize;
    let push_text = |out: &mut Vec<Span<'a>>, from: usize, to: usize| {
        if to > from {
            out.push(Span::Text(&line[from..to]));
        }
    };
    while !rest.is_empty() {
        let found = if let Some(inner) = rest.strip_prefix("**") {
            inner.find("**").filter(|&end| end > 0).map(|end| (Span::Bold(&inner[..end]), 4 + end))
        } else if let Some(inner) = rest.strip_prefix('`') {
            inner.find('`').filter(|&end| end > 0).map(|end| (Span::Code(&inner[..end]), 2 + end))
        } else if let Some(inner) = rest.strip_prefix('[') {
            // A link's text ends at the first `]`, with its address right
            // after: `[a, b]` earlier on the line is text, not the start of
            // a link that comes later.
            inner.find(']').filter(|&mid| mid > 0 && inner[mid + 1..].starts_with('(')).and_then(|mid| {
                let url_part = &inner[mid + 2..];
                url_part.find(')').map(|end| (Span::Link(&inner[..mid], &url_part[..end]), 1 + mid + 2 + end + 1))
            })
        } else {
            None
        };
        match found {
            Some((span, len)) => {
                push_text(&mut out, text_from, at);
                out.push(span);
                at += len;
                text_from = at;
            }
            None => at += rest.chars().next().map_or(1, char::len_utf8),
        }
        rest = &line[at..];
    }
    push_text(&mut out, text_from, line.len());
    out
}

/// One line's pieces, wrapping at the edge.
fn inline(ui: &mut Ui, theme: &Theme, line: &str, size: f32, bold: bool) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
        for span in spans(line) {
            match span {
                Span::Text(t) if bold => {
                    ui.label(RichText::new(t).font(crate::ui::fonts::bold(size)).color(theme.text));
                }
                Span::Text(t) => {
                    ui.label(RichText::new(t).size(size).color(theme.text));
                }
                Span::Bold(t) => {
                    ui.label(RichText::new(t).font(crate::ui::fonts::bold(size)).color(theme.text));
                }
                Span::Code(t) => {
                    ui.label(RichText::new(t).monospace().size(size - 1.0).color(theme.text).background_color(theme.field));
                }
                Span::Link(t, url) if url.starts_with("https://") || url.starts_with("http://") => {
                    ui.hyperlink_to(RichText::new(t).size(size).color(theme.accent), url).on_hover_text(url);
                }
                Span::Link(t, _) => {
                    ui.label(RichText::new(t).size(size).color(theme.text));
                }
            }
        }
    });
}

/// Shows `text` as Markdown.
pub fn show(ui: &mut Ui, theme: &Theme, text: &str, size: f32) {
    ui.spacing_mut().item_spacing.y = 3.0;
    let mut first = true;
    for raw in text.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        let indent = (line.len() - trimmed.len()) as f32 * 6.0;
        if trimmed.is_empty() {
            ui.add_space(4.0);
            continue;
        }
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&level) && trimmed[level..].starts_with(' ') {
            if !first {
                ui.add_space(6.0);
            }
            inline(ui, theme, trimmed[level..].trim(), size + (4 - level.min(3)) as f32 * 1.5, true);
        } else if let Some(item) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            bullet(ui, theme, indent, "•", item, size);
        } else if let Some((number, item)) = trimmed.split_once(". ").filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())) {
            bullet(ui, theme, indent, &format!("{number}."), item, size);
        } else {
            inline(ui, theme, trimmed, size, false);
        }
        first = false;
    }
}

/// A list item: its mark, then its text, which wraps under itself.
fn bullet(ui: &mut Ui, theme: &Theme, indent: f32, mark: &str, item: &str, size: f32) {
    ui.horizontal_top(|ui| {
        ui.add_space(indent + 4.0);
        ui.add_sized(egui::vec2(14.0, size + 4.0), egui::Label::new(RichText::new(mark).size(size).color(theme.text_muted)));
        ui.vertical(|ui| inline(ui, theme, item, size, false));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_a_line() {
        assert_eq!(
            spans("**Updates** install with `Update` — see [the page](https://x.y/z)."),
            vec![Span::Bold("Updates"), Span::Text(" install with "), Span::Code("Update"), Span::Text(" — see "), Span::Link("the page", "https://x.y/z"), Span::Text(".")]
        );
        assert_eq!(spans("a ** b and `c"), vec![Span::Text("a ** b and `c")], "unclosed marks stay text");
        assert_eq!(spans("Ä **ö**"), vec![Span::Text("Ä "), Span::Bold("ö")]);
        // Brackets that aren't a link stay text, also before a real link.
        assert_eq!(
            spans("An array [a, b] and `list[0]`: see [the docs](https://x.y/z)."),
            vec![Span::Text("An array [a, b] and "), Span::Code("list[0]"), Span::Text(": see "), Span::Link("the docs", "https://x.y/z"), Span::Text(".")]
        );
        assert_eq!(spans("[] and [x] (y)"), vec![Span::Text("[] and [x] (y)")]);
    }

    /// The real release notes draw without trouble, and no mark is left as text.
    #[test]
    fn shows_release_notes() {
        let theme = Theme::from_def(&circuit_plugins::ThemeDef::default());
        let notes = "### Fixed\n- **Bold** and `code`\n  - nested\n1. first\n\nPlain [link](https://example.com).\n";
        let ctx = egui::Context::default();
        crate::ui::fonts::install(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(ui, &theme, notes, 12.5));
        out.textures_delta.clear();
        let texts: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "Fixed"), "{texts:?}");
        assert!(!texts.iter().any(|t| t.contains("###") || t.contains("**") || t.contains('`')), "{texts:?}");
    }

    /// Long notes wrap inside their width (the update window's).
    #[test]
    fn wraps_inside_its_width() {
        let theme = Theme::from_def(&circuit_plugins::ThemeDef::default());
        let notes = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).unwrap();
        let ctx = egui::Context::default();
        crate::ui::fonts::install(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(480.0, 4000.0));
        let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| show(ui, &theme, &notes, 12.5));
        out.textures_delta.clear();
        let widest = out.shapes.iter().map(|s| s.shape.visual_bounding_rect().right()).fold(0.0, f32::max);
        assert!(widest <= 480.5, "drawn up to x = {widest}");
    }
}
