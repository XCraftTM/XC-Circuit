//! The node canvas: pan/zoom, blocks, pins, wires and wire dragging.

use std::collections::HashMap;

use circuit_core::graph::ACTION_IN;
use circuit_core::{BlockDef, BlockKind, BlockShape, Compat, Link, PinMode, PinRef, PinType, Workspace};
use circuit_plugins::Registry;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Key, LayerId, PointerButton, Pos2, Rect, Sense, Shadow, Shape,
    Stroke, StrokeKind, Ui, UiBuilder, Vec2, emath::TSTransform, epaint::CubicBezierShape, pos2, vec2,
};
use serde_json::Value;

use crate::editor::{Editor, node_shape};
use crate::picker::{PendingWire, Picker};
use crate::theme::{Theme, mix, with_alpha};
use crate::widgets;

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
const GRID: f32 = 22.0;
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
    /// The wire whose right-click menu is open, and where (screen position).
    pub wire_menu: Option<(Link, Pos2)>,
}

impl CanvasView {
    fn to_global(&self, origin: Pos2) -> TSTransform {
        TSTransform::from_translation(origin.to_vec2() + self.offset) * TSTransform::from_scaling(self.zoom)
    }

    fn zoom_at(&mut self, origin: Pos2, pointer: Pos2, factor: f32) {
        let scene = (pointer - origin - self.offset) / self.zoom;
        self.zoom = (self.zoom * factor).clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
        self.offset = pointer - origin - scene * self.zoom;
    }
}

#[derive(Clone, Debug)]
struct PinSpot {
    pin: String,
    dir: Dir,
    ty: PinType,
    pos: Pos2,
    label: String,
    description: Option<String>,
    /// An inline field that can be turned into a pin; drawn faintly.
    faint: bool,
    multi: bool,
}

#[derive(Clone, Debug)]
enum RowKind {
    Flow(String),
    Field { key: String, pinned: bool },
    Output(String),
}

#[derive(Clone, Debug)]
struct NodeLayout {
    id: String,
    rect: Rect,
    header: Rect,
    value_block: bool,
    rows: Vec<(RowKind, Rect)>,
    pins: Vec<PinSpot>,
}

/// `tall` gives rows that need more room than one line (embed previews), by field key.
fn layout_node(id: &str, at: [f32; 2], pins_on: &[String], def: Option<&BlockDef>, shape: Option<&BlockShape>, tall: &HashMap<String, f32>) -> NodeLayout {
    let (Some(def), Some(shape)) = (def, shape) else {
        let rect = Rect::from_min_size(pos2(at[0], at[1]), vec2(NODE_W, HEADER_H + ROW_H + PAD_BOTTOM));
        let header = Rect::from_min_size(rect.min, vec2(NODE_W, HEADER_H));
        return NodeLayout { id: id.into(), rect, header, value_block: false, rows: Vec::new(), pins: Vec::new() };
    };
    let value_block = def.kind == BlockKind::Value;
    let width = def.width.unwrap_or(if def.preview.as_deref() == Some("embed") { EMBED_NODE_W } else { NODE_W });
    let header_h = if value_block { VALUE_HEADER_H } else { HEADER_H };
    let origin = pos2(at[0], at[1]);
    let header = Rect::from_min_size(origin, vec2(width, header_h));

    let mut rows = Vec::new();
    let single_next = shape.flows.len() == 1 && shape.flows[0] == "next";
    if !single_next {
        rows.extend(shape.flows.iter().map(|f| RowKind::Flow(f.clone())));
    }
    for (key, f) in shape.visible_fields() {
        if f.advanced {
            continue;
        }
        let pinned = f.pin == PinMode::Only || pins_on.contains(key);
        rows.push(RowKind::Field { key: key.clone(), pinned });
    }
    rows.extend(shape.outputs.iter().map(|(k, _)| RowKind::Output(k.clone())));

    let row_h = |row: &RowKind| match row {
        RowKind::Field { key, .. } => tall.get(key).copied().unwrap_or(ROW_H),
        _ => ROW_H,
    };
    let height = header_h + PAD_TOP + rows.iter().map(row_h).sum::<f32>() + PAD_BOTTOM;
    let rect = Rect::from_min_size(origin, vec2(width, height));
    let mut pins = Vec::new();
    let header_y = header.center().y;
    if shape.action_in {
        pins.push(PinSpot {
            pin: ACTION_IN.into(),
            dir: Dir::In,
            ty: PinType::Action,
            pos: pos2(rect.left(), header_y),
            label: "Action".into(),
            description: Some("Runs this block.".into()),
            faint: false,
            multi: true,
        });
    }
    if single_next {
        pins.push(PinSpot {
            pin: "next".into(),
            dir: Dir::Out,
            ty: PinType::Action,
            pos: pos2(rect.right(), header_y),
            label: "Action".into(),
            description: Some("Runs the next blocks when this one is done.".into()),
            faint: false,
            multi: true,
        });
    }

    let mut laid = Vec::new();
    let mut top = header.bottom() + PAD_TOP;
    for row in rows.into_iter() {
        let h = row_h(&row);
        let r = Rect::from_min_size(pos2(rect.left(), top), vec2(width, h));
        top += h;
        // Pins sit at the middle of the first line, even on tall rows.
        let y = r.top() + ROW_H / 2.0;
        match &row {
            RowKind::Flow(f) => pins.push(PinSpot {
                pin: f.clone(),
                dir: Dir::Out,
                ty: PinType::Action,
                pos: pos2(rect.right(), y),
                label: circuit_core::block::title_case(f),
                description: None,
                faint: false,
                multi: true,
            }),
            RowKind::Field { key, pinned } => {
                let f = shape.field(key).expect("visible field");
                if *pinned || f.pin != PinMode::Inline {
                    pins.push(PinSpot {
                        pin: key.clone(),
                        dir: Dir::In,
                        ty: f.ty.clone(),
                        pos: pos2(rect.left(), y),
                        label: f.label(key),
                        description: f.description.clone(),
                        faint: !*pinned,
                        multi: f.multi,
                    });
                }
            }
            RowKind::Output(k) => {
                let o = shape.output(k).expect("output");
                pins.push(PinSpot {
                    pin: k.clone(),
                    dir: Dir::Out,
                    ty: o.ty.clone(),
                    pos: pos2(rect.right(), y),
                    label: o.label(k),
                    description: o.description.clone(),
                    faint: false,
                    multi: true,
                });
            }
        }
        laid.push((row, r));
    }
    NodeLayout { id: id.into(), rect, header, value_block, rows: laid, pins }
}

