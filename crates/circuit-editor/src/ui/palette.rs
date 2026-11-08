//! The command palette (Ctrl+P): search menu commands, workspaces and blocks.

use eframe::egui::{self, Align2, Frame, Key, Margin, Order, RichText, ScrollArea, Stroke, TextEdit, vec2};

use crate::ui::theme::{Theme, with_alpha};

/// What a palette entry does. The app carries it out.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    NewProject,
    OpenProject,
    OpenExample,
    OpenRecent(std::path::PathBuf),
    CloseProject,
    Save,
    ReloadPlugins,
    Undo,
    Redo,
    Duplicate,
    Delete,
    SelectAll,
    TogglePanel(crate::app::Tab),
    /// Shows a side bar view, or folds the side bar away.
    Theme(String),
    ResetLayout,
    Run,
    Stop,
    SlashCommands,
    Settings,
    Export,
    /// Bot › Server…, uploading to it, and a Calagopus server's console.
    Hosting,
    Upload,
    ServerConsole,
    ImportWorkspaces,
    ExportWorkspaces,
    About,
    OpenWorkspace(String),
    RevealBlock(String, String),
    PluginCommand(String),
    TogglePluginPanel(String, String),
    ManagePlugins,
    OpenSettings,
    Tour,
    Backups,
}

pub struct Item {
    /// Group shown on the left, such as "Bot" or "Block".
    pub group: &'static str,
    pub label: String,
    /// Extra words shown dimmed and searched, such as the workspace a block is in.
    pub detail: String,
    pub shortcut: String,
    pub cmd: Cmd,
    /// Texts inside the item (a block's fields): searched after the label,
    /// as (field label, text).
    pub content: Vec<(String, String)>,
}

impl Item {
    pub fn new(group: &'static str, label: impl Into<String>, cmd: Cmd) -> Item {
        Item { group, label: label.into(), detail: String::new(), shortcut: String::new(), cmd, content: Vec::new() }
    }

    pub fn content(mut self, c: Vec<(String, String)>) -> Item {
        self.content = c;
        self
    }

    /// Workspaces and blocks are places in the project; the rest are commands.
    fn is_command(&self) -> bool {
        !matches!(self.cmd, Cmd::OpenWorkspace(_) | Cmd::RevealBlock(..))
    }

    pub fn detail(mut self, d: impl Into<String>) -> Item {
        self.detail = d.into();
        self
    }

    pub fn shortcut(mut self, s: impl Into<String>) -> Item {
        self.shortcut = s.into();
        self
    }
}

#[derive(Default)]
pub struct Palette {
    query: String,
    highlighted: usize,
    opened_frames: u32,
}

/// Like VS Code: `>` searches only commands, `#` only blocks and the texts in them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    All,
    Commands,
    Blocks,
}

/// The scope a query asks for, and the query without its prefix.
fn scope(query: &str) -> (Scope, &str) {
    let q = query.trim_start();
    if let Some(rest) = q.strip_prefix('>') {
        (Scope::Commands, rest)
    } else if let Some(rest) = q.strip_prefix('#') {
        (Scope::Blocks, rest)
    } else {
        (Scope::All, q)
    }
}

/// The text around the first of `words` found in the item's content, with its
/// field: `Text: …GG ${user}! Here's your rank…`.
fn snippet(item: &Item, words: &[String]) -> Option<String> {
    const BEFORE: usize = 18;
    const LENGTH: usize = 56;
    item.content.iter().find_map(|(field, text)| {
        let lower = text.to_lowercase();
        let at = words.iter().filter_map(|w| lower.find(w.as_str())).min()?;
        let chars: Vec<char> = text.chars().map(|c| if c.is_whitespace() { ' ' } else { c }).collect();
        let found = lower[..at].chars().count();
        let mut start = found.saturating_sub(BEFORE);
        // Begin at a word, not in the middle of one.
        if start > 0
            && let Some(space) = chars[start..found].iter().position(|c| *c == ' ')
        {
            start += space + 1;
        }
        let end = (start + LENGTH).min(chars.len());
        let mut s: String = chars[start..end].iter().collect();
        if start > 0 {
            s = format!("…{}", s.trim_start());
        }
        if end < chars.len() {
            s = format!("{}…", s.trim_end());
        }
        Some(format!("{field}: {s}"))
    })
}

