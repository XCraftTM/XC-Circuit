//! Wires and pins: drawing them, finding the wire under the pointer, and what a dragged wire may connect to.

use super::*;

/// The bezier control points of a wire from output `a` to input `b`.
pub(super) fn wire_points(a: Pos2, b: Pos2) -> [Pos2; 4] {
    let dx = ((b.x - a.x).abs() * 0.5).max(40.0);
    [a, a + vec2(dx, 0.0), b - vec2(dx, 0.0), b]
}

/// How far `p` is from the wire between `a` and `b`.
pub(super) fn wire_distance(a: Pos2, b: Pos2, p: Pos2) -> f32 {
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
pub(super) fn draw_wire(painter: &egui::Painter, a: Pos2, b: Pos2, ty: &PinType, theme: &Theme, reg: &Registry, lit: f32) {
    let color = theme.pin_color(ty, reg);
    let points = wire_points(a, b);
    if ty.is_action() {
        let halo = Stroke::new(theme.wire_width * (3.0 + 3.0 * lit), with_alpha(color, (40.0 + 120.0 * lit) as u8));
        painter.add(CubicBezierShape::from_points_stroke(points, false, Color32::TRANSPARENT, halo));
    }
    painter.add(CubicBezierShape::from_points_stroke(points, false, Color32::TRANSPARENT, Stroke::new(theme.wire_width, color)));
}

pub(super) fn draw_pin(painter: &egui::Painter, spot: &PinSpot, connected: bool, candidate: Option<Compat>, theme: &Theme, reg: &Registry) {
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

/// How well the pin under the pointer fits the wire being dragged. A wire
/// that would close a loop doesn't fit.
pub(super) fn drag_compat(d: &PinEnd, node: &str, spot: &PinSpot, reg: &Registry, flow: Option<&Flow>) -> Compat {
    if node == d.node || spot.dir == d.dir {
        return Compat::No;
    }
    let (from, to) = if d.dir == Dir::Out { (&d.ty, &spot.ty) } else { (&spot.ty, &d.ty) };
    let fits = from.compat(to, reg);
    if fits != Compat::No && flow.is_some_and(|f| f.would_loop(&drag_link(d, node, spot))) {
        return Compat::No;
    }
    fits
}

/// Whether the dragged wire would fit `spot` but close a loop there.
pub(super) fn drag_loops(d: &PinEnd, node: &str, spot: &PinSpot, reg: &Registry, flow: &Flow) -> bool {
    drag_compat(d, node, spot, reg, None) != Compat::No && flow.would_loop(&drag_link(d, node, spot))
}

/// The wire the dragged one becomes when dropped on `spot` of `node`.
pub(super) fn drag_link(d: &PinEnd, node: &str, spot: &PinSpot) -> Link {
    match d.dir {
        Dir::Out => Link { from: PinRef::new(&d.node, &d.pin), to: PinRef::new(node, &spot.pin) },
        Dir::In => Link { from: PinRef::new(node, &spot.pin), to: PinRef::new(&d.node, &d.pin) },
    }
}

/// Adds the link for a drop of `d` onto `spot` of `node`.
pub(super) fn connect(ws: &mut Workspace, d: &PinEnd, node: &str, spot: &PinSpot) {
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

/// Blocks that work out their outputs when read (rather than when they run).
pub fn is_value_block(ws: &Workspace, reg: &Registry, id: &str) -> bool {
    ws.nodes.get(id).and_then(|n| reg.block(&n.block)).is_some_and(|d| d.kind == BlockKind::Value)
}
