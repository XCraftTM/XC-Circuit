//! The block picker: opens from the canvas toolbar's Add block, right-click, Space, or a wire dropped on empty canvas.

use circuit_core::graph::ACTION_IN;
use circuit_core::{BlockDef, BlockKind, Compat, Link, PinMode, PinRef, PinType};
use circuit_plugins::{BuildMode, Registry};
use eframe::egui::{self, Align2, Frame, Key, Margin, Order, Pos2, RichText, ScrollArea, Stroke, TextEdit, vec2};

use crate::ui::kit;
use crate::canvas::{Dir, PinEnd};
use crate::editor::Editor;
use crate::ui::theme::{Theme, with_alpha};

/// Picker size: the result list on the left, details of the highlighted block on the right.
const PICKER_W: f32 = 760.0;
const BODY_H: f32 = 380.0;
const LIST_W: f32 = 340.0;
const ROW_H: f32 = 32.0;
const HEADER_H: f32 = 24.0;
/// Categories closed in the picker, kept between sessions.
/// The picker's own sections above the categories (no category is named like these).
const FAVORITES: &str = "\u{1}favorites";
const RECENT: &str = "\u{1}recent";
/// How many recently placed blocks the picker lists.
pub const RECENT_SHOWN: usize = 6;

const COLLAPSED_KEY: &str = "picker-collapsed";
/// The kind filter (All, Events, Actions, Values), kept between sessions.
const KIND_KEY: &str = "picker-kind";

/// What a block is, as its chip says it: an event starts a flow, an action
/// does something on the way, a value gives a value to other blocks.
fn kind_name(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Event => t!("Event"),
        BlockKind::Action => t!("Action"),
        BlockKind::Value => t!("Value"),
    }
}

fn kind_color(kind: BlockKind, theme: &Theme) -> egui::Color32 {
    match kind {
        BlockKind::Event => theme.action,
        BlockKind::Action => theme.accent,
        BlockKind::Value => theme.text_muted,
    }
}

/// Events first in a category: that's where a flow starts.
fn kind_order(kind: BlockKind) -> u8 {
    match kind {
        BlockKind::Event => 0,
        BlockKind::Action => 1,
        BlockKind::Value => 2,
    }
}

#[derive(Clone, Debug)]
pub struct PendingWire {
    pub node: String,
    pub pin: String,
    pub dir: Dir,
    pub ty: PinType,
}

impl From<&PinEnd> for PendingWire {
    fn from(e: &PinEnd) -> Self {
        Self { node: e.node.clone(), pin: e.pin.clone(), dir: e.dir, ty: e.ty.clone() }
    }
}

pub struct Picker {
    pub ws: String,
    pub screen_pos: Pos2,
    pub scene_pos: Pos2,
    pub query: String,
    pub pending: Option<PendingWire>,
    pub highlighted: usize,
    opened_frames: u32,
}

impl Picker {
    pub fn new(ws: &str, screen_pos: Pos2, scene_pos: Pos2, pending: Option<PendingWire>) -> Picker {
        Picker { ws: ws.into(), screen_pos, scene_pos, query: String::new(), pending, highlighted: 0, opened_frames: 0 }
    }
}

/// The pin on a new block that a pending wire should attach to.
fn matching_pin(def: &BlockDef, pending: &PendingWire, reg: &Registry) -> Option<String> {
    match pending.dir {
        // Dragged from an output: attach to an input of the new block.
        Dir::Out => {
            if pending.ty.is_action() {
                return (def.kind == BlockKind::Action).then(|| ACTION_IN.to_string());
            }
            def.fields
                .iter()
                .filter(|(_, f)| f.pin != PinMode::Inline)
                .map(|(k, f)| (k, pending.ty.compat(&f.ty, reg)))
                .filter(|(_, c)| *c != Compat::No)
                .max_by_key(|(_, c)| *c)
                .map(|(k, _)| k.clone())
        }
        // Dragged from an input: attach to an output of the new block.
        Dir::In => {
            if pending.ty.is_action() {
                return def.base_flows().into_iter().next();
            }
            def.outputs
                .iter()
                .map(|(k, o)| (k, o.ty.compat(&pending.ty, reg)))
                .filter(|(_, c)| *c != Compat::No)
                .max_by_key(|(_, c)| *c)
                .map(|(k, _)| k.clone())
        }
    }
}

