//! The block picker: opens on right-click, Space, or a wire dropped on empty canvas.

use circuit_core::graph::ACTION_IN;
use circuit_core::{BlockDef, BlockKind, Compat, Link, PinMode, PinRef, PinType};
use circuit_plugins::{BuildMode, Registry};
use eframe::egui::{self, Align2, Frame, Key, Margin, Order, Pos2, RichText, ScrollArea, Stroke, TextEdit, vec2};

use crate::canvas::{Dir, PinEnd};
use crate::editor::Editor;
use crate::theme::{Theme, with_alpha};

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

fn matches_query(def: &BlockDef, query: &str) -> bool {
    let hay = format!("{} {} {} {} {}", def.name, def.id, def.description, def.tags.join(" "), def.category.as_deref().unwrap_or("")).to_lowercase();
    query.split_whitespace().all(|w| hay.contains(&w.to_lowercase()))
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

    let mut list: Vec<(&BlockDef, Option<String>)> = ed
        .reg
        .blocks
        .values()
        .filter(|d| {
            mode.as_ref().is_none_or(|m: &BuildMode| m.includes(&d.plugin, &d.id))
                || project_root.as_ref().is_some_and(|r| d.source.starts_with(r))
        })
        .filter(|d| d.deprecated.is_none())
        .filter(|d| matches_query(d, &picker.query))
        .filter_map(|d| match &picker.pending {
            Some(p) => matching_pin(d, p, &ed.reg).map(|pin| (d, Some(pin))),
            None => Some((d, None)),
        })
        .collect();
    let rank = |d: &BlockDef| mode.as_ref().map(|m| m.category_rank(d.category.as_deref().unwrap_or(""))).unwrap_or(usize::MAX);
    list.sort_by(|a, b| rank(a.0).cmp(&rank(b.0)).then_with(|| a.0.name.cmp(&b.0.name)));
    if !list.is_empty() {
        picker.highlighted = picker.highlighted.min(list.len() - 1);
    }

    let (up, down, enter, escape) = ctx.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
    if up {
        picker.highlighted = picker.highlighted.saturating_sub(1);
    }
    if down && picker.highlighted + 1 < list.len() {
        picker.highlighted += 1;
    }

    let mut chosen: Option<(String, Option<String>)> = if enter { list.get(picker.highlighted).map(|(d, pin)| (d.id.clone(), pin.clone())) } else { None };
    let mut close = escape;

    let screen = ctx.content_rect();
    let size = vec2(520.0, 300.0);
    let mut pos = picker.screen_pos;
    pos.x = pos.x.min(screen.right() - size.x - 8.0);
    pos.y = pos.y.min(screen.bottom() - size.y - 8.0);

    let area = egui::Area::new(egui::Id::new("block-picker")).order(Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        Frame::new()
            .fill(theme.panel)
            .stroke(Stroke::new(1.0, theme.node_border))
            .corner_radius(10)
            .shadow(ctx.global_style().visuals.popup_shadow)
            .show(ui, |ui| {
                ui.set_width(size.x);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                // Search row.
                Frame::new().inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let edit = ui.add(
                            TextEdit::singleline(&mut picker.query)
                                .hint_text("Search blocks")
                                .frame(Frame::new())
                                .desired_width(size.x - 140.0)
                                .font(egui::TextStyle::Body),
                        );
                        if picker.opened_frames <= 2 {
                            edit.request_focus();
                        }
                        if edit.changed() {
                            picker.highlighted = 0;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let tag = if picker.pending.is_some() { "Only compatible" } else { "All blocks" };
                            Frame::new().fill(theme.panel_header).corner_radius(99).inner_margin(Margin::symmetric(8, 2)).show(ui, |ui| {
                                ui.label(RichText::new(tag).small().color(theme.text_muted));
                            });
                        });
                    });
                });
                ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, theme.line));

                let column = egui::Layout::top_down(egui::Align::Min);
                ui.horizontal_top(|ui| {
                    // Result list grouped by category.
                    ui.allocate_ui_with_layout(vec2(220.0, 210.0), column, |ui| {
                        ScrollArea::vertical().id_salt("picker-list").max_height(210.0).auto_shrink([false, false]).show(ui, |ui| {
                            ui.set_width(220.0);
                            let mut last_cat: Option<String> = None;
                            for (i, (def, pin)) in list.iter().enumerate() {
                                let cat = def.category.clone().unwrap_or_default();
                                if last_cat.as_deref() != Some(cat.as_str()) {
                                    let name = mode.as_ref().and_then(|m| m.category(&cat)).map(|c| c.name.clone()).unwrap_or(cat.clone());
                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        ui.add_space(12.0);
                                        ui.label(RichText::new(name.to_uppercase()).size(10.0).color(theme.text_muted));
                                    });
                                    ui.add_space(2.0);
                                    last_cat = Some(cat);
                                }
                                let selected = i == picker.highlighted;
                                let (rect, resp) = ui.allocate_exact_size(vec2(212.0, 28.0), egui::Sense::click());
                                let rect = rect.translate(vec2(4.0, 0.0));
                                if selected {
                                    ui.painter().rect_filled(rect, 6.0, with_alpha(theme.accent, 45));
                                } else if resp.hovered() {
                                    ui.painter().rect_filled(rect, 6.0, theme.panel_header);
                                }
                                if let Some(path) = ed.reg.block_icon(def, mode.as_ref()) {
                                    let icon = egui::Rect::from_center_size(rect.left_center() + vec2(16.0, 0.0), vec2(14.0, 14.0));
                                    let tint = if selected { theme.accent } else { theme.text_muted };
                                    egui::Image::from_uri(crate::editor::file_uri(&path)).tint(tint).paint_at(ui, icon);
                                }
                                ui.painter().text(rect.left_center() + vec2(32.0, 0.0), Align2::LEFT_CENTER, &def.name, egui::FontId::proportional(13.0), theme.text);
                                if resp.hovered() {
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
                                ui.label(RichText::new("   No blocks match").color(theme.text_muted));
                            }
                        });
                    });
                    let sep = ui.min_rect();
                    ui.painter().vline(sep.right(), sep.y_range(), Stroke::new(1.0, theme.line));

                    // Details of the highlighted block.
                    ui.allocate_ui_with_layout(vec2(size.x - 221.0, 210.0), column, |ui| {
                        Frame::new().inner_margin(Margin::same(14)).show(ui, |ui| {
                            ui.set_max_width(size.x - 221.0 - 28.0);
                            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                            if let Some((def, _)) = list.get(picker.highlighted) {
                                details(ui, def, &ed.reg, &theme);
                            }
                        });
                    });
                });

                ui.painter().hline(ui.min_rect().x_range(), ui.min_rect().bottom(), Stroke::new(1.0, theme.line));
                Frame::new().fill(theme.panel_header).inner_margin(Margin::symmetric(12, 6)).corner_radius(egui::CornerRadius { nw: 0, ne: 0, sw: 10, se: 10 }).show(ui, |ui| {
                    ui.set_width(size.x - 24.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(14.0, 0.0);
                        for hint in ["Enter: place", "Up/Down: move", "Esc: close"] {
                            ui.label(RichText::new(hint).size(11.0).color(theme.text_muted));
                        }
                    });
                });
            });
    });

    // Clicking anywhere else closes it.
    let clicked_outside = ctx.input(|i| i.pointer.any_pressed())
        && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area.response.rect.contains(p))
        && picker.opened_frames > 2;
    close |= clicked_outside;

    if let Some((block_id, pin)) = chosen {
        let picker = ed.picker.take().unwrap();
        place(ed, &picker, &block_id, pin);
    } else if close {
        ed.picker = None;
    }
}

fn details(ui: &mut egui::Ui, def: &BlockDef, reg: &Registry, theme: &Theme) {
    ui.label(RichText::new(&def.name).strong().size(14.0));
    ui.label(RichText::new(format!("by {} · v{} · {}", def.author, def.version, def.plugin)).size(11.0).color(theme.text_muted));
    ui.label(&def.description);
    let fields: Vec<String> = def.fields.iter().filter(|(_, f)| !f.advanced).map(|(k, f)| f.label(k)).collect();
    let outputs: Vec<String> = def.outputs.iter().map(|(k, o)| format!("{} ({})", o.label(k), o.ty.display(reg))).collect();
    if !fields.is_empty() {
        ui.label(RichText::new(format!("In: {}", fields.join(", "))).size(11.5).color(theme.text_muted));
    }
    if !outputs.is_empty() {
        ui.label(RichText::new(format!("Out: {}", outputs.join(", "))).size(11.5).color(theme.text_muted));
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