enum NodeMenu {
    Delete(String),
    Duplicate(String),
    OpenSource(String),
    WrapInZone(String),
    LeaveZone(String),
    RemoveZone(String),
}

const ZONE_PANEL_W: f32 = 150.0;
const ZONE_ROW_H: f32 = 22.0;
const ZONE_PANEL_H: f32 = 4.0 * ZONE_ROW_H + 8.0;

/// Rectangles of the zones that have blocks, fitted around them.
fn zone_rects(ws: &Workspace, layouts: &[NodeLayout]) -> Vec<(String, Rect)> {
    ws.zones
        .keys()
        .filter_map(|zid| {
            let bounds = layouts
                .iter()
                .filter(|l| ws.nodes.get(&l.id).is_some_and(|n| n.zone.as_deref() == Some(zid)))
                .map(|l| l.rect)
                .reduce(|a, b| a.union(b))?;
            let rect = Rect::from_min_max(bounds.min - vec2(28.0, 44.0), bounds.max + vec2(28.0 + ZONE_PANEL_W * 0.4, 28.0 + ZONE_PANEL_H));
            Some((zid.clone(), rect))
        })
        .collect()
}

fn zone_layout(zid: &str, rect: Rect) -> NodeLayout {
    let header = Rect::from_min_size(rect.min, vec2(rect.width(), 28.0));
    let top = rect.bottom() - 8.0 - ZONE_PANEL_H + 4.0;
    let outputs: [(&str, &str, PinType, &str); 4] = [
        ("done", "Done", PinType::Action, "Runs when every block inside finished without an error."),
        ("error", "Error", PinType::Action, "Runs when a block inside failed (after all retries)."),
        ("message", "Error message", PinType::Text, "What went wrong."),
        ("details", "Error details", PinType::Object, "Block name, error code, workspace and stack trace."),
    ];
    let pins = outputs
        .into_iter()
        .enumerate()
        .map(|(i, (pin, label, ty, description))| PinSpot {
            pin: pin.into(),
            dir: Dir::Out,
            ty,
            pos: pos2(rect.right(), top + ZONE_ROW_H * (i as f32 + 0.5)),
            label: label.into(),
            description: Some(description.into()),
            faint: false,
            multi: true,
        })
        .collect();
    NodeLayout { id: zid.into(), rect, header, value_block: false, rows: Vec::new(), pins }
}

