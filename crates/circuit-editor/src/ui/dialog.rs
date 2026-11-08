//! A small kit for dialogs: a title, body and footer, shown in a dock tab or
//! (for short questions) as a modal card over a dimmed backdrop, and the
//! controls that go in it, all drawn from the theme.

use std::path::Path;

use eframe::egui::{self, Align, Color32, CornerRadius, Frame, Id, Layout, Margin, Response, RichText, Sense, Stroke, Ui, vec2};

use crate::editor::file_uri;
use crate::ui::theme::{Theme, mix, with_alpha};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Body,
    Footer,
}

pub struct Spec<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub subtitle: Option<&'a str>,
    pub icon: Option<&'a Path>,
    pub width: f32,
}

/// Shows a modal dialog: the rest of the window is dimmed and waits. For short
/// messages and questions. `content` is called once for the body and once for
/// the footer (whose layout runs right to left, so add the primary button first).
/// Returns true when the user asked to close it (×, Escape or a click outside).
pub fn show(ctx: &egui::Context, theme: &Theme, spec: Spec, mut content: impl FnMut(&mut Ui, Part)) -> bool {
    let mut close = false;
    let card = Frame::new()
        .fill(theme.panel)
        .stroke(Stroke::new(1.0, theme.node_border))
        .corner_radius(12)
        .shadow(egui::Shadow { offset: [0, 18], blur: 48, spread: 0, color: Color32::from_black_alpha(140) });
    let response = egui::Modal::new(Id::unique(spec.id)).frame(card).backdrop_color(Color32::from_black_alpha(150)).show(ctx, |ui| card_contents(ui, theme, &spec, &mut close, &mut content));
    close || response.should_close()
}

/// Shows a dialog inside a dock tab: it can be docked on any side, stacked with
/// other tabs or dragged out into its own window. For editors and settings. The
/// tab's own × closes it. The body scrolls; the footer stays at the bottom.
/// Where a dialog tab's body ends on screen, for [`remaining_height`]. Set
/// right before the body is drawn, so one key serves every tab.
const BODY_BOTTOM: &str = "dialog-body-bottom";

/// The room from the cursor down to the bottom of the dialog tab's body (above
/// its footer), but at least `min`. Builders size their lists, previews and JSON
/// boxes with it, so they grow and shrink with the tab.
pub fn remaining_height(ui: &Ui, min: f32) -> f32 {
    let top = ui.cursor().top();
    let bottom = ui.ctx().data(|d| d.get_temp::<f32>(Id::unique(BODY_BOTTOM))).unwrap_or(top + min);
    (bottom - top).max(min)
}

/// A builder's preview on top of its form, for when the tab is too narrow
/// for them side by side: up to `share` of the room down to the footer, and
/// scrolling past that, so the form below keeps the rest.
pub fn preview_on_top(ui: &mut Ui, id: &str, share: f32, add: impl FnOnce(&mut Ui)) {
    let h = (remaining_height(ui, 300.0) * share).max(140.0);
    egui::ScrollArea::vertical().id_salt(id).max_height(h).auto_shrink([false, true]).show(ui, add);
    ui.add_space(12.0);
}

/// Whether the builders show the values from earlier blocks (kept between runs).
const VALUES_SHOWN: &str = "builder-values-shown";
/// The most room a builder's preview takes above the values, as a share of
/// what's left of its column (kept between runs; the line between them sets it).
const PREVIEW_SHARE: &str = "builder-preview-share";

fn values_shown(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| *d.get_persisted_mut_or(Id::unique(VALUES_SHOWN), true))
}

/// The heading of a builder's values from earlier blocks, with a button to
/// hide them (or show them again). True while they're shown.
pub fn values_heading(ui: &mut Ui, theme: &Theme) -> bool {
    let mut shown = values_shown(ui.ctx());
    ui.horizontal(|ui| {
        crate::ui::kit::section(ui, theme, t!("Values from earlier blocks"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (label, help) = if shown { (t!("Hide"), t!("Hide the values, so the preview gets their room")) } else { (t!("Show"), t!("Show the values you can use in ${…}")) };
            let b = ui.add(egui::Button::new(RichText::new(label).size(11.5).color(theme.accent)).frame(false)).on_hover_text(help).on_hover_cursor(egui::CursorIcon::PointingHand);
            if b.clicked() {
                shown = !shown;
                ui.ctx().data_mut(|d| d.insert_persisted(Id::unique(VALUES_SHOWN), shown));
            }
        });
    });
    shown
}

