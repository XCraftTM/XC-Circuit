//! The custom window title bar: dragging, double-click to maximize, window
//! buttons and resize edges, for when the system title bar is turned off.

use eframe::egui::{self, Color32, CursorIcon, Pos2, Rect, ResizeDirection, Sense, Stroke, ViewportCommand, vec2};

use crate::ui::theme::{Theme, mix};

pub const HEIGHT: f32 = 34.0;
const BUTTON_W: f32 = 46.0;
const EDGE: f32 = 5.0;

fn maximized(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false))
}

/// Makes the empty parts of `rect` move the window. Call before adding the
/// title bar's own widgets so they stay clickable.
pub fn drag_region(ui: &mut egui::Ui, rect: Rect) {
    let resp = ui.interact(rect, ui.id().with("title-drag"), Sense::click_and_drag());
    if resp.double_clicked() {
        let max = maximized(ui.ctx());
        ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!max));
    } else if resp.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

pub const SEARCH_W: f32 = 380.0;

/// The search box in the middle of the title bar. Clicking it opens the
/// command palette, which appears on top of it.
pub fn search_box(ui: &mut egui::Ui, rect: Rect, theme: &Theme, label: &str, shortcut: &str) -> bool {
    let resp = ui.interact(rect, ui.id().with("title-search"), Sense::click()).on_hover_text(t!("Search commands, workspaces and blocks"));
    let fill = if resp.hovered() { mix(theme.field, theme.text, 0.06) } else { theme.field };
    let p = ui.painter();
    p.rect(rect, 6.0, fill, Stroke::new(1.0, theme.line), egui::StrokeKind::Inside);
    // Magnifier.
    let c = rect.left_center() + vec2(16.0, -1.0);
    let stroke = Stroke::new(1.4, theme.text_muted);
    p.circle_stroke(c, 4.5, stroke);
    p.line_segment([c + vec2(3.3, 3.3), c + vec2(6.5, 6.5)], stroke);
    p.text(rect.left_center() + vec2(30.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), theme.text_muted);
    p.text(rect.right_center() - vec2(10.0, 0.0), egui::Align2::RIGHT_CENTER, shortcut, egui::FontId::monospace(10.5), mix(theme.text_muted, theme.field, 0.35));
    resp.clicked()
}

#[derive(Clone, Copy)]
enum Button {
    Minimize,
    Maximize,
    Close,
}

/// Minimize, maximize/restore and close, laid out right to left.
pub fn window_buttons(ui: &mut egui::Ui, theme: &Theme) {
    for b in [Button::Close, Button::Maximize, Button::Minimize] {
        let (rect, resp) = ui.allocate_exact_size(vec2(BUTTON_W, HEIGHT), Sense::click());
        let hover = resp.hovered();
        let (bg, fg) = match b {
            Button::Close if hover => (Color32::from_rgb(0xC4, 0x2B, 0x1C), Color32::WHITE),
            _ if hover => (mix(theme.frame, theme.text, 0.1), theme.text),
            _ => (Color32::TRANSPARENT, theme.text_muted),
        };
        let p = ui.painter();
        p.rect_filled(rect, 0.0, bg);
        let c = rect.center();
        let stroke = Stroke::new(1.0, fg);
        match b {
            Button::Minimize => {
                p.line_segment([c + vec2(-5.0, 0.0), c + vec2(5.0, 0.0)], stroke);
            }
            Button::Maximize if maximized(ui.ctx()) => {
                p.rect_stroke(Rect::from_center_size(c + vec2(-1.0, 1.0), vec2(8.0, 8.0)), 1.0, stroke, egui::StrokeKind::Inside);
                p.line_segment([c + vec2(-3.0, -4.0), c + vec2(5.0, -4.0)], stroke);
                p.line_segment([c + vec2(5.0, -4.0), c + vec2(5.0, 4.0)], stroke);
            }
            Button::Maximize => {
                p.rect_stroke(Rect::from_center_size(c, vec2(10.0, 10.0)), 1.0, stroke, egui::StrokeKind::Inside);
            }
            Button::Close => {
                p.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], stroke);
                p.line_segment([c + vec2(5.0, -5.0), c + vec2(-5.0, 5.0)], stroke);
            }
        }
        let tip = match b {
            Button::Minimize => "Minimize",
            Button::Maximize if maximized(ui.ctx()) => "Restore",
            Button::Maximize => "Maximize",
            Button::Close => "Close",
        };
        if resp.on_hover_text(tip).clicked() {
            let ctx = ui.ctx();
            match b {
                Button::Minimize => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
                Button::Maximize => ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized(ctx))),
                Button::Close => ctx.send_viewport_cmd(ViewportCommand::Close),
            }
        }
    }
}

/// The resize direction for a pointer near the window's edges.
fn edge_at(screen: Rect, p: Pos2) -> Option<ResizeDirection> {
    let (l, r) = (p.x - screen.left() < EDGE, screen.right() - p.x < EDGE);
    let (t, b) = (p.y - screen.top() < EDGE, screen.bottom() - p.y < EDGE);
    Some(match (l, r, t, b) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (_, true, true, _) => ResizeDirection::NorthEast,
        (true, _, _, true) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        (true, ..) => ResizeDirection::West,
        (_, true, ..) => ResizeDirection::East,
        (_, _, true, _) => ResizeDirection::North,
        (_, _, _, true) => ResizeDirection::South,
        _ => return None,
    })
}

/// Resizing from the window edges, and a thin border around the window.
/// Runs after the panels so it wins over whatever is under the edge.
pub fn frame_edges(ctx: &egui::Context, theme: &Theme) {
    if maximized(ctx) || ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
        return;
    }
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("window-border")));
    painter.rect_stroke(screen, 0.0, Stroke::new(1.0, theme.line), egui::StrokeKind::Inside);

    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let Some(dir) = edge_at(screen, pos) else { return };
    ctx.set_cursor_icon(match dir {
        ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
        ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::ResizeNeSw,
    });
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_and_corners() {
        let s = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        assert!(matches!(edge_at(s, egui::pos2(2.0, 2.0)), Some(ResizeDirection::NorthWest)));
        assert!(matches!(edge_at(s, egui::pos2(798.0, 300.0)), Some(ResizeDirection::East)));
        assert!(matches!(edge_at(s, egui::pos2(400.0, 598.0)), Some(ResizeDirection::South)));
        assert!(edge_at(s, egui::pos2(400.0, 300.0)).is_none());
    }
}