pub fn show(ui: &mut Ui, ed: &mut Editor, ws_key: &str) {
    let Editor { reg, engine, theme, project, shapes, views, selected, dirty, picker, active_workspace, activity, multi, define_ctx, embed_edit, selected_wire, .. } = ed;
    let Some(project) = project.as_mut() else { return };
    let mode = reg.mode(&project.meta.mode).or_else(|| reg.modes.first());
    let Some(ws) = project.workspaces.get_mut(ws_key) else {
        ui.label("This workspace no longer exists.");
        return;
    };
    let view = views.entry(ws_key.to_string()).or_insert_with(|| CanvasView { zoom: 1.0, ..Default::default() });

    // Shapes and layouts for every node, in scene coordinates.
    let measure = ui.painter().clone();
    let ids: Vec<String> = ws.nodes.keys().cloned().collect();
    let mut layouts: Vec<NodeLayout> = Vec::with_capacity(ids.len());
    let mut node_shapes: HashMap<String, BlockShape> = HashMap::new();
    for id in &ids {
        let node = &ws.nodes[id];
        let def = reg.block(&node.block);
        let shape = node_shape(reg, engine, shapes, ws_key, ws, id, define_ctx);
        let mut tall = HashMap::new();
        if let (Some(s), Some(d)) = (&shape, def) {
            for (key, f) in s.visible_fields().filter(|(_, f)| f.type_name == "embed") {
                let data = crate::embed::EmbedData::from_value(node.values.get(key).or(f.default.as_ref()).unwrap_or(&Value::Null));
                let w = d.width.unwrap_or(EMBED_NODE_W) - 28.0;
                tall.insert(key.clone(), crate::embed::layout(&measure, &data, w, Some(4), EMBED_SCALE).height + 44.0);
            }
        }
        layouts.push(layout_node(id, node.at, &node.pins, def, shape.as_ref(), &tall));
        if let Some(s) = shape {
            node_shapes.insert(id.clone(), s);
        }
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
    let pin_index: HashMap<(String, String, Dir), usize> = layouts
        .iter()
        .enumerate()
        .flat_map(|(li, l)| l.pins.iter().enumerate().map(move |(pi, p)| ((l.id.clone(), p.pin.clone(), p.dir), li * 1000 + pi)))
        .collect();
    let pin_at = |key: &(String, String, Dir)| pin_index.get(key).map(|i| &layouts[i / 1000].pins[i % 1000]);

    let (outer, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    ui.painter().rect_filled(outer, 0.0, theme.canvas);

    if !view.initialized {
        fit_view(view, outer, &layouts[..node_count]);
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
                }
                multi.extend(hits.iter().cloned());
                *selected = multi.iter().next().map(|n| (ws_key.to_string(), n.clone()));
            }
        }
    } else if bg.dragged_by(PointerButton::Primary) || bg.dragged_by(PointerButton::Middle) {
        view.offset += bg.drag_delta() * view.zoom;
        ctx.set_cursor_icon(CursorIcon::Grabbing);
    }
    let painter = cui.painter().clone();

    draw_grid(&painter, visible, theme, view.zoom);
    let recent = |id: &str| activity.get(&(ws_key.to_string(), id.to_string()));
    let glow = |id: &str| recent(id).filter(|a| a.error.is_none()).map(|a| (1.0 - a.at.elapsed().as_secs_f32()).max(0.0)).unwrap_or(0.0);

    // Try Zones: drawn behind everything, dragged by their title band.
    let mut menu: Option<NodeMenu> = None;
    let mut start_drag: Option<PinEnd> = None;
    let drag = view.drag.clone();
    for layout in &layouts[node_count..] {
        let zid = layout.id.clone();
        let r = layout.rect;
        let failed = recent(&zid).and_then(|a| a.error.clone());
        let edge = if failed.is_some() { theme.error } else { mix(with_alpha(theme.accent, 150), theme.action, glow(&zid)) };
        painter.rect(r, 16.0, with_alpha(theme.accent, 14), Stroke::new(1.5, edge), StrokeKind::Inside);
        painter.rect_filled(layout.header, CornerRadius { nw: 16, ne: 16, sw: 0, se: 0 }, with_alpha(theme.accent, 36));
        let (title, retry, timeout) = ws.zones.get(&zid).map(|z| (z.title.clone(), z.retry, z.timeout_ms)).unwrap_or_default();
        painter.text(layout.header.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, format!("Try · {title}"), FontId::proportional(12.5), theme.accent);
        let mut extra = Vec::new();
        if retry > 0 {
            extra.push(format!("retry {retry}×"));
        }
        if let Some(t) = timeout {
            extra.push(format!("{:.0} s timeout", t as f32 / 1000.0));
        }
        painter.text(layout.header.right_center() - vec2(14.0, 0.0), Align2::RIGHT_CENTER, extra.join(" · "), FontId::proportional(11.0), theme.text_muted);
        // Outputs panel in the bottom-right corner.
        let panel = Rect::from_min_max(pos2(r.right() - ZONE_PANEL_W, r.bottom() - ZONE_PANEL_H - 8.0), pos2(r.right(), r.bottom() - 8.0));
        painter.rect(panel, CornerRadius { nw: 9, ne: 0, sw: 9, se: 0 }, theme.node, Stroke::new(1.0, theme.node_border), StrokeKind::Inside);
        for spot in &layout.pins {
            let color = if spot.pin == "error" { theme.error } else if spot.ty.is_action() { theme.text } else { theme.text_muted };
            painter.text(spot.pos - vec2(16.0, 0.0), Align2::RIGHT_CENTER, &spot.label, FontId::proportional(12.0), color);
        }
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
            *dirty = true;
        }
        band.on_hover_text("A Try Zone catches errors from the blocks inside it. Right-click for settings.").context_menu(|ui| {
            if let Some(z) = ws.zones.get_mut(&zid) {
                ui.label("Title");
                *dirty |= ui.text_edit_singleline(&mut z.title).changed();
                ui.horizontal(|ui| {
                    ui.label("Retries");
                    *dirty |= ui.add(egui::DragValue::new(&mut z.retry).range(0..=10)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Timeout (s)");
                    let mut secs = z.timeout_ms.map(|t| t as f64 / 1000.0).unwrap_or(0.0);
                    if ui.add(egui::DragValue::new(&mut secs).range(0.0..=600.0).speed(0.5)).changed() {
                        z.timeout_ms = (secs > 0.0).then_some((secs * 1000.0) as u64);
                        *dirty = true;
                    }
                });
                ui.separator();
                if ui.button("Remove Try Zone").clicked() {
                    menu = Some(NodeMenu::RemoveZone(zid.clone()));
                }
            }
        });
        for spot in &layout.pins {
            let connected = ws.links.iter().any(|l| l.from.node == zid && l.from.pin == spot.pin);
            let candidate = drag.as_ref().map(|d| drag_compat(d, &zid, spot, reg));
            draw_pin(&painter, spot, connected, candidate, theme, reg);
            let resp = cui.interact(Rect::from_center_size(spot.pos, vec2(16.0, 16.0)), cui.id().with(("zpin", &zid, &spot.pin)), Sense::click_and_drag());
            if resp.drag_started() {
                start_drag = Some(PinEnd { node: zid.clone(), pin: spot.pin.clone(), dir: Dir::Out, ty: spot.ty.clone(), picked_up: None });
            }
            resp.on_hover_ui(|ui| pin_tooltip(ui, spot, reg, theme));
        }
    }

    // Wires. Action wires light up while the bot runs through them. The wire
    // under the pointer (on empty canvas) can be clicked or right-clicked.
    let over_block = pointer_scene.is_some_and(|p| layouts[..node_count].iter().any(|l| l.rect.expand(8.0).contains(p)));
    let wire_pick = (bg.hovered() && view.drag.is_none() && !over_block).then_some(pointer_scene).flatten();
    let reach = 6.0 / view.zoom.max(0.3);
    let mut hovered_wire: Option<Link> = None;
    for link in &ws.links {
        let a = pin_at(&(link.from.node.clone(), link.from.pin.clone(), Dir::Out));
        let b = pin_at(&(link.to.node.clone(), link.to.pin.clone(), Dir::In));
        if let (Some(a), Some(b)) = (a, b)
            && let Some(p) = wire_pick
            && wire_distance(a.pos, b.pos, p) < reach
        {
            hovered_wire = Some(link.clone());
        }
    }
    let wire_selected = |l: &Link| selected_wire.as_ref().is_some_and(|(w, s)| w == ws_key && s == l) || view.wire_menu.as_ref().is_some_and(|(s, _)| s == l);
    for link in &ws.links {
        let a = pin_at(&(link.from.node.clone(), link.from.pin.clone(), Dir::Out));
        let b = pin_at(&(link.to.node.clone(), link.to.pin.clone(), Dir::In));
        if let (Some(a), Some(b)) = (a, b) {
            let lit = if a.ty.is_action() { glow(&link.to.node) } else { 0.0 };
            if wire_selected(link) || hovered_wire.as_ref() == Some(link) {
                let strength = if wire_selected(link) { 110 } else { 60 };
                painter.add(CubicBezierShape::from_points_stroke(wire_points(a.pos, b.pos), false, Color32::TRANSPARENT, Stroke::new(theme.wire_width + 6.0, with_alpha(theme.accent, strength))));
            }
            draw_wire(&painter, a.pos, b.pos, &a.ty, theme, reg, lit);
        }
    }
    if hovered_wire.is_some() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }

    // Blocks.
    let mut toggle_pin: Option<(String, String)> = None;
    let mut open_embed_for: Option<(String, String)> = None;
    let mut dropped: Option<String> = None;
    let mut drag_zones: Option<Vec<(String, Rect)>> = None;
    for layout in &layouts[..node_count] {
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
            painter.text(layout.header.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, "Missing block", FontId::proportional(13.0), theme.error);
            painter.text(layout.header.left_bottom() + vec2(14.0, ROW_H / 2.0 + 4.0), Align2::LEFT_CENTER, &block_ref, FontId::monospace(11.0), theme.text_muted);
            if body.clicked() {
                *selected = Some((ws_key.to_string(), id.clone()));
                *multi = std::iter::once(id.clone()).collect();
            }
            body.context_menu(|ui| {
                if ui.button("Delete block").clicked() {
                    menu = Some(NodeMenu::Delete(id.clone()));
                }
            });
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
        let shape = node_shapes.get(id);
        let mut open_embed: Option<String> = None;
        for (row, r) in &layout.rows {
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
                    let data = crate::embed::EmbedData::from_value(ws.nodes[id].values.get(key).or(f.default.as_ref()).unwrap_or(&Value::Null));
                    let w = r.width() - 28.0;
                    let l = crate::embed::layout(&painter, &data, w, Some(4), EMBED_SCALE);
                    crate::embed::paint(&painter, r.min + vec2(14.0, 4.0), w, &l);
                    let button = Rect::from_min_size(pos2(r.left() + 14.0, r.top() + 4.0 + l.height + 8.0), vec2(w, 26.0));
                    if cui.put(button, egui::Button::new("Edit embed…")).clicked() {
                        open_embed = Some(key.clone());
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
                    painter.text(r.left_center() + vec2(16.0, 0.0), Align2::LEFT_CENTER, &label, FontId::proportional(12.5), label_color);
                    if *pinned {
                        if !connected {
                            painter.text(r.right_center() - vec2(16.0, 0.0), Align2::RIGHT_CENTER, "connect", FontId::proportional(11.0), with_alpha(theme.text_muted, 140));
                        }
                    } else if widgets::is_inline_editable(f) {
                        let wr = Rect::from_min_max(pos2(r.left() + 16.0 + label_w + 6.0, r.top() + 3.0), pos2(r.right() - 12.0, r.bottom() - 3.0));
                        let mut value = ws.nodes[id].values.get(key).cloned().or_else(|| f.default.clone()).unwrap_or(Value::Null);
                        let wid = cui.id().with(("field", id, key));
                        let changed = cui
                            .scope_builder(UiBuilder::new().max_rect(wr).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| {
                                widgets::value_editor(ui, wid, f, &mut value, wr.width(), false, theme)
                            })
                            .inner;
                        if changed {
                            ws.nodes.get_mut(id).unwrap().values.insert(key.clone(), value);
                            *dirty = true;
                        }
                    }
                }
            }
        }

        if let Some(k) = open_embed {
            open_embed_for = Some((id.clone(), k));
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
            *dirty = true;
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
        let in_zone = ws.nodes[id].zone.is_some();
        let head = head.on_hover_ui(|ui| block_tooltip(ui, def, theme));
        for r in [&head, &body] {
            r.context_menu(|ui| {
                if ui.button("Duplicate").clicked() {
                    menu = Some(NodeMenu::Duplicate(id.clone()));
                }
                if in_zone {
                    if ui.button("Take out of Try Zone").clicked() {
                        menu = Some(NodeMenu::LeaveZone(id.clone()));
                    }
                } else if def.kind != BlockKind::Event && ui.button("Wrap in Try Zone").clicked() {
                    menu = Some(NodeMenu::WrapInZone(id.clone()));
                }
                if ui.button("Open block source").clicked() {
                    menu = Some(NodeMenu::OpenSource(def.source.display().to_string()));
                }
                ui.separator();
                if ui.button("Delete").clicked() {
                    menu = Some(NodeMenu::Delete(id.clone()));
                }
            });
        }

        // Pins.
        for spot in &layout.pins {
            let connected = if spot.dir == Dir::In { ws.links_into(id, &spot.pin).next().is_some() } else { ws.links.iter().any(|l| l.from.node == *id && l.from.pin == spot.pin) };
            let candidate = drag.as_ref().map(|d| drag_compat(d, id, spot, reg));
            draw_pin(&painter, spot, connected, candidate, theme, reg);

            let resp = cui.interact(Rect::from_center_size(spot.pos, vec2(16.0, 16.0)), cui.id().with(("pin", id, &spot.pin, spot.dir)), Sense::click_and_drag());
            if resp.hovered() {
                ctx.set_cursor_icon(CursorIcon::Crosshair);
            }
            if resp.drag_started() {
                // Dragging from a connected input picks up its (latest) wire.
                let existing = (spot.dir == Dir::In).then(|| ws.links_into(id, &spot.pin).last().cloned()).flatten();
                start_drag = Some(match existing {
                    Some(link) => {
                        ws.links.retain(|l| *l != link);
                        *dirty = true;
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
        *embed_edit = Some(crate::embed::EmbedEdit { ws: ws_key.to_string(), node, key, draft: crate::embed::EmbedData::from_value(&value) });
    }
    if let Some((node, pin)) = toggle_pin {
        let n = ws.nodes.get_mut(&node).unwrap();
        if let Some(i) = n.pins.iter().position(|p| *p == pin) {
            n.pins.remove(i);
        } else {
            n.pins.push(pin);
        }
        *dirty = true;
    }

    // The wire being dragged, and what happens when it is dropped.
    if let Some(d) = view.drag.clone() {
        let source = pin_at(&(d.node.clone(), d.pin.clone(), d.dir)).map(|s| s.pos);
        if let (Some(a), Some(p)) = (source, pointer_scene) {
            let (from, to) = if d.dir == Dir::Out { (a, p) } else { (p, a) };
            draw_wire(&painter, from, to, &d.ty, theme, reg, 0.0);
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
                    .filter(|(l, s)| s.pos.distance(p) < 14.0 && drag_compat(&d, &l.id, s, reg) != Compat::No)
                    .min_by(|a, b| a.1.pos.distance(p).total_cmp(&b.1.pos.distance(p)))
                    .map(|(l, s)| (l.id.clone(), s.clone()))
            });
            match target {
                Some((node, spot)) => {
                    connect(ws, &d, &node, &spot);
                    *dirty = true;
                }
                // A picked-up wire dropped on nothing is deleted.
                None if d.picked_up.is_some() => {}
                None if over_canvas => {
                    let p = pointer.unwrap();
                    *picker = Some(Picker::new(ws_key, p, to_global.inverse() * p, Some(PendingWire::from(&d))));
                }
                None => {}
            }
        }
    }

    // Background: a click selects the wire under the pointer; right-click opens
    // its menu, or the block picker on empty canvas (Space too).
    if bg.secondary_clicked() {
        if let Some(p) = pointer.or_else(|| ctx.input(|i| i.pointer.interact_pos())) {
            match hovered_wire.clone() {
                Some(link) => view.wire_menu = Some((link, p)),
                None => *picker = Some(Picker::new(ws_key, p, to_global.inverse() * p, None)),
            }
        }
    } else if bg.clicked() {
        *selected = None;
        multi.clear();
        *selected_wire = hovered_wire.clone().map(|l| (ws_key.to_string(), l));
    }
    if selected.is_some() {
        *selected_wire = None;
    }
    if let Some((link, at)) = view.wire_menu.clone() {
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
        let area = egui::Area::new(cui.id().with("wire-menu")).order(egui::Order::Foreground).fixed_pos(at).show(&ctx, |ui| {
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_min_width(140.0);
                if ui.button("Delete wire").clicked() {
                    ws.links.retain(|l| *l != link);
                    *dirty = true;
                    close = true;
                }
            });
        });
        let outside = ctx.input(|i| i.pointer.any_pressed()) && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area.response.rect.contains(p)) && !bg.secondary_clicked();
        if close || outside || !ws.links.contains(&link) {
            view.wire_menu = None;
        }
    }
    let typing = ctx.text_edit_focused();
    if over_canvas && !typing && picker.is_none() && ctx.input(|i| i.key_pressed(Key::Space)) {
        let p = pointer.unwrap();
        *picker = Some(Picker::new(ws_key, p, to_global.inverse() * p, None));
    }

    match menu {
        Some(NodeMenu::Delete(n)) => {
            let ids: Vec<String> = if multi.contains(&n) { multi.drain().collect() } else { vec![n] };
            for id in &ids {
                ws.remove_node(id);
                shapes.remove(&(ws_key.to_string(), id.clone()));
            }
            if selected.as_ref().is_some_and(|(_, s)| ids.contains(s)) {
                *selected = None;
                multi.clear();
            }
            *dirty = true;
        }
        Some(NodeMenu::Duplicate(n)) => {
            let ids: Vec<String> = if multi.contains(&n) { multi.iter().cloned().collect() } else { vec![n] };
            let new_ids = ws.paste(&ws.extract(&ids), [30.0, 30.0]);
            *selected = new_ids.first().map(|f| (ws_key.to_string(), f.clone()));
            *multi = new_ids.into_iter().collect();
            *dirty = true;
        }
        Some(NodeMenu::OpenSource(path)) => {
            let _ = open::that(path);
        }
        Some(NodeMenu::WrapInZone(n)) => {
            let zid = (1..).map(|i| format!("z{i}")).find(|z| !ws.zones.contains_key(z) && !ws.nodes.contains_key(z)).unwrap();
            ws.zones.insert(zid.clone(), circuit_core::Zone { title: "Try".into(), retry: 0, ..Default::default() });
            if let Some(node) = ws.nodes.get_mut(&n) {
                node.zone = Some(zid);
            }
            *dirty = true;
        }
        Some(NodeMenu::LeaveZone(n)) => {
            if let Some(node) = ws.nodes.get_mut(&n) {
                node.zone = None;
            }
            *dirty = true;
        }
        Some(NodeMenu::RemoveZone(zid)) => {
            for node in ws.nodes.values_mut().filter(|n| n.zone.as_deref() == Some(zid.as_str())) {
                node.zone = None;
            }
            *dirty = true;
        }
        None => {}
    }

    if let Some(z) = drag_zones {
        view.drag_zones = z;
    }
    // A dropped block stays in its zone while it is inside the zone's outline
    // from before the drag, joins another zone it was dropped into, or leaves.
    // A zone's only block always stays: the zone moves with it.
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
            *dirty = true;
        }
    }
    // Zones without blocks disappear, with the wires from their outputs.
    let used: std::collections::HashSet<String> = ws.nodes.values().filter_map(|n| n.zone.clone()).collect();
    if ws.zones.keys().any(|z| !used.contains(z)) {
        ws.zones.retain(|z, _| used.contains(z));
        let zones: std::collections::HashSet<String> = ws.zones.keys().cloned().collect();
        ws.links.retain(|l| ws.nodes.contains_key(&l.from.node) || zones.contains(&l.from.node));
        *dirty = true;
    }

    if let Some(b) = selection_box {
        painter.rect(b, 4.0, with_alpha(theme.accent, 30), Stroke::new(1.0, theme.accent), StrokeKind::Inside);
    }
    sharpen_text(&ctx, layer, to_global);

    // Zoom readout.
    let label = format!("{:.0}%", view.zoom * 100.0);
    let r = Rect::from_min_size(outer.left_bottom() + vec2(12.0, -36.0), vec2(64.0, 24.0));
    ui.painter().rect(r, 6.0, theme.panel, Stroke::new(1.0, theme.line), StrokeKind::Inside);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, FontId::monospace(11.0), theme.text_muted);
}

