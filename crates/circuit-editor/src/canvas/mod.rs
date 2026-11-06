//! The node canvas: pan/zoom, blocks, pins, wires and wire dragging.

pub mod picker;

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use circuit_core::graph::ACTION_IN;
use circuit_core::{BlockDef, BlockKind, BlockShape, Compat, Flow, Link, Note, PinMode, PinRef, PinType, Workspace};
use circuit_plugins::Registry;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Key, LayerId, PointerButton, Pos2, Rect, Response, RichText, Sense,
    Shadow, Shape, Stroke, StrokeKind, Ui, UiBuilder, Vec2, emath::TSTransform, epaint::CubicBezierShape, pos2, vec2,
};
use serde_json::Value;

use crate::ui::kit;
use crate::editor::{Editor, node_shape};
use crate::canvas::picker::{PendingWire, Picker};
use crate::ui::theme::{Theme, mix, with_alpha};
use crate::ui::widgets;

mod layout;
use layout::*;
mod arrange;
pub use arrange::*;
mod notes;
use notes::*;
mod wires;
pub use wires::*;
mod draw;
pub use draw::*;
#[cfg(test)]
mod tests;

pub const NODE_W: f32 = 240.0;
/// Blocks with an embed preview are wider, so the preview stays readable.
const EMBED_NODE_W: f32 = 300.0;
const EMBED_SCALE: f32 = 0.86;
pub const HEADER_H: f32 = 34.0;
pub const VALUE_HEADER_H: f32 = 30.0;
pub const ROW_H: f32 = 28.0;
const PAD_TOP: f32 = 6.0;
const PAD_BOTTOM: f32 = 8.0;
const PIN_R: f32 = 5.5;
/// Text boxes on blocks: how tall they start (two lines, five for long text)
/// and how far they can be resized.
const TEXT_BOX_H: f32 = 42.0;
const TEXT_AREA_H: f32 = 94.0;
const TEXT_BOX_RANGE: (f32, f32) = (30.0, 900.0);
/// How narrow and wide a block can be made with a text box's grip.
const NODE_WIDTH_RANGE: (f32, f32) = (200.0, 1400.0);
/// A layout field lists up to this many parts, a line each.
const LAYOUT_LINES: usize = 6;
const LAYOUT_LINE_H: f32 = 17.0;
const GRID: f32 = 22.0;
/// How far outside the view a block still counts as in view.
const OFF_SCREEN_MARGIN: f32 = 48.0;
const ZOOM_RANGE: (f32, f32) = (0.25, 2.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    In,
    Out,
}

/// One end of a wire being dragged.
#[derive(Clone, Debug)]
pub struct PinEnd {
    pub node: String,
    pub pin: String,
    pub dir: Dir,
    pub ty: PinType,
    /// A wire picked up from its input: dropped on empty canvas it is deleted,
    /// Esc puts it back.
    pub picked_up: Option<Link>,
}

/// Pan and zoom of one workspace canvas, plus a wire being dragged.
#[derive(Default)]
pub struct CanvasView {
    /// Each block's layout from an earlier frame, with what it was made from
    /// (a fingerprint, and the shape): used again while those stay the same.
    layout_cache: HashMap<String, (u64, Option<std::sync::Arc<BlockShape>>, NodeLayout)>,
    /// Screen offset of scene origin from the canvas' top-left corner.
    pub offset: Vec2,
    pub zoom: f32,
    pub initialized: bool,
    pub drag: Option<PinEnd>,
    /// Scene position where a box selection started.
    pub box_start: Option<Pos2>,
    /// Last pointer position over this canvas, in scene coordinates (used for pasting).
    pub pointer: Option<Pos2>,
    /// A block to center on the next time this canvas is drawn.
    pub focus: Option<String>,
    /// Zone outlines from when the current block drag started.
    pub drag_zones: Vec<(String, Rect)>,
    /// Screen rectangles of the text boxes drawn last frame: (block, field) → box.
    pub text_fields: Vec<((String, String), Rect)>,
    /// How tall each field's control was last drawn, so its row fits it:
    /// (block, field) → height.
    pub field_heights: HashMap<(String, String), f32>,
    /// Where the canvas' top-left corner was, and in which frame.
    pub last_origin: Option<(Pos2, u64)>,
    /// A note whose editor opens the next time this canvas is drawn (a new one).
    pub edit_note: Option<String>,
}

impl CanvasView {
    /// A view at 100%, before it was first drawn.
    pub fn at_full_size() -> CanvasView {
        CanvasView { zoom: 1.0, ..Default::default() }
    }

    fn to_global(&self, origin: Pos2) -> TSTransform {
        TSTransform::from_translation(origin.to_vec2() + self.offset) * TSTransform::from_scaling(self.zoom)
    }

    fn zoom_at(&mut self, origin: Pos2, pointer: Pos2, factor: f32) {
        let scene = (pointer - origin - self.offset) / self.zoom;
        self.zoom = (self.zoom * factor).clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
        self.offset = pointer - origin - scene * self.zoom;
    }
}

enum NodeMenu {
    Delete(String),
    Duplicate(String),
    OpenSource(String),
    WrapInZone(String),
    LeaveZone(String),
    RemoveZone(String),
    NoteAround(String),
    Arrange(String, Arrange),
}

