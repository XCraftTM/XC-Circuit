//! The Search view of the side bar: type a few words, and it lists the
//! workspaces and blocks that have them (in a block's name, its kind or
//! what is typed into its fields), block by block under their workspace.
//! Clicking one opens its workspace and shows the block.

use std::sync::Arc;

use eframe::egui::{self, Align2, FontId, Id, Ui};

use crate::editor::Editor;
use crate::ui::side;

/// A block that was found: its number, its name, and the text that has the
/// words (empty when its name has them).
struct Hit {
    id: String,
    name: String,
    text: String,
}

/// A workspace with what was found in it; `named` when its own name has the words.
struct Group {
    key: String,
    name: String,
    named: bool,
    hits: Vec<Hit>,
}

/// What a search found, kept until the words or the project change.
struct Found {
    for_words: String,
    for_revision: u64,
    groups: Vec<Group>,
    /// More was found than is listed.
    cut: bool,
}

/// The most blocks listed: a word like "a" would list them all.
const MOST: usize = 300;

/// Whether `text` has every one of `words` (which are lower case).
fn has(text: &str, words: &[String]) -> bool {
    let text = text.to_lowercase();
    words.iter().all(|w| text.contains(w))
}

/// A piece of `text` around the first of `words` in it, on one line.
fn around(text: &str, words: &[String]) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = flat.to_lowercase();
    let at = words.iter().filter_map(|w| lower.find(w)).min().unwrap_or(0);
    // A little before it, from the start of a character.
    let mut from = at.saturating_sub(18);
    while from > 0 && !flat.is_char_boundary(from) {
        from -= 1;
    }
    let piece: String = flat[from..].chars().take(120).collect();
    if from > 0 { format!("…{piece}") } else { piece }
}

fn find(ed: &Editor, words: &str) -> Found {
    let list: Vec<String> = words.to_lowercase().split_whitespace().map(str::to_string).collect();
    let mut groups = Vec::new();
    let (mut count, mut cut) = (0, false);
    if let (Some(p), false) = (&ed.project, list.is_empty()) {
        for (key, ws) in &p.workspaces {
            let mut hits = Vec::new();
            for (id, node) in &ws.nodes {
                let block = node.block.split('@').next().unwrap_or(&node.block);
                let def = ed.reg.block(block);
                let name = def.map(|d| d.name.clone()).unwrap_or_else(|| block.to_string());
                let text = if has(&format!("{name} {block} #{id}"), &list) {
                    Some(String::new())
                } else {
                    crate::app::block_texts(def, &node.values).into_iter().find(|(_, t)| has(t, &list)).map(|(label, t)| format!("{label}: {}", around(&t, &list)))
                };
                if let Some(text) = text {
                    if count >= MOST {
                        cut = true;
                        break;
                    }
                    count += 1;
                    hits.push(Hit { id: id.clone(), name, text });
                }
            }
            // By number, as far as they are numbers.
            hits.sort_by_key(|h| (h.id.parse::<u64>().unwrap_or(u64::MAX), h.id.clone()));
            let named = has(&ws.name, &list);
            if named || !hits.is_empty() {
                groups.push(Group { key: key.clone(), name: ws.name.clone(), named, hits });
            }
        }
    }
    Found { for_words: words.to_string(), for_revision: ed.history.revision(), groups, cut }
}