/// Centers the view on all blocks, at 100% when they fit.
fn fit_view(view: &mut CanvasView, outer: Rect, layouts: &[NodeLayout]) {
    view.initialized = true;
    view.zoom = 1.0;
    let Some(bounds) = layouts.iter().map(|l| l.rect).reduce(|a, b| a.union(b)) else {
        view.offset = vec2(40.0, 40.0);
        return;
    };
    let bounds = bounds.expand(40.0);
    view.zoom = (outer.width() / bounds.width()).min(outer.height() / bounds.height()).clamp(0.5, 1.0);
    view.offset = outer.size() / 2.0 - bounds.center().to_vec2() * view.zoom;
}

fn draw_grid(painter: &egui::Painter, visible: Rect, theme: &Theme, zoom: f32) {
    let step = if zoom < 0.5 { GRID * 2.0 } else { GRID };
    let x0 = (visible.left() / step).floor() as i32;
    let y0 = (visible.top() / step).floor() as i32;
    let cols = (visible.width() / step).ceil() as i32 + 1;
    let rows = (visible.height() / step).ceil() as i32 + 1;
    if cols * rows > 40_000 {
        return;
    }
    let r = 1.3 / zoom.max(0.5);
    for i in 0..cols {
        for j in 0..rows {
            painter.circle_filled(pos2((x0 + i) as f32 * step, (y0 + j) as f32 * step), r, theme.canvas_dots);
        }
    }
}