/// Where the search's words are in a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Found {
    /// Every word is in its name.
    Name,
    /// Every word is in one of its other names. They only help finding it
    /// and aren't shown.
    Alias,
    /// The words are somewhere in its id, description, tags or category.
    Elsewhere,
}

/// How well the words fit where they were found, best first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Fit {
    /// It starts with what was typed: "date" in Date & Time.
    Begins,
    /// Each word is a whole word of it: "date" in Read Date.
    Words,
    /// Each word starts one of its words: "mess" in Send Message.
    Starts,
    /// A word is inside one of its words: "date" in Member Updated.
    Inside,
}

/// A block the search found, and how well.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Match {
    found: Found,
    fit: Fit,
}

impl Match {
    /// Lower is better: the name, other names and the rest by how well the
    /// words fit; words only inside other words after all of those.
    fn score(self) -> u8 {
        match self.fit {
            Fit::Inside => 20 + self.found as u8,
            fit => self.found as u8 * 4 + fit as u8,
        }
    }
}

/// How well every word of `words` (lower case) fits `text`, if all are in it.
fn fit(text: &str, query: &str, words: &[String]) -> Option<Fit> {
    let text = text.to_lowercase();
    let own: Vec<&str> = text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let mut worst = Fit::Words;
    for w in words {
        let one = if own.iter().any(|o| o == w) {
            Fit::Words
        } else if own.iter().any(|o| o.starts_with(w.as_str())) {
            Fit::Starts
        } else if text.contains(w.as_str()) {
            Fit::Inside
        } else {
            return None;
        };
        worst = worst.max(one);
    }
    let typed = query.trim().to_lowercase();
    Some(if worst <= Fit::Starts && !typed.is_empty() && text.starts_with(&typed) { Fit::Begins } else { worst })
}

fn search(def: &BlockDef, query: &str) -> Option<Match> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let timing = match def.timing() {
        Some(true) => "async",
        Some(false) => "sync",
        None => "",
    };
    let elsewhere = format!("{} {} {} {} {timing}", def.id, def.description, def.tags.join(" "), def.category.as_deref().unwrap_or(""));
    // Each place it can be found, best first; the best fit wins (a whole word
    // in the description beats a word inside the name).
    let name = fit(&def.name, query, &words).map(|fit| Match { found: Found::Name, fit });
    let alias = def.aliases.iter().filter_map(|a| fit(a, query, &words)).min().map(|fit| Match { found: Found::Alias, fit });
    let rest = fit(&format!("{} {} {elsewhere}", def.name, def.aliases.join(" ")), "", &words).map(|fit| Match { found: Found::Elsewhere, fit });
    [name, alias, rest].into_iter().flatten().min_by_key(|m| m.score())
}

