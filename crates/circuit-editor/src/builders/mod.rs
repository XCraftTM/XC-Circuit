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

/// The builders that edit one field of a block, for showing their tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Embed,
    Layout,
    Modal,
    Canvas,
}

/// A builder's draft, as far as opening another builder of its kind needs to know.
pub trait Draft {
    /// The field it edits: workspace, block and field.
    fn field(&self) -> (&str, &str, &str);
    /// Whether it holds changes that aren't applied to the block yet.
    fn changed(&self) -> bool;
}

/// Opens a builder for a field. Not when one of its kind is open for that
/// field already (it comes to the front as it is), nor when it's open for
/// another field with changes not applied yet, which this would throw away:
/// that one comes to the front, and the Console says why.
pub fn open<E: Draft>(slot: &mut Option<E>, actions: &mut Vec<crate::editor::Action>, kind: Kind, ws: &str, node: &str, key: &str, make: impl FnOnce() -> E) {
    match slot.as_ref() {
        Some(open) if open.field() == (ws, node, key) => actions.push(crate::editor::Action::ShowBuilder(kind, false)),
        Some(open) if open.changed() => actions.push(crate::editor::Action::ShowBuilder(kind, true)),
        _ => *slot = Some(make()),
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

    /// Opening a builder never throws away changes not applied yet: one open
    /// for the same field comes to the front as it is, and one for another
    /// field with changes stays (the app shows it and says why).
    #[test]
    fn opening_a_builder_keeps_changes_not_applied() {
        use super::layout::LayoutEdit;
        use super::{Draft, Kind, open};
        use crate::editor::Action;
        let value = serde_json::json!({ "components": [] });
        let (mut slot, mut actions): (Option<LayoutEdit>, Vec<Action>) = (None, Vec::new());
        open(&mut slot, &mut actions, Kind::Layout, "w", "a", "layout", || LayoutEdit::new("w", "a", "layout", &value));
        assert!(slot.as_ref().is_some_and(|e| e.field() == ("w", "a", "layout") && !e.changed()) && actions.is_empty());

        // Unchanged: another block's builder takes its place.
        open(&mut slot, &mut actions, Kind::Layout, "w", "b", "layout", || LayoutEdit::new("w", "b", "layout", &value));
        assert_eq!(slot.as_ref().unwrap().field(), ("w", "b", "layout"));

        // Changed: it stays, and is shown with a word why.
        slot.as_mut().unwrap().draft["components"] = serde_json::json!([{ "type": 10, "content": "Hi" }]);
        assert!(slot.as_ref().unwrap().changed());
        open(&mut slot, &mut actions, Kind::Layout, "w", "a", "layout", || LayoutEdit::new("w", "a", "layout", &value));
        assert_eq!(slot.as_ref().unwrap().field(), ("w", "b", "layout"));
        assert!(matches!(actions.as_slice(), [Action::ShowBuilder(Kind::Layout, true)]));

        // The same field again: shown as it is, changes and all.
        actions.clear();
        open(&mut slot, &mut actions, Kind::Layout, "w", "b", "layout", || LayoutEdit::new("w", "b", "layout", &value));
        assert!(slot.as_ref().unwrap().changed());
        assert!(matches!(actions.as_slice(), [Action::ShowBuilder(Kind::Layout, false)]));
    }
}