/// The bezier control points of a wire from output `a` to input `b`.
fn wire_points(a: Pos2, b: Pos2) -> [Pos2; 4] {
    let dx = ((b.x - a.x).abs() * 0.5).max(40.0);
    [a, a + vec2(dx, 0.0), b - vec2(dx, 0.0), b]
}

/// How far `p` is from the wire between `a` and `b`.
fn wire_distance(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    let curve = CubicBezierShape::from_points_stroke(wire_points(a, b), false, Color32::TRANSPARENT, Stroke::NONE);
    if !curve.logical_bounding_rect().expand(12.0).contains(p) {
        return f32::INFINITY;
    }
    let pts: Vec<Pos2> = (0..=32).map(|i| curve.sample(i as f32 / 32.0)).collect();
    pts.windows(2)
        .map(|w| {
            let (s, e) = (w[0], w[1]);
            let d = e - s;
            let t = if d.length_sq() > 0.0 { ((p - s).dot(d) / d.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
            p.distance(s + d * t)
        })
        .fold(f32::INFINITY, f32::min)
}

/// `lit` (0..1) makes an Action wire glow while the bot runs through it.
fn draw_wire(painter: &egui::Painter, a: Pos2, b: Pos2, ty: &PinType, theme: &Theme, reg: &Registry, lit: f32) {
    let color = theme.pin_color(ty, reg);
    let points = wire_points(a, b);
    if ty.is_action() {
        let halo = Stroke::new(theme.wire_width * (3.0 + 3.0 * lit), with_alpha(color, (40.0 + 120.0 * lit) as u8));
        painter.add(CubicBezierShape::from_points_stroke(points, false, Color32::TRANSPARENT, halo));
    }
    painter.add(CubicBezierShape::from_points_stroke(points, false, Color32::TRANSPARENT, Stroke::new(theme.wire_width, color)));
}

fn draw_pin(painter: &egui::Painter, spot: &PinSpot, connected: bool, candidate: Option<Compat>, theme: &Theme, reg: &Registry) {
    let color = theme.pin_color(&spot.ty, reg);
    let dim = candidate == Some(Compat::No);
    if let Some(c) = candidate
        && c != Compat::No
    {
        painter.circle_filled(spot.pos, 11.0, with_alpha(color, 60));
    }
    if spot.ty.is_action() {
        let p = spot.pos;
        let tri = vec![p + vec2(-5.0, -7.0), p + vec2(7.0, 0.0), p + vec2(-5.0, 7.0)];
        let fill = if dim { with_alpha(color, 60) } else { color };
        painter.add(Shape::convex_polygon(tri.iter().map(|q| *q + vec2(0.0, 0.0)).collect(), with_alpha(color, 50), Stroke::new(4.0, with_alpha(color, 40))));
        painter.add(Shape::convex_polygon(tri, fill, Stroke::NONE));
        return;
    }
    if spot.faint && !connected {
        painter.circle_stroke(spot.pos, 3.5, Stroke::new(1.2, with_alpha(color, if dim { 40 } else { 110 })));
        return;
    }
    let alpha = if dim { 70 } else { 255 };
    let fill = if connected { with_alpha(color, alpha) } else { theme.node };
    let r = if spot.ty.is_optional() { PIN_R - 0.5 } else { PIN_R };
    painter.circle(spot.pos, r, fill, Stroke::new(2.0, with_alpha(color, alpha)));
}

/// How well the pin under the pointer fits the wire being dragged.
fn drag_compat(d: &PinEnd, node: &str, spot: &PinSpot, reg: &Registry) -> Compat {
    if node == d.node || spot.dir == d.dir {
        return Compat::No;
    }
    let (from, to) = if d.dir == Dir::Out { (&d.ty, &spot.ty) } else { (&spot.ty, &d.ty) };
    from.compat(to, reg)
}

/// Adds the link for a drop of `d` onto `spot` of `node`.
fn connect(ws: &mut Workspace, d: &PinEnd, node: &str, spot: &PinSpot) {
    let (from, to, to_multi) = match d.dir {
        Dir::Out => (PinRef::new(&d.node, &d.pin), PinRef::new(node, &spot.pin), spot.multi),
        Dir::In => (PinRef::new(node, &spot.pin), PinRef::new(&d.node, &d.pin), false),
    };
    let is_action = d.ty.is_action();
    let to_node = to.node.clone();
    let to_pin = to.pin.clone();
    ws.connect(Link { from, to }, !is_action && !to_multi);
    if !is_action
        && let Some(n) = ws.nodes.get_mut(&to_node)
        && !n.pins.contains(&to_pin)
    {
        n.pins.push(to_pin);
    }
}

pub fn block_tooltip(ui: &mut Ui, def: &BlockDef, theme: &Theme) {
    ui.set_max_width(280.0);
    ui.horizontal(|ui| {
        ui.strong(&def.name);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format!("v{}", def.version)).small().color(theme.text_muted));
        });
    });
    ui.label(egui::RichText::new(format!("by {} · {}", def.author, def.category.as_deref().unwrap_or("Other"))).small().color(theme.text_muted));
    ui.label(&def.description);
}