/// A builder's column with the preview on top and the values from earlier
/// blocks under it. The values can be hidden, so the preview gets the whole
/// column, and the line between the two dragged up or down to share it
/// differently; both are kept for every builder.
pub fn preview_and_values(ui: &mut Ui, theme: &Theme, id: &str, preview: impl FnOnce(&mut Ui), values: impl FnOnce(&mut Ui)) {
    crate::ui::kit::section(ui, theme, t!("Preview"));
    let room = remaining_height(ui, 300.0);
    let shown = values_shown(ui.ctx());
    let share_id = Id::unique(PREVIEW_SHARE);
    let share = ui.ctx().data_mut(|d| *d.get_persisted_mut_or(share_id, 0.6f32));
    // The values' heading and the line between take about 44 px.
    let max = if shown { ((room - 44.0) * share).max(120.0) } else { (room - 30.0).max(120.0) };
    let drawn = egui::ScrollArea::vertical().id_salt(id).max_height(max).auto_shrink([false, true]).show(ui, preview).inner_rect;
    if shown {
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 12.0), Sense::hover());
        let grip = ui.interact(line, Id::unique(("builder-split", id)), Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeVertical);
        let active = grip.hovered() || grip.dragged();
        ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, if active { theme.accent } else { theme.line }));
        ui.painter().rect_filled(egui::Rect::from_center_size(line.center(), vec2(36.0, 4.0)), 2.0, if active { theme.accent } else { with_alpha(theme.text_muted, 140) });
        if grip.dragged() && grip.drag_delta().y != 0.0 {
            // From where the preview ends now (it may be shorter than its most).
            let new = ((drawn.height() + grip.drag_delta().y) / (room - 44.0).max(1.0)).clamp(0.1, 0.92);
            ui.ctx().data_mut(|d| d.insert_persisted(share_id, new));
        }
    } else {
        ui.add_space(10.0);
    }
    if values_heading(ui, theme) {
        egui::ScrollArea::vertical().id_salt((id, "values")).max_height(remaining_height(ui, 100.0)).auto_shrink([false, true]).show(ui, values);
    }
}

pub fn tab(ui: &mut Ui, theme: &Theme, spec: Spec, mut content: impl FnMut(&mut Ui, Part)) {
    let id = Id::unique(("dialog-tab", spec.id));
    egui::Panel::bottom(id.with("footer"))
        .resizable(false)
        .frame(Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).inner_margin(Margin::symmetric(16, 12)))
        .show(ui, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                content(ui, Part::Footer);
            });
        });
    egui::CentralPanel::default().frame(Frame::new().fill(theme.panel)).show(ui, |ui| {
        // The body's bottom margin is 18.
        let bottom = ui.max_rect().bottom() - 18.0;
        ui.ctx().data_mut(|d| d.insert_temp(Id::unique(BODY_BOTTOM), bottom));
        egui::ScrollArea::both().id_salt(id.with("body")).auto_shrink([false, false]).show(ui, |ui| {
            // The body is as wide as it was designed for, centred when there is
            // room; in a narrow panel it narrows, and scrolls sideways past that.
            let width = (spec.width - 40.0).min(ui.available_width() - 40.0).max(280.0);
            let side = ((ui.available_width() - width - 40.0) / 2.0).max(0.0);
            ui.horizontal_top(|ui| {
                ui.add_space(side);
                Frame::new().inner_margin(Margin::symmetric(20, 18)).show(ui, |ui| ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.set_width(width);
                    ui.set_max_width(width);
                    ui.horizontal(|ui| {
                        if let Some(icon) = spec.icon {
                            let (rect, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
                            ui.painter().rect_filled(rect, 8.0, with_alpha(theme.accent, 34));
                            egui::Image::from_uri(file_uri(icon)).tint(theme.accent).paint_at(ui, rect.shrink(8.0));
                            ui.add_space(6.0);
                        }
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(RichText::new(spec.title).size(15.5).strong().color(theme.text));
                            if let Some(sub) = spec.subtitle {
                                ui.label(RichText::new(sub).size(12.0).color(theme.text_muted));
                            }
                        });
                    });
                    ui.add_space(14.0);
                    ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                    content(ui, Part::Body);
                }));
            });
        });
    });
}

