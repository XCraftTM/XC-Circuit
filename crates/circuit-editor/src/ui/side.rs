//! The pieces a side bar view is made of: sections that fold away under a
//! heading with a count and its own small buttons, rows that light up under
//! the pointer, a search box and count badges.

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui::{self, Align, Align2, Color32, CursorIcon, FontId, Id, Layout, Rect, Response, Sense, Stroke, Ui, vec2};

use crate::editor::file_uri;
use crate::ui::theme::{Theme, mix, with_alpha};

/// How tall a section's heading is.
pub const HEAD: f32 = 24.0;

/// A small round count, as at the end of a section's heading.
pub fn badge(ui: &mut Ui, theme: &Theme, count: usize) -> Response {
    let text = count.to_string();
    let galley = ui.painter().layout_no_wrap(text, FontId::proportional(10.5), theme.text);
    let size = vec2((galley.size().x + 10.0).max(18.0), 16.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, 8.0, mix(theme.panel, theme.text, 0.16));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, theme.text);
    resp
}

/// A small icon button for a heading or a row: 22 px, lit under the pointer.
pub fn action(ui: &mut Ui, theme: &Theme, icons: &HashMap<String, PathBuf>, icon: &str, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
    crate::ui::kit::describe(&resp, egui::Role::Button, tip);
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, mix(theme.panel, theme.text, 0.12));
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let ink = if resp.hovered() { theme.text } else { theme.text_muted };
    match icons.get(icon) {
        Some(path) => egui::Image::from_uri(file_uri(path)).tint(ink).paint_at(ui, rect.shrink(4.0)),
        None => {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, "•", FontId::proportional(13.0), ink);
        }
    }
    resp.on_hover_text(tip)
}

/// Whether the section `id` is unfolded (kept between runs).
fn is_open(ui: &Ui, id: Id) -> bool {
    ui.ctx().data_mut(|d| *d.get_persisted_mut_or(id, true))
}

/// A section of a side bar view: a heading with an arrow, its `title`, the
/// buttons `actions` adds (right to left, shown while the pointer is over
/// the heading) and a `count`; under it `body`, while unfolded. Clicking
/// the heading folds it away or brings it back.
pub fn section(ui: &mut Ui, theme: &Theme, key: &str, title: &str, count: Option<usize>, actions: impl FnOnce(&mut Ui), body: impl FnOnce(&mut Ui)) {
    let id = Id::unique(("side-section", key));
    let mut open = is_open(ui, id);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), HEAD), Sense::click());
    crate::ui::kit::describe_selected(&resp, egui::Role::Button, open, title);
    let hovered = ui.rect_contains_pointer(rect);
    let p = ui.painter();
    p.hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, theme.line));
    if hovered {
        p.rect_filled(rect.shrink2(vec2(0.0, 1.0)), 0.0, mix(theme.panel, theme.text, 0.04));
    }
    // The arrow: down while unfolded, right while folded.
    let c = egui::pos2(rect.left() + 14.0, rect.center().y);
    let ink = Stroke::new(1.4, theme.text_muted);
    if open {
        p.line_segment([c + vec2(-3.5, -1.5), c + vec2(0.0, 2.0)], ink);
        p.line_segment([c + vec2(0.0, 2.0), c + vec2(3.5, -1.5)], ink);
    } else {
        p.line_segment([c + vec2(-1.5, -3.5), c + vec2(2.0, 0.0)], ink);
        p.line_segment([c + vec2(2.0, 0.0), c + vec2(-1.5, 3.5)], ink);
    }
    p.text(egui::pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, title, FontId::proportional(11.5), theme.text);
    // The count at the end, the section's own buttons before it.
    let mut right = ui.new_child(egui::UiBuilder::new().id_salt(("side-section-right", key)).max_rect(rect.shrink2(vec2(8.0, 0.0))).layout(Layout::right_to_left(Align::Center)));
    right.spacing_mut().item_spacing.x = 2.0;
    if let Some(count) = count {
        badge(&mut right, theme, count);
        right.add_space(4.0);
    }
    if hovered {
        actions(&mut right);
    }
    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
        open = !open;
        ui.ctx().data_mut(|d| d.insert_persisted(id, open));
    }
    if open {
        body(ui);
        ui.add_space(4.0);
    }
}

/// A row of a list in a side bar view, `height` tall and as wide as the
/// view: lit under the pointer or while `picked`. Returns where it is and
/// what happened to it; what it shows is painted by the caller.
pub fn row(ui: &mut Ui, theme: &Theme, height: f32, picked: bool) -> (Rect, Response) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if picked {
        ui.painter().rect_filled(rect, 0.0, with_alpha(theme.accent, 46));
    } else if ui.rect_contains_pointer(rect) {
        ui.painter().rect_filled(rect, 0.0, mix(theme.panel, theme.text, 0.06));
    }
    (rect, resp)
}

/// The search box at the top of a view: as wide as the view, with `hint`
/// while empty. Returns its response.
pub fn search(ui: &mut Ui, theme: &Theme, text: &mut String, hint: &str) -> Response {
    let mut resp = None;
    egui::Frame::new().inner_margin(egui::Margin { left: 12, right: 12, top: 4, bottom: 8 }).show(ui, |ui| {
        let edit = egui::TextEdit::singleline(text).hint_text(hint).desired_width(f32::INFINITY).background_color(theme.field);
        resp = Some(ui.add_sized([ui.available_width(), crate::ui::kit::input_height(ui)], edit));
    });
    resp.expect("drawn")
}

/// A line of muted text with the view's margins: what a view says while it
/// has nothing to list.
pub fn note(ui: &mut Ui, theme: &Theme, text: &str) {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 6)).show(ui, |ui| {
        ui.add(egui::Label::new(egui::RichText::new(text).size(12.0).color(theme.text_muted)).wrap());
    });
}

/// A square picture for a row: the icon at `path` on a tinted tile, or
/// `letters` on it when there is none.
pub fn tile(ui: &Ui, rect: Rect, color: Color32, icon: Option<&std::path::Path>, letters: &str) {
    ui.painter().rect_filled(rect, 6.0, with_alpha(color, 60));
    match icon {
        Some(path) => egui::Image::from_uri(file_uri(path)).tint(Color32::WHITE).paint_at(ui, rect.shrink(rect.width() * 0.22)),
        None => {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, letters, FontId::proportional(rect.height() * 0.38), Color32::WHITE);
        }
    }
}

/// `text` cut to fit `width` in `font`, with … at the end when it was cut.
pub fn fitted(ui: &Ui, text: &str, font: FontId, width: f32) -> String {
    let fits = |s: &str| ui.painter().layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x <= width;
    if fits(text) {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let cut: String = chars[..mid].iter().collect::<String>() + "…";
        if fits(&cut) { lo = mid } else { hi = mid - 1 }
    }
    chars[..lo].iter().collect::<String>() + "…"
}