fn pin_tooltip(ui: &mut Ui, spot: &PinSpot, reg: &Registry, theme: &Theme) {
    ui.set_max_width(260.0);
    let color = theme.pin_color(&spot.ty, reg);
    ui.horizontal(|ui| {
        ui.strong(&spot.label);
        let chip = egui::RichText::new(spot.ty.display(reg)).small().color(mix(color, theme.text, 0.25));
        egui::Frame::new().fill(with_alpha(color, 40)).corner_radius(99).inner_margin(egui::Margin::symmetric(7, 1)).show(ui, |ui| ui.label(chip));
    });
    if let Some(d) = &spot.description {
        ui.label(egui::RichText::new(d).color(theme.text_muted));
    }
    if spot.faint {
        ui.label(egui::RichText::new("Click to turn this field into a pin, or drop a wire on it.").small().italics().color(theme.text_muted));
    }
}

/// egui draws text in a zoomed layer from glyphs rendered for 100% and then
/// scales them, which blurs them (at 65% the letters are shrunken bitmaps).
/// This lays every text in the canvas out again at the size it appears on
/// screen, shrinks it back so the layer's zoom brings it to exactly that size,
/// and snaps it to whole pixels.
fn sharpen_text(ctx: &egui::Context, layer: LayerId, to_global: TSTransform) {
    let zoom = to_global.scaling;
    let ppp = ctx.pixels_per_point();
    let texts: Vec<(usize, Pos2, std::sync::Arc<egui::Galley>)> = ctx.graphics(|g| {
        g.get(layer)
            .map(|list| list.all_entries().enumerate().filter_map(|(i, c)| if let egui::Shape::Text(t) = &c.shape { Some((i, t.pos, t.galley.clone())) } else { None }).collect())
            .unwrap_or_default()
    });
    if texts.is_empty() {
        return;
    }
    let rescale = (zoom - 1.0).abs() > 0.001;
    // Laid out before touching the paint lists: both need the context's lock.
    let sharp: Vec<(usize, Pos2, Option<std::sync::Arc<egui::Galley>>)> = texts
        .into_iter()
        .map(|(i, pos, galley)| {
            let screen = to_global * pos;
            let snapped = to_global.inverse() * pos2((screen.x * ppp).round() / ppp, (screen.y * ppp).round() / ppp);
            let big = rescale.then(|| ctx.fonts_mut(|f| f.layout_job(scaled_job(&galley.job, zoom))));
            (i, snapped, big)
        })
        .collect();
    ctx.graphics_mut(|g| {
        let list = g.entry(layer);
        for (i, pos, big) in sharp {
            list.mutate_shape(egui::layers::ShapeIdx(i), |c| {
                if let egui::Shape::Text(t) = &mut c.shape {
                    if let Some(big) = big {
                        t.galley = big;
                        t.transform(TSTransform::from_scaling(1.0 / zoom));
                    }
                    t.pos = pos;
                }
            });
        }
    });
}

/// A copy of a layout job with every size multiplied by `k`.
fn scaled_job(job: &egui::text::LayoutJob, k: f32) -> egui::text::LayoutJob {
    let mut job = job.clone();
    if job.wrap.max_width.is_finite() {
        job.wrap.max_width *= k;
    }
    job.first_row_min_height *= k;
    for s in &mut job.sections {
        s.leading_space *= k;
        let f = &mut s.format;
        f.font_id.size *= k;
        f.extra_letter_spacing *= k;
        f.line_height = f.line_height.map(|h| h * k);
        f.expand_bg *= k;
        f.underline.width *= k;
        f.strikethrough.width *= k;
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_hit_test() {
        let (a, b) = (pos2(0.0, 0.0), pos2(200.0, 100.0));
        assert!(wire_distance(a, b, pos2(2.0, 1.0)) < 3.0, "near the start");
        assert!(wire_distance(a, b, pos2(100.0, 50.0)) < 3.0, "the middle of an S curve");
        assert!(wire_distance(a, b, pos2(100.0, 0.0)) > 10.0);
        assert_eq!(wire_distance(a, b, pos2(500.0, 500.0)), f32::INFINITY);
    }
}
