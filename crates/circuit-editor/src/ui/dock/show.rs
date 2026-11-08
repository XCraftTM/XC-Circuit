//! Drawing the dock and what the pointer does to it: areas, tab bars,
//! separators, windows, and a tab being dragged.

use super::*;
use crate::ui::theme::{Theme, with_alpha};
use eframe::egui::{Align, Color32, CornerRadius, Id, Layout, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use std::hash::Hash;
use std::path::{Path, PathBuf};

/// How far either side of a separator it can be grabbed.
const GRAB: f32 = 4.0;
/// The smallest a part gets by dragging a separator.
const MIN_PART: f32 = 60.0;

/// What the dock asks of the app: what its tabs are called and show, and
/// what happens when one closes or is dropped outside the dock.
pub trait DockViewer<T> {
    fn title(&mut self, tab: &T) -> String;
    /// The icon file shown before the title.
    fn icon(&self, tab: &T) -> Option<PathBuf>;
    fn closeable(&self, _tab: &T) -> bool {
        true
    }
    /// A tab's × (or middle click, or its menu's Close): it's taken out after
    /// this, or replaced by the tab returned (a placeholder keeping its place).
    fn on_close(&mut self, _tab: &T) -> Option<T> {
        None
    }
    /// The tab's content, under its tab bar.
    fn ui(&mut self, ui: &mut Ui, tab: &T);
    /// The smallest a window holding this tab may be made (its content,
    /// without the tab bar).
    fn min_size(&self, _tab: &T) -> egui::Vec2 {
        egui::Vec2::ZERO
    }
    /// More entries for the tab's right-click menu.
    fn tab_menu(&mut self, _ui: &mut Ui, _tab: &T) {}
    /// A tab let go of outside every area: true when the app takes it (it
    /// then leaves the dock); otherwise it gets a window of its own.
    fn take_drop(&mut self, _tab: &T, _pos: Pos2) -> bool {
        false
    }
}

enum Act<T> {
    Close(T),
    Focus(T),
    Drag(T),
    /// Into a window of its own, outside the app's.
    PopOut(T),
    /// The window of its own with this id, back into the app's.
    PutBack(u64),
}

/// What drawing one frame collects.
struct Pass<'t, T> {
    theme: &'t Theme,
    bar_h: f32,
    font: egui::FontId,
    acts: Vec<Act<T>>,
    places: Vec<Place<T>>,
    focused: Option<T>,
    dragged: Option<T>,
    /// Windows dragged by their tab bar, and how far.
    moves: Vec<(u64, egui::Vec2)>,
    /// Drawing a window of its own (outside the app's): its tabs don't drag
    /// (the pointer there isn't the app window's), and it moves as the system's.
    os: bool,
    /// Windows of their own are possible (the system and graphics allow them).
    can_pop: bool,
}

/// Paints an icon file (once its texture is loaded) into `rect`.
pub fn paint_icon(ctx: &egui::Context, painter: &egui::Painter, path: &Path, rect: Rect, tint: Color32) {
    let size = egui::load::SizeHint::Size { width: (rect.width() * 2.0) as u32, height: (rect.height() * 2.0) as u32, maintain_aspect_ratio: true };
    if let Ok(egui::load::TexturePoll::Ready { texture }) = ctx.try_load_texture(&crate::editor::file_uri(path), egui::TextureOptions::LINEAR, size) {
        painter.image(texture.id, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
    }
}

/// Something being dragged, next to the pointer: an icon and a name, as a tab looks.
pub fn paint_ghost(ctx: &egui::Context, theme: &Theme, icon: Option<&Path>, title: &str, pos: Pos2) {
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::unique("dock-ghost")));
    let font = egui::TextStyle::Button.resolve(&ctx.global_style());
    let galley = painter.layout_no_wrap(title.to_string(), font, theme.text);
    let mut rect = Rect::from_min_size(pos + vec2(12.0, 8.0), vec2(galley.size().x + 42.0, 28.0));
    // Near the window's right edge it goes on the pointer's left.
    if rect.right() > ctx.content_rect().right() {
        rect = rect.translate(vec2(-rect.width() - 24.0, 0.0));
    }
    painter.rect_filled(rect, 4.0, theme.panel);
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0, theme.accent), egui::StrokeKind::Inside);
    if let Some(path) = icon {
        paint_icon(ctx, &painter, path, Rect::from_center_size(pos2(rect.left() + 17.0, rect.center().y), vec2(14.0, 14.0)), theme.text);
    }
    painter.galley(pos2(rect.left() + 32.0, rect.center().y - galley.size().y / 2.0), galley, theme.text);
}

