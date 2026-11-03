//! Discord's markdown for message previews: **bold**, *italic*, __underline__,
//! ~~strikethrough~~, `code`, ||spoilers||, [links](https://…), headings,
//! -# small text, > quotes, lists and code blocks, mentions, custom emoji and
//! <t:…> timestamps. Each `${…}` is a chip (see `chips`), since its value is
//! only known when the bot runs.
//!
//! The built-in fonts have no bold weight, so bold text is drawn brighter.

use std::ops::Range;
use std::sync::Arc;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Color32, CornerRadius, FontId, Galley, Painter, Pos2, Rect, Stroke, Vec2, pos2, vec2};

/// Discord's colors for the parts.
pub mod colors {
    use eframe::egui::Color32;
    pub const TEXT: Color32 = Color32::from_rgb(0xDB, 0xDE, 0xE1);
    pub const STRONG: Color32 = Color32::from_rgb(0xF2, 0xF3, 0xF5);
    pub const MUTED: Color32 = Color32::from_rgb(0x94, 0x9B, 0xA4);
    pub const LINK: Color32 = Color32::from_rgb(0x00, 0xA8, 0xFC);
    pub const CODE_BG: Color32 = Color32::from_rgb(0x1E, 0x1F, 0x22);
    pub const QUOTE_BAR: Color32 = Color32::from_rgb(0x4E, 0x50, 0x58);
    /// As a revealed spoiler: a light band over the text.
    pub const SPOILER_BG: Color32 = Color32::from_rgba_premultiplied(30, 30, 30, 30);
    pub const MENTION_BG: Color32 = Color32::from_rgba_premultiplied(26, 30, 73, 77);
    pub const MENTION: Color32 = Color32::from_rgb(0xC9, 0xCD, 0xFB);
}
use colors::*;

/// What may be in the text: everything (message text, embed descriptions and
/// field values), or only what works inside one line (embed titles, field names).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Full,
    Inline,
}

/// Laid-out markdown: the text, plus the quote bars and code block boxes
/// that go behind it.
pub struct Marked {
    pub galley: Arc<Galley>,
    quotes: Vec<Range<usize>>,
    code: Vec<Range<usize>>,
    width: f32,
}

impl Marked {
    /// Text without markdown (its chips drawn already), to paint the same way.
    pub fn plain(galley: Arc<Galley>) -> Marked {
        let width = galley.size().x;
        Marked { galley, quotes: Vec::new(), code: Vec::new(), width }
    }

    pub fn size(&self) -> Vec2 {
        self.galley.size()
    }

    /// Draws it with its top-left corner at `at`.
    pub fn paint(&self, painter: &Painter, at: Pos2) {
        // Which characters each row holds, to find the rows of quotes and code blocks.
        let mut rows: Vec<(Range<usize>, Rect)> = Vec::new();
        let mut index = 0;
        for row in &self.galley.rows {
            let len = row.row.glyphs.len();
            rows.push((index..index + len.max(1), row.rect().translate(at.to_vec2())));
            index += len + usize::from(row.ends_with_newline);
        }
        let covered = |ranges: &[Range<usize>], r: &Range<usize>| ranges.iter().any(|q| r.start < q.end && q.start < r.end);
        // Code blocks: one box around their rows.
        let mut boxes: Vec<Rect> = Vec::new();
        for block in &self.code {
            let rects: Vec<Rect> = rows.iter().filter(|(r, _)| covered(std::slice::from_ref(block), r)).map(|(_, rect)| *rect).collect();
            if let (Some(first), Some(last)) = (rects.first(), rects.last()) {
                // Its first and last rows are thin empty ones: the room inside the box.
                boxes.push(Rect::from_min_max(pos2(at.x, first.top()), pos2(at.x + self.width.max(first.width()), last.bottom())));
            }
        }
        for b in boxes {
            painter.rect(b, CornerRadius::same(4), CODE_BG, Stroke::new(1.0, Color32::from_rgb(0x2B, 0x2D, 0x31)), eframe::egui::StrokeKind::Inside);
        }
        painter.galley(at, self.galley.clone(), TEXT);
        for (r, rect) in &rows {
            if covered(&self.quotes, r) {
                painter.rect_filled(Rect::from_min_size(pos2(at.x, rect.top()), vec2(4.0, rect.height())), 2.0, QUOTE_BAR);
            }
        }
    }
}

