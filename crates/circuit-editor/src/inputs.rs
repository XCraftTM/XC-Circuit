//! The two text input components used across the app: one-line inputs and
//! text areas that can be resized by dragging their corner.

use eframe::egui::{self, CursorIcon, Id, Margin, Rect, Response, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, Vec2, Widget, vec2};

const PAD: Vec2 = vec2(10.0, 8.0);

/// The height of a TextLine, for controls that sit in a row with one.
pub fn input_height(ui: &Ui) -> f32 {
    ui.text_style_height(&egui::TextStyle::Body) + 2.0 * PAD.y
}
const MIN_ROWS: f32 = 2.0;
const MAX_HEIGHT: f32 = 1200.0;

/// A one-line text input.
pub struct TextLine<'a> {
    value: &'a mut String,
    hint: String,
    password: bool,
    monospace: bool,
    width: Option<f32>,
    compact: bool,
}

impl<'a> TextLine<'a> {
    pub fn new(value: &'a mut String) -> Self {
        TextLine { value, hint: String::new(), password: false, monospace: false, width: None, compact: false }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    /// Hides the text, for tokens and other secrets.
    pub fn password(mut self, on: bool) -> Self {
        self.password = on;
        self
    }

    pub fn monospace(mut self) -> Self {
        self.monospace = true;
        self
    }

    /// A fixed width instead of all the available width.
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    /// Less padding, for tight rows.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
}

impl Widget for TextLine<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let margin = if self.compact { Margin::symmetric(6, 4) } else { Margin::symmetric(PAD.x as i8, PAD.y as i8) };
        let mut edit = TextEdit::singleline(self.value).hint_text(self.hint).password(self.password).margin(margin).desired_width(self.width.unwrap_or(f32::INFINITY));
        if self.monospace {
            edit = edit.font(egui::TextStyle::Monospace);
        }
        // A fixed id, known before the box is drawn, for the values list.
        let id = ui.next_auto_id();
        crate::values::before(ui, id);
        let mut resp = ui.add(edit.id(id));
        if !self.password && crate::values::after(ui, &resp, self.value) {
            resp.mark_changed();
        }
        resp
    }
}

/// A multi-line text input whose height can be changed by dragging the grip
/// in its bottom-right corner (double-click the grip to reset it). The height
/// is remembered per `id_salt`. Longer text scrolls inside.
pub struct TextArea<'a> {
    id_salt: Id,
    value: &'a mut String,
    hint: String,
    rows: usize,
}

impl<'a> TextArea<'a> {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, value: &'a mut String) -> Self {
        TextArea { id_salt: Id::new(id_salt), value, hint: String::new(), rows: 4 }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    /// How many lines tall it starts out.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }
}

impl Widget for TextArea<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let id = ui.make_persistent_id(self.id_salt);
        let height_id = id.with("height");
        let line = ui.text_style_height(&egui::TextStyle::Body);
        let default_h = self.rows as f32 * line + PAD.y * 2.0;
        let min_h = MIN_ROWS * line + PAD.y * 2.0;
        let height = ui.data_mut(|d| d.get_persisted::<f32>(height_id)).unwrap_or(default_h).clamp(min_h, MAX_HEIGHT);

        let (outer, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
        let visuals = ui.visuals().clone();
        ui.painter().rect_filled(outer, visuals.widgets.inactive.corner_radius, visuals.text_edit_bg_color());

        let inner = outer.shrink2(PAD);
        let mut child = ui.new_child(UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(inner.intersect(ui.clip_rect()));
        crate::values::before(ui, id);
        let mut resp = ScrollArea::vertical()
            .id_salt(id.with("scroll"))
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                ui.add(
                    TextEdit::multiline(self.value)
                        .id(id)
                        .hint_text(self.hint)
                        .frame(egui::Frame::NONE)
                        .desired_width(f32::INFINITY)
                        .min_size(vec2(inner.width() - 8.0, inner.height())),
                )
            })
            .inner;
        if crate::values::after(ui, &resp, self.value) {
            resp.mark_changed();
        }

        let border = if resp.has_focus() { visuals.selection.stroke } else if resp.hovered() { visuals.widgets.hovered.bg_stroke } else { visuals.widgets.inactive.bg_stroke };
        ui.painter().rect_stroke(outer, visuals.widgets.inactive.corner_radius, border, StrokeKind::Inside);

        // The grip.
        let grip = Rect::from_min_size(outer.right_bottom() - vec2(15.0, 15.0), vec2(13.0, 13.0));
        let g = ui.interact(grip, id.with("grip"), Sense::click_and_drag()).on_hover_cursor(CursorIcon::ResizeVertical).on_hover_text(t!("Drag to resize, double-click to reset"));
        let color = if g.hovered() || g.dragged() { visuals.selection.stroke.color } else { visuals.weak_text_color() };
        for i in 1..=3 {
            let o = i as f32 * 3.5;
            ui.painter().line_segment([grip.right_bottom() - vec2(o, 1.5), grip.right_bottom() - vec2(1.5, o)], Stroke::new(1.2, color));
        }
        if g.dragged() {
            let h = (height + g.drag_delta().y).clamp(min_h, MAX_HEIGHT);
            ui.data_mut(|d| d.insert_persisted(height_id, h));
        }
        if g.double_clicked() {
            ui.data_mut(|d| d.remove::<f32>(height_id));
        }
        resp
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Event, Modifiers, PointerButton, Pos2, pos2};

    use super::*;

    fn run(ctx: &egui::Context, text: &mut String, events: Vec<Event>) -> Rect {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500.0, 900.0))), events, ..Default::default() };
        let mut rect = Rect::NOTHING;
        let mut out = ctx.run_ui(raw, |ui| {
            let r = ui.add(TextArea::new("t", text).rows(4));
            rect = r.rect;
        });
        out.textures_delta.clear();
        rect
    }

    #[test]
    fn text_area_resizes_by_its_grip() {
        let ctx = egui::Context::default();
        let mut text = String::from("hello");
        let mut area = Rect::NOTHING;
        for _ in 0..2 {
            area = run(&ctx, &mut text, vec![]);
        }
        // The text edit fills the area; its parent rect is the whole input minus padding.
        let outer = area.expand2(PAD);
        let grip = outer.right_bottom() - vec2(8.0, 8.0);
        let before = outer.height();
        run(&ctx, &mut text, vec![Event::PointerMoved(grip)]);
        run(&ctx, &mut text, vec![Event::PointerButton { pos: grip, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]);
        for i in 1..=10 {
            run(&ctx, &mut text, vec![Event::PointerMoved(grip + vec2(0.0, 12.0 * i as f32))]);
        }
        run(&ctx, &mut text, vec![Event::PointerButton { pos: grip + vec2(0.0, 120.0), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
        let after = run(&ctx, &mut text, vec![]).expand2(PAD).height();
        assert!((after - before - 120.0).abs() < 3.0, "grew by {}", after - before);

        // Typing still works.
        let center = pos2(outer.center().x, outer.top() + 20.0);
        run(&ctx, &mut text, vec![Event::PointerMoved(center), Event::PointerButton { pos: center, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }, Event::PointerButton { pos: center, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
        run(&ctx, &mut text, vec![Event::Text("!".into())]);
        assert!(text.contains('!'), "{text}");
    }
}