/// Where a drop would go: the part it would take up lit, or a line where it would go.
pub fn paint_drop<T>(ctx: &egui::Context, theme: &Theme, drop: &Drop<T>) {
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::unique("dock-drop")));
    if drop.line {
        painter.rect_filled(drop.preview, 1.0, theme.accent);
    } else {
        painter.rect_filled(drop.preview, 4.0, with_alpha(theme.accent, 40));
        painter.rect_stroke(drop.preview, 4.0, Stroke::new(1.5, theme.accent), egui::StrokeKind::Inside);
    }
}

/// How much bigger a window is than the size egui is asked to give it (its outline).
const WINDOW_EDGES: egui::Vec2 = vec2(2.0, 2.0);

/// The left and top edges of a window (`inside` its outline, which is at
/// `now`), and their corner: dragged, where the window goes and how big it
/// gets, no smaller than `least`. `drag` keeps where the drag started.
fn edge_handles(ui: &mut Ui, inside: Rect, id: u64, now: Rect, least: egui::Vec2, drag: &mut Option<(Rect, Pos2)>) -> Option<Rect> {
    let grab = 5.0;
    let left = Rect::from_min_max(inside.left_top(), pos2(inside.left() + grab, inside.bottom()));
    let top = Rect::from_min_max(inside.left_top(), pos2(inside.right(), inside.top() + grab));
    let corner = Rect::from_min_size(inside.left_top(), vec2(grab * 2.0, grab * 2.0));
    let edge = |ui: &mut Ui, r: Rect, salt: u8, cursor: egui::CursorIcon| ui.interact(r, Id::unique(("dock-window-edge", id, salt)), Sense::drag()).on_hover_cursor(cursor);
    let edges = [
        (edge(ui, left, 0, egui::CursorIcon::ResizeHorizontal), true, false),
        (edge(ui, top, 1, egui::CursorIcon::ResizeVertical), false, true),
        (edge(ui, corner, 2, egui::CursorIcon::ResizeNorthWest), true, true),
    ];
    let pointer = ui.ctx().pointer_interact_pos()?;
    for (resp, sideways, up) in edges {
        if resp.drag_started() {
            *drag = Some((now, pointer));
        }
        if resp.dragged()
            && let Some((start, from)) = *drag
        {
            let by = pointer - from;
            let mut min = start.min;
            if sideways {
                min.x = (start.left() + by.x).min(start.right() - least.x);
            }
            if up {
                min.y = (start.top() + by.y).min(start.bottom() - least.y).max(0.0);
            }
            return Some(Rect::from_min_max(min, start.max));
        }
        if resp.drag_stopped() {
            *drag = None;
        }
    }
    None
}