/// Lower is better; `None` when the item doesn't match every word of the
/// query. Items found by the texts inside them come last, with where.
fn score(item: &Item, query: &str) -> Option<(u32, Option<String>)> {
    let (scope, query) = scope(query);
    match scope {
        Scope::Commands if !item.is_command() => return None,
        Scope::Blocks if !matches!(item.cmd, Cmd::RevealBlock(..)) => return None,
        _ => {}
    }
    let label = item.label.to_lowercase();
    let hay = format!("{} {} {}", item.group, label, item.detail).to_lowercase();
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if !words.iter().all(|w| hay.contains(w.as_str())) {
        // Each word in the name or in the block's texts.
        let inside: Vec<String> = item.content.iter().map(|(_, t)| t.to_lowercase()).collect();
        let found = |w: &String| hay.contains(w.as_str()) || inside.iter().any(|t| t.contains(w.as_str()));
        if item.content.is_empty() || !words.iter().all(found) {
            return None;
        }
        let missing: Vec<String> = words.into_iter().filter(|w| !hay.contains(w.as_str())).collect();
        return Some((4, snippet(item, &missing)));
    }
    let q = query.trim().to_lowercase();
    Some((if q.is_empty() {
        1
    } else if label.starts_with(&q) {
        0
    } else if label.split_whitespace().any(|w| w.starts_with(&q)) {
        1
    } else if label.contains(&q) {
        2
    } else {
        3
    }, None))
}