/// Lays out `text` at `size` in `color`, wrapped at `wrap`.
pub fn layout(painter: &Painter, text: &str, kind: Kind, size: f32, color: Color32, wrap: f32) -> Marked {
    let built = build(text, kind, size, color, wrap);
    let galley = painter.layout_job(built.job);
    Marked { galley, quotes: built.quotes, code: built.code, width: wrap }
}

/// The job and where its quotes and code blocks are (char ranges of the job's text).
pub struct Built {
    pub job: LayoutJob,
    pub quotes: Vec<Range<usize>>,
    pub code: Vec<Range<usize>>,
}

/// Text as Discord spaces it: lines 1.375 times the font size, text in the middle.
fn text_format(font: FontId, color: Color32) -> TextFormat {
    let line = font.size * 1.375;
    TextFormat { line_height: Some(line), valign: eframe::egui::Align::Center, ..TextFormat::simple(font, color) }
}

/// A marker and the style it switches; longer markers first (** before *).
type Toggle = (&'static str, fn(&mut Style) -> &mut bool);
const TOGGLES: [Toggle; 5] = [
    ("||", |s| &mut s.spoiler),
    ("**", |s| &mut s.bold),
    ("__", |s| &mut s.underline),
    ("~~", |s| &mut s.strike),
    ("*", |s| &mut s.italic),
];

#[derive(Clone, Copy, Default)]
struct Style {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    spoiler: bool,
}

/// How one line looks as a whole.
#[derive(Clone, Copy)]
struct Line {
    size: f32,
    color: Color32,
    strong: bool,
    indent: f32,
    quote: bool,
}

struct Out {
    job: LayoutJob,
    chars: usize,
    quotes: Vec<Range<usize>>,
    code: Vec<Range<usize>>,
}

impl Out {
    fn push(&mut self, text: &str, leading: f32, format: TextFormat) {
        if text.is_empty() {
            return;
        }
        self.job.append(text, leading, format);
        self.chars += text.chars().count();
    }
}

pub fn build(text: &str, kind: Kind, size: f32, color: Color32, wrap: f32) -> Built {
    let mut out = Out { job: LayoutJob::default(), chars: 0, quotes: Vec::new(), code: Vec::new() };
    out.job.wrap.max_width = wrap;
    let chars: Vec<char> = text.chars().collect();
    // ${…} spans (from just after "${" to the "}"), never read as markdown.
    let expressions: Vec<(usize, usize)> = circuit_core::template::expressions(text);
    let base = Line { size, color, strong: false, indent: 0.0, quote: false };
    let mut style = Style::default();
    let mut quote_rest = false;
    let mut i = 0;
    let mut first_line = true;
    while i <= chars.len() {
        // One line: from i to the next newline outside a ${…}.
        let end = line_end(&chars, i, &expressions);
        let raw: String = chars[i..end].iter().collect();
        if !first_line {
            out.push("\n", 0.0, text_format(FontId::proportional(size), color));
        }
        first_line = false;

        // A code block: ``` up to the closing ```.
        if kind == Kind::Full && raw.trim_start().starts_with("```") {
            let (body, next) = code_block(&chars, i);
            let start = out.chars;
            let mono = text_format(FontId::monospace(size * 0.92), TEXT);
            // A thin empty row above and below the code, inside its box, so the
            // code sits in the middle of it.
            let pad = TextFormat { line_height: Some(size * 0.5), ..text_format(FontId::monospace(size * 0.3), TEXT) };
            out.push(" ", 0.0, pad.clone());
            out.push("\n", 0.0, pad.clone());
            let body = if body.is_empty() { " ".to_string() } else { body };
            for (n, l) in body.split('\n').enumerate() {
                if n > 0 {
                    out.push("\n", 0.0, mono.clone());
                }
                out.push(if l.is_empty() { " " } else { l }, 6.0, mono.clone());
            }
            out.push("\n", 0.0, pad.clone());
            out.push(" ", 0.0, pad);
            out.code.push(start..out.chars);
            style = Style::default();
            i = next;
            if i >= chars.len() {
                break;
            }
            continue;
        }

        let mut line = base;
        let mut skip = 0;
        if kind == Kind::Full {
            if quote_rest || raw.starts_with(">>> ") {
                if !quote_rest {
                    skip = 4;
                }
                quote_rest = true;
                line.quote = true;
            } else if raw.starts_with("> ") || raw == ">" {
                skip = raw.len().min(2);
                line.quote = true;
            }
            let rest = &raw[skip..];
            let heading = [("### ", 1.12), ("## ", 1.3), ("# ", 1.55)].into_iter().find(|(p, _)| rest.starts_with(p));
            if let Some((p, scale)) = heading {
                skip += p.len();
                line.size = size * scale;
                line.strong = true;
                line.color = STRONG;
            } else if let Some(p) = ["-# "].into_iter().find(|p| rest.starts_with(p)) {
                skip += p.len();
                line.size = size * 0.85;
                line.color = MUTED;
            } else {
                let spaces = rest.len() - rest.trim_start().len();
                let trimmed = rest.trim_start();
                let bullet = ["- ", "* "].into_iter().find(|p| trimmed.starts_with(p));
                let numbered = trimmed.split_once(". ").filter(|(n, _)| !n.is_empty() && n.len() <= 3 && n.chars().all(|c| c.is_ascii_digit()));
                if bullet.is_some() || numbered.is_some() {
                    let depth = (spaces / 2) as f32;
                    line.indent = 6.0 + depth * 14.0;
                    let marker = match numbered {
                        Some((n, _)) => format!("{n}. "),
                        None => "•  ".to_string(),
                    };
                    skip += spaces + bullet.map(|p| p.len()).unwrap_or_else(|| numbered.map(|(n, _)| n.len() + 2).unwrap_or(0));
                    out.push(&marker, line.indent + if line.quote { 12.0 } else { 0.0 }, text_format(FontId::proportional(line.size), MUTED));
                    line.indent = 0.0;
                }
            }
        }
        let start = out.chars;
        let leading = line.indent + if line.quote && out.chars == start { 12.0 } else { 0.0 };
        // Byte offset to char offset for the skipped prefix.
        let skip_chars = raw[..skip].chars().count();
        inline(&mut out, &chars, i + skip_chars, end, &expressions, &mut style, line, leading);
        if out.chars == start {
            // An empty line still needs a glyph, for its height and a quote bar.
            out.push(" ", leading, text_format(FontId::proportional(line.size), line.color));
        }
        if line.quote {
            out.quotes.push(start..out.chars);
        }
        if end >= chars.len() {
            break;
        }
        i = end + 1;
    }
    Built { job: out.job, quotes: out.quotes, code: out.code }
}

/// Where the line starting at `from` ends: at a newline outside a `${…}`.
fn line_end(chars: &[char], from: usize, expressions: &[(usize, usize)]) -> usize {
    let mut i = from;
    while i < chars.len() {
        if let Some(&(_, e)) = expressions.iter().find(|(s, _)| *s == i + 2 && chars.get(i) == Some(&'$')) {
            i = e + 1;
            continue;
        }
        if chars[i] == '\n' {
            return i;
        }
        i += 1;
    }
    chars.len()
}

/// A code block starting at `from` (its ```): the code, and where the text goes on.
fn code_block(chars: &[char], from: usize) -> (String, usize) {
    let text: String = chars[from..].iter().collect();
    let lead = text.len() - text.trim_start().len();
    let after = &text[lead + 3..];
    match after.find("```") {
        Some(close) => {
            let mut body = &after[..close];
            // A language name on the first line is not shown.
            if let Some((first, rest)) = body.split_once('\n')
                && !first.trim().contains(' ')
            {
                body = rest;
            }
            let body = body.strip_suffix('\n').unwrap_or(body).to_string();
            let used = text[..lead + 3 + close + 3].chars().count();
            // Go on after the line the block closes on.
            let mut next = from + used;
            while next < chars.len() && chars[next] != '\n' {
                next += 1;
            }
            (body, next + 1)
        }
        None => (after.to_string(), chars.len()),
    }
}

fn format(line: Line, style: Style) -> TextFormat {
    let color = if style.bold || line.strong { STRONG } else { line.color };
    let mut f = text_format(FontId::proportional(line.size), color);
    f.italics = style.italic;
    if style.underline {
        f.underline = Stroke::new(1.0, color);
    }
    if style.strike {
        f.strikethrough = Stroke::new(1.0, color);
    }
    if style.spoiler {
        f.background = SPOILER_BG;
    }
    f
}

/// Whether `marker` appears again from `from` on (before `end`), outside code spans.
fn closes(chars: &[char], from: usize, end: usize, marker: &str) -> bool {
    let m: Vec<char> = marker.chars().collect();
    let mut i = from;
    while i + m.len() <= end {
        if chars[i] == '`' {
            match chars[i + 1..end].iter().position(|c| *c == '`') {
                Some(p) => i += p + 2,
                None => return false,
            }
            continue;
        }
        if chars[i..i + m.len()] == m[..] {
            return true;
        }
        i += 1;
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn inline(out: &mut Out, chars: &[char], from: usize, end: usize, expressions: &[(usize, usize)], style: &mut Style, line: Line, mut leading: f32) {
    let mut plain = String::new();
    let flush = |out: &mut Out, plain: &mut String, style: Style, leading: &mut f32| {
        if !plain.is_empty() {
            out.push(plain, *leading, format(line, style));
            plain.clear();
            *leading = 0.0;
        }
    };
    let rest_end = chars.len();
    let at = |i: usize, s: &str| s.chars().enumerate().all(|(k, c)| chars.get(i + k) == Some(&c));
    let mut i = from;
    while i < end {
        let c = chars[i];
        // ${…}: a chip.
        if c == '$'
            && let Some(&(s, e)) = expressions.iter().find(|(s, _)| *s == i + 2)
        {
            flush(out, &mut plain, *style, &mut leading);
            let code: String = chars[s..e].iter().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
            let mut f = format(line, *style);
            f.color = MENTION;
            f.background = MENTION_BG;
            out.push(&format!(" {code} "), leading, f);
            leading = 0.0;
            i = e + 1;
            continue;
        }
        // \* and the like: the character itself.
        if c == '\\' && i + 1 < end && "*_~`|\\<>#-[]()".contains(chars[i + 1]) {
            plain.push(chars[i + 1]);
            i += 2;
            continue;
        }
        // `code`
        if c == '`'
            && let Some(p) = chars[i + 1..end].iter().position(|ch| *ch == '`')
            && p > 0
        {
            flush(out, &mut plain, *style, &mut leading);
            let code: String = chars[i + 1..i + 1 + p].iter().collect();
            let mut f = text_format(FontId::monospace(line.size * 0.9), TEXT);
            f.line_height = Some(line.size * 1.375);
            f.background = CODE_BG;
            out.push(&code, leading, f);
            leading = 0.0;
            i += p + 2;
            continue;
        }
        // Toggles: on only when they close later, off when on.
        let mut toggled = false;
        for (marker, flag) in TOGGLES {
            if at(i, marker) {
                let on = *flag(style);
                if on || closes(chars, i + marker.len(), rest_end, marker) {
                    flush(out, &mut plain, *style, &mut leading);
                    *flag(style) = !on;
                    i += marker.len();
                    toggled = true;
                }
                break;
            }
        }
        if toggled {
            continue;
        }
        // _italic_: only at word edges, so snake_case stays as it is.
        if c == '_' {
            let before_ok = i == 0 || !chars[i - 1].is_alphanumeric();
            let after_ok = i + 1 >= chars.len() || !chars[i + 1].is_alphanumeric();
            let opening = before_ok && !style.italic && chars[i + 1..end].iter().enumerate().any(|(k, ch)| *ch == '_' && chars.get(i + 2 + k).is_none_or(|n| !n.is_alphanumeric()));
            if opening || (style.italic && after_ok) {
                flush(out, &mut plain, *style, &mut leading);
                style.italic = !style.italic;
                i += 1;
                continue;
            }
        }
        // [text](https://…): the text, as a link.
        if c == '['
            && let Some((label, used)) = masked_link(&chars[i..end])
        {
            flush(out, &mut plain, *style, &mut leading);
            let mut f = format(line, *style);
            f.color = LINK;
            out.push(&label, leading, f);
            leading = 0.0;
            i += used;
            continue;
        }
        // <@id>, <#id>, <:emoji:id>, <t:…>, <https://…>
        if c == '<'
            && let Some(close) = chars[i..end].iter().position(|ch| *ch == '>')
        {
            let inner: String = chars[i + 1..i + close].iter().collect();
            if let Some((text, chip)) = angle(&inner) {
                flush(out, &mut plain, *style, &mut leading);
                let mut f = format(line, *style);
                if chip {
                    f.color = MENTION;
                    f.background = MENTION_BG;
                } else {
                    f.color = LINK;
                }
                out.push(&text, leading, f);
                leading = 0.0;
                i += close + 1;
                continue;
            }
        }
        // Bare links.
        if (at(i, "https://") || at(i, "http://")) && (i == 0 || chars[i - 1].is_whitespace() || chars[i - 1] == '(') {
            flush(out, &mut plain, *style, &mut leading);
            let len = chars[i..end].iter().position(|ch| ch.is_whitespace()).unwrap_or(end - i);
            let url: String = chars[i..i + len].iter().collect();
            let mut f = format(line, *style);
            f.color = LINK;
            out.push(&url, leading, f);
            leading = 0.0;
            i += len;
            continue;
        }
        plain.push(c);
        i += 1;
    }
    flush(out, &mut plain, *style, &mut leading);
}

/// `[text](http…)` at the start of `chars`: the text and how many chars it took.
fn masked_link(chars: &[char]) -> Option<(String, usize)> {
    let close = chars.iter().position(|c| *c == ']')?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let end = chars[close + 2..].iter().position(|c| *c == ')')? + close + 2;
    let url: String = chars[close + 2..end].iter().collect();
    let label: String = chars[1..close].iter().collect();
    (url.starts_with("http://") || url.starts_with("https://") && !label.is_empty()).then_some((label, end + 1))
}

/// What `<inner>` shows as, and whether it is a chip (mentions, timestamps)
/// rather than a link. None when it is just text.
fn angle(inner: &str) -> Option<(String, bool)> {
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    if let Some(id) = inner.strip_prefix("@&") {
        return digits(id).then(|| (format!("@role {id}"), true));
    }
    if let Some(id) = inner.strip_prefix("@!").or_else(|| inner.strip_prefix('@')) {
        return digits(id).then(|| (format!("@{id}"), true));
    }
    if let Some(id) = inner.strip_prefix('#') {
        return digits(id).then(|| (format!("#{id}"), true));
    }
    if let Some(rest) = inner.strip_prefix("t:") {
        let (secs, style) = rest.split_once(':').unwrap_or((rest, "f"));
        let secs: i64 = secs.parse().ok()?;
        return Some((timestamp(secs, style)?, true));
    }
    if let Some(rest) = inner.strip_prefix(':').or_else(|| inner.strip_prefix("a:")) {
        let (name, id) = rest.split_once(':')?;
        return digits(id).then(|| (format!(":{name}:"), false)).map(|(t, _)| (t, true));
    }
    if inner.starts_with("https://") || inner.starts_with("http://") {
        return Some((inner.to_string(), false));
    }
    None
}

/// A `<t:…>` timestamp in one of Discord's styles, in this computer's time zone.
fn timestamp(secs: i64, style: &str) -> Option<String> {
    use chrono::{Local, TimeZone};
    let at = Local.timestamp_opt(secs, 0).single()?;
    Some(match style {
        "t" => at.format("%H:%M").to_string(),
        "T" => at.format("%H:%M:%S").to_string(),
        "d" => at.format("%d/%m/%Y").to_string(),
        "D" => at.format("%-d %B %Y").to_string(),
        "F" => at.format("%A, %-d %B %Y %H:%M").to_string(),
        "R" => relative(secs - Local::now().timestamp()),
        _ => at.format("%-d %B %Y %H:%M").to_string(),
    })
}

/// "in 3 hours", "2 days ago".
fn relative(delta: i64) -> String {
    let n = delta.unsigned_abs();
    let (count, unit) = [(31_536_000, "year"), (2_592_000, "month"), (86_400, "day"), (3_600, "hour"), (60, "minute"), (1, "second")]
        .into_iter()
        .find(|(s, _)| n >= *s)
        .map(|(s, u)| (n / s, u))
        .unwrap_or((0, "second"));
    let unit = if count == 1 { unit.to_string() } else { format!("{unit}s") };
    if delta >= 0 { format!("in {count} {unit}") } else { format!("{count} {unit} ago") }
}

/// The labels inside a laid-out text, with whether they are bold, italic,
/// underlined, struck, spoilers, code or chips: for tests.
#[cfg(test)]
fn parts(b: &Built) -> Vec<(String, &'static str)> {
    b.job
        .sections
        .iter()
        .map(|s| {
            let (start, end): (usize, usize) = (s.byte_range.start.into(), s.byte_range.end.into());
            let f = &s.format;
            let kind = if f.background == MENTION_BG {
                "chip"
            } else if f.background == CODE_BG && f.font_id.family == eframe::egui::FontFamily::Monospace {
                "code"
            } else if f.font_id.family == eframe::egui::FontFamily::Monospace {
                "block"
            } else if f.background == SPOILER_BG {
                "spoiler"
            } else if f.color == LINK {
                "link"
            } else if f.italics {
                "italic"
            } else if f.underline.width > 0.0 {
                "underline"
            } else if f.strikethrough.width > 0.0 {
                "strike"
            } else if f.color == STRONG {
                "bold"
            } else if f.color == MUTED {
                "muted"
            } else {
                "plain"
            };
            (b.job.text[start..end].to_string(), kind)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(text: &str) -> Vec<(String, &'static str)> {
        parts(&build(text, Kind::Full, 14.0, TEXT, 400.0))
    }
    fn p(text: &str, kind: &'static str) -> (String, &'static str) {
        (text.to_string(), kind)
    }

    #[test]
    fn inline_styles() {
        assert_eq!(md("a **b** *c* __d__ ~~e~~ `f` ||g||"), [
            p("a ", "plain"), p("b", "bold"), p(" ", "plain"), p("c", "italic"), p(" ", "plain"), p("d", "underline"),
            p(" ", "plain"), p("e", "strike"), p(" ", "plain"), p("f", "code"), p(" ", "plain"), p("g", "spoiler"),
        ]);
        // Unclosed markers and snake_case stay as they are; \ escapes.
        assert_eq!(md("2 * 3 and snake_case_name"), [p("2 * 3 and snake_case_name", "plain")]);
        assert_eq!(md("\\*not\\* _yes_"), [p("*not* ", "plain"), p("yes", "italic")]);
        // Nothing inside code.
        assert_eq!(md("`**x**`"), [p("**x**", "code")]);
    }

    #[test]
    fn links_mentions_and_chips() {
        assert_eq!(md("[site](https://x.y) <https://a.b> https://c.d"), [p("site", "link"), p(" ", "plain"), p("https://a.b", "link"), p(" ", "plain"), p("https://c.d", "link")]);
        assert_eq!(md("<@12> <@&34> <#56> <:wave:78>"), [p("@12", "chip"), p(" ", "plain"), p("@role 34", "chip"), p(" ", "plain"), p("#56", "chip"), p(" ", "plain"), p(":wave:", "chip")]);
        assert_eq!(md("Hi **${user}**!"), [p("Hi ", "plain"), p(" user ", "chip"), p("!", "plain")]);
        assert_eq!(md("${a ** b}"), [p(" a ** b ", "chip")], "no markdown inside ${{…}}");
        assert_eq!(md("<not a tag>"), [p("<not a tag>", "plain")]);
        assert!(md("<t:0:R>")[0].0.ends_with("years ago"));
    }

    #[test]
    fn lines_and_blocks() {
        let b = build("# Big\n-# small\n> quoted\n- one\n  - two\n1. first\nplain", Kind::Full, 14.0, TEXT, 400.0);
        let found = parts(&b);
        assert_eq!(found[0], ("Big".to_string(), "bold"));
        assert!(b.job.sections[0].format.font_id.size > 20.0, "headings are bigger");
        assert!(found.contains(&("small".to_string(), "muted")));
        assert!(found.contains(&("•  ".to_string(), "muted")) && found.contains(&("1. ".to_string(), "muted")));
        assert_eq!(b.quotes.len(), 1);
        let quoted: String = b.job.text.chars().skip(b.quotes[0].start).take(b.quotes[0].len()).collect();
        assert_eq!(quoted, "quoted");

        let b = build("before\n```rust\nlet x = 1;\nlet y = 2;\n```\nafter", Kind::Full, 14.0, TEXT, 400.0);
        // The code, between a thin empty row above and below it (room inside its box).
        assert_eq!(b.job.text, "before\n \nlet x = 1;\nlet y = 2;\n \nafter");
        let code: String = b.job.text.chars().skip(b.code[0].start).take(b.code[0].len()).collect();
        assert_eq!(code, " \nlet x = 1;\nlet y = 2;\n ");

        // Titles: only what fits in a line.
        let b = build("# not a heading **but bold**", Kind::Inline, 14.0, TEXT, 400.0);
        assert_eq!(parts(&b), [p("# not a heading ", "plain"), p("but bold", "bold")]);
    }
}
