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

