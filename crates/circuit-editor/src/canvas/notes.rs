//! Notes: frames with a title and a description, behind the blocks.

use super::*;

/// Colours a note can have, besides the theme's accent.
pub(super) const NOTE_COLORS: [&str; 6] = ["#F5C451", "#3DFF6E", "#4FB3FF", "#B98CFF", "#FF6F91", "#9AA3AB"];

pub(super) const NOTE_MIN: Vec2 = vec2(160.0, 90.0);

pub(super) fn note_rect(n: &Note) -> Rect {
    Rect::from_min_size(pos2(n.rect[0], n.rect[1]), vec2(n.rect[2].max(NOTE_MIN.x), n.rect[3].max(NOTE_MIN.y)))
}

/// Notes: frames with a title and a description, behind everything else.
/// The title band moves a note with the blocks inside it, or alone when the
/// note is set to (Alt swaps that for one drag); the edges and corners resize
/// it, and the pencil button (or a double-click on the title) edits it.
/// What clicking a note's title changes.
pub(super) struct NoteSelection<'a> {
    pub(super) blocks: &'a mut Option<(String, String)>,
    pub(super) multi: &'a mut std::collections::HashSet<String>,
    pub(super) notes: &'a mut std::collections::HashSet<(String, String)>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_notes(cui: &Ui, painter: &egui::Painter, ws: &mut Workspace, ws_key: &str, blocks: &[NodeLayout], theme: &Theme, reg: &Registry, view: &mut CanvasView, edited: &mut bool, sel: NoteSelection, snap_to_grid: bool) {
    let ctx = cui.ctx().clone();
    let zoom = view.zoom;
    // As big on screen at every zoom, like the Try Zone's gear.
    let button = if theme.touch { ZONE_BUTTON_TOUCH } else { ZONE_BUTTON } / zoom;
    let mut delete: Option<String> = None;
    let ids: Vec<String> = ws.notes.keys().cloned().collect();
    for nid in ids {
        let Some(note) = ws.notes.get(&nid).cloned() else { continue };
        let r = note_rect(&note);
        let color = note.color.as_deref().and_then(|c| Color32::from_hex(c).ok()).unwrap_or(theme.accent);
        let pad = 14.0;
        let text_w = (r.width() - 2.0 * pad).max(20.0);
        let title = if note.title.trim().is_empty() { t!("Note").to_string() } else { note.title.clone() };
        let job = egui::text::LayoutJob::simple(title, crate::ui::fonts::bold(15.0), theme.text, (text_w - button - 8.0).max(20.0));
        let title_g = painter.layout_job(crate::ui::fonts::usable(&ctx, job));
        let desc_g = (!note.text.trim().is_empty()).then(|| painter.layout(note.text.clone(), FontId::proportional(12.5), theme.text_muted, text_w));
        let line = title_g.size().y.max(button);
        let header_h = (10.0 + line + desc_g.as_ref().map_or(0.0, |g| 4.0 + g.size().y) + 10.0).min(r.height());
        let header = Rect::from_min_size(r.min, vec2(r.width(), header_h));

        let key = (ws_key.to_string(), nid.clone());
        let chosen = sel.notes.contains(&key);
        let edge = if chosen { Stroke::new(2.5, theme.accent) } else { Stroke::new(1.5, with_alpha(color, 120)) };
        painter.rect(r, 14.0, with_alpha(color, 10), edge, StrokeKind::Inside);
        painter.rect_filled(header, CornerRadius { nw: 14, ne: 14, sw: 0, se: 0 }, with_alpha(color, 30));
        let inside = painter.with_clip_rect(r.intersect(painter.clip_rect()));
        let title_h = title_g.size().y;
        inside.galley(header.min + vec2(pad, 10.0 + (line - title_h) / 2.0), title_g, theme.text);
        if let Some(g) = desc_g {
            inside.galley(header.min + vec2(pad, 10.0 + line + 4.0), g, theme.text_muted);
        }
        // A grip in the bottom-right corner.
        for i in 1..=2 {
            let d = 5.0 * i as f32;
            painter.line_segment([r.right_bottom() - vec2(d + 4.0, 4.0), r.right_bottom() - vec2(4.0, d + 4.0)], Stroke::new(1.2, with_alpha(color, 160)));
        }

        // The title band moves the note.
        let band = cui.interact(header, cui.id().with(("note", &nid)), Sense::click_and_drag());
        if band.hovered() {
            ctx.set_cursor_icon(CursorIcon::Grab);
        }
        // Whether the blocks inside move along: the note's choice, Alt swaps it.
        let carry = note.loose == ctx.input(|i| i.modifiers.alt);
        if band.dragged_by(PointerButton::Primary) {
            let d = band.drag_delta();
            if carry {
                for l in blocks.iter().filter(|l| r.contains_rect(l.rect)) {
                    if let Some(n) = ws.nodes.get_mut(&l.id) {
                        n.at[0] += d.x;
                        n.at[1] += d.y;
                    }
                }
            }
            if let Some(n) = ws.notes.get_mut(&nid) {
                n.rect[0] += d.x;
                n.rect[1] += d.y;
            }
            *edited = true;
        }
        // Dropped: onto the grid, with the blocks it carried.
        if band.drag_stopped()
            && snap_to_grid
            && let Some(n) = ws.notes.get_mut(&nid)
        {
            let d = vec2(snap(n.rect[0]) - n.rect[0], snap(n.rect[1]) - n.rect[1]);
            n.rect[0] += d.x;
            n.rect[1] += d.y;
            if carry {
                let moved = r.translate(band.drag_delta());
                for l in blocks.iter().filter(|l| moved.contains_rect(l.rect.translate(band.drag_delta()))) {
                    if let Some(node) = ws.nodes.get_mut(&l.id) {
                        node.at[0] += d.x;
                        node.at[1] += d.y;
                    }
                }
            }
        }
        // A click selects it (Shift or Ctrl: adds it, or takes it away).
        if band.clicked() {
            if ctx.input(|i| i.modifiers.shift || i.modifiers.command) {
                if !sel.notes.remove(&key) {
                    sel.notes.insert(key.clone());
                }
            } else {
                sel.notes.clear();
                sel.notes.insert(key.clone());
                *sel.blocks = None;
                sel.multi.clear();
            }
        }
        let band = band.on_hover_text(if note.loose {
            t!("Click to select, drag to move the note (hold Alt to move the blocks in it too). Double-click to edit it.")
        } else {
            t!("Click to select, drag to move the note with the blocks in it (hold Alt to move only the note). Double-click to edit it.")
        });

        // The edit button, on the right of the title.
        let pr = Rect::from_center_size(pos2(header.right() - 8.0 / zoom - button / 2.0, header.top() + 10.0 + line / 2.0), Vec2::splat(button));
        let pencil = cui.interact(pr, cui.id().with(("note-edit", &nid)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let fill = if pencil.hovered() { mix(theme.panel, color, 0.25) } else { theme.panel };
        painter.rect(pr, 6.0 / zoom, fill, Stroke::new(1.0 / zoom, with_alpha(color, 120)), StrokeKind::Inside);
        if let Some(path) = reg.icons.get("pencil") {
            egui::Image::from_uri(crate::editor::file_uri(path)).tint(color).paint_at(cui, pr.shrink(pr.width() * 0.22));
        }
        let pencil = pencil.on_hover_text(t!("Edit note"));
        if band.double_clicked() || view.edit_note.as_deref() == Some(nid.as_str()) {
            egui::Popup::open_id(&ctx, egui::Popup::default_response_id(&pencil));
            view.edit_note = None;
        }
        egui::Popup::menu(&pencil).width(300.0).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            let Some(n) = ws.notes.get_mut(&nid) else { return };
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(RichText::new(t!("Note")).strong());
            ui.label(t!("Title"));
            *edited |= ui.add(crate::ui::kit::TextLine::new(&mut n.title)).changed();
            ui.label(t!("Description"));
            *edited |= ui.add(egui::TextEdit::multiline(&mut n.text).desired_rows(4).desired_width(f32::INFINITY)).changed();
            ui.label(t!("Color"));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let choices = std::iter::once(None).chain(NOTE_COLORS.iter().map(|c| Some(c.to_string())));
                for choice in choices {
                    let swatch = choice.as_deref().and_then(|c| Color32::from_hex(c).ok()).unwrap_or(theme.accent);
                    let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                    let chosen = n.color == choice;
                    ui.painter().circle_filled(rect.center(), 9.0, swatch);
                    if chosen || resp.hovered() {
                        ui.painter().circle_stroke(rect.center(), 11.0, Stroke::new(1.5, theme.text));
                    }
                    let tip = if choice.is_none() { t!("The theme's accent") } else { t!("Color") };
                    if resp.on_hover_text(tip).clicked() && !chosen {
                        n.color = choice;
                        *edited = true;
                    }
                }
            });
            let mut carry = !n.loose;
            if crate::ui::kit::toggle_row(ui, theme, &mut carry, t!("Move blocks with the note"), t!("Dragging the title also moves the blocks inside. Hold Alt for the opposite.")).changed() {
                n.loose = !carry;
                *edited = true;
            }
            ui.separator();
            if crate::ui::kit::button(ui, theme, t!("Delete note")).on_hover_text(t!("The blocks in it stay")).clicked() {
                delete = Some(nid.clone());
                ui.close();
            }
        });

        // Edges and corners resize it; registered last, so they win over the band.
        let e = 5.0 / zoom;
        let c = 10.0 / zoom;
        let (left, right, top, bottom) = (r.left(), r.right(), r.top(), r.bottom());
        let handles = [
            ((true, false, false, false), Rect::from_min_max(pos2(left - e, top + c), pos2(left + e, bottom - c)), CursorIcon::ResizeHorizontal),
            ((false, true, false, false), Rect::from_min_max(pos2(right - e, top + c), pos2(right + e, bottom - c)), CursorIcon::ResizeHorizontal),
            ((false, false, true, false), Rect::from_min_max(pos2(left + c, top - e), pos2(right - c, top + e)), CursorIcon::ResizeVertical),
            ((false, false, false, true), Rect::from_min_max(pos2(left + c, bottom - e), pos2(right - c, bottom + e)), CursorIcon::ResizeVertical),
            ((true, false, true, false), Rect::from_center_size(r.left_top(), Vec2::splat(2.0 * c)), CursorIcon::ResizeNwSe),
            ((false, true, false, true), Rect::from_center_size(r.right_bottom(), Vec2::splat(2.0 * c)), CursorIcon::ResizeNwSe),
            ((false, true, true, false), Rect::from_center_size(r.right_top(), Vec2::splat(2.0 * c)), CursorIcon::ResizeNeSw),
            ((true, false, false, true), Rect::from_center_size(r.left_bottom(), Vec2::splat(2.0 * c)), CursorIcon::ResizeNeSw),
        ];
        for (i, ((l, rt, t, b), hr, cursor)) in handles.into_iter().enumerate() {
            let h = cui.interact(hr, cui.id().with(("note-size", &nid, i)), Sense::drag());
            if h.hovered() || h.dragged() {
                ctx.set_cursor_icon(cursor);
            }
            if h.dragged_by(PointerButton::Primary)
                && let Some(n) = ws.notes.get_mut(&nid)
            {
                let d = h.drag_delta();
                let now = note_rect(n);
                let (mut min, mut max) = (now.min, now.max);
                if l {
                    min.x = (min.x + d.x).min(max.x - NOTE_MIN.x);
                }
                if rt {
                    max.x = (max.x + d.x).max(min.x + NOTE_MIN.x);
                }
                if t {
                    min.y = (min.y + d.y).min(max.y - NOTE_MIN.y);
                }
                if b {
                    max.y = (max.y + d.y).max(min.y + NOTE_MIN.y);
                }
                n.rect = [min.x, min.y, max.x - min.x, max.y - min.y];
                *edited = true;
            }
            if h.drag_stopped()
                && snap_to_grid
                && let Some(n) = ws.notes.get_mut(&nid)
            {
                let (x2, y2) = (snap(n.rect[0] + n.rect[2]), snap(n.rect[1] + n.rect[3]));
                let (x, y) = (snap(n.rect[0]), snap(n.rect[1]));
                n.rect = [x, y, (x2 - x).max(NOTE_MIN.x), (y2 - y).max(NOTE_MIN.y)];
            }
        }
    }
    if let Some(id) = delete {
        ws.notes.shift_remove(&id);
        *edited = true;
    }
}