impl<T: Clone + PartialEq + Hash + std::fmt::Debug> Dock<T> {
    /// Draws the main area in what's left of `ui`, the windows over it, and
    /// handles the pointer: tabs clicked, closed, dragged and dropped.
    pub fn show(&mut self, ui: &mut Ui, theme: &Theme, viewer: &mut impl DockViewer<T>) {
        let ctx = ui.ctx().clone();
        let rect = ui.available_rect_before_wrap();
        let mut p = Pass {
            theme,
            bar_h: if theme.touch { 42.0 } else { 32.0 },
            font: egui::TextStyle::Button.resolve(ui.style()),
            acts: Vec::new(),
            places: Vec::new(),
            focused: self.focused().cloned(),
            dragged: self.drag.clone(),
            moves: Vec::new(),
            os: false,
            can_pop: !ctx.embed_viewports(),
        };
        if let Some(main) = &mut self.main {
            node_ui(ui, main, rect, &mut p, viewer, None, &mut Vec::new());
        }
        ui.allocate_rect(rect, Sense::hover());
        // Where the app's window is on the screen: windows of their own are placed from it.
        let screen = ctx.input(|i| i.viewport().inner_rect).map_or(egui::Vec2::ZERO, |r| r.min.to_vec2());
        let mut back = Vec::new();
        let mut keys = Vec::new();
        for w in &mut self.windows {
            if let Some(first) = w.os {
                if !p.can_pop {
                    w.os = None;
                    continue;
                }
                let need = w.node.leaves().iter().flat_map(|l| l.tabs.iter()).map(|t| viewer.min_size(t)).fold(vec2(320.0, 240.0), |a, b| a.max(b + vec2(0.0, p.bar_h)));
                let title = w.node.leaves().first().and_then(|l| l.active_tab()).map(|t| viewer.title(t)).unwrap_or_default();
                // Where it first went, and how big: the system keeps it after.
                let builder = egui::ViewportBuilder::default().with_title(format!("{title} · XC Circuit")).with_position(first.min).with_inner_size(first.size()).with_min_inner_size(need);
                let id = w.id;
                let node = &mut w.node;
                let before = p.places.len();
                p.os = true;
                let (closed, inner) = ctx.show_viewport_immediate(egui::ViewportId::from_hash_of(("dock-os-window", id)), builder, |ui, _| {
                    let all = ui.max_rect();
                    ui.painter().rect_filled(all, 0.0, theme.panel);
                    node_ui(ui, node, all, &mut p, viewer, Some(id), &mut Vec::new());
                    ui.allocate_rect(all, Sense::hover());
                    // Its key presses, for the app's shortcuts (Ctrl+S, F5 …).
                    keys.extend(ui.ctx().input(|i| i.events.iter().filter(|e| matches!(e, egui::Event::Key { pressed: true, .. })).cloned().collect::<Vec<_>>()));
                    ui.ctx().input(|i| (i.viewport().close_requested(), i.viewport().inner_rect))
                });
                p.os = false;
                // Nothing is dropped into it from the app's window.
                p.places.truncate(before);
                if let Some(r) = inner {
                    w.size = r.size();
                }
                if closed {
                    back.push(id);
                }
                continue;
            }
            let frame = egui::Frame::new().fill(theme.panel).stroke(Stroke::new(1.0, theme.line)).corner_radius(8).shadow(ctx.global_style().visuals.popup_shadow);
            // As big as the biggest its tabs need, and its tab bar.
            let need = w.node.leaves().iter().flat_map(|l| l.tabs.iter()).map(|t| viewer.min_size(t)).fold(vec2(320.0, 240.0), |a, b| a.max(b + vec2(0.0, p.bar_h)));
            let mut window = egui::Window::new("dock-window")
                .id(Id::unique(("dock-window", w.id)))
                .title_bar(false)
                .collapsible(false)
                .resizable(true)
                .movable(false)
                .default_pos(w.pos)
                .default_size(w.size.max(need))
                .min_size(need)
                .frame(frame);
            if let Some(to) = w.move_to.take() {
                window = window.current_pos(to);
            }
            if let Some(r) = w.resize_to.take() {
                // Held to that size for a frame, egui keeps it after.
                let size = (r.size() - WINDOW_EDGES).max(need);
                window = window.current_pos(r.min).min_size(size).max_size(size);
                ctx.request_repaint();
            }
            let id = w.id;
            let (pos, size) = (w.pos, w.size);
            let node = &mut w.node;
            let (edge_drag, resize_to) = (&mut w.edge_drag, &mut w.resize_to);
            let shown = window.show(&ctx, |ui| {
                let inside = ui.available_rect_before_wrap();
                node_ui(ui, node, inside, &mut p, viewer, Some(id), &mut Vec::new());
                ui.allocate_rect(inside, Sense::hover());
                // egui's own handles do the right and bottom edges; these the
                // left and top ones (and their corner), on top of the content.
                *resize_to = edge_handles(ui, inside, id, Rect::from_min_size(pos, size), need + WINDOW_EDGES, edge_drag);
            });
            if let Some(shown) = shown {
                w.pos = shown.response.rect.min;
                w.size = shown.response.rect.size();
                w.layer = Some(shown.response.layer_id);
            }
        }
        let Pass { acts, places, moves, .. } = p;
        self.places = places;
        if !keys.is_empty() {
            self.keys.extend(keys);
            ctx.request_repaint();
        }
        // Closed by the system's ×: back into the app's window, nothing lost.
        for id in back {
            self.put_back(id);
        }
        for (id, by) in moves {
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
                w.move_to = Some(w.pos + by);
                ctx.request_repaint();
            }
        }
        for act in acts {
            match act {
                Act::Close(tab) => match viewer.on_close(&tab) {
                    Some(instead) => self.replace(&tab, instead),
                    None => {
                        self.remove(&tab);
                    }
                },
                Act::Focus(tab) => self.focus(&tab),
                Act::Drag(tab) => {
                    self.focus(&tab);
                    self.drag = Some(tab);
                }
                Act::PopOut(tab) => {
                    // Over where it was, a little down and right.
                    let at = self.places.iter().find(|p| p.tabs.contains(&tab)).map_or(Rect::from_min_size(pos2(80.0, 80.0), vec2(640.0, 480.0)), |p| p.rect);
                    let size = at.size().clamp(vec2(360.0, 260.0), vec2(1100.0, 800.0));
                    self.pop_out(&tab, Rect::from_min_size(at.min + screen + vec2(40.0, 40.0), size));
                }
                Act::PutBack(id) => self.put_back(id),
            }
        }
        self.drag_ui(&ctx, theme, viewer);
    }

    /// A tab being dragged: where it would go lights up and it follows the
    /// pointer; let go, it goes there.
    fn drag_ui(&mut self, ctx: &egui::Context, theme: &Theme, viewer: &mut impl DockViewer<T>) {
        let Some(tab) = self.drag.clone() else { return };
        if !self.contains(&tab) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.drag = None;
            return;
        }
        let Some(pos) = ctx.pointer_latest_pos() else { return };
        let drop = self.target_at(pos, Some(&tab), ctx.layer_id_at(pos));
        if ctx.input(|i| !i.pointer.any_down()) {
            self.drag = None;
            match drop {
                Some(drop) => self.move_tab(&tab, &drop.target),
                None if self.over(pos) => {}
                None if viewer.take_drop(&tab, pos) => {
                    self.remove(&tab);
                }
                // Let go of outside the app's window: a window of its own there.
                None if !ctx.content_rect().contains(pos) && !ctx.embed_viewports() => {
                    let size = self.size_of(&tab).unwrap_or(vec2(480.0, 360.0)).clamp(vec2(360.0, 260.0), vec2(1100.0, 800.0));
                    let screen = ctx.input(|i| i.viewport().inner_rect).map_or(egui::Vec2::ZERO, |r| r.min.to_vec2());
                    self.pop_out(&tab, Rect::from_min_size(pos + screen - vec2(24.0, 12.0), size));
                }
                None => {
                    let size = self.size_of(&tab).unwrap_or(vec2(480.0, 360.0)).clamp(vec2(320.0, 240.0), vec2(900.0, 700.0));
                    self.float(&tab, Rect::from_min_size(pos - vec2(24.0, 12.0), size));
                }
            }
            return;
        }
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        if let Some(drop) = &drop {
            paint_drop(ctx, theme, drop);
        }
        let title = viewer.title(&tab);
        paint_ghost(ctx, theme, viewer.icon(&tab).as_deref(), &title, pos);
        ctx.request_repaint();
    }
}