/// Draws the picker if open. Places the chosen block and connects a pending wire.
pub fn show(ctx: &egui::Context, ed: &mut Editor) {
    if ed.picker.is_none() {
        return;
    }
    let theme = ed.theme.clone();
    let mode = ed.mode().cloned();
    // Blocks from the project's own plugins/ folder always belong.
    let project_root = ed.project.as_ref().map(|p| p.root.clone());
    let picker = ed.picker.as_mut().unwrap();
    picker.opened_frames += 1;

    let searching = !picker.query.trim().is_empty();
    // None shows every kind.
    let kind_filter: Option<BlockKind> = ctx.data_mut(|d| d.get_persisted::<u8>(egui::Id::unique(KIND_KEY))).and_then(|k| match k {
        1 => Some(BlockKind::Event),
        2 => Some(BlockKind::Action),
        3 => Some(BlockKind::Value),
        _ => None,
    });
    let mut list: Vec<(&BlockDef, Option<String>, Match)> = ed
        .reg
        .blocks
        .values()
        .filter(|d| {
            mode.as_ref().is_none_or(|m: &BuildMode| m.includes(&d.plugin, &d.id))
                || project_root.as_ref().is_some_and(|r| d.source.starts_with(r))
        })
        .filter(|d| d.deprecated.is_none())
        .filter(|d| kind_filter.is_none_or(|k| d.kind == k))
        .filter_map(|d| search(d, &picker.query).map(|found| (d, found)))
        .filter_map(|(d, found)| match &picker.pending {
            Some(p) => matching_pin(d, p, &ed.reg).map(|pin| (d, Some(pin), found)),
            None => Some((d, None, found)),
        })
        .collect();
    let rank = |d: &BlockDef| mode.as_ref().map(|m| m.category_rank(d.category.as_deref().unwrap_or(""))).unwrap_or(usize::MAX);
    if searching {
        // Best matches first (see Match::score), events first among equally good ones.
        list.sort_by(|a, b| a.2.score().cmp(&b.2.score()).then_with(|| kind_order(a.0.kind).cmp(&kind_order(b.0.kind))).then_with(|| a.0.name.cmp(&b.0.name)));
    } else {
        list.sort_by(|a, b| rank(a.0).cmp(&rank(b.0)).then_with(|| kind_order(a.0.kind).cmp(&kind_order(b.0.kind))).then_with(|| a.0.name.cmp(&b.0.name)));
    }
    // Favourites, then recently placed blocks, above the categories (not while searching).
    let mut sections: Vec<Option<&str>> = vec![None; list.len()];
    if !searching {
        let pick = |ids: &[String], limit: usize| ids.iter().filter_map(|id| list.iter().find(|e| e.0.id == *id).cloned()).take(limit).collect::<Vec<_>>();
        let favorites = pick(&ed.prefs.favorite_blocks, usize::MAX);
        let recent = pick(&ed.prefs.recent_blocks, RECENT_SHOWN);
        let mut top: Vec<_> = favorites.iter().cloned().map(|e| (e, Some(FAVORITES))).collect();
        top.extend(recent.into_iter().map(|e| (e, Some(RECENT))));
        let mut head_sections: Vec<Option<&str>> = top.iter().map(|(_, s)| *s).collect();
        head_sections.append(&mut sections);
        sections = head_sections;
        let mut head: Vec<_> = top.into_iter().map(|(e, _)| e).collect();
        head.append(&mut list);
        list = head;
    }
    let favorite_ids = ed.prefs.favorite_blocks.clone();
    // Closed categories hide their blocks, except while searching.
    let collapsed: Vec<String> = ctx.data_mut(|d| d.get_persisted::<Vec<String>>(egui::Id::unique(COLLAPSED_KEY)).unwrap_or_default());
    let category_of = |d: &BlockDef| d.category.clone().unwrap_or_default();
    let cat_at = |i: usize| sections[i].map(str::to_string).unwrap_or_else(|| category_of(list[i].0));
    let shown: Vec<usize> = (0..list.len()).filter(|&i| searching || !collapsed.contains(&cat_at(i))).collect();
    // A star clicked: the block to add to or take from the favourites.
    let mut star: Option<String> = None;
    if !shown.contains(&picker.highlighted) {
        picker.highlighted = shown.iter().copied().find(|&i| i >= picker.highlighted).or(shown.last().copied()).unwrap_or(0);
    }

    // The row under the pointer is highlighted when the pointer moves there,
    // not while it rests: resting on the list (the picker opens under it),
    // it took the highlight back each frame, so the arrow keys did nothing
    // and Enter after a search placed the block under the pointer.
    let pointer_moved = ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
    let (up, down, enter, escape) = ctx.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
    let at = shown.iter().position(|&i| i == picker.highlighted);
    if up && let Some(at) = at {
        picker.highlighted = shown[at.saturating_sub(1)];
    }
    if down && let Some(at) = at {
        picker.highlighted = shown[(at + 1).min(shown.len() - 1)];
    }

    let mut chosen: Option<(String, Option<String>)> =
        if enter && at.is_some() { list.get(picker.highlighted).map(|(d, pin, _)| (d.id.clone(), pin.clone())) } else { None };
    // A category header clicked, or Expand all / Collapse all.
    let mut fold: Option<Fold> = None;
    // A kind filter chip clicked.
    let mut new_filter: Option<Option<BlockKind>> = None;
    let mut close = escape;

    let screen = ctx.content_rect();
    // Smaller windows get a smaller picker.
    let body_h = BODY_H.min(screen.height() - 120.0).max(160.0);
    let size = vec2(PICKER_W.min(screen.width() - 16.0), body_h + 126.0);
    let mut pos = picker.screen_pos;
    pos.x = pos.x.min(screen.right() - size.x - 8.0);
    pos.y = pos.y.min(screen.bottom() - size.y - 8.0);

    let area = egui::Area::new(egui::Id::unique("block-picker")).order(Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        Frame::new()
            .fill(theme.panel)
            .stroke(Stroke::new(1.0, theme.node_border))
            .corner_radius(10)
            .shadow(ctx.global_style().visuals.popup_shadow)
            .show(ui, |ui| {
                ui.set_width(size.x);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                // Search row.
                Frame::new().inner_margin(Margin::symmetric(14, 11)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let edit = ui.add(
                            TextEdit::singleline(&mut picker.query)
                                .hint_text(t!("Search blocks"))
                                .frame(Frame::new())
                                .desired_width(size.x - 210.0)
                                .font(egui::FontId::proportional(15.0)),
                        );
                        ed.ui_rects.insert("picker-search".into(), edit.rect);
                        // Typing always searches, also after a category was clicked.
                        if picker.opened_frames <= 2 || ui.ctx().memory(|m| m.focused().is_none()) {
                            edit.request_focus();
                        }
                        if edit.changed() {
                            picker.highlighted = 0;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let tag = if picker.pending.is_some() { t!("Only compatible") } else { t!("All blocks") };
                            Frame::new().fill(theme.panel_header).corner_radius(99).inner_margin(Margin::symmetric(8, 2)).show(ui, |ui| {
                                ui.label(RichText::new(tag).small().color(theme.text_muted));
                            });
                            if !searching {
                                ui.add_space(8.0);
                                ui.spacing_mut().item_spacing.x = 4.0;
                                if fold_button(ui, &theme, "unfold", t!("Expand all"), &ed.reg).clicked() {
                                    fold = Some(Fold::All(false));
                                }
                                if fold_button(ui, &theme, "fold", t!("Collapse all"), &ed.reg).clicked() {
                                    fold = Some(Fold::All(true));
                                }
                            }
                        });
                    });
                });
                // Kind filter: All, Events, Actions, Values.
                Frame::new().inner_margin(Margin { left: 12, right: 12, top: 0, bottom: 8 }).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let options = [(None, t!("All")), (Some(BlockKind::Event), t!("Events")), (Some(BlockKind::Action), t!("Actions")), (Some(BlockKind::Value), t!("Values"))];
                        for (kind, label) in options {
                            let color = kind.map(|k| kind_color(k, &theme)).unwrap_or(theme.text);
                            if crate::ui::kit::filter_chip(ui, &theme, label, color, kind == kind_filter).clicked() {
                                new_filter = Some(kind);
                            }
                        }
                    });
                });
                ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, theme.line));

                let column = egui::Layout::top_down(egui::Align::Min);
                ui.horizontal_top(|ui| {
                    // Result list grouped by category; while searching, best matches first.
                    ui.allocate_ui_with_layout(vec2(LIST_W, body_h), column, |ui| {
                        ScrollArea::vertical().id_salt("picker-list").max_height(body_h).auto_shrink([false, false]).show(ui, |ui| {
                            ui.set_width(LIST_W);
                            let category_name = |cat: &str| match cat {
                                FAVORITES => t!("Favourites").to_string(),
                                RECENT => t!("Recent").to_string(),
                                _ => mode.as_ref().and_then(|m| m.category(cat)).map(|c| crate::i18n::tr(&c.name).into_owned()).unwrap_or(cat.to_string()),
                            };
                            let mut last_cat: Option<String> = None;
                            for (i, (def, pin, _)) in list.iter().enumerate() {
                                let cat = cat_at(i);
                                if !searching && last_cat.as_deref() != Some(cat.as_str()) {
                                    let closed = collapsed.contains(&cat);
                                    let count = (0..list.len()).filter(|&j| cat_at(j) == cat).count();
                                    if category_header(ui, &theme, &category_name(&cat), count, closed).clicked() {
                                        fold = Some(Fold::One(cat.clone()));
                                    }
                                    last_cat = Some(cat.clone());
                                }
                                if !shown.contains(&i) {
                                    continue;
                                }
                                let selected = i == picker.highlighted;
                                let (rect, resp) = ui.allocate_exact_size(vec2(LIST_W - 8.0, ROW_H), egui::Sense::click());
                                crate::ui::kit::describe_selected(&resp, egui::Role::ListBoxOption, selected, &def.name);
                                let rect = rect.translate(vec2(4.0, 0.0));
                                if selected {
                                    ui.painter().rect_filled(rect, 6.0, with_alpha(theme.accent, 45));
                                } else if resp.hovered() {
                                    ui.painter().rect_filled(rect, 6.0, theme.panel_header);
                                }
                                if let Some(path) = ed.reg.block_icon(def, mode.as_ref()) {
                                    let icon = egui::Rect::from_center_size(rect.left_center() + vec2(18.0, 0.0), vec2(16.0, 16.0));
                                    let tint = if selected { theme.accent } else { theme.text_muted };
                                    egui::Image::from_uri(crate::editor::file_uri(&path)).tint(tint).paint_at(ui, icon);
                                }
                                let name_rect = ui.painter().text(rect.left_center() + vec2(36.0, 0.0), Align2::LEFT_CENTER, &def.name, egui::FontId::proportional(14.0), theme.text);
                                // Its kind as a chip at the right; while searching, its category before that.
                                let chip = paint_kind_chip(ui, &theme, def.kind, rect.right() - 8.0, rect.center().y);
                                let chip = match def.timing() {
                                    Some(waits) => paint_timing_chip(ui, &theme, waits, chip.left() - 4.0, rect.center().y),
                                    None => chip,
                                };
                                // A star: filled for favourites, outlined on the hovered row.
                                let starred = favorite_ids.contains(&def.id);
                                let chip = if starred || resp.hovered() || selected {
                                    let sr = egui::Rect::from_center_size(egui::pos2(chip.left() - 14.0, rect.center().y), vec2(20.0, 20.0));
                                    let sresp = ui.interact(sr, ui.scope_id().with(("star", i)), egui::Sense::click());
                                    crate::ui::kit::describe_selected(&sresp, egui::Role::Button, starred, &format!("{}: {}", t!("Favourites"), def.name));
                                    let tint = if starred { theme.warning } else if sresp.hovered() { theme.text } else { with_alpha(theme.text_muted, 140) };
                                    if let Some(path) = ed.reg.icons.get(if starred { "star-filled" } else { "star" }) {
                                        egui::Image::from_uri(crate::editor::file_uri(path)).tint(tint).paint_at(ui, sr.shrink(3.0));
                                    }
                                    if sresp.on_hover_text(if starred { t!("Remove from favourites") } else { t!("Add to favourites") }).clicked() {
                                        star = Some(def.id.clone());
                                    }
                                    sr
                                } else {
                                    chip
                                };
                                if searching {
                                    let note = category_name(&cat);
                                    let font = egui::FontId::proportional(11.5);
                                    let room = chip.left() - 8.0 - (name_rect.right() + 8.0);
                                    let galley = ui.painter().layout(note, font, theme.text_muted, f32::INFINITY);
                                    if galley.size().x <= room {
                                        ui.painter().galley(egui::pos2(chip.left() - 8.0 - galley.size().x, rect.center().y - galley.size().y / 2.0), galley, theme.text_muted);
                                    }
                                }
                                if ed.tour_parts && ui.clip_rect().intersects(rect) {
                                    ed.ui_rects.insert(format!("picker:{}", def.id), rect.intersect(ui.clip_rect()));
                                }
                                if resp.hovered() && pointer_moved {
                                    picker.highlighted = i;
                                }
                                if resp.clicked() {
                                    chosen = Some((def.id.clone(), pin.clone()));
                                }
                                if selected && (up || down) {
                                    resp.scroll_to_me(None);
                                }
                            }
                            if list.is_empty() {
                                ui.add_space(16.0);
                                ui.label(RichText::new(t!("   No blocks match")).color(theme.text_muted));
                            }
                        });
                    });
                    let sep = ui.min_rect();
                    ui.painter().vline(sep.right(), sep.y_range(), Stroke::new(1.0, theme.line));

                    // Details of the highlighted block.
                    ui.allocate_ui_with_layout(vec2(size.x - LIST_W - 1.0, body_h), column, |ui| {
                        Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                            ui.set_max_width(size.x - LIST_W - 1.0 - 32.0);
                            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                            if let Some((def, _, _)) = list.get(picker.highlighted) {
                                details(ui, def, &ed.reg, &theme);
                                ui.add_space(4.0);
                                let starred = favorite_ids.contains(&def.id);
                                let label = if starred { t!("★ Favourite") } else { t!("☆ Add to favourites") };
                                if crate::ui::kit::button(ui, &theme, label).on_hover_text(t!("Favourites are listed at the top of the picker")).clicked() {
                                    star = Some(def.id.clone());
                                }
                            }
                        });
                    });
                });

                ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, theme.line));
                Frame::new().fill(theme.panel_header).inner_margin(Margin::symmetric(12, 6)).corner_radius(egui::CornerRadius { nw: 0, ne: 0, sw: 10, se: 10 }).show(ui, |ui| {
                    ui.set_width(size.x - 24.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(14.0, 0.0);
                        for hint in [t!("Enter: place"), t!("Up/Down: move"), t!("Esc: close")] {
                            ui.label(RichText::new(hint).size(11.0).color(theme.text_muted));
                        }
                    });
                });
            });
    });

    ed.ui_rects.insert("picker".into(), area.response.rect);
    // Clicking anywhere else closes it.
    let clicked_outside = ctx.input(|i| i.pointer.any_pressed())
        && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area.response.rect.contains(p))
        && picker.opened_frames > 2;
    close |= clicked_outside;

    if let Some(kind) = new_filter {
        let stored: u8 = match kind {
            None => 0,
            Some(BlockKind::Event) => 1,
            Some(BlockKind::Action) => 2,
            Some(BlockKind::Value) => 3,
        };
        ctx.data_mut(|d| d.insert_persisted(egui::Id::unique(KIND_KEY), stored));
        if let Some(p) = ed.picker.as_mut() {
            p.highlighted = 0;
        }
    }
    if let Some(id) = star {
        let favorites = &mut ed.prefs.favorite_blocks;
        if favorites.contains(&id) {
            favorites.retain(|f| *f != id);
        } else {
            favorites.push(id);
        }
    }
    if let Some(fold) = fold {
        let cats: Vec<String> = (0..list.len()).map(cat_at).collect();
        ctx.data_mut(|d| {
            let closed = d.get_persisted_mut_or_default::<Vec<String>>(egui::Id::unique(COLLAPSED_KEY));
            match fold {
                Fold::One(cat) if closed.contains(&cat) => closed.retain(|c| *c != cat),
                Fold::One(cat) => closed.push(cat),
                Fold::All(false) => closed.clear(),
                Fold::All(true) => {
                    for cat in cats {
                        if !closed.contains(&cat) {
                            closed.push(cat);
                        }
                    }
                }
            }
        });
    }
    if let Some((block_id, pin)) = chosen {
        let picker = ed.picker.take().unwrap();
        // Remembered for the Recent section.
        let recent = &mut ed.prefs.recent_blocks;
        recent.retain(|r| *r != block_id);
        recent.insert(0, block_id.clone());
        recent.truncate(20);
        place(ed, &picker, &block_id, pin);
    } else if close {
        ed.picker = None;
    }
}

