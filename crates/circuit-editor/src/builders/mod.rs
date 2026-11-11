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

/// The builders that edit one field of a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    Embed,
    Layout,
    Modal,
    Canvas,
}

/// What a builder is called, in the interface's language.
pub fn name(kind: Kind) -> &'static str {
    match kind {
        Kind::Embed => t!("Embed Builder"),
        Kind::Layout => t!("Layout Builder"),
        Kind::Modal => t!("Modal Builder"),
        Kind::Canvas => t!("Canvas Builder"),
    }
}

/// A number for a builder just opened: several can be open, each in a tab of its own.
pub fn next_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// An open builder's draft, as far as opening another one needs to know.
pub trait Draft {
    /// Its number (see [`next_id`]).
    fn id(&self) -> u64;
    /// The field it edits: workspace, block and field.
    fn field(&self) -> (&str, &str, &str);
}

/// Takes an open builder out by its number (its tab closed).
pub fn take<E: Draft>(open: &mut Vec<E>, id: u64) -> Option<E> {
    open.iter().position(|e| e.id() == id).map(|i| open.remove(i))
}

/// Opens a builder for a field, in a tab of its own beside those already
/// open. One open for that field already comes to the front as it is,
/// changes and all.
pub fn open<E: Draft>(open: &mut Vec<E>, actions: &mut Vec<crate::editor::Action>, kind: Kind, ws: &str, node: &str, key: &str, make: impl FnOnce() -> E) {
    match open.iter().find(|e| e.field() == (ws, node, key)) {
        Some(e) => actions.push(crate::editor::Action::ShowBuilder(kind, e.id())),
        None => open.push(make()),
    }
}

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

    /// Builders for different fields are open side by side, each with its own
    /// number; asking again for a field that has one shows that one, as it is.
    #[test]
    fn builders_open_side_by_side() {
        use super::layout::LayoutEdit;
        use super::{Draft, Kind, open};
        use crate::editor::Action;
        let value = serde_json::json!({ "components": [] });
        let (mut list, mut actions): (Vec<LayoutEdit>, Vec<Action>) = (Vec::new(), Vec::new());
        open(&mut list, &mut actions, Kind::Layout, "w", "a", "layout", || LayoutEdit::new("w", "a", "layout", &value));
        open(&mut list, &mut actions, Kind::Layout, "w", "b", "layout", || LayoutEdit::new("w", "b", "layout", &value));
        assert_eq!(list.iter().map(|e| e.field().1).collect::<Vec<_>>(), ["a", "b"]);
        assert_ne!(list[0].id(), list[1].id());
        assert!(actions.is_empty());

        list[0].draft["components"] = serde_json::json!([{ "type": 10, "content": "Hi" }]);
        open(&mut list, &mut actions, Kind::Layout, "w", "a", "layout", || LayoutEdit::new("w", "a", "layout", &value));
        assert_eq!(list.len(), 2, "not a second one for the same field");
        assert_eq!(list[0].draft["components"][0]["content"], "Hi", "kept as it was");
        assert!(matches!(actions.as_slice(), [Action::ShowBuilder(Kind::Layout, id)] if *id == list[0].id()));
    }
}