/// A part of the layout: an area, or two parts with a separator between.
fn node_ui<T: Clone + PartialEq + Hash + std::fmt::Debug, V: DockViewer<T>>(ui: &mut Ui, node: &mut Node<T>, rect: Rect, p: &mut Pass<T>, viewer: &mut V, window: Option<u64>, path: &mut Vec<bool>) {
    let s = match node {
        Node::Leaf(leaf) => return leaf_ui(ui, leaf, rect, p, viewer, window),
        Node::Split(s) => s,
    };
    s.rect = rect;
    let across = s.dir == Dir::Horizontal;
    let size = if across { rect.width() } else { rect.height() };
    let at = (size * s.fraction).round().clamp(0.0, (size - 1.0).max(0.0));
    let (first, second, line) = if across {
        let x = rect.left() + at;
        (
            Rect::from_min_max(rect.min, pos2(x, rect.bottom())),
            Rect::from_min_max(pos2(x + 1.0, rect.top()), rect.max),
            Rect::from_min_max(pos2(x, rect.top()), pos2(x + 1.0, rect.bottom())),
        )
    } else {
        let y = rect.top() + at;
        (
            Rect::from_min_max(rect.min, pos2(rect.right(), y)),
            Rect::from_min_max(pos2(rect.left(), y + 1.0), rect.max),
            Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + 1.0)),
        )
    };
    // The separator. A tab's content (a canvas, on a layer of its own) may be
    // over it, so pressing on it claims the drag before that content can.
    let grab = if across { line.expand2(vec2(GRAB, 0.0)) } else { line.expand2(vec2(0.0, GRAB)) };
    let cursor = if across { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::ResizeVertical };
    let id = Id::unique(("dock-separator", window, path.as_slice()));
    let ctx = ui.ctx().clone();
    let pointer = ctx.pointer_latest_pos().filter(|p| grab.contains(*p) && ui.clip_rect().contains(*p));
    // Not under a window or a menu (in a window: that window on top).
    let here = pointer.is_some_and(|p| ctx.layer_id_at(p).is_none_or(|l| l == ui.layer_id() || (l.order == egui::Order::Background && ui.layer_id().order == egui::Order::Background)));
    if here && ctx.dragged_id().is_none() && ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.set_dragged_id(id);
    }
    path.push(false);
    node_ui(ui, &mut s.first, first, p, viewer, window, path);
    path.pop();
    path.push(true);
    node_ui(ui, &mut s.second, second, p, viewer, window, path);
    path.pop();
    let resp = ui.interact(grab, id, Sense::drag());
    let dragged = resp.dragged() || ctx.dragged_id() == Some(id);
    if dragged
        && size > 2.0 * MIN_PART
        && let Some(pos) = ctx.pointer_latest_pos()
    {
        let along = if across { pos.x - rect.left() } else { pos.y - rect.top() };
        s.fraction = (along / size).clamp(MIN_PART / size, 1.0 - MIN_PART / size);
    }
    let lit = dragged || (here && ctx.dragged_id().is_none());
    if lit {
        ctx.set_cursor_icon(cursor);
    }
    let color = if lit { p.theme.accent } else { p.theme.line };
    ui.painter().rect_filled(line, 0.0, color);
}