/// Opening or closing categories in the list.
enum Fold {
    /// One category, by id.
    One(String),
    /// Every category: closed (`true`) or open.
    All(bool),
}

/// A category's row: an arrow (right when closed, down when open), its name
/// and how many blocks it has. The whole row opens or closes it.
fn category_header(ui: &mut egui::Ui, theme: &Theme, name: &str, count: usize, closed: bool) -> egui::Response {
    ui.add_space(4.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(LIST_W - 8.0, HEADER_H), egui::Sense::click());
    crate::ui::kit::describe(&resp, egui::Role::Button, &format!("{name} ({count})"));
    crate::ui::kit::accesskit_expanded(&resp, !closed);
    let rect = rect.translate(vec2(4.0, 0.0));
    if resp.hovered() {
        ui.painter().rect_filled(rect, 6.0, theme.panel_header);
    }
    let color = if resp.hovered() { theme.text } else { theme.text_muted };
    let c = rect.left_center() + vec2(14.0, 0.0);
    let arrow = if closed {
        vec![c + vec2(-2.5, -4.0), c + vec2(3.0, 0.0), c + vec2(-2.5, 4.0)]
    } else {
        vec![c + vec2(-4.0, -2.5), c + vec2(4.0, -2.5), c + vec2(0.0, 3.0)]
    };
    ui.painter().add(egui::Shape::convex_polygon(arrow, color, Stroke::NONE));
    let font = egui::FontId::proportional(10.5);
    let label = ui.painter().text(rect.left_center() + vec2(26.0, 0.0), Align2::LEFT_CENTER, name.to_uppercase(), font.clone(), color);
    ui.painter().text(egui::pos2(label.right() + 6.0, rect.center().y), Align2::LEFT_CENTER, count.to_string(), font, with_alpha(theme.text_muted, 140));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A kind's chip, its right edge at `right`. Returns where it was painted.
fn paint_kind_chip(ui: &egui::Ui, theme: &Theme, kind: BlockKind, right: f32, center_y: f32) -> egui::Rect {
    let color = kind_color(kind, theme);
    let galley = ui.painter().layout_no_wrap(kind_name(kind).to_string(), egui::FontId::proportional(10.5), color);
    let size = galley.size() + vec2(12.0, 4.0);
    let rect = egui::Rect::from_min_size(egui::pos2(right - size.x, center_y - size.y / 2.0), size);
    ui.painter().rect_filled(rect, 99.0, with_alpha(color, 32));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    rect
}

/// Async or Sync, as a quiet outlined chip, its right edge at `right`.
fn paint_timing_chip(ui: &egui::Ui, theme: &Theme, waits: bool, right: f32, center_y: f32) -> egui::Rect {
    let text = if waits { t!("Async") } else { t!("Sync") };
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(10.5), theme.text_muted);
    let size = galley.size() + vec2(12.0, 4.0);
    let rect = egui::Rect::from_min_size(egui::pos2(right - size.x, center_y - size.y / 2.0), size);
    ui.painter().rect_stroke(rect, 99.0, egui::Stroke::new(1.0, with_alpha(theme.text_muted, 90)), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, theme.text_muted);
    rect
}

