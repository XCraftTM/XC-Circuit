//! The command palette (Ctrl+P): search menu commands, workspaces and blocks.

use eframe::egui::{self, Align2, Frame, Key, Margin, Order, RichText, ScrollArea, Stroke, TextEdit, vec2};

use crate::theme::{Theme, with_alpha};

/// What a palette entry does. The app carries it out.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    NewProject,
    OpenProject,
    OpenExample,
    Save,
    ReloadPlugins,
    Undo,
    Redo,
    Duplicate,
    Delete,
    SelectAll,
    TogglePanel(crate::app::Tab),
    /// Shows a side bar view, or folds the side bar away.
    SideView(Option<crate::prefs::SideView>),
    Theme(String),
    ResetLayout,
    Run,
    Stop,
    SlashCommands,
    Settings,
    Export,
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
    pub shortcut: &'static str,
    pub cmd: Cmd,
}

impl Item {
    pub fn new(group: &'static str, label: impl Into<String>, cmd: Cmd) -> Item {
        Item { group, label: label.into(), detail: String::new(), shortcut: "", cmd }
    }

    pub fn detail(mut self, d: impl Into<String>) -> Item {
        self.detail = d.into();
        self
    }

    pub fn shortcut(mut self, s: &'static str) -> Item {
        self.shortcut = s;
        self
    }
}

#[derive(Default)]
pub struct Palette {
    query: String,
    highlighted: usize,
    opened_frames: u32,
}

/// Lower is better; `None` when the item doesn't match every word of the query.
fn score(item: &Item, query: &str) -> Option<u32> {
    let label = item.label.to_lowercase();
    let hay = format!("{} {} {}", item.group, label, item.detail).to_lowercase();
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if !words.iter().all(|w| hay.contains(w.as_str())) {
        return None;
    }
    let q = query.trim().to_lowercase();
    Some(if q.is_empty() {
        1
    } else if label.starts_with(&q) {
        0
    } else if label.split_whitespace().any(|w| w.starts_with(&q)) {
        1
    } else if label.contains(&q) {
        2
    } else {
        3
    })
}

/// Draws the palette. Returns the chosen command, and whether it should close.
pub fn show(ctx: &egui::Context, theme: &Theme, state: &mut Palette, items: &[Item]) -> (Option<Cmd>, bool) {
    state.opened_frames += 1;
    let mut list: Vec<(u32, usize)> = items.iter().enumerate().filter_map(|(i, it)| score(it, &state.query).map(|s| (s, i))).collect();
    list.sort_by_key(|(s, i)| (*s, *i));
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
    let mut chosen = if enter { list.get(state.highlighted).map(|(_, i)| items[*i].cmd.clone()) } else { None };

    let width = 560.0;
    let screen = ctx.content_rect();
    // Opens over the title bar's search box.
    let pos = egui::pos2(screen.center().x - width / 2.0, screen.top() + 4.0);
    let area = egui::Area::new(egui::Id::new("command-palette")).order(Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
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
                            .hint_text(t!("Type a command, workspace or block"))
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
                    for (row, (_, i)) in list.iter().enumerate() {
                        let item = &items[*i];
                        let selected = row == state.highlighted;
                        let (rect, resp) = ui.allocate_exact_size(vec2(width, 30.0), egui::Sense::click());
                        let rect = rect.shrink2(vec2(6.0, 1.0));
                        if selected {
                            ui.painter().rect_filled(rect, 6.0, with_alpha(theme.accent, 45));
                        } else if resp.hovered() {
                            ui.painter().rect_filled(rect, 6.0, theme.panel_header);
                        }
                        let p = ui.painter();
                        p.text(rect.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, item.group, egui::FontId::proportional(11.0), theme.text_muted);
                        let label = p.text(rect.left_center() + vec2(92.0, 0.0), Align2::LEFT_CENTER, &item.label, egui::FontId::proportional(13.5), theme.text);
                        if !item.detail.is_empty() {
                            p.text(label.right_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, &item.detail, egui::FontId::proportional(11.5), with_alpha(theme.text_muted, 200));
                        }
                        if !item.shortcut.is_empty() {
                            p.text(rect.right_center() - vec2(10.0, 0.0), Align2::RIGHT_CENTER, item.shortcut, egui::FontId::monospace(11.0), theme.text_muted);
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
        assert_eq!(score(&items[1], "ru"), Some(0));
        assert_eq!(score(&items[2], "ru"), Some(2));
        assert_eq!(score(&items[0], "ru"), None);
        // Every word must match somewhere, group included.
        assert_eq!(score(&items[1], "bot run"), Some(3));
        assert!(score(&items[1], "bot stop").is_none());
    }
}
