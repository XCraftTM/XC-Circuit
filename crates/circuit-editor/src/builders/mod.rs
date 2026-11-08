//! The builders for Discord things: embeds, layouts, forms (modals), canvases and slash commands, with their previews (markdown, colors, JSON, locales).

pub mod canvas;
pub mod color;
pub mod embed;
pub mod json_view;
pub mod layout;
pub mod locales;
pub mod markdown;
pub mod modal;
pub mod slash;

/// How many characters of `text` are sure to be sent: those outside `${…}`
/// (what a `${…}` becomes is only known when the bot runs). Discord's limits
/// are checked against this, so a text is only called too long when it is.
pub fn fixed_len(text: &str) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let (mut n, mut i) = (0, 0);
    while i < chars.len() {
        if chars[i] == '$' && chars.get(i + 1) == Some(&'{') {
            // Skip to the matching }, counting nested { }.
            let mut depth = 0;
            let mut j = i + 1;
            while j < chars.len() {
                match chars[j] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            if j < chars.len() {
                i = j + 1;
                continue;
            }
        }
        n += 1;
        i += 1;
    }
    n
}

/// Whether `text` is sure to be over `max` characters (see [`fixed_len`]).
pub fn too_long(text: &str, max: usize) -> bool {
    fixed_len(text) > max
}

/// A web address Discord takes for a link or picture: http(s)://, or
/// attachment:// for a file sent along; `${…}` is only known when it runs.
pub fn web_address_ok(text: &str) -> bool {
    let t = text.trim();
    t.is_empty() || t.contains("${") || ["http://", "https://", "attachment://"].iter().any(|p| t.to_ascii_lowercase().starts_with(p))
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholders_dont_count_toward_limits() {
        assert_eq!(super::fixed_len("Hi ${user.username}!"), 4);
        assert_eq!(super::fixed_len("${ {a: 1}.a } x"), 2);
        assert_eq!(super::fixed_len("cost: $5 {ok}"), 13, "a $ or {{ alone is text");
        assert_eq!(super::fixed_len("open ${never"), 12, "an unclosed ${{ is text");
        assert!(super::too_long(&"x".repeat(257), 256) && !super::too_long(&format!("{}${{x}}", "x".repeat(256)), 256));
        assert!(super::web_address_ok("https://x.example") && super::web_address_ok("${avatar}") && super::web_address_ok("attachment://a.png") && super::web_address_ok(""));
        assert!(!super::web_address_ok("x.example/a.png"));
    }
}