/// Draws the palette. Returns the chosen command, and whether it should close.
pub fn show(ctx: &egui::Context, theme: &Theme, state: &mut Palette, items: &[Item]) -> (Option<Cmd>, bool) {
    state.opened_frames += 1;
    let mut list: Vec<(u32, usize, Option<String>)> = items.iter().enumerate().filter_map(|(i, it)| score(it, &state.query).map(|(s, found)| (s, i, found))).collect();
    list.sort_by_key(|(s, i, _)| (*s, *i));
    if !list.is_empty() {
        state.highlighted = state.highlighted.min(list.len() - 1);
    }

    let (up, down, enter, escape) = ctx.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
    if up {
        state.highlighted = state.highlighted.saturating_sub(1);
    }
    if down && state.highlighted + 1 < list.len() {
        state.highlighted += 1;
    }
    let mut chosen = if enter { list.get(state.highlighted).map(|(_, i, _)| items[*i].cmd.clone()) } else { None };

    let width = 560.0;
    let screen = ctx.content_rect();
    // Opens over the title bar's search box.
    let pos = egui::pos2(screen.center().x - width / 2.0, screen.top() + 4.0);
    let area = egui::Area::new(egui::Id::unique("command-palette")).order(Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        Frame::new()
            .fill(theme.panel)
            .stroke(Stroke::new(1.0, theme.node_border))
            .corner_radius(10)
            .shadow(ctx.global_style().visuals.popup_shadow)
            .show(ui, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                Frame::new().inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
                    let edit = ui.add(
                        TextEdit::singleline(&mut state.query)
                            .hint_text(t!("Type a command, workspace, block or text in a block"))
                            .frame(Frame::new())
                            .desired_width(width - 28.0)
                            .font(egui::FontId::proportional(15.0)),
                    );
                    if state.opened_frames <= 2 {
                        edit.request_focus();
                    }
                    if edit.changed() {
                        state.highlighted = 0;
                    }
                });
                ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, theme.line));

                ScrollArea::vertical().id_salt("palette-list").max_height(340.0).auto_shrink([false, true]).show(ui, |ui| {
                    ui.add_space(4.0);
                    for (row, (_, i, found)) in list.iter().enumerate() {
                        let item = &items[*i];
                        let selected = row == state.highlighted;
                        let (rect, resp) = ui.allocate_exact_size(vec2(width, 30.0), egui::Sense::click());
                        crate::ui::kit::describe_selected(&resp, egui::Role::ListBoxOption, selected, &format!("{}: {}", item.group, item.label));
                        let rect = rect.shrink2(vec2(6.0, 1.0));
                        if selected {
                            ui.painter().rect_filled(rect, 6.0, with_alpha(theme.accent, 45));
                        } else if resp.hovered() {
                            ui.painter().rect_filled(rect, 6.0, theme.panel_header);
                        }
                        let p = ui.painter().with_clip_rect(rect.shrink2(vec2(4.0, 0.0)));
                        p.text(rect.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, item.group, egui::FontId::proportional(11.0), theme.text_muted);
                        let label = p.text(rect.left_center() + vec2(92.0, 0.0), Align2::LEFT_CENTER, &item.label, egui::FontId::proportional(13.5), theme.text);
                        let mut after = label.right_center() + vec2(10.0, 0.0);
                        if !item.detail.is_empty() {
                            after = p.text(after, Align2::LEFT_CENTER, &item.detail, egui::FontId::proportional(11.5), with_alpha(theme.text_muted, 200)).right_center() + vec2(10.0, 0.0);
                        }
                        // Where the query was found inside the block.
                        if let Some(found) = found {
                            p.text(after, Align2::LEFT_CENTER, found, egui::FontId::proportional(11.5), theme.accent);
                        }
                        if !item.shortcut.is_empty() {
                            p.text(rect.right_center() - vec2(10.0, 0.0), Align2::RIGHT_CENTER, &item.shortcut, egui::FontId::monospace(11.0), theme.text_muted);
                        }
                        if resp.hovered() && ui.ctx().input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                            state.highlighted = row;
                        }
                        if resp.clicked() {
                            chosen = Some(item.cmd.clone());
                        }
                        if selected && (up || down) {
                            resp.scroll_to_me(None);
                        }
                    }
                    if list.is_empty() {
                        ui.add_space(10.0);
                        ui.label(RichText::new(t!("     Nothing matches")).color(theme.text_muted));
                        ui.add_space(10.0);
                    }
                    ui.add_space(4.0);
                });
                ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, theme.line));
                Frame::new().inner_margin(Margin::symmetric(14, 6)).show(ui, |ui| {
                    ui.set_width(width - 28.0);
                    ui.label(RichText::new(t!("> only commands    # only blocks and the texts in them")).size(11.0).color(theme.text_muted));
                });
            });
    });

    let clicked_outside = state.opened_frames > 2
        && ctx.input(|i| i.pointer.any_pressed())
        && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area.response.rect.contains(p));
    let close = chosen.is_some() || escape || clicked_outside;
    (chosen, close)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_prefix_matches_first() {
        let items = [Item::new("Bot", "Export…", Cmd::Export), Item::new("Bot", "Run", Cmd::Run), Item::new("Block", "Rerun step", Cmd::Undo)];
        assert_eq!(score(&items[1], "ru"), Some((0, None)));
        assert_eq!(score(&items[2], "ru"), Some((2, None)));
        assert_eq!(score(&items[0], "ru"), None);
        // Every word must match somewhere, group included.
        assert_eq!(score(&items[1], "bot run"), Some((3, None)));
        assert!(score(&items[1], "bot stop").is_none());
    }

    #[test]
    fn finds_blocks_by_the_texts_in_them() {
        let send = Item::new("Block", "Send Message", Cmd::RevealBlock("rank".into(), "4".into()))
            .detail("Rank command · #4")
            .content(vec![("Text".into(), "GG ${user}! Here's your rank.\nKeep chatting to level up and unlock more.".into())]);
        let run = Item::new("Bot", "Run bot", Cmd::Run);
        // By name first; by text after, with where it was found.
        assert_eq!(score(&send, "send"), Some((0, None)));
        assert_eq!(score(&send, "chatting"), Some((4, Some("Text: …your rank. Keep chatting to level up and unlock more.".into()))));
        assert_eq!(score(&send, "send rank chatting").map(|s| s.0), Some(4), "words may be in the name or the text");
        assert!(score(&send, "nowhere").is_none());
        // > only commands, # only blocks.
        assert!(score(&send, ">send").is_none());
        assert_eq!(score(&run, "> run"), Some((0, None)));
        assert!(score(&run, "#run").is_none());
        assert_eq!(score(&send, "#gg").map(|s| s.0), Some(4));
        assert_eq!(score(&send, "#").map(|s| s.0), Some(1), "# alone lists every block");
    }
}