/// An area: its tab bar, and the tab in front below it.
fn leaf_ui<T: Clone + PartialEq + Hash + std::fmt::Debug, V: DockViewer<T>>(ui: &mut Ui, leaf: &mut Leaf<T>, rect: Rect, p: &mut Pass<T>, viewer: &mut V, window: Option<u64>) {
    let t = p.theme;
    leaf.rect = rect;
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), p.bar_h.min(rect.height())));
    let body = Rect::from_min_max(pos2(rect.left(), bar.bottom()), rect.max);
    leaf.body = body;
    // In a window, its rounded corners.
    let (top, bottom) = if window.is_some() { (CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, CornerRadius { nw: 0, ne: 0, sw: 8, se: 8 }) } else { (CornerRadius::ZERO, CornerRadius::ZERO) };
    let painter = ui.painter_at(rect);
    painter.rect_filled(bar, top, t.frame);
    painter.rect_filled(body, bottom, t.panel);
    painter.hline(bar.x_range(), bar.bottom() - 0.5, Stroke::new(1.0, t.line));
    // A window moves by the empty part of its tab bar (the tabs come after, on top).
    if let Some(id) = window.filter(|_| !p.os) {
        let moved = ui.interact(bar, Id::unique(("dock-window-bar", id, leaf.tabs.first())), Sense::drag());
        if moved.dragged() {
            p.moves.push((id, moved.drag_delta()));
        }
    }

    // The tabs' widths: room for the icon, the title and the ×.
    let galleys: Vec<_> = leaf.tabs.iter().map(|tab| painter.layout_no_wrap(viewer.title(tab), p.font.clone(), Color32::PLACEHOLDER)).collect();
    let closeable: Vec<bool> = leaf.tabs.iter().map(|tab| viewer.closeable(tab)).collect();
    let widths: Vec<f32> = galleys.iter().zip(&closeable).map(|(g, c)| 31.0 + g.size().x + if *c { 30.0 } else { 12.0 }).collect();
    let total: f32 = widths.iter().sum();
    // Tabs that don't fit: the bar scrolls (the wheel over it), and the tab
    // brought to the front is scrolled into view.
    if total > bar.width() && ui.rect_contains_pointer(bar) {
        let d = ui.input(|i| i.smooth_scroll_delta);
        leaf.scroll -= d.x + d.y;
    }
    if leaf.reveal {
        let start: f32 = widths.iter().take(leaf.active).sum();
        let end = start + widths.get(leaf.active).copied().unwrap_or(0.0);
        if start < leaf.scroll {
            leaf.scroll = start;
        } else if end > leaf.scroll + bar.width() {
            leaf.scroll = end - bar.width();
        }
        leaf.reveal = false;
    }
    leaf.scroll = leaf.scroll.clamp(0.0, (total - bar.width()).max(0.0));

    let bar_painter = ui.painter_at(bar);
    let mut x = bar.left() - leaf.scroll;
    let mut tab_rects = Vec::with_capacity(leaf.tabs.len());
    for (i, tab) in leaf.tabs.iter().enumerate() {
        let r = Rect::from_min_size(pos2(x, bar.top()), vec2(widths[i], bar.height()));
        x += widths[i];
        tab_rects.push(r);
        let visible = r.intersect(bar);
        if visible.width() <= 0.0 {
            continue;
        }
        let id = Id::unique(("dock-tab", tab));
        let resp = ui.interact(visible, id, Sense::click_and_drag());
        let active = i == leaf.active;
        // Tabs and their × are drawn by hand: named for screen readers.
        let title = galleys[i].text();
        crate::ui::kit::describe_selected(&resp, egui::Role::Tab, active, title);
        let close = closeable[i].then(|| {
            let c = Rect::from_center_size(pos2(r.right() - 17.0, r.center().y), vec2(18.0, 18.0));
            let close = ui.interact(c.intersect(bar), id.with("close"), Sense::click());
            crate::ui::kit::describe(&close, egui::Role::Button, &tf!("Close {name}", name = title));
            (c, close)
        });
        let focused = p.focused.as_ref() == Some(tab);
        let lifted = p.dragged.as_ref() == Some(tab);
        let hovered = resp.hovered() || close.as_ref().is_some_and(|(_, c)| c.hovered());
        let fade = |c: Color32| if lifted { with_alpha(c, 90) } else { c };
        // The tab in front has the body's colour, over the bar's line.
        let bg = if active { t.panel } else if hovered { t.panel_header } else { t.frame };
        bar_painter.rect_filled(r, 0.0, bg);
        if focused {
            bar_painter.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), 2.0)), 0.0, t.accent);
        }
        let text = fade(if active || hovered { t.text } else { t.text_muted });
        if let Some(path) = viewer.icon(tab) {
            let tint = if focused { fade(t.accent) } else { text };
            paint_icon(ui.ctx(), &bar_painter, &path, Rect::from_center_size(pos2(r.left() + 17.0, r.center().y), vec2(14.0, 14.0)), tint);
        }
        let galley = galleys[i].clone();
        bar_painter.galley(pos2(r.left() + 31.0, r.center().y - galley.size().y / 2.0), galley, text);
        if let Some((c, close)) = &close {
            if close.hovered() {
                bar_painter.rect_filled(*c, 4.0, with_alpha(t.text, 24));
            }
            let color = fade(if close.hovered() || active || hovered { t.text } else { t.text_muted });
            let s = c.shrink(5.0);
            bar_painter.line_segment([s.left_top(), s.right_bottom()], Stroke::new(1.3, color));
            bar_painter.line_segment([s.right_top(), s.left_bottom()], Stroke::new(1.3, color));
            if close.clicked() {
                p.acts.push(Act::Close(tab.clone()));
            }
        }
        if resp.clicked() {
            p.acts.push(Act::Focus(tab.clone()));
        }
        if resp.middle_clicked() && closeable[i] {
            p.acts.push(Act::Close(tab.clone()));
        }
        if resp.drag_started() && !p.os {
            p.acts.push(Act::Drag(tab.clone()));
        }
        let mut close_it = false;
        resp.context_menu(|ui| {
            if closeable[i] && crate::ui::kit::menu_item(ui, t!("Close"), None, true).clicked() {
                close_it = true;
                ui.close();
            }
            // A window of its own goes back; anything else can get one.
            let act = match window {
                Some(id) if p.os => crate::ui::kit::menu_item(ui, t!("Back into the main window"), None, true).clicked().then_some(Act::PutBack(id)),
                _ if p.can_pop => crate::ui::kit::menu_item(ui, t!("Open in its own window"), None, true)
                    .on_hover_text(t!("A window outside the app's, to put on another monitor"))
                    .clicked()
                    .then(|| Act::PopOut(tab.clone())),
                _ => None,
            };
            if let Some(act) = act {
                p.acts.push(act);
                ui.close();
            }
            viewer.tab_menu(ui, tab);
        });
        if close_it {
            p.acts.push(Act::Close(tab.clone()));
        }
    }

    // Clicking into an area makes its tab in front the one the user is in.
    if let Some(tab) = leaf.active_tab()
        && p.focused.as_ref() != Some(tab)
        && ui.input(|i| i.pointer.any_pressed())
        && ui.rect_contains_pointer(body)
    {
        p.acts.push(Act::Focus(tab.clone()));
    }
    if let Some(tab) = leaf.tabs.get(leaf.active) {
        let mut child = ui.new_child(UiBuilder::new().max_rect(body).id_salt(("dock-body", tab)).layout(Layout::top_down(Align::Min)));
        child.set_clip_rect(body.intersect(ui.clip_rect()));
        viewer.ui(&mut child, tab);
    }
    p.places.push(Place { tabs: leaf.tabs.clone(), rect, bar, body, tab_rects, layer: ui.layer_id() });
}