/// What Async and Sync mean, for tooltips.
pub fn timing_text(waits: bool) -> &'static str {
    if waits {
        t!("Async: waits for something (Discord, the web, a timer, stored data) before the flow goes on")
    } else {
        t!("Sync: done at once, without waiting")
    }
}

/// A small icon button in the search row, with its name as a tooltip.
fn fold_button(ui: &mut egui::Ui, theme: &Theme, icon: &str, tip: &str, reg: &Registry) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), egui::Sense::click());
    crate::ui::kit::describe(&resp, egui::Role::Button, tip);
    if resp.hovered() {
        ui.painter().rect_filled(rect, 6.0, theme.panel_header);
    }
    let tint = if resp.hovered() { theme.text } else { theme.text_muted };
    if let Some(path) = reg.icons.get(icon) {
        egui::Image::from_uri(crate::editor::file_uri(path)).tint(tint).paint_at(ui, egui::Rect::from_center_size(rect.center(), vec2(14.0, 14.0)));
    }
    resp.on_hover_text(tip)
}

fn details(ui: &mut egui::Ui, def: &BlockDef, reg: &Registry, theme: &Theme) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(&def.name).strong().size(16.0));
        let what = match def.kind {
            BlockKind::Event => t!("Event: starts a flow"),
            BlockKind::Action => t!("Action: does something"),
            BlockKind::Value => t!("Value: gives a value (in a flow, it runs there)"),
        };
        ui.label(RichText::new(what).size(11.5).color(kind_color(def.kind, theme)));
        if let Some(waits) = def.timing() {
            kit::hint(ui, theme, if waits { t!("Async") } else { t!("Sync") }).on_hover_text(timing_text(waits));
        }
    });
    ui.label(RichText::new(tf!("by {author} · v{version} · {plugin}", author = def.author, version = def.version, plugin = def.plugin)).size(12.0).color(theme.text_muted));
    ui.label(&def.description);
    let fields: Vec<String> = def.fields.iter().filter(|(_, f)| !f.advanced).map(|(k, f)| f.label(k)).collect();
    let outputs: Vec<String> = def.outputs.iter().map(|(k, o)| format!("{} ({})", o.label(k), o.ty.display(reg))).collect();
    if !fields.is_empty() {
        ui.label(RichText::new(tf!("In: {list}", list = fields.join(", "))).size(12.5).color(theme.text_muted));
    }
    if !outputs.is_empty() {
        ui.label(RichText::new(tf!("Out: {list}", list = outputs.join(", "))).size(12.5).color(theme.text_muted));
    }
}