/// The title bar, body and footer of a dialog card.
fn card_contents(ui: &mut Ui, theme: &Theme, spec: &Spec, close: &mut bool, content: &mut impl FnMut(&mut Ui, Part)) {
    // Fixed width: body rows must fit, so the header and footer always span the card.
    ui.set_width(spec.width);
    ui.set_max_width(spec.width);
    ui.spacing_mut().item_spacing = vec2(8.0, 0.0);

    // Title bar.
    Frame::new()
        .fill(theme.panel_header)
        .corner_radius(CornerRadius { nw: 12, ne: 12, sw: 0, se: 0 })
        .inner_margin(Margin { left: 20, right: 12, top: 14, bottom: 14 })
        .show(ui, |ui| {
            ui.set_width(spec.width - 32.0);
            ui.horizontal(|ui| {
                if let Some(icon) = spec.icon {
                    let (rect, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
                    ui.painter().rect_filled(rect, 8.0, with_alpha(theme.accent, 34));
                    egui::Image::from_uri(file_uri(icon)).tint(theme.accent).paint_at(ui, rect.shrink(8.0));
                    ui.add_space(6.0);
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(RichText::new(spec.title).size(15.5).strong().color(theme.text));
                    if let Some(sub) = spec.subtitle {
                        ui.label(RichText::new(sub).size(12.0).color(theme.text_muted));
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if close_button(ui, theme).clicked() {
                        *close = true;
                    }
                });
            });
        });
    hline(ui, theme);

    // Body.
    Frame::new().inner_margin(Margin::symmetric(20, 18)).show(ui, |ui| {
        ui.set_width(spec.width - 40.0);
        ui.set_max_width(spec.width - 40.0);
        ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        content(ui, Part::Body);
    });

    // Footer.
    hline(ui, theme);
    Frame::new()
        .fill(theme.panel_header)
        .corner_radius(CornerRadius { nw: 0, ne: 0, sw: 12, se: 12 })
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(spec.width - 32.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                content(ui, Part::Footer);
            });
        });
}

fn hline(ui: &mut Ui, theme: &Theme) {
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, theme.line));
}

fn close_button(ui: &mut Ui, theme: &Theme) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
    crate::ui::kit::describe(&resp, egui::Role::Button, t!("Close"));
    if resp.hovered() {
        ui.painter().rect_filled(rect, 7.0, mix(theme.panel_header, theme.text, 0.1));
    }
    let c = rect.center();
    let s = Stroke::new(1.6, if resp.hovered() { theme.text } else { theme.text_muted });
    ui.painter().line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], s);
    ui.painter().line_segment([c + vec2(5.0, -5.0), c + vec2(-5.0, 5.0)], s);
    resp.on_hover_text(t!("Close (Esc)"))
}


#[cfg(test)]
mod tests {
    use super::*;

    /// One frame of a builder column with a tall preview: where the preview's
    /// visible part was drawn, and whether the values were.
    fn column(ctx: &egui::Context, events: Vec<egui::Event>) -> (egui::Rect, bool) {
        let theme = Theme::fallback();
        let (mut preview, mut values) = (egui::Rect::NOTHING, false);
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(600.0, 900.0))), events, ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.ctx().data_mut(|d| d.insert_temp(Id::unique(BODY_BOTTOM), 880.0f32));
                preview_and_values(
                    ui,
                    &theme,
                    "test-preview",
                    |ui| {
                        preview = ui.clip_rect();
                        ui.add_space(2000.0);
                    },
                    |ui| {
                        values = true;
                        ui.add_space(400.0);
                    },
                );
            });
        });
        out.textures_delta.clear();
        (preview, values)
    }

    #[test]
    fn the_values_hide_and_the_preview_takes_their_room() {
        let ctx = egui::Context::default();
        let press = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        column(&ctx, vec![]);
        let (first, values) = column(&ctx, vec![]);
        assert!(values, "the values show at first");

        // Dragging the line under the preview down gives the preview more room.
        let line = egui::pos2(300.0, first.bottom() + 8.0);
        column(&ctx, vec![egui::Event::PointerMoved(line)]);
        column(&ctx, vec![press(line, true)]);
        for step in 1..=6 {
            column(&ctx, vec![egui::Event::PointerMoved(line + vec2(0.0, 10.0 * step as f32))]);
        }
        column(&ctx, vec![press(line + vec2(0.0, 60.0), false)]);
        let (dragged, values) = column(&ctx, vec![]);
        assert!((dragged.height() - first.height() - 60.0).abs() < 3.0, "grew {} px", dragged.height() - first.height());
        assert!(values);

        // Hiding the values gives the preview the whole column.
        ctx.data_mut(|d| d.insert_persisted(Id::unique(VALUES_SHOWN), false));
        let (hidden, values) = column(&ctx, vec![]);
        assert!(!values, "the values are hidden");
        assert!(hidden.height() > dragged.height() + 100.0, "{} vs {}", hidden.height(), dragged.height());
    }
}