/// The view: the search box, then what it found.
pub fn panel(ui: &mut Ui, ed: &mut Editor) {
    let theme = ed.theme.clone();
    let text_id = Id::unique("side-search-text");
    let found_id = Id::unique("side-search-found");
    let mut words: String = ui.ctx().data(|d| d.get_temp(text_id)).unwrap_or_default();
    let resp = side::search(ui, &theme, &mut words, t!("Search workspaces and blocks"));
    // Shown for the first time: ready to type.
    if ui.ctx().data(|d| d.get_temp::<bool>(Id::unique("side-search-focused"))).is_none() {
        resp.request_focus();
        ui.ctx().data_mut(|d| d.insert_temp(Id::unique("side-search-focused"), true));
    }
    ui.ctx().data_mut(|d| d.insert_temp(text_id, words.clone()));
    if ed.project.is_none() {
        side::note(ui, &theme, t!("Open a project to search it."));
        return;
    }
    if words.trim().is_empty() {
        side::note(ui, &theme, t!("Finds workspaces by name, and blocks by their name, their number (#12) or what is typed into them."));
        return;
    }
    let kept: Option<Arc<Found>> = ui.ctx().data(|d| d.get_temp(found_id));
    let found = match kept.filter(|f| f.for_words == words && f.for_revision == ed.history.revision()) {
        Some(f) => f,
        None => {
            let f = Arc::new(find(ed, &words));
            ui.ctx().data_mut(|d| d.insert_temp(found_id, f.clone()));
            f
        }
    };
    let blocks: usize = found.groups.iter().map(|g| g.hits.len()).sum();
    let summary = match (blocks, found.groups.len()) {
        (0, 0) => t!("Nothing found.").to_string(),
        (b, w) if found.cut => tf!("The first {blocks} blocks in {workspaces} workspaces; type more to narrow it.", blocks = b, workspaces = w),
        (b, w) => tf!("{blocks} blocks in {workspaces} workspaces", blocks = b, workspaces = w),
    };
    side::note(ui, &theme, &summary);
    let mut open = None;
    let mut reveal = None;
    egui::ScrollArea::vertical().id_salt("side-search").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for group in &found.groups {
            let mut go = false;
            side::section(
                ui,
                &theme,
                &format!("search-{}", group.key),
                &group.name,
                Some(group.hits.len()),
                |ui| go = side::action(ui, &theme, &ed.reg.icons, "workspace", t!("Open this workspace")).clicked(),
                |ui| {
                    if group.hits.is_empty() && group.named {
                        let (rect, resp) = side::row(ui, &theme, 24.0, false);
                        ui.painter().text(egui::pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, t!("Its name has the words"), FontId::proportional(12.0), theme.text_muted);
                        if resp.clicked() {
                            open = Some(group.key.clone());
                        }
                    }
                    for hit in &group.hits {
                        let tall = if hit.text.is_empty() { 24.0 } else { 38.0 };
                        let (rect, resp) = side::row(ui, &theme, tall, false);
                        crate::ui::kit::describe(&resp, egui::Role::Button, &format!("{} #{}", hit.name, hit.id));
                        let x = rect.left() + 26.0;
                        let font = FontId::proportional(12.5);
                        let number = format!("#{}", hit.id);
                        let number_w = ui.painter().layout_no_wrap(number.clone(), FontId::proportional(11.0), theme.text_muted).size().x;
                        let top = if hit.text.is_empty() { rect.center().y - 8.0 } else { rect.top() + 4.0 };
                        ui.painter().text(egui::pos2(x, top), Align2::LEFT_TOP, side::fitted(ui, &hit.name, font.clone(), rect.right() - x - number_w - 20.0), font, theme.text);
                        ui.painter().text(egui::pos2(rect.right() - 12.0, top + 1.0), Align2::RIGHT_TOP, &number, FontId::proportional(11.0), theme.text_muted);
                        if !hit.text.is_empty() {
                            ui.painter().text(egui::pos2(x, rect.top() + 21.0), Align2::LEFT_TOP, side::fitted(ui, &hit.text, FontId::proportional(11.5), rect.right() - x - 12.0), FontId::proportional(11.5), theme.text_muted);
                        }
                        let resp = if hit.text.is_empty() { resp } else { resp.on_hover_text(&hit.text) };
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            reveal = Some((group.key.clone(), hit.id.clone()));
                        }
                    }
                },
            );
            if go {
                open = Some(group.key.clone());
            }
        }
    });
    if let Some(key) = open {
        ed.actions.push(crate::editor::Action::OpenWorkspace(key));
    }
    if let Some((ws, id)) = reveal {
        ed.reveal(&ws, &id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_found_and_shown_in_their_place() {
        let words = vec!["ticket".to_string(), "open".to_string()];
        assert!(has("Open a Ticket for ${user}", &words));
        assert!(!has("Close the ticket", &words));
        assert_eq!(around("Hello there", &["hello".into()]), "Hello there");
        let long = format!("{} the ticket is here\nand more", "x ".repeat(40));
        let piece = around(&long, &["ticket".into()]);
        assert!(piece.starts_with('…') && piece.contains("ticket is here and more"), "{piece}");
        // Letters of more than one byte before the word don't cut a character.
        let piece = around(&format!("{}ticket", "ä".repeat(30)), &["ticket".into()]);
        assert!(piece.ends_with("ticket"), "{piece}");
    }
}