pub fn show(ui: &mut Ui, ed: &mut Editor, ws_key: &str) {
    let Editor { reg, engine, theme, project, shapes, views, selected, edited, picker, active_workspace, activity, wire_values, multi, selected_notes, recording_frame, define_ctx, embed_edit, layout_edit, canvas_edit, selected_wire, prefs, ui_rects, tour_parts, .. } = ed;
    let Some(project) = project.as_mut() else { return };
    let mode = reg.mode(&project.meta.mode).or_else(|| reg.modes.first());
    let Some(ws) = project.workspaces.get_mut(ws_key) else {
        ui.label(t!("This workspace no longer exists."));
        return;
    };
    let view = views.entry(ws_key.to_string()).or_insert_with(CanvasView::at_full_size);

    // Shapes and layouts for every node, in scene coordinates.
    let measure = ui.painter().clone();
    let ids: Vec<String> = ws.nodes.keys().cloned().collect();
    let mut layouts: Vec<NodeLayout> = Vec::with_capacity(ids.len());
    let mut node_shapes: HashMap<&str, std::sync::Arc<BlockShape>> = HashMap::new();
    shapes.begin();
    // Measured control heights by block, so each block finds its own quickly.
    let mut heights_of: HashMap<&str, Vec<(&str, f32)>> = HashMap::new();
    for ((n, f), h) in &view.field_heights {
        heights_of.entry(n.as_str()).or_default().push((f.as_str(), *h));
    }
    // Scene sizes depend on the text size too.
    let text_scale = (ui.ctx().pixels_per_point().to_bits(), theme.touch);
    for id in &ids {
        let node = &ws.nodes[id];
        let def = reg.block(&node.block);
        let shape = node_shape(reg, engine, shapes, ws_key, ws, id, define_ctx);
        // Everything the layout is made from besides the shape: where the
        // block is, its width, pins, resized text boxes and measured rows.
        let mut h = crate::editor::Fingerprint::default();
        (node.at[0].to_bits(), node.at[1].to_bits(), node.width.map(f32::to_bits), &node.pins, text_scale).hash(&mut h);
        for (k, v) in &node.heights {
            (k, v.to_bits()).hash(&mut h);
        }
        for (k, v) in heights_of.get(id.as_str()).into_iter().flatten() {
            (k, v.to_bits()).hash(&mut h);
        }
        let key = h.finish();
        let same_shape = |cached: &Option<std::sync::Arc<BlockShape>>| match (cached, &shape) {
            (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if let Some((_, _, layout)) = view.layout_cache.get(id).filter(|(k, s, _)| *k == key && same_shape(s)) {
            layouts.push(layout.clone());
            if let Some(s) = shape {
                node_shapes.insert(id.as_str(), s);
            }
            continue;
        }
        let mut tall = HashMap::new();
        if let Some(s) = &shape {
            // Text fields that are typed into: a label line with the box under it.
            for (key, f) in s.visible_fields().filter(|(k, f)| widgets::is_plain_text(f) && f.pin != PinMode::Only && !node.pins.contains(k)) {
                tall.insert(key.clone(), ROW_H - 4.0 + text_box_h(node, key, f) + 8.0);
            }
        }
        if let Some(s) = &shape {
            for (key, f) in s.visible_fields().filter(|(_, f)| f.type_name == "layout") {
                let lines = crate::builders::layout::summary(node.values.get(key).or(f.default.as_ref()).unwrap_or(&Value::Null)).len().clamp(1, LAYOUT_LINES);
                tall.insert(key.clone(), 8.0 + lines as f32 * LAYOUT_LINE_H + 8.0 + 26.0 + 8.0);
            }
        }
        if let (Some(s), Some(d)) = (&shape, def) {
            for (key, f) in s.visible_fields().filter(|(_, f)| f.type_name == "canvas") {
                let data = crate::builders::canvas::CanvasData::from_value(node.values.get(key).or(f.default.as_ref()).unwrap_or(&Value::Null));
                let w = node.width.unwrap_or_else(|| default_width(d)) - 28.0;
                tall.insert(key.clone(), 4.0 + crate::builders::canvas::block_height(&data, w) + 8.0 + 26.0 + 8.0);
            }
        }
        if let (Some(s), Some(d)) = (&shape, def) {
            for (key, f) in s.visible_fields().filter(|(_, f)| f.type_name == "embed") {
                let data = crate::builders::embed::EmbedData::from_value(node.values.get(key).or(f.default.as_ref()).unwrap_or(&Value::Null));
                let w = node.width.unwrap_or_else(|| default_width(d)) - 28.0;
                tall.insert(key.clone(), crate::builders::embed::layout(&measure, &data, w, Some(4), EMBED_SCALE).height + 44.0);
            }
        }
        // Rows as tall as their control, measured when last drawn.
        let mut fit = HashMap::new();
        for (key, h) in heights_of.get(id.as_str()).into_iter().flatten() {
            if !tall.contains_key(*key) && *h + 8.0 > ROW_H {
                fit.insert(key.to_string(), h + 8.0);
            }
        }
        let layout = layout_node(id, node.at, &node.pins, node.width, def, shape.as_deref(), &tall, &fit);
        view.layout_cache.insert(id.clone(), (key, shape.clone(), layout.clone()));
        layouts.push(layout);
        if let Some(s) = shape {
            node_shapes.insert(id.as_str(), s);
        }
    }
    // Blocks that are gone.
    if view.layout_cache.len() > ids.len() {
        view.layout_cache.retain(|id, _| ws.nodes.contains_key(id));
    }
    // Try Zones fit around their blocks; their outputs act like a block's pins.
    let node_count = layouts.len();
    let fitted = zone_rects(ws, &layouts);
    for (zid, rect) in &fitted {
        if let Some(z) = ws.zones.get_mut(zid) {
            z.rect = [rect.min.x, rect.min.y, rect.width(), rect.height()];
        }
        layouts.push(zone_layout(zid, *rect));
    }
    let pin_index: HashMap<(&str, &str, Dir), usize> = layouts
        .iter()
        .enumerate()
        .flat_map(|(li, l)| l.pins.iter().enumerate().map(move |(pi, p)| ((l.id.as_str(), p.pin.as_str(), p.dir), li * 1000 + pi)))
        .collect();
    let pin_at = |node: &str, pin: &str, dir: Dir| pin_index.get(&(node, pin, dir)).map(|i| &layouts[i / 1000].pins[i % 1000]);

    let (outer, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    ui.painter().rect_filled(outer, 0.0, theme.canvas);

    // When the canvas' edge moves (the side bar opens or closes, a panel next
    // to it is resized), its blocks stay where they are on screen.
    let frame = ui.ctx().cumulative_pass_nr();
    if let Some((before, drawn)) = view.last_origin
        && drawn + 1 == frame
        && view.initialized
    {
        view.offset -= outer.min - before;
    }
    view.last_origin = Some((outer.min, frame));

    if !view.initialized {
        fit_view(view, outer, &layouts[..node_count], ws);
    }
    if let Some(id) = view.focus.take()
        && let Some(l) = layouts.iter().find(|l| l.id == id)
    {
        view.offset = outer.size() / 2.0 - l.rect.center().to_vec2() * view.zoom;
    }

    // Pan and zoom.
    let ctx = ui.ctx().clone();
    let pointer = ctx.input(|i| i.pointer.hover_pos());
    let parent_layer = ui.layer_id();
    let layer = LayerId::new(parent_layer.order, ui.id().with(("canvas", ws_key)));
    let over_canvas = pointer.is_some_and(|p| {
        outer.contains(p) && ctx.layer_id_at(p).is_none_or(|l| l == layer || l == parent_layer)
    });
    if over_canvas && (active_workspace.is_none() || ctx.input(|i| i.pointer.any_pressed())) {
        *active_workspace = Some(ws_key.to_string());
    }
    if over_canvas && picker.is_none() {
        let p = pointer.unwrap();
        let (zoom_delta, scroll, shift) = ctx.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.modifiers.shift));
        if zoom_delta != 1.0 {
            view.zoom_at(outer.min, p, zoom_delta);
        } else if scroll.y != 0.0 && !shift {
            view.zoom_at(outer.min, p, (scroll.y / 500.0).exp());
        }
        if scroll.x != 0.0 {
            view.offset.x += scroll.x;
        }
    }

    let to_global = view.to_global(outer.min);
    let visible = to_global.inverse() * outer;
    ctx.set_sublayer(parent_layer, layer);
    ctx.set_transform_layer(layer, to_global);
    let mut cui = ui.new_child(UiBuilder::new().layer_id(layer).max_rect(visible));
    crate::ui::theme::compact(cui.style_mut());
    cui.set_clip_rect(visible);
    // Registered before any block, so blocks and pins sit on top of it.
    let bg = cui.interact(visible, cui.id().with("background"), Sense::click_and_drag());
    let pointer_scene = pointer.map(|p| to_global.inverse() * p);
    if over_canvas {
        view.pointer = pointer_scene;
    }
    // Shift- or Ctrl-drag on empty canvas draws a selection box; a plain drag pans.
    let boxing = ctx.input(|i| i.modifiers.shift || i.modifiers.command);
    if bg.drag_started_by(PointerButton::Primary) && boxing {
        view.box_start = pointer_scene;
    }
    let mut selection_box: Option<Rect> = None;
    if let Some(start) = view.box_start {
        if let Some(p) = pointer_scene {
            selection_box = Some(Rect::from_two_pos(start, p));
        }
        if bg.drag_stopped() || !ctx.input(|i| i.pointer.primary_down()) {
            view.box_start = None;
            if let Some(b) = selection_box {
                let hits: Vec<String> = layouts[..node_count].iter().filter(|l| l.rect.intersects(b)).map(|l| l.id.clone()).collect();
                let keep = selected.as_ref().is_some_and(|(w, _)| w == ws_key);
                if !keep {
                    multi.clear();
                    selected_notes.retain(|(w, _)| w != ws_key);
                }
                // Notes the box holds whole.
                selected_notes.extend(ws.notes.iter().filter(|(_, n)| b.contains_rect(note_rect(n))).map(|(id, _)| (ws_key.to_string(), id.clone())));
                multi.extend(hits.iter().cloned());
                *selected = multi.iter().next().map(|n| (ws_key.to_string(), n.clone()));
            }
        }
    } else if bg.dragged_by(PointerButton::Primary) || bg.dragged_by(PointerButton::Middle) {
        view.offset += bg.drag_delta() * view.zoom;
        ctx.set_cursor_icon(CursorIcon::Grabbing);
    }
    let painter = cui.painter().clone();

    if prefs.show_grid {
        draw_grid(&painter, visible, theme, view.zoom);
    }
    let recent = |id: &str| activity.get(&(ws_key.to_string(), id.to_string()));
    let glow = |id: &str| recent(id).map(|a| a.glow()).unwrap_or(0.0);

    // How big a pin is to the pointer (in touch mode, about a fingertip).
    let pin_hit = if theme.touch { 30.0 } else { 16.0 };
    let zone_button = if theme.touch { ZONE_BUTTON_TOUCH } else { ZONE_BUTTON };

    // Try Zones: drawn behind everything, dragged by their title band.
    let mut menu: Option<NodeMenu> = None;
    let mut action_bar: Option<BarFor> = None;
    let mut start_drag: Option<PinEnd> = None;
    let drag = view.drag.clone();
    // While a wire is dragged: what follows what, so pins that would close a loop refuse it.
    let flow = drag.as_ref().map(|_| ws.flow(|id| is_value_block(ws, reg, id)));
    draw_notes(&cui, &painter, ws, ws_key, &layouts[..node_count], theme, reg, view, edited, NoteSelection { blocks: selected, multi, notes: selected_notes }, prefs.snap_to_grid);

    for layout in &layouts[node_count..] {
        let zid = layout.id.clone();
        let r = layout.rect;
        let failed = recent(&zid).and_then(|a| a.error.clone());
        let edge = if failed.is_some() { theme.error } else { mix(with_alpha(theme.accent, 150), theme.action, glow(&zid)) };
        painter.rect(r, 16.0, with_alpha(theme.accent, 14), Stroke::new(1.5, edge), StrokeKind::Inside);
        painter.rect_filled(layout.header, CornerRadius { nw: 16, ne: 16, sw: 0, se: 0 }, with_alpha(theme.accent, 36));
        let (title, retry, timeout) = ws.zones.get(&zid).map(|z| (z.title.clone(), z.retry, z.timeout_ms)).unwrap_or_default();
        painter.text(layout.header.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, tf!("Try · {title}", title = title), FontId::proportional(12.5), theme.accent);
        let mut extra = Vec::new();
        if retry > 0 {
            extra.push(tf!("retry {count}×", count = retry));
        }
        if let Some(t) = timeout {
            extra.push(tf!("{seconds} s timeout", seconds = format!("{:.0}", t as f32 / 1000.0)));
        }
        // Left of the settings button, which is drawn on top in screen space.
        painter.text(layout.header.right_center() - vec2(14.0 + zone_button / view.zoom, 0.0), Align2::RIGHT_CENTER, extra.join(" · "), FontId::proportional(11.0), theme.text_muted);
        if let Some(err) = &failed {
            painter.text(r.left_bottom() + vec2(14.0, -16.0), Align2::LEFT_CENTER, format!("Caught: {err}"), FontId::proportional(11.5), theme.error);
        }

        let band = cui.interact(layout.header, cui.id().with(("zone", &zid)), Sense::click_and_drag());
        if band.hovered() {
            ctx.set_cursor_icon(CursorIcon::Grab);
        }
        if band.dragged_by(PointerButton::Primary) {
            let d = band.drag_delta();
            for n in ws.nodes.values_mut().filter(|n| n.zone.as_deref() == Some(zid.as_str())) {
                n.at[0] += d.x;
                n.at[1] += d.y;
            }
            *edited = true;
        }
        band.on_hover_text(t!("A Try Zone catches errors from the blocks inside it. The gear button sets its title, retries and timeout."));

        // The settings button, on the right of the title band. On the canvas
        // (not over it), so blocks and their buttons cover it; as big on
        // screen at every zoom.
        let side = if theme.touch { zone_button } else { zone_button.min(layout.header.height() * view.zoom - 2.0).max(18.0) } / view.zoom;
        let gr = Rect::from_center_size(layout.header.right_center() - vec2(side / 2.0 + 6.0 / view.zoom, 0.0), Vec2::splat(side));
        let gear = cui.interact(gr, cui.id().with(("zone-settings", &zid)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let fill = if gear.hovered() { mix(theme.panel, theme.accent, 0.25) } else { theme.panel };
        painter.rect(gr, 6.0 / view.zoom, fill, Stroke::new(1.0 / view.zoom, with_alpha(theme.accent, 120)), StrokeKind::Inside);
        if let Some(path) = reg.icons.get("settings") {
            egui::Image::from_uri(crate::editor::file_uri(path)).tint(theme.accent).paint_at(&cui, gr.shrink(gr.width() * 0.2));
        }
        let gear = gear.on_hover_text(t!("Try Zone settings"));
        egui::Popup::menu(&gear).width(240.0).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            let Some(z) = ws.zones.get_mut(&zid) else { return };
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(RichText::new(t!("Try Zone")).strong());
            ui.label(t!("Title"));
            *edited |= ui.add(crate::ui::kit::TextLine::new(&mut z.title)).changed();
            ui.horizontal(|ui| {
                ui.label(t!("Retries"));
                *edited |= kit::Number::new(&mut z.retry).range(0..=10).show(ui, theme).changed();
            });
            ui.horizontal(|ui| {
                ui.label(t!("Timeout (s)"));
                let mut secs = z.timeout_ms.map(|t| t as f64 / 1000.0).unwrap_or(0.0);
                if kit::Number::new(&mut secs).range(0.0..=600.0).speed(0.5).show(ui, theme).changed() {
                    z.timeout_ms = (secs > 0.0).then_some((secs * 1000.0) as u64);
                    *edited = true;
                }
            });
            ui.separator();
            if crate::ui::kit::button(ui, theme, t!("Remove Try Zone")).clicked() {
                menu = Some(NodeMenu::RemoveZone(zid.clone()));
                ui.close();
            }
        });

        for spot in layout.pins.iter() {
            let resp = cui.interact(Rect::from_center_size(spot.pos, Vec2::splat(pin_hit)), cui.id().with(("zpin", &zid, &spot.pin)), Sense::click_and_drag());
            if resp.drag_started() {
                start_drag = Some(PinEnd { node: zid.clone(), pin: spot.pin.clone(), dir: Dir::Out, ty: spot.ty.clone(), picked_up: None });
            }
            resp.on_hover_ui(|ui| pin_tooltip(ui, spot, reg, theme));
        }
    }

    // Wires. Action wires light up while the bot runs through them. The wire
    // under the pointer (on empty canvas) can be clicked or right-clicked to select it.
    let over_block = pointer_scene.is_some_and(|p| layouts[..node_count].iter().any(|l| l.rect.expand(8.0).contains(p)));
    let wire_pick = (bg.hovered() && view.drag.is_none() && !over_block).then_some(pointer_scene).flatten();
    let reach = if theme.touch { 14.0 } else { 6.0 } / view.zoom.max(0.3);
    let mut hovered_wire: Option<Link> = None;
    for link in &ws.links {
        let a = pin_at(&link.from.node, &link.from.pin, Dir::Out);
        let b = pin_at(&link.to.node, &link.to.pin, Dir::In);
        if let (Some(a), Some(b)) = (a, b)
            && let Some(p) = wire_pick
            && wire_distance(a.pos, b.pos, p) < reach
        {
            hovered_wire = Some(link.clone());
        }
    }
    let wire_selected = |l: &Link| selected_wire.as_ref().is_some_and(|(w, s)| w == ws_key && s == l);
    // Where the selected wire's Delete button goes (screen).
    let mut wire_mid: Option<Pos2> = None;
    for link in &ws.links {
        let a = pin_at(&link.from.node, &link.from.pin, Dir::Out);
        let b = pin_at(&link.to.node, &link.to.pin, Dir::In);
        if let (Some(a), Some(b)) = (a, b) {
            let lit = if a.ty.is_action() { glow(&link.to.node) } else { 0.0 };
            if wire_selected(link) {
                // A wire's curve is symmetric, so its middle is halfway between its ends.
                wire_mid = Some(to_global * pos2((a.pos.x + b.pos.x) / 2.0, (a.pos.y + b.pos.y) / 2.0));
            }
            if wire_selected(link) || hovered_wire.as_ref() == Some(link) {
                let strength = if wire_selected(link) { 110 } else { 60 };
                painter.add(CubicBezierShape::from_points_stroke(wire_points(a.pos, b.pos), false, Color32::TRANSPARENT, Stroke::new(theme.wire_width + 6.0, with_alpha(theme.accent, strength))));
            }
            draw_wire(&painter, a.pos, b.pos, &a.ty, theme, reg, lit);
        }
    }
    if let Some(link) = &hovered_wire {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
        // The last value that went through it, while the bot ran.
        let is_action = pin_at(&link.from.node, &link.from.pin, Dir::Out).is_some_and(|p| p.ty.is_action());
        if !is_action {
            let last = wire_values.get(&(ws_key.to_string(), link.from.node.clone(), link.from.pin.clone()));
            egui::Tooltip::always_open(ctx.clone(), parent_layer, cui.id().with("wire-value"), egui::PopupAnchor::Pointer).show(|ui| {
                ui.set_max_width(360.0);
                match last {
                    Some((value, at)) => {
                        ui.label(RichText::new(t!("Last value")).size(11.0).color(theme.text_muted));
                        ui.add(egui::Label::new(RichText::new(value).monospace().size(12.0).color(theme.text)).wrap());
                        ui.label(RichText::new(crate::bot::ago(at.elapsed())).size(11.0).color(theme.text_muted));
                    }
                    None => {
                        kit::hint(ui, theme, t!("No value yet: run the bot and this shows the last value that went through this wire."));
                    }
                }
            });
        }
    }

    // Zones' outputs panels, in their bottom-right corners: over the wires,
    // as blocks are, with their pins on top.
    for layout in &layouts[node_count..] {
        let r = layout.rect;
        let panel = Rect::from_min_max(pos2(r.right() - ZONE_PANEL_W, r.bottom() - ZONE_PANEL_H - 8.0), pos2(r.right(), r.bottom() - 8.0));
        painter.rect(panel, CornerRadius { nw: 9, ne: 0, sw: 9, se: 0 }, theme.node, Stroke::new(1.0, theme.node_border), StrokeKind::Inside);
        for spot in layout.pins.iter() {
            let color = if spot.pin == "error" { theme.error } else if spot.ty.is_action() { theme.text } else { theme.text_muted };
            painter.text(spot.pos - vec2(16.0, 0.0), Align2::RIGHT_CENTER, &spot.label, FontId::proportional(12.0), color);
            let connected = ws.links.iter().any(|l| l.from.node == layout.id && l.from.pin == spot.pin);
            let candidate = drag.as_ref().map(|d| drag_compat(d, &layout.id, spot, reg, flow.as_ref()));
            draw_pin(&painter, spot, connected, candidate, theme, reg);
        }
    }

    // Blocks.
    let mut toggle_pin: Option<(String, String)> = None;
    let mut open_embed_for: Option<(String, String)> = None;
    let mut dropped: Option<String> = None;
    let mut drag_zones: Option<Vec<(String, Rect)>> = None;
    view.text_fields.clear();
    for layout in &layouts[..node_count] {
        // A block out of view builds none of its controls: its wires and pins
        // were placed from its layout, and dragging one of several selected
        // blocks moves the others from the dragged one. The margin covers its
        // shadow, glow and error note.
        if !layout.rect.expand(OFF_SCREEN_MARGIN).intersects(visible) {
            continue;
        }
        let id = &layout.id;
        let is_selected = selected.as_ref().is_some_and(|(w, n)| w == ws_key && (n == id || multi.contains(id)));
        let (block_ref, def) = {
            let node = &ws.nodes[id];
            (node.block.clone(), reg.block(&node.block))
        };
        let radius = if layout.value_block { 14.0 } else { theme.node_radius };

        painter.add(Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(if theme.dark { 90 } else { 30 }) }.as_shape(layout.rect, radius));
        let failed = recent(id).and_then(|a| a.error.clone());
        let lit = glow(id);
        let border = if failed.is_some() {
            Stroke::new(2.0, theme.error)
        } else if is_selected {
            Stroke::new(2.0, theme.accent)
        } else {
            Stroke::new(1.0, theme.node_border)
        };
        if lit > 0.0 {
            painter.rect_stroke(layout.rect.expand(3.0), radius + 3.0, Stroke::new(3.0, with_alpha(theme.action, (lit * 200.0) as u8)), StrokeKind::Outside);
        }
        painter.rect(layout.rect, radius, theme.node, border, StrokeKind::Inside);
        // How long it took the last time it ran, above its top-right corner, for a while.
        if def.is_some_and(|d| d.kind != BlockKind::Event)
            && let Some((ms, alpha)) = recent(id).and_then(|a| a.time_shown())
        {
            let color = with_alpha(theme.text_muted, (alpha * 255.0) as u8);
            let galley = painter.layout_no_wrap(crate::bot::format_ms(ms), FontId::monospace(10.5), color);
            let pill = Rect::from_min_size(layout.rect.right_top() - vec2(galley.size().x + 12.0, galley.size().y + 10.0), galley.size() + vec2(12.0, 6.0));
            painter.rect_filled(pill, 6.0, with_alpha(theme.panel_header, (alpha * 230.0) as u8));
            painter.galley(pill.min + vec2(6.0, 3.0), galley, color);
        }
        if let Some(err) = &failed {
            let text = if err.len() > 60 { format!("{}…", &err[..err.char_indices().nth(60).map(|(i, _)| i).unwrap_or(err.len())]) } else { err.clone() };
            let galley = painter.layout_no_wrap(text, FontId::proportional(11.5), Color32::WHITE);
            let pill = Rect::from_min_size(layout.rect.left_bottom() + vec2(0.0, 6.0), galley.size() + vec2(16.0, 8.0));
            painter.rect_filled(pill, 6.0, theme.error);
            painter.galley(pill.min + vec2(8.0, 4.0), galley, Color32::WHITE);
        }
        let r8 = radius as u8;
        painter.rect_filled(layout.header.shrink(if is_selected { 2.0 } else { 1.0 }), CornerRadius { nw: r8, ne: r8, sw: 0, se: 0 }, theme.node_header);
        painter.hline(layout.header.x_range(), layout.header.bottom(), Stroke::new(1.0, theme.node_border));

        let body = cui.interact(layout.rect, cui.id().with(("body", id)), Sense::click_and_drag());

        let Some(def) = def else {
            painter.text(layout.header.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, t!("Missing block"), FontId::proportional(13.0), theme.error);
            painter.text(layout.header.left_bottom() + vec2(14.0, ROW_H / 2.0 + 4.0), Align2::LEFT_CENTER, &block_ref, FontId::monospace(11.0), theme.text_muted);
            if body.clicked() || body.secondary_clicked() {
                *selected = Some((ws_key.to_string(), id.clone()));
                *multi = std::iter::once(id.clone()).collect();
            }
            if is_selected {
                action_bar = Some(BarFor { node: id.clone(), rect: to_global * layout.rect, in_zone: false, can_wrap: false, source: None });
            }
            continue;
        };

        // Header: icon, title, id.
        let icon_rect = Rect::from_center_size(layout.header.left_center() + vec2(22.0, 0.0), vec2(16.0, 16.0));
        if let Some(path) = reg.block_icon(def, mode) {
            egui::Image::from_uri(crate::editor::file_uri(&path)).tint(theme.text_muted).paint_at(&cui, icon_rect);
        }
        painter.text(
            pos2(icon_rect.right() + 8.0, layout.header.center().y),
            Align2::LEFT_CENTER,
            &def.name,
            FontId::proportional(13.5),
            theme.text,
        );
        painter.text(
            layout.header.right_center() - vec2(16.0, 0.0),
            Align2::RIGHT_CENTER,
            format!("#{id}"),
            FontId::monospace(10.5),
            theme.text_muted,
        );

        // Rows.
        let shape = node_shapes.get(id.as_str()).map(|s| &**s);
        let mut open_embed: Option<String> = None;
        let mut open_layout: Option<String> = None;
        let mut open_canvas: Option<String> = None;
        for (row, r) in layout.rows.iter() {
            // Hovering a row's name shows what it is, like hovering its pin.
            let (tip_key, tip_dir, tip_rect) = match row {
                RowKind::Flow(f) => (f, Dir::Out, *r),
                RowKind::Output(k) => (k, Dir::Out, *r),
                RowKind::Field { key, .. } => {
                    let label = shape.and_then(|s| s.field(key)).map(|f| f.label(key)).unwrap_or_else(|| key.clone());
                    let w = (r.width() * 0.4).min(label.len() as f32 * 7.0 + 8.0);
                    (key, Dir::In, Rect::from_min_max(r.min, pos2(r.left() + 16.0 + w, r.bottom())))
                }
            };
            if let Some(spot) = layout.pins.iter().find(|p| p.pin == *tip_key && p.dir == tip_dir) {
                cui.interact(tip_rect, cui.id().with(("row-tip", id, tip_key, tip_dir)), Sense::hover()).on_hover_ui(|ui| pin_tooltip(ui, spot, reg, theme));
            }
            match row {
                RowKind::Flow(f) => {
                    painter.text(r.right_center() - vec2(16.0, 0.0), Align2::RIGHT_CENTER, circuit_core::block::title_case(f), FontId::proportional(12.5), theme.text);
                }
                RowKind::Output(k) => {
                    let o = shape.and_then(|s| s.output(k));
                    let label = o.map(|o| o.label(k)).unwrap_or_else(|| k.clone());
                    painter.text(r.right_center() - vec2(16.0, 0.0), Align2::RIGHT_CENTER, label, FontId::proportional(12.5), theme.text_muted);
                }
                RowKind::Field { key, .. } if shape.and_then(|s| s.field(key)).is_some_and(|f| f.type_name == "embed") => {
                    let f = shape.and_then(|s| s.field(key)).unwrap();
                    let data = crate::builders::embed::EmbedData::from_value(ws.nodes[id].values.get(key).or(f.default.as_ref()).unwrap_or(&Value::Null));
                    let w = r.width() - 28.0;
                    let l = crate::builders::embed::layout(&painter, &data, w, Some(4), EMBED_SCALE);
                    crate::builders::embed::paint(&painter, r.min + vec2(14.0, 4.0), w, &l);
                    let button = Rect::from_min_size(pos2(r.left() + 14.0, r.top() + 4.0 + l.height + 8.0), vec2(w, 26.0));
                    if cui.put(button, egui::Button::new(t!("Edit embed…"))).clicked() {
                        open_embed = Some(key.clone());
                    }
                }
                RowKind::Field { key, .. } if shape.and_then(|s| s.field(key)).is_some_and(|f| f.type_name == "canvas") => {
                    let f = shape.and_then(|s| s.field(key)).unwrap();
                    let data = crate::builders::canvas::CanvasData::from_value(ws.nodes[id].values.get(key).or(f.default.as_ref()).unwrap_or(&Value::Null));
                    let w = r.width() - 28.0;
                    crate::builders::canvas::paint_on_block(&painter, r.min + vec2(14.0, 4.0), w, &data);
                    let button = Rect::from_min_size(pos2(r.left() + 14.0, r.top() + 4.0 + crate::builders::canvas::block_height(&data, w) + 8.0), vec2(w, 26.0));
                    if cui.put(button, egui::Button::new(t!("Edit canvas…"))).clicked() {
                        open_canvas = Some(key.clone());
                    }
                }
                RowKind::Field { key, .. } if shape.and_then(|s| s.field(key)).is_some_and(|f| f.type_name == "layout") => {
                    let f = shape.and_then(|s| s.field(key)).unwrap();
                    let lines = crate::builders::layout::summary(ws.nodes[id].values.get(key).or(f.default.as_ref()).unwrap_or(&Value::Null));
                    let shown = lines.len().clamp(1, LAYOUT_LINES);
                    let w = r.width() - 28.0;
                    let list = Rect::from_min_size(r.min + vec2(14.0, 4.0), vec2(w, 8.0 + shown as f32 * LAYOUT_LINE_H));
                    painter.rect_filled(list, 6.0, Color32::from_rgb(0x2B, 0x2D, 0x31));
                    if lines.is_empty() {
                        painter.text(list.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, t!("Empty: add parts in the builder"), FontId::proportional(11.5), theme.text_muted);
                    }
                    for (i, l) in lines.iter().take(LAYOUT_LINES).enumerate() {
                        let text = if i + 1 == LAYOUT_LINES && lines.len() > LAYOUT_LINES { tf!("… and {count} more", count = lines.len() - i) } else { l.clone() };
                        painter.text(list.left_top() + vec2(8.0, 4.0 + (i as f32 + 0.5) * LAYOUT_LINE_H), Align2::LEFT_CENTER, text, FontId::proportional(11.5), Color32::from_rgb(0xDB, 0xDE, 0xE1));
                    }
                    let button = Rect::from_min_size(pos2(r.left() + 14.0, list.bottom() + 8.0), vec2(w, 26.0));
                    if cui.put(button, egui::Button::new(t!("Edit layout…"))).clicked() {
                        open_layout = Some(key.clone());
                    }
                }
                RowKind::Field { key, pinned } => {
                    let Some(f) = shape.and_then(|s| s.field(key)) else { continue };
                    let connected = ws.is_connected(id, key);
                    let node_values = &ws.nodes[id].values;
                    let empty = node_values.get(key).or(f.default.as_ref()).is_none_or(|v| v.is_null() || v == &Value::String(String::new()));
                    let missing = f.required && empty && !connected;
                    let label_color = if missing { theme.warning } else { theme.text_muted };
                    let label = f.label(key);
                    let label_w = (r.width() * 0.4).min(label.len() as f32 * 7.0 + 8.0);
                    // On the first line above a text box, else centered on the row's control.
                    let label_y = if widgets::is_plain_text(f) && !*pinned { r.top() + ROW_H / 2.0 } else { r.center().y };
                    painter.text(pos2(r.left() + 16.0, label_y), Align2::LEFT_CENTER, &label, FontId::proportional(12.5), label_color);
                    if *pinned {
                        if !connected {
                            painter.text(r.right_center() - vec2(16.0, 0.0), Align2::RIGHT_CENTER, "connect", FontId::proportional(11.0), with_alpha(theme.text_muted, 140));
                        }
                    } else if widgets::is_plain_text(f) {
                        // Free text: a box under the label, as tall as it was last resized to.
                        let bx = Rect::from_min_max(pos2(r.left() + 14.0, r.top() + ROW_H - 4.0), pos2(r.right() - 14.0, r.bottom() - 8.0));
                        let edit_id = cui.id().with(("field", id, key));
                        let focused = ctx.memory(|m| m.has_focus(edit_id));
                        painter.rect(bx, 6.0, theme.field, Stroke::new(1.0, if focused { theme.accent } else { theme.node_border }), StrokeKind::Inside);
                        let inner = bx.shrink2(vec2(8.0, 5.0));
                        let mut text = widgets::as_text(ws.nodes[id].values.get(key).or(f.default.as_ref()).unwrap_or(&Value::Null));
                        // While it has the text cursor or the pointer, it offers the values of
                        // earlier blocks (after `{` and on right-click).
                        let offer = focused || ctx.pointer_hover_pos().is_some_and(|p| (to_global * bx).contains(p)) || crate::editor::values::menu_open(&ctx, edit_id);
                        if offer {
                            let values = crate::editor::values::available(ws, id, reg, &mut |n| node_shapes.get(n).cloned());
                            crate::editor::values::set_scope(&ctx, Some(std::sync::Arc::new(values)));
                        }
                        crate::editor::values::before(&cui, edit_id);
                        let resp = cui
                            .scope_builder(UiBuilder::new().id_salt(("field-ui", id, key)).max_rect(inner), |ui| {
                                // Longer text scrolls inside the box.
                                egui::ScrollArea::vertical().id_salt("scroll").max_height(inner.height()).auto_shrink([false, false]).show(ui, |ui| {
                                    let mut resp = ui.add(
                                        egui::TextEdit::multiline(&mut text)
                                            .id(edit_id)
                                            .frame(egui::Frame::NONE)
                                            .hint_text(f.placeholder.clone().unwrap_or_default())
                                            .desired_width(inner.width())
                                            .min_size(vec2(inner.width(), inner.height())),
                                    );
                                    if crate::editor::values::after(ui, &resp, &mut text) {
                                        resp.mark_changed();
                                    }
                                    resp
                                })
                                .inner
                            })
                            .inner;
                        if offer {
                            crate::editor::values::set_scope(&ctx, None);
                        }
                        if resp.changed() {
                            ws.nodes.get_mut(id).unwrap().values.insert(key.clone(), Value::String(text));
                            *edited = true;
                        }
                        // The grip in its corner resizes it: down for the box, sideways for
                        // the whole block. A double-click puts both back.
                        let side = if theme.touch { 24.0 } else { 14.0 };
                        let grip = Rect::from_min_size(bx.right_bottom() - vec2(side + 2.0, side + 2.0), vec2(side, side));
                        let g = cui.interact(grip, cui.id().with(("text-grip", id, key)), Sense::click_and_drag()).on_hover_cursor(CursorIcon::ResizeNwSe);
                        let c = if g.hovered() || g.dragged() { theme.accent } else { theme.text_muted };
                        for i in 1..=3 {
                            let o = i as f32 * side / 4.0;
                            painter.line_segment([grip.right_bottom() - vec2(o, 1.0), grip.right_bottom() - vec2(1.0, o)], Stroke::new(1.2, c));
                        }
                        let node = ws.nodes.get_mut(id).unwrap();
                        if g.double_clicked() {
                            node.heights.remove(key);
                            node.width = None;
                            *edited = true;
                        } else if g.dragged() {
                            let d = g.drag_delta();
                            if d.y != 0.0 {
                                let h = (text_box_h(node, key, f) + d.y).clamp(TEXT_BOX_RANGE.0, TEXT_BOX_RANGE.1);
                                node.heights.insert(key.clone(), h.round());
                            }
                            if d.x != 0.0 {
                                let w = (node.width.unwrap_or(layout.rect.width()) + d.x).clamp(NODE_WIDTH_RANGE.0, NODE_WIDTH_RANGE.1);
                                node.width = Some(w);
                            }
                            *edited |= d != Vec2::ZERO;
                        }
                        view.text_fields.push(((id.clone(), key.clone()), to_global * bx));
                    } else if widgets::is_inline_editable(f) {
                        let wr = Rect::from_min_max(pos2(r.left() + 16.0 + label_w + 6.0, r.top() + 3.0), pos2(r.right() - 12.0, r.bottom() - 3.0));
                        let mut value = ws.nodes[id].values.get(key).cloned().or_else(|| f.default.clone()).unwrap_or(Value::Null);
                        let wid = cui.id().with(("field", id, key));
                        // Drawn top-aligned in a box as tall as the control was last time,
                        // centered on the row (egui centers a row's items on its first line,
                        // not on the room given). A fixed id: number boxes keep their editing
                        // state while typing.
                        let field = (id.clone(), key.clone());
                        let place = match view.field_heights.get(&field) {
                            Some(h) => Rect::from_center_size(pos2(wr.center().x, r.center().y), vec2(wr.width(), *h)),
                            None => wr,
                        };
                        let drawn = cui.scope_builder(UiBuilder::new().id_salt(("field-ui", id, key)).max_rect(place).layout(egui::Layout::right_to_left(egui::Align::Min)), |ui| {
                            widgets::value_editor(ui, wid, f, &mut value, wr.width(), false, theme)
                        });
                        // Its own height: the row fits it, and it is centered from the next frame on.
                        view.field_heights.insert(field, drawn.response.rect.height());
                        let changed = drawn.inner;
                        if changed {
                            ws.nodes.get_mut(id).unwrap().values.insert(key.clone(), value);
                            *edited = true;
                        }
                    }
                }
            }
        }

        if let Some(k) = open_embed {
            open_embed_for = Some((id.clone(), k));
        }
        if let Some(k) = open_canvas {
            let value = ws.nodes[id].values.get(&k).cloned().or_else(|| shape.and_then(|s| s.field(&k)).and_then(|f| f.default.clone())).unwrap_or(Value::Null);
            *canvas_edit = Some(crate::builders::canvas::CanvasEdit::new(ws_key, id, &k, &value));
        }
        if let Some(k) = open_layout {
            let value = ws.nodes[id].values.get(&k).cloned().or_else(|| shape.and_then(|s| s.field(&k)).and_then(|f| f.default.clone())).unwrap_or(Value::Null);
            *layout_edit = Some(crate::builders::layout::LayoutEdit::new(ws_key, id, &k, &value));
        }

        // Header interaction: the only place a block can be dragged.
        let head = cui.interact(layout.header, cui.id().with(("head", id)), Sense::click_and_drag());
        if head.hovered() {
            ctx.set_cursor_icon(CursorIcon::Grab);
        }
        if head.dragged_by(PointerButton::Primary) {
            ctx.set_cursor_icon(CursorIcon::Grabbing);
            let d = head.drag_delta();
            // Dragging one of several selected blocks moves them all.
            let group = is_selected && multi.len() > 1;
            for (nid, node) in ws.nodes.iter_mut() {
                if nid == id || (group && multi.contains(nid)) {
                    node.at[0] += d.x;
                    node.at[1] += d.y;
                }
            }
            *edited = true;
        }
        if head.drag_started() {
            drag_zones = Some(fitted.clone());
        }
        if head.drag_stopped() {
            dropped = Some(id.clone());
        }
        let toggle = ctx.input(|i| i.modifiers.command || i.modifiers.shift);
        if head.clicked() || body.clicked() {
            if toggle && selected.as_ref().is_some_and(|(w, _)| w == ws_key) {
                if !multi.remove(id) {
                    multi.insert(id.clone());
                }
                *selected = Some((ws_key.to_string(), id.clone()));
            } else {
                *selected = Some((ws_key.to_string(), id.clone()));
                *multi = std::iter::once(id.clone()).collect();
            }
        } else if (head.drag_started() || body.drag_started()) && !is_selected {
            *selected = Some((ws_key.to_string(), id.clone()));
            *multi = std::iter::once(id.clone()).collect();
        }
        // Picking a block alone lets go of the notes.
        if ((head.clicked() || body.clicked()) && !toggle) || ((head.drag_started() || body.drag_started()) && !is_selected) {
            selected_notes.retain(|(w, _)| w != ws_key);
        }
        // Right-click selects too; the selected block's actions are buttons above it.
        if (head.secondary_clicked() || body.secondary_clicked()) && !multi.contains(id) {
            *selected = Some((ws_key.to_string(), id.clone()));
            *multi = std::iter::once(id.clone()).collect();
        }
        head.on_hover_ui(|ui| block_tooltip(ui, def, theme));
        if selected.as_ref().is_some_and(|(w, s)| w == ws_key && s == id) {
            let in_zone = ws.nodes[id].zone.is_some();
            action_bar = Some(BarFor {
                node: id.clone(),
                rect: to_global * layout.rect,
                in_zone,
                can_wrap: !in_zone && def.kind != BlockKind::Event,
                source: Some(def.source.display().to_string()),
            });
        }

        // Pins.
        for spot in layout.pins.iter() {
            let connected = if spot.dir == Dir::In { ws.links_into(id, &spot.pin).next().is_some() } else { ws.links.iter().any(|l| l.from.node == *id && l.from.pin == spot.pin) };
            let candidate = drag.as_ref().map(|d| drag_compat(d, id, spot, reg, flow.as_ref()));
            draw_pin(&painter, spot, connected, candidate, theme, reg);

            let resp = cui.interact(Rect::from_center_size(spot.pos, Vec2::splat(pin_hit)), cui.id().with(("pin", id, &spot.pin, spot.dir)), Sense::click_and_drag());
            if resp.hovered() {
                ctx.set_cursor_icon(CursorIcon::Crosshair);
            }
            if resp.drag_started() {
                // Dragging from a connected input picks up its (latest) wire.
                let existing = (spot.dir == Dir::In).then(|| ws.links_into(id, &spot.pin).last().cloned()).flatten();
                start_drag = Some(match existing {
                    Some(link) => {
                        ws.links.retain(|l| *l != link);
                        *edited = true;
                        PinEnd { node: link.from.node.clone(), pin: link.from.pin.clone(), dir: Dir::Out, ty: spot.ty.clone(), picked_up: Some(link) }
                    }
                    None => PinEnd { node: id.clone(), pin: spot.pin.clone(), dir: spot.dir, ty: spot.ty.clone(), picked_up: None },
                });
            } else if resp.clicked() && spot.dir == Dir::In && !spot.ty.is_action() && !connected {
                toggle_pin = Some((id.clone(), spot.pin.clone()));
            }
            resp.on_hover_ui(|ui| pin_tooltip(ui, spot, reg, theme));
        }
    }

    if let Some(d) = start_drag {
        view.drag = Some(d);
    }
    if let Some((node, key)) = open_embed_for {
        let value = ws.nodes.get(&node).and_then(|n| n.values.get(&key).cloned()).unwrap_or(Value::Null);
        *embed_edit = Some(crate::builders::embed::EmbedEdit { ws: ws_key.to_string(), node, key, draft: crate::builders::embed::EmbedData::from_value(&value), json: None });
    }
    if let Some((node, pin)) = toggle_pin {
        let n = ws.nodes.get_mut(&node).unwrap();
        if let Some(i) = n.pins.iter().position(|p| *p == pin) {
            n.pins.remove(i);
        } else {
            n.pins.push(pin);
        }
        *edited = true;
    }

    // The wire being dragged, and what happens when it is dropped.
    if let Some(d) = view.drag.clone() {
        let source = pin_at(&d.node, &d.pin, d.dir).map(|s| s.pos);
        if let (Some(a), Some(p)) = (source, pointer_scene) {
            let (from, to) = if d.dir == Dir::Out { (a, p) } else { (p, a) };
            draw_wire(&painter, from, to, &d.ty, theme, reg, 0.0);
        }
        // Over a pin that would close a loop: say why it doesn't connect.
        if let (Some(p), Some(flow)) = (pointer_scene, flow.as_ref())
            && layouts.iter().flat_map(|l| l.pins.iter().map(move |s| (l, s))).any(|(l, s)| s.pos.distance(p) < pin_hit * 0.9 && drag_loops(&d, &l.id, s, reg, flow))
        {
            let text = if d.ty.is_action() {
                t!("This would make a loop that never ends. To repeat blocks, use Repeat, Loop While or For Each.")
            } else {
                t!("These blocks would wait for each other's values forever.")
            };
            let galley = painter.layout(text.to_string(), egui::FontId::proportional(12.0 / view.zoom.max(0.01)), theme.text, 280.0 / view.zoom.max(0.01));
            let at = p + vec2(16.0, 16.0) / view.zoom.max(0.01);
            let pad = vec2(8.0, 5.0) / view.zoom.max(0.01);
            painter.rect(Rect::from_min_size(at, galley.size() + pad * 2.0), 6.0 / view.zoom.max(0.01), theme.panel, Stroke::new(1.0, theme.error), StrokeKind::Inside);
            painter.galley(at + pad, galley, theme.text);
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            view.drag = None;
            if let Some(link) = d.picked_up.clone() {
                ws.links.push(link);
            }
        } else if ctx.input(|i| i.pointer.any_released()) {
            view.drag = None;
            let target = pointer_scene.and_then(|p| {
                layouts
                    .iter()
                    .flat_map(|l| l.pins.iter().map(move |s| (l, s)))
                    .filter(|(l, s)| s.pos.distance(p) < pin_hit * 0.9 && drag_compat(&d, &l.id, s, reg, flow.as_ref()) != Compat::No)
                    .min_by(|a, b| a.1.pos.distance(p).total_cmp(&b.1.pos.distance(p)))
                    .map(|(l, s)| (l.id.clone(), s.clone()))
            });
            // Dropped on a pin that refuses it (another type, or a loop): nothing happens.
            let on_pin = pointer_scene.is_some_and(|p| layouts.iter().flat_map(|l| l.pins.iter()).any(|s| s.pos.distance(p) < pin_hit * 0.9));
            match target {
                Some((node, spot)) => {
                    connect(ws, &d, &node, &spot);
                    *edited = true;
                }
                // A picked-up wire dropped on nothing is deleted.
                None if d.picked_up.is_some() => {}
                None if on_pin => {}
                None if over_canvas => {
                    let p = pointer.unwrap();
                    *picker = Some(Picker::new(ws_key, p, to_global.inverse() * p, Some(PendingWire::from(&d))));
                }
                None => {}
            }
        }
    }

    // Background: a click (or right-click) selects the wire under the pointer,
    // which shows its Delete button; right-click on empty canvas opens the
    // block picker (Space and the toolbar's Add block too).
    if bg.secondary_clicked() {
        if let Some(p) = pointer.or_else(|| ctx.input(|i| i.pointer.interact_pos())) {
            match hovered_wire.clone() {
                Some(link) => {
                    *selected = None;
                    multi.clear();
                    *selected_wire = Some((ws_key.to_string(), link));
                }
                None => *picker = Some(Picker::new(ws_key, p, to_global.inverse() * p, None)),
            }
        }
    } else if bg.clicked() {
        *selected = None;
        multi.clear();
        selected_notes.retain(|(w, _)| w != ws_key);
        *selected_wire = hovered_wire.clone().map(|l| (ws_key.to_string(), l));
    }
    if selected.is_some() {
        *selected_wire = None;
    }
    // The canvas' shortcuts (Settings › Shortcuts): while the mouse is over it and nothing is typed.
    let typing = ctx.text_edit_focused();
    let recording = recording_frame.is_some_and(|f| f + 1 >= ctx.cumulative_pass_nr());
    if over_canvas && !typing && picker.is_none() && !recording {
        use crate::editor::keymap::{Action, consume};
        let keys = &prefs.shortcuts;
        if consume(&ctx, keys, Action::AddBlock)
            && let Some(p) = pointer
        {
            *picker = Some(Picker::new(ws_key, p, to_global.inverse() * p, None));
        }
        if consume(&ctx, keys, Action::AddNote)
            && let Some(p) = pointer_scene
        {
            let id = ws.next_note_id();
            ws.notes.insert(id.clone(), Note { title: t!("Note").to_string(), rect: [p.x - 40.0, p.y - 30.0, 420.0, 260.0], loose: true, ..Default::default() });
            view.edit_note = Some(id);
            *edited = true;
        }
        if consume(&ctx, keys, Action::ToggleSnap) {
            prefs.snap_to_grid = !prefs.snap_to_grid;
        }
        if consume(&ctx, keys, Action::FitView) {
            fit_view(view, outer, &layouts[..node_count], ws);
        }
        let at = pointer.unwrap_or(outer.center());
        if consume(&ctx, keys, Action::ZoomIn) {
            view.zoom_at(outer.min, at, 1.25);
        }
        if consume(&ctx, keys, Action::ZoomOut) {
            view.zoom_at(outer.min, at, 1.0 / 1.25);
        }
        if consume(&ctx, keys, Action::ZoomReset) {
            view.zoom_at(outer.min, at, 1.0 / view.zoom);
        }
    }

    // Buttons over the canvas, in screen space and on a layer above it: the
    // toolbar, the selected block's actions, the selected wire's Delete and
    // each zone's settings. Everything here works with a finger too. Each
    // group gets its own small Ui: a layer only takes the clicks inside the
    // rectangles it uses, so the blocks below stay clickable.
    let tools_layer = LayerId::new(parent_layer.order, ui.id().with(("canvas-tools", ws_key)));
    ctx.set_sublayer(parent_layer, tools_layer);
    let mut tools = |rect: Rect, layout: egui::Layout| {
        let mut t = ui.new_child(UiBuilder::new().layer_id(tools_layer).max_rect(rect).layout(layout));
        t.set_clip_rect(outer);
        t
    };
    let row = egui::Layout::left_to_right(egui::Align::Center);
    let icon = |name: &str| reg.icons.get(name).cloned();

    // A workspace that is turned off says so above its blocks.
    if ws.disabled {
        let notice = Rect::from_min_size(outer.left_top() + vec2(12.0, 12.0), vec2(outer.width() - 24.0, if theme.touch { 44.0 } else { 34.0 }));
        tools(notice, row).scope(|ui| {
            egui::Frame::new()
                .fill(theme.panel)
                .stroke(Stroke::new(1.0, theme.warning))
                .corner_radius(7)
                .inner_margin(egui::Margin::symmetric(10, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t!("This workspace is turned off: the bot leaves it out, so its blocks don't run.")).color(theme.text));
                        if kit::button(ui, theme, t!("Turn on")).clicked() {
                            ws.disabled = false;
                            *edited = true;
                        }
                    });
                });
        });
    }

    // Toolbar, bottom left.
    let bar_h = if theme.touch { 44.0 } else { 34.0 };
    let bar = Rect::from_min_size(outer.left_bottom() + vec2(12.0, -(bar_h + 12.0)), vec2(outer.width() - 24.0, bar_h));
    tools(bar, row).scope(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let center = outer.center();
        let add = tool_button(ui, theme, icon("plus").as_deref(), t!("Add block"));
        ui_rects.insert("add-block".into(), add.rect);
        if add.on_hover_text(t!("Add a block (Space or right-click on the canvas)")).clicked() {
            *picker = Some(Picker::new(ws_key, center, to_global.inverse() * center, None));
        }
        if tool_button(ui, theme, icon("note").as_deref(), t!("Add note")).on_hover_text(t!("A note: a frame with a title and a description, to explain or group blocks")).clicked() {
            let c = to_global.inverse() * center;
            let id = ws.next_note_id();
            ws.notes.insert(id.clone(), Note { title: t!("Note").to_string(), rect: [c.x - 210.0, c.y - 130.0, 420.0, 260.0], loose: true, ..Default::default() });
            view.edit_note = Some(id);
            *edited = true;
        }
        let snap_tip = if prefs.snap_to_grid { t!("Snap to grid is on: dropped blocks and notes line up with the grid") } else { t!("Snap to grid is off") };
        if crate::ui::kit::styled_button(
            ui,
            theme,
            egui::Button::image(egui::Image::from_uri(icon("magnet").map(|p| crate::editor::file_uri(&p)).unwrap_or_default()).tint(if prefs.snap_to_grid { theme.accent } else { theme.text_muted }).fit_to_exact_size(vec2(15.0, 15.0)))
                .corner_radius(7)
                .min_size(Vec2::splat(ui.spacing().interact_size.y.max(32.0))),
            if prefs.snap_to_grid { mix(theme.panel, theme.accent, 0.18) } else { theme.panel },
            Stroke::new(1.0, if prefs.snap_to_grid { with_alpha(theme.accent, 150) } else { theme.node_border }),
        )
        .on_hover_text(snap_tip)
        .clicked()
        {
            prefs.snap_to_grid = !prefs.snap_to_grid;
        }
        ui.add_space(6.0);
        if crate::ui::kit::icon_button(ui, theme, icon("zoom-out").as_deref(), "−", t!("Zoom out")).clicked() {
            view.zoom_at(outer.min, center, 1.0 / 1.25);
        }
        let percent = format!("{:.0}%", view.zoom * 100.0);
        if tool_button(ui, theme, None, &percent).on_hover_text(t!("Back to 100%")).clicked() {
            view.zoom_at(outer.min, center, 1.0 / view.zoom);
        }
        if crate::ui::kit::icon_button(ui, theme, icon("zoom-in").as_deref(), "+", t!("Zoom in")).clicked() {
            view.zoom_at(outer.min, center, 1.25);
        }
        if crate::ui::kit::icon_button(ui, theme, icon("maximize").as_deref(), t!("Fit"), t!("Show every block and note")).clicked() {
            fit_view(view, outer, &layouts[..node_count], ws);
        }
    });

    // The selected block's actions, above it (or below when there is no room).
    if let Some(b) = action_bar.filter(|b| outer.intersects(b.rect) && view.drag.is_none()) {
        let count = if multi.contains(&b.node) { multi.len().max(1) } else { 1 };
        let many = |text: &str| if count > 1 { format!("{text} {count}") } else { text.to_string() };
        let bar_h = if theme.touch { ACTION_BAR_TOUCH_H } else { ACTION_BAR_H };
        let above = b.rect.left_top() - vec2(0.0, bar_h + 8.0);
        let at = if above.y >= outer.top() + 4.0 { above } else { b.rect.left_bottom() + vec2(0.0, 8.0) };
        let area = Rect::from_min_size(pos2(at.x.max(outer.left() + 4.0), at.y), vec2(outer.width(), bar_h));
        tools(area, row).scope(|ui| {
            egui::Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.node_border)).corner_radius(9).inner_margin(4).shadow(ui.visuals().popup_shadow).show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if b.source.is_some() && tool_button(ui, theme, icon("copy").as_deref(), &many(t!("Duplicate"))).clicked() {
                    menu = Some(NodeMenu::Duplicate(b.node.clone()));
                }
                if count == 1 && b.in_zone && tool_button(ui, theme, icon("shield").as_deref(), t!("Leave Try Zone")).clicked() {
                    menu = Some(NodeMenu::LeaveZone(b.node.clone()));
                }
                if count == 1 && b.can_wrap && tool_button(ui, theme, icon("shield").as_deref(), t!("Try Zone")).on_hover_text(t!("Wrap in a Try Zone, which catches its errors")).clicked() {
                    menu = Some(NodeMenu::WrapInZone(b.node.clone()));
                }
                if tool_button(ui, theme, icon("note").as_deref(), t!("Note")).on_hover_text(t!("Put a note around the selected blocks")).clicked() {
                    menu = Some(NodeMenu::NoteAround(b.node.clone()));
                }
                if count > 1 {
                    let arrange_button = tool_button(ui, theme, icon("align").as_deref(), t!("Arrange")).on_hover_text(t!("Line up or spread out the selected blocks"));
                    egui::Popup::menu(&arrange_button).show(|ui| {
                        for (i, how) in Arrange::ALL.into_iter().enumerate() {
                            if i == 3 || i == 6 {
                                ui.separator();
                            }
                            if kit::menu_item(ui, how.name(), None, true).clicked() {
                                menu = Some(NodeMenu::Arrange(b.node.clone(), how));
                            }
                        }
                    });
                }
                if let Some(src) = b.source.as_ref().filter(|_| count == 1)
                    && tool_button(ui, theme, icon("function").as_deref(), t!("Source")).on_hover_text(t!("Open the block's source file")).clicked()
                {
                    menu = Some(NodeMenu::OpenSource(src.clone()));
                }
                if tool_button(ui, theme, icon("trash").as_deref(), &many(t!("Delete"))).on_hover_text(t!("Delete (Del)")).clicked() {
                    menu = Some(NodeMenu::Delete(b.node.clone()));
                }
            });
        });
    }

    // The selected wire's Delete button, on its middle.
    if let Some(mid) = wire_mid.filter(|m| outer.shrink(20.0).contains(*m))
        && let Some((_, link)) = selected_wire.clone()
    {
        let r = Rect::from_center_size(mid, vec2(124.0, 34.0));
        let resp = tool_button(&mut tools(r, egui::Layout::centered_and_justified(egui::Direction::LeftToRight)), theme, icon("trash").as_deref(), t!("Delete wire")).on_hover_text(t!("Delete wire (Del)"));
        if resp.clicked() {
            ws.links.retain(|l| *l != link);
            *selected_wire = None;
            *edited = true;
        }
    }

    match menu {
        Some(NodeMenu::Delete(n)) => {
            let ids: Vec<String> = if multi.contains(&n) { multi.drain().collect() } else { vec![n] };
            for id in &ids {
                ws.remove_node(id);
                shapes.forget(ws_key, id);
            }
            if selected.as_ref().is_some_and(|(_, s)| ids.contains(s)) {
                *selected = None;
                multi.clear();
            }
            *edited = true;
        }
        Some(NodeMenu::Duplicate(n)) => {
            let ids: Vec<String> = if multi.contains(&n) { multi.iter().cloned().collect() } else { vec![n] };
            let new_ids = ws.paste(&ws.extract(&ids), [30.0, 30.0]);
            *selected = new_ids.first().map(|f| (ws_key.to_string(), f.clone()));
            *multi = new_ids.into_iter().collect();
            *edited = true;
        }
        Some(NodeMenu::OpenSource(path)) => {
            let _ = open::that(path);
        }
        Some(NodeMenu::Arrange(n, how)) => {
            let ids: Vec<String> = if multi.contains(&n) { multi.iter().cloned().collect() } else { vec![n] };
            let rects: Vec<(String, Rect)> = layouts[..node_count].iter().filter(|l| ids.contains(&l.id)).map(|l| (l.id.clone(), l.rect)).collect();
            for (id, p) in arrange(&rects, how) {
                if let Some(node) = ws.nodes.get_mut(&id) {
                    node.at = [p.x, p.y];
                }
            }
            *edited = true;
        }
        Some(NodeMenu::NoteAround(n)) => {
            let ids: Vec<String> = if multi.contains(&n) { multi.iter().cloned().collect() } else { vec![n] };
            let around = layouts[..node_count].iter().filter(|l| ids.contains(&l.id)).map(|l| l.rect).reduce(|a, b| a.union(b));
            if let Some(r) = around {
                // Room for the title above the blocks.
                let r = Rect::from_min_max(r.min - vec2(30.0, 70.0), r.max + vec2(30.0, 30.0));
                let id = ws.next_note_id();
                ws.notes.insert(id.clone(), Note { title: t!("Note").to_string(), rect: [r.min.x, r.min.y, r.width(), r.height()], loose: true, ..Default::default() });
                view.edit_note = Some(id);
                *edited = true;
            }
        }
        Some(NodeMenu::WrapInZone(n)) => {
            let zid = (1..).map(|i| format!("z{i}")).find(|z| !ws.zones.contains_key(z) && !ws.nodes.contains_key(z)).unwrap();
            ws.zones.insert(zid.clone(), circuit_core::Zone { title: "Try".into(), retry: 0, ..Default::default() });
            if let Some(node) = ws.nodes.get_mut(&n) {
                node.zone = Some(zid);
            }
            *edited = true;
        }
        Some(NodeMenu::LeaveZone(n)) => {
            if let Some(node) = ws.nodes.get_mut(&n) {
                node.zone = None;
            }
            *edited = true;
        }
        Some(NodeMenu::RemoveZone(zid)) => {
            for node in ws.nodes.values_mut().filter(|n| n.zone.as_deref() == Some(zid.as_str())) {
                node.zone = None;
            }
            *edited = true;
        }
        None => {}
    }

    if let Some(z) = drag_zones {
        view.drag_zones = z;
    }
    // A dropped block stays in its zone while it is inside the zone's outline
    // from before the drag, joins another zone it was dropped into, or leaves.
    // A zone's only block always stays: the zone moves with it.
    if prefs.snap_to_grid
        && let Some(n) = dropped.as_ref()
        && let Some(node) = ws.nodes.get(n)
    {
        let d = [snap(node.at[0]) - node.at[0], snap(node.at[1]) - node.at[1]];
        let group = multi.contains(n) && multi.len() > 1;
        for (nid, node) in ws.nodes.iter_mut() {
            if nid == n || (group && multi.contains(nid)) {
                node.at = [node.at[0] + d[0], node.at[1] + d[1]];
            }
        }
    }
    if let Some(n) = dropped
        && let Some(layout) = layouts[..node_count].iter().find(|l| l.id == n)
    {
        let at = ws.nodes[&n].at;
        let center = pos2(at[0], at[1]) + layout.rect.size() / 2.0;
        let zones = match std::mem::take(&mut view.drag_zones) {
            z if z.is_empty() => fitted.clone(),
            z => z,
        };
        let own = ws.nodes[&n].zone.clone();
        let alone = own.as_ref().is_some_and(|z| ws.nodes.values().filter(|m| m.zone.as_ref() == Some(z)).count() == 1);
        let inside = |z: &str| zones.iter().any(|(id, r)| id == z && r.contains(center));
        let target = if own.as_deref().is_some_and(inside) {
            own.clone()
        } else if let Some((other, _)) = zones.iter().find(|(id, r)| Some(id) != own.as_ref() && r.contains(center)) {
            Some(other.clone())
        } else if alone {
            own.clone()
        } else {
            None
        };
        let node = ws.nodes.get_mut(&n).unwrap();
        if node.zone != target && (target.is_some() || node.zone.is_some()) {
            node.zone = target;
            *edited = true;
        }
    }
    // Zones without blocks disappear, with the wires from their outputs.
    let used: std::collections::HashSet<String> = ws.nodes.values().filter_map(|n| n.zone.clone()).collect();
    if ws.zones.keys().any(|z| !used.contains(z)) {
        ws.zones.retain(|z, _| used.contains(z));
        let zones: std::collections::HashSet<String> = ws.zones.keys().cloned().collect();
        ws.links.retain(|l| ws.nodes.contains_key(&l.from.node) || zones.contains(&l.from.node));
        *edited = true;
    }

    if let Some(b) = selection_box {
        painter.rect(b, 4.0, with_alpha(theme.accent, 30), Stroke::new(1.0, theme.accent), StrokeKind::Inside);
    }
    sharpen_text(&ctx, layer, to_global);
    for l in &layouts[..node_count] {
        ui_rects.insert(format!("node:{}", l.id), to_global * l.rect);
        if *tour_parts {
            // pin:<block>:in|out:<pin> and row:<block>:<field>, for the tour.
            for p in l.pins.iter() {
                let dir = if matches!(p.dir, Dir::In) { "in" } else { "out" };
                ui_rects.insert(format!("pin:{}:{dir}:{}", l.id, p.pin), to_global * Rect::from_center_size(p.pos, vec2(12.0, 12.0)));
            }
            for (kind, r) in l.rows.iter() {
                if let RowKind::Field { key, .. } = kind {
                    ui_rects.insert(format!("row:{}:{key}", l.id), to_global * *r);
                }
            }
        }
    }
}

/// The block whose action bar is shown.
struct BarFor {
    node: String,
    /// The block's rectangle on screen.
    rect: Rect,
    in_zone: bool,
    can_wrap: bool,
    /// The block's source file; None for a missing block.
    source: Option<String>,
}

const ACTION_BAR_H: f32 = 42.0;
const ACTION_BAR_TOUCH_H: f32 = 52.0;

/// The size of a Try Zone's settings button, on screen.
const ZONE_BUTTON: f32 = 24.0;
const ZONE_BUTTON_TOUCH: f32 = 36.0;
