//! The node canvas: pan/zoom, blocks, pins, wires and wire dragging.

use std::collections::HashMap;

use circuit_core::graph::ACTION_IN;
use circuit_core::{BlockDef, BlockKind, BlockShape, Compat, Link, PinMode, PinRef, PinType, Workspace};
use circuit_plugins::Registry;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Key, LayerId, PointerButton, Pos2, Rect, Response, RichText, Sense,
    Shadow, Shape, Stroke, StrokeKind, Ui, UiBuilder, Vec2, emath::TSTransform, epaint::CubicBezierShape, pos2, vec2,
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
    /// Screen rectangles of the text boxes drawn last frame: (block, field) → box.
    pub text_fields: Vec<((String, String), Rect)>,
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
/// `width` is the block's own width when it was resized.
fn layout_node(id: &str, at: [f32; 2], pins_on: &[String], width: Option<f32>, def: Option<&BlockDef>, shape: Option<&BlockShape>, tall: &HashMap<String, f32>) -> NodeLayout {
    let (Some(def), Some(shape)) = (def, shape) else {
        let rect = Rect::from_min_size(pos2(at[0], at[1]), vec2(NODE_W, HEADER_H + ROW_H + PAD_BOTTOM));
        let header = Rect::from_min_size(rect.min, vec2(NODE_W, HEADER_H));
        return NodeLayout { id: id.into(), rect, header, value_block: false, rows: Vec::new(), pins: Vec::new() };
    };
    let value_block = def.kind == BlockKind::Value;
    let width = width.unwrap_or_else(|| default_width(def));
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
    let Editor { reg, engine, theme, project, shapes, views, selected, dirty, picker, active_workspace, activity, multi, define_ctx, embed_edit, layout_edit, selected_wire, prefs, ui_rects, .. } = ed;
    let Some(project) = project.as_mut() else { return };
    let mode = reg.mode(&project.meta.mode).or_else(|| reg.modes.first());
    let Some(ws) = project.workspaces.get_mut(ws_key) else {
        ui.label(t!("This workspace no longer exists."));
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
        if let Some(s) = &shape {
            // Text fields that are typed into: a label line with the box under it.
            for (key, f) in s.visible_fields().filter(|(k, f)| widgets::is_plain_text(f) && f.pin != PinMode::Only && !node.pins.contains(k)) {
                tall.insert(key.clone(), ROW_H - 4.0 + text_box_h(node, key, f) + 8.0);
            }
        }
        if let Some(s) = &shape {
            for (key, f) in s.visible_fields().filter(|(_, f)| f.type_name == "layout") {
                let lines = crate::layout::summary(node.values.get(key).or(f.default.as_ref()).unwrap_or(&Value::Null)).len().clamp(1, LAYOUT_LINES);
                tall.insert(key.clone(), 8.0 + lines as f32 * LAYOUT_LINE_H + 8.0 + 26.0 + 8.0);
            }
        }
        if let (Some(s), Some(d)) = (&shape, def) {
            for (key, f) in s.visible_fields().filter(|(_, f)| f.type_name == "embed") {
                let data = crate::embed::EmbedData::from_value(node.values.get(key).or(f.default.as_ref()).unwrap_or(&Value::Null));
                let w = node.width.unwrap_or_else(|| default_width(d)) - 28.0;
                tall.insert(key.clone(), crate::embed::layout(&measure, &data, w, Some(4), EMBED_SCALE).height + 44.0);
            }
        }
        layouts.push(layout_node(id, node.at, &node.pins, node.width, def, shape.as_ref(), &tall));
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
    crate::theme::compact(cui.style_mut());
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

    if prefs.show_grid {
        draw_grid(&painter, visible, theme, view.zoom);
    }
    let recent = |id: &str| activity.get(&(ws_key.to_string(), id.to_string()));
    let glow = |id: &str| recent(id).filter(|a| a.error.is_none()).map(|a| (1.0 - a.at.elapsed().as_secs_f32()).max(0.0)).unwrap_or(0.0);

    // How big a pin is to the pointer (in touch mode, about a fingertip).
    let pin_hit = if theme.touch { 30.0 } else { 16.0 };
    let zone_button = if theme.touch { ZONE_BUTTON_TOUCH } else { ZONE_BUTTON };

    // Try Zones: drawn behind everything, dragged by their title band.
    let mut menu: Option<NodeMenu> = None;
    // Screen rectangles of the zones' title bands, for their settings buttons.
    let mut zone_heads: Vec<(String, Rect)> = Vec::new();
    let mut action_bar: Option<BarFor> = None;
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
        zone_heads.push((zid.clone(), to_global * layout.header));
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
        band.on_hover_text(t!("A Try Zone catches errors from the blocks inside it. The gear button sets its title, retries and timeout."));
        for spot in &layout.pins {
            let connected = ws.links.iter().any(|l| l.from.node == zid && l.from.pin == spot.pin);
            let candidate = drag.as_ref().map(|d| drag_compat(d, &zid, spot, reg));
            draw_pin(&painter, spot, connected, candidate, theme, reg);
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
        let a = pin_at(&(link.from.node.clone(), link.from.pin.clone(), Dir::Out));
        let b = pin_at(&(link.to.node.clone(), link.to.pin.clone(), Dir::In));
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
        let a = pin_at(&(link.from.node.clone(), link.from.pin.clone(), Dir::Out));
        let b = pin_at(&(link.to.node.clone(), link.to.pin.clone(), Dir::In));
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
    if hovered_wire.is_some() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }

    // Blocks.
    let mut toggle_pin: Option<(String, String)> = None;
    let mut open_embed_for: Option<(String, String)> = None;
    let mut dropped: Option<String> = None;
    let mut drag_zones: Option<Vec<(String, Rect)>> = None;
    view.text_fields.clear();
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
        let shape = node_shapes.get(id);
        let mut open_embed: Option<String> = None;
        let mut open_layout: Option<String> = None;
        for (row, r) in &layout.rows {
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
                    let data = crate::embed::EmbedData::from_value(ws.nodes[id].values.get(key).or(f.default.as_ref()).unwrap_or(&Value::Null));
                    let w = r.width() - 28.0;
                    let l = crate::embed::layout(&painter, &data, w, Some(4), EMBED_SCALE);
                    crate::embed::paint(&painter, r.min + vec2(14.0, 4.0), w, &l);
                    let button = Rect::from_min_size(pos2(r.left() + 14.0, r.top() + 4.0 + l.height + 8.0), vec2(w, 26.0));
                    if cui.put(button, egui::Button::new(t!("Edit embed…"))).clicked() {
                        open_embed = Some(key.clone());
                    }
                }
                RowKind::Field { key, .. } if shape.and_then(|s| s.field(key)).is_some_and(|f| f.type_name == "layout") => {
                    let f = shape.and_then(|s| s.field(key)).unwrap();
                    let lines = crate::layout::summary(ws.nodes[id].values.get(key).or(f.default.as_ref()).unwrap_or(&Value::Null));
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
                    painter.text(pos2(r.left() + 16.0, r.top() + ROW_H / 2.0), Align2::LEFT_CENTER, &label, FontId::proportional(12.5), label_color);
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
                        let offer = focused || ctx.pointer_hover_pos().is_some_and(|p| (to_global * bx).contains(p)) || crate::values::menu_open(&ctx, edit_id);
                        if offer {
                            let values = crate::values::available(ws, id, reg, &mut |n| node_shapes.get(n).cloned());
                            crate::values::set_scope(&ctx, Some(std::sync::Arc::new(values)));
                        }
                        crate::values::before(&cui, edit_id);
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
                                    if crate::values::after(ui, &resp, &mut text) {
                                        resp.mark_changed();
                                    }
                                    resp
                                })
                                .inner
                            })
                            .inner;
                        if offer {
                            crate::values::set_scope(&ctx, None);
                        }
                        if resp.changed() {
                            ws.nodes.get_mut(id).unwrap().values.insert(key.clone(), Value::String(text));
                            *dirty = true;
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
                            *dirty = true;
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
                            *dirty |= d != Vec2::ZERO;
                        }
                        view.text_fields.push(((id.clone(), key.clone()), to_global * bx));
                    } else if widgets::is_inline_editable(f) {
                        let wr = Rect::from_min_max(pos2(r.left() + 16.0 + label_w + 6.0, r.top() + 3.0), pos2(r.right() - 12.0, r.bottom() - 3.0));
                        let mut value = ws.nodes[id].values.get(key).cloned().or_else(|| f.default.clone()).unwrap_or(Value::Null);
                        let wid = cui.id().with(("field", id, key));
                        let changed = cui
                            // A fixed id: number boxes keep their editing state while typing.
                            .scope_builder(UiBuilder::new().id_salt(("field-ui", id, key)).max_rect(wr).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| {
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
        if let Some(k) = open_layout {
            let value = ws.nodes[id].values.get(&k).cloned().or_else(|| shape.and_then(|s| s.field(&k)).and_then(|f| f.default.clone())).unwrap_or(Value::Null);
            *layout_edit = Some(crate::layout::LayoutEdit::new(ws_key, id, &k, &value));
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
        for spot in &layout.pins {
            let connected = if spot.dir == Dir::In { ws.links_into(id, &spot.pin).next().is_some() } else { ws.links.iter().any(|l| l.from.node == *id && l.from.pin == spot.pin) };
            let candidate = drag.as_ref().map(|d| drag_compat(d, id, spot, reg));
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
        *embed_edit = Some(crate::embed::EmbedEdit { ws: ws_key.to_string(), node, key, draft: crate::embed::EmbedData::from_value(&value), json: None });
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
                    .filter(|(l, s)| s.pos.distance(p) < pin_hit * 0.9 && drag_compat(&d, &l.id, s, reg) != Compat::No)
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
        *selected_wire = hovered_wire.clone().map(|l| (ws_key.to_string(), l));
    }
    if selected.is_some() {
        *selected_wire = None;
    }
    let typing = ctx.text_edit_focused();
    if over_canvas && !typing && picker.is_none() && ctx.input(|i| i.key_pressed(Key::Space)) {
        let p = pointer.unwrap();
        *picker = Some(Picker::new(ws_key, p, to_global.inverse() * p, None));
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

    // Toolbar, bottom left.
    let bar_h = if theme.touch { 44.0 } else { 34.0 };
    let bar = Rect::from_min_size(outer.left_bottom() + vec2(12.0, -(bar_h + 12.0)), vec2(outer.width() - 24.0, bar_h));
    tools(bar, row).scope(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let center = outer.center();
        if tool_button(ui, theme, icon("plus").as_deref(), t!("Add block")).on_hover_text(t!("Add a block (Space or right-click on the canvas)")).clicked() {
            *picker = Some(Picker::new(ws_key, center, to_global.inverse() * center, None));
        }
        ui.add_space(6.0);
        if crate::dialog::icon_button(ui, theme, icon("zoom-out").as_deref(), "−", t!("Zoom out")).clicked() {
            view.zoom_at(outer.min, center, 1.0 / 1.25);
        }
        let percent = format!("{:.0}%", view.zoom * 100.0);
        if tool_button(ui, theme, None, &percent).on_hover_text(t!("Back to 100%")).clicked() {
            view.zoom_at(outer.min, center, 1.0 / view.zoom);
        }
        if crate::dialog::icon_button(ui, theme, icon("zoom-in").as_deref(), "+", t!("Zoom in")).clicked() {
            view.zoom_at(outer.min, center, 1.25);
        }
        if crate::dialog::icon_button(ui, theme, icon("maximize").as_deref(), t!("Fit"), t!("Show every block")).clicked() {
            fit_view(view, outer, &layouts[..node_count]);
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
            *dirty = true;
        }
    }

    // Each zone's settings button, on the right of its title band.
    for (zid, head) in &zone_heads {
        let side = if theme.touch { zone_button } else { zone_button.min(head.height() - 2.0).max(18.0) };
        let r = Rect::from_center_size(head.right_center() - vec2(side / 2.0 + 6.0, 0.0), Vec2::splat(side));
        if !outer.contains_rect(r) {
            continue;
        }
        let t = tools(r, row);
        let gear = t.interact(r, t.id().with(("zone-settings", zid)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let fill = if gear.hovered() { mix(theme.panel, theme.accent, 0.25) } else { theme.panel };
        t.painter().rect(r, 6.0, fill, Stroke::new(1.0, with_alpha(theme.accent, 120)), StrokeKind::Inside);
        if let Some(path) = icon("settings") {
            egui::Image::from_uri(crate::editor::file_uri(&path)).tint(theme.accent).paint_at(&t, r.shrink(r.width() * 0.2));
        }
        let gear = gear.on_hover_text(t!("Try Zone settings"));
        egui::Popup::menu(&gear).width(240.0).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            let Some(z) = ws.zones.get_mut(zid) else { return };
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(RichText::new(t!("Try Zone")).strong());
            ui.label(t!("Title"));
            *dirty |= ui.add(crate::inputs::TextLine::new(&mut z.title)).changed();
            ui.horizontal(|ui| {
                ui.label(t!("Retries"));
                *dirty |= ui.add(egui::DragValue::new(&mut z.retry).range(0..=10)).changed();
            });
            ui.horizontal(|ui| {
                ui.label(t!("Timeout (s)"));
                let mut secs = z.timeout_ms.map(|t| t as f64 / 1000.0).unwrap_or(0.0);
                if ui.add(egui::DragValue::new(&mut secs).range(0.0..=600.0).speed(0.5)).changed() {
                    z.timeout_ms = (secs > 0.0).then_some((secs * 1000.0) as u64);
                    *dirty = true;
                }
            });
            ui.separator();
            if crate::dialog::button(ui, theme, t!("Remove Try Zone")).clicked() {
                menu = Some(NodeMenu::RemoveZone(zid.clone()));
                ui.close();
            }
        });
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
    for l in &layouts[..node_count] {
        ui_rects.insert(format!("node:{}", l.id), to_global * l.rect);
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

/// A framed button for the canvas toolbars: an icon and a label.
fn tool_button(ui: &mut Ui, theme: &Theme, icon: Option<&std::path::Path>, text: &str) -> Response {
    let text = RichText::new(text).size(12.5).color(theme.text);
    let button = match icon {
        Some(p) => egui::Button::image_and_text(egui::Image::from_uri(crate::editor::file_uri(p)).tint(theme.text).fit_to_exact_size(vec2(14.0, 14.0)), text),
        None => egui::Button::new(text),
    };
    let h = ui.spacing().interact_size.y.max(32.0);
    crate::dialog::styled_button(ui, theme, button.corner_radius(7).min_size(vec2(h, h)), theme.panel, Stroke::new(1.0, theme.node_border))
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
        ui.label(egui::RichText::new(t!("Click to turn this field into a pin, or drop a wire on it.")).small().italics().color(theme.text_muted));
    }
}

/// How wide a block is before it is resized.
fn default_width(def: &BlockDef) -> f32 {
    def.width.unwrap_or(if def.preview.as_deref() == Some("embed") { EMBED_NODE_W } else { NODE_W })
}

/// How tall a text box on a block is: as it was resized, or as it starts.
fn text_box_h(node: &circuit_core::Node, key: &str, f: &circuit_core::FieldDef) -> f32 {
    node.heights.get(key).copied().unwrap_or(if f.type_name == "textarea" { TEXT_AREA_H } else { TEXT_BOX_H })
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

    /// Runs one frame of a workspace canvas headlessly.
    fn frame(ctx: &egui::Context, ed: &mut Editor, ws: &str, events: Vec<egui::Event>) {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0))), events, ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| show(ui, ed, ws));
        out.textures_delta.clear();
    }

    #[test]
    fn typing_a_brace_offers_earlier_values() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        let ctx = egui::Context::default();
        let settle = |ed: &mut Editor| {
            for _ in 0..6 {
                frame(&ctx, ed, "rank-command", vec![]);
            }
        };
        settle(&mut ed);
        let field = ("4".to_string(), "text".to_string());
        let rect = ed.views["rank-command"].text_fields.iter().find(|(k, _)| *k == field).map(|(_, r)| *r).unwrap();
        let p = rect.center();
        let press = |pressed: bool| egui::Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Default::default() };
        frame(&ctx, &mut ed, "rank-command", vec![egui::Event::PointerMoved(p), press(true), press(false)]);
        settle(&mut ed);
        let key = |key: Key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        frame(&ctx, &mut ed, "rank-command", vec![key(Key::End), key(Key::Enter)]);
        frame(&ctx, &mut ed, "rank-command", vec![egui::Event::Text("Hi {us".into())]);
        settle(&mut ed);
        // The list is open; Enter puts in the first match instead of a new line.
        frame(&ctx, &mut ed, "rank-command", vec![key(Key::Enter)]);
        settle(&mut ed);
        let text = ed.workspace("rank-command").unwrap().nodes["4"].values["text"].as_str().unwrap().to_string();
        assert!(text.ends_with("
Hi {user}"), "{text:?}");
        // Typing goes on after the placeholder.
        frame(&ctx, &mut ed, "rank-command", vec![egui::Event::Text("!".into())]);
        let text = ed.workspace("rank-command").unwrap().nodes["4"].values["text"].as_str().unwrap().to_string();
        assert!(text.ends_with("{user}!"), "{text:?}");
    }

    #[test]
    fn example_action_paths_dont_split() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut dirs = vec![root.join("examples/DemoBot")];
        dirs.extend(std::fs::read_dir(root.join("examples/templates")).unwrap().map(|e| e.unwrap().path()));
        for dir in dirs {
            let project = circuit_core::Project::load(&dir).unwrap();
            for (key, ws) in &project.workspaces {
                assert!(ws.split_paths().is_empty(), "{} {key}: {:?}", dir.display(), ws.split_paths());
            }
        }
    }

    #[test]
    fn blocks_in_the_examples_dont_overlap() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut dirs = vec![root.join("examples/DemoBot")];
        dirs.extend(std::fs::read_dir(root.join("examples/templates")).unwrap().map(|e| e.unwrap().path()));
        let mut overlaps = Vec::new();
        for dir in dirs {
            let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
            ed.open_project(&dir);
            let keys: Vec<String> = ed.project.as_ref().unwrap().workspaces.keys().cloned().collect();
            for ws in keys {
                let ctx = egui::Context::default();
                ed.ui_rects.clear();
                for _ in 0..3 {
                    frame(&ctx, &mut ed, &ws, vec![]);
                }
                let nodes: Vec<(&String, &Rect)> = ed.ui_rects.iter().filter(|(k, _)| k.starts_with("node:")).collect();
                for (i, (a, ra)) in nodes.iter().enumerate() {
                    for (b, rb) in &nodes[i + 1..] {
                        if ra.shrink(1.0).intersects(**rb) {
                            overlaps.push(format!("{} {ws}: {a} and {b}", dir.file_name().unwrap().to_string_lossy()));
                        }
                    }
                }
            }
        }
        assert!(overlaps.is_empty(), "blocks overlap:
{}", overlaps.join("
"));
    }

    /// Moves overlapping blocks in the examples down until they fit:
    /// `cargo test -p circuit-editor -- --ignored spread_example_blocks`.
    #[test]
    #[ignore]
    fn spread_example_blocks() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut dirs = vec![root.join("examples/DemoBot")];
        dirs.extend(std::fs::read_dir(root.join("examples/templates")).unwrap().map(|e| e.unwrap().path()));
        for dir in dirs {
            let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
            ed.open_project(&dir);
            let keys: Vec<String> = ed.project.as_ref().unwrap().workspaces.keys().cloned().collect();
            for ws in keys {
                let ctx = egui::Context::default();
                ed.ui_rects.clear();
                for _ in 0..3 {
                    frame(&ctx, &mut ed, &ws, vec![]);
                }
                let zoom = ed.views[&ws].zoom;
                let w = ed.workspace(&ws).unwrap();
                // Scene rectangles, top to bottom.
                let mut rects: Vec<(String, Rect)> = w
                    .nodes
                    .iter()
                    .filter_map(|(id, n)| ed.ui_rects.get(&format!("node:{id}")).map(|r| (id.clone(), Rect::from_min_size(pos2(n.at[0], n.at[1]), r.size() / zoom))))
                    .collect();
                rects.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
                let mut placed: Vec<Rect> = Vec::new();
                let mut moved = Vec::new();
                for (id, mut r) in rects {
                    while let Some(hit) = placed.iter().find(|p| p.expand(12.0).intersects(r)) {
                        r = r.translate(vec2(0.0, hit.bottom() + 30.0 - r.top()));
                    }
                    placed.push(r);
                    moved.push((id, r.min));
                }
                let w = ed.project.as_mut().unwrap().workspaces.get_mut(&ws).unwrap();
                for (id, at) in moved {
                    w.nodes.get_mut(&id).unwrap().at = [at.x, at.y];
                }
            }
            ed.project.as_mut().unwrap().save().unwrap();
        }
    }

    #[test]
    fn text_fields_keep_focus_and_their_size() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        let ctx = egui::Context::default();
        let settle = |ed: &mut Editor| {
            for _ in 0..6 {
                frame(&ctx, ed, "rank-command", vec![]);
            }
        };
        settle(&mut ed);
        let field = ("4".to_string(), "text".to_string());
        let box_of = |ed: &Editor| ed.views["rank-command"].text_fields.iter().find(|(k, _)| *k == field).map(|(_, r)| *r).unwrap();
        let text = |ed: &Editor| ed.workspace("rank-command").unwrap().nodes["4"].values["text"].as_str().unwrap().to_string();
        let press = |pos: Pos2, pressed: bool| egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() };

        // The box is several lines tall from the start (this one is for long text).
        let before = box_of(&ed);
        let zoom = ed.views["rank-command"].zoom;
        assert!((before.height() - TEXT_AREA_H * zoom).abs() < 1.0, "starts {} px tall", before.height());

        // Clicking into it and typing, then typing again later, all goes in.
        let p = before.center();
        frame(&ctx, &mut ed, "rank-command", vec![egui::Event::PointerMoved(p), press(p, true), press(p, false)]);
        settle(&mut ed);
        frame(&ctx, &mut ed, "rank-command", vec![egui::Event::Text("!!".into())]);
        settle(&mut ed);
        frame(&ctx, &mut ed, "rank-command", vec![egui::Event::Text("?".into())]);
        assert!(text(&ed).ends_with("!!?"), "typing stays in the box: {}", text(&ed));

        // Dragging the corner grip down and right makes it taller and wider, and the block with it.
        let grip = before.right_bottom() - vec2(8.0, 8.0);
        for _ in 0..60 {
            frame(&ctx, &mut ed, "rank-command", vec![egui::Event::PointerMoved(grip)]);
        }
        frame(&ctx, &mut ed, "rank-command", vec![press(grip, true)]);
        for step in 1..=10 {
            frame(&ctx, &mut ed, "rank-command", vec![egui::Event::PointerMoved(grip + vec2(8.0 * step as f32, 12.0 * step as f32))]);
        }
        frame(&ctx, &mut ed, "rank-command", vec![press(grip + vec2(80.0, 120.0), false)]);
        settle(&mut ed);
        let after = box_of(&ed);
        assert!((after.height() - before.height() - 120.0).abs() < 3.0, "grew {} px", after.height() - before.height());
        assert!((after.width() - before.width() - 80.0).abs() < 3.0, "grew {} px wider", after.width() - before.width());
        let node = &ed.workspace("rank-command").unwrap().nodes["4"];
        assert!((node.heights["text"] * zoom - after.height()).abs() < 2.0, "the height is kept with the block: {}", node.heights["text"]);
        assert!(node.width.is_some_and(|w| (w - NODE_W - 80.0 / zoom).abs() < 3.0), "so is the width: {:?}", node.width);

        // Double-clicking the grip puts the size back.
        let grip = after.right_bottom() - vec2(8.0, 8.0);
        for _ in 0..60 {
            frame(&ctx, &mut ed, "rank-command", vec![egui::Event::PointerMoved(grip)]);
        }
        for pressed in [true, false, true, false] {
            frame(&ctx, &mut ed, "rank-command", vec![press(grip, pressed)]);
        }
        settle(&mut ed);
        let node = &ed.workspace("rank-command").unwrap().nodes["4"];
        assert!(!node.heights.contains_key("text") && node.width.is_none(), "the size is reset");
        assert!((box_of(&ed).size() - before.size()).length() < 1.0);
    }

    #[test]
    fn toolbar_buttons_show_the_pointer() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        let ctx = egui::Context::default();
        ed.theme.apply(&ctx);
        // Add block is the first button of the toolbar, bottom left.
        let over = pos2(60.0, 900.0 - 12.0 - 17.0);
        let mut cursor = egui::CursorIcon::Default;
        for _ in 0..4 {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0))), events: vec![egui::Event::PointerMoved(over)], ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| show(ui, &mut ed, "rank-command"));
            out.textures_delta.clear();
            cursor = out.platform_output.cursor_icon;
        }
        assert_eq!(cursor, egui::CursorIcon::PointingHand);
    }

    #[test]
    fn wire_hit_test() {
        let (a, b) = (pos2(0.0, 0.0), pos2(200.0, 100.0));
        assert!(wire_distance(a, b, pos2(2.0, 1.0)) < 3.0, "near the start");
        assert!(wire_distance(a, b, pos2(100.0, 50.0)) < 3.0, "the middle of an S curve");
        assert!(wire_distance(a, b, pos2(100.0, 0.0)) > 10.0);
        assert_eq!(wire_distance(a, b, pos2(500.0, 500.0)), f32::INFINITY);
    }
}