fn place(ed: &mut Editor, picker: &Picker, block_id: &str, pin: Option<String>) {
    let at = picker.scene_pos - vec2(20.0, 16.0);
    let Some(new_id) = ed.add_node(&picker.ws, block_id, at) else { return };
    let (Some(pending), Some(pin)) = (&picker.pending, pin) else { return };
    let Some(ws) = ed.project.as_mut().and_then(|p| p.workspaces.get_mut(&picker.ws)) else { return };
    let link = match pending.dir {
        Dir::Out => Link { from: PinRef::new(&pending.node, &pending.pin), to: PinRef::new(&new_id, &pin) },
        Dir::In => Link { from: PinRef::new(&new_id, &pin), to: PinRef::new(&pending.node, &pending.pin) },
    };
    let action = pending.ty.is_action();
    let pin_node = link.to.node.clone();
    let pin_name = link.to.pin.clone();
    ws.connect(link, !action);
    if !action
        && let Some(n) = ws.nodes.get_mut(&pin_node)
        && !n.pins.contains(&pin_name)
    {
        n.pins.push(pin_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the pointer resting on the list, the arrow keys move the
    /// highlight and a search highlights its best match.
    #[test]
    fn keys_work_while_the_pointer_rests_on_the_list() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        ed.tour_parts = true;
        ed.picker = Some(Picker::new("rank-command", egui::pos2(100.0, 100.0), egui::pos2(0.0, 0.0), None));
        let ctx = egui::Context::default();
        crate::ui::fonts::install(&ctx);
        let run = |ed: &mut Editor, events: Vec<egui::Event>| {
            let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1200.0, 900.0))), events, ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| show(ui.ctx(), ed));
            out.textures_delta.clear();
        };
        let highlighted = |ed: &Editor| ed.picker.as_ref().unwrap().highlighted;
        for _ in 0..4 {
            run(&mut ed, vec![]);
        }
        // Onto the third row drawn: moving there highlights it.
        let mut rows: Vec<egui::Rect> = ed.ui_rects.iter().filter(|(k, _)| k.starts_with("picker:")).map(|(_, r)| *r).collect();
        rows.sort_by(|a, b| a.top().total_cmp(&b.top()));
        run(&mut ed, vec![egui::Event::PointerMoved(egui::pos2(10.0, 10.0))]);
        run(&mut ed, vec![egui::Event::PointerMoved(rows[2].center())]);
        run(&mut ed, vec![]);
        let at = highlighted(&ed);
        assert!(at > 0);
        let key = |key: Key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        run(&mut ed, vec![key(Key::ArrowDown)]);
        run(&mut ed, vec![]);
        assert_eq!(highlighted(&ed), at + 1, "Down moves on, and stays there");
        run(&mut ed, vec![egui::Event::Text("send".into())]);
        run(&mut ed, vec![]);
        run(&mut ed, vec![]);
        assert_eq!(highlighted(&ed), 0, "the best match, not the row under the pointer");
    }

    #[test]
    fn other_names_count_like_the_name() {
        let def = BlockDef { name: "Send Message".into(), aliases: vec!["Reply".into(), "Send DM".into()], tags: vec!["private".into()], ..Default::default() };
        let found = |q: &str| search(&def, q).map(|m| m.found);
        assert_eq!(found("send msg"), None);
        assert_eq!(found("send"), Some(Found::Name));
        assert_eq!(found("reply"), Some(Found::Alias));
        assert_eq!(found("private"), Some(Found::Elsewhere));
        assert_eq!(found(""), Some(Found::Name));
    }

    /// "date" finds Date & Time first, then blocks with the word Date, and
    /// only after those ones where it is inside a word (Member Updated).
    #[test]
    fn whole_words_come_before_words_inside_words() {
        let block = |name: &str, description: &str, kind: BlockKind| BlockDef { name: name.into(), description: description.into(), kind, ..Default::default() };
        let blocks = [
            block("Member Updated", "A member changed", BlockKind::Event),
            block("Time Between", "How long lies between two dates", BlockKind::Value),
            block("Read Date", "Turns text into a date", BlockKind::Value),
            block("Date & Time", "The current time", BlockKind::Value),
        ];
        let mut found: Vec<(u8, &str)> = blocks.iter().filter_map(|b| search(b, "date").map(|m| (m.score(), b.name.as_str()))).collect();
        found.sort();
        let names: Vec<&str> = found.iter().map(|(_, n)| *n).collect();
        assert_eq!(names, ["Date & Time", "Read Date", "Time Between", "Member Updated"]);
        assert_eq!(search(&blocks[0], "date").unwrap().fit, Fit::Inside);
        // A start of a word still counts: "mem upd".
        assert_eq!(search(&blocks[0], "mem upd").unwrap(), Match { found: Found::Name, fit: Fit::Starts });
    }
}
