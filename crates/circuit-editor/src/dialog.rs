//! A small kit for dialogs: a title, body and footer, shown in a dock tab or
//! (for short questions) as a modal card over a dimmed backdrop, and the
//! controls that go in it, all drawn from the theme.

use std::path::Path;

use eframe::egui::{self, Align, Color32, CornerRadius, Frame, Id, Layout, Margin, Response, RichText, Sense, Stroke, Ui, vec2};

use crate::editor::file_uri;
use crate::theme::{Theme, mix, with_alpha};

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
    let response = egui::Modal::new(Id::new(spec.id)).frame(card).backdrop_color(Color32::from_black_alpha(150)).show(ctx, |ui| card_contents(ui, theme, &spec, &mut close, &mut content));
    close || response.should_close()
}

/// Shows a dialog inside a dock tab: it can be docked on any side, stacked with
/// other tabs or dragged out into its own window. For editors and settings. The
/// tab's own × closes it. The body scrolls; the footer stays at the bottom.
pub fn tab(ui: &mut Ui, theme: &Theme, spec: Spec, mut content: impl FnMut(&mut Ui, Part)) {
    let id = Id::new(("dialog-tab", spec.id));
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
    if resp.hovered() {
        ui.painter().rect_filled(rect, 7.0, mix(theme.panel_header, theme.text, 0.1));
    }
    let c = rect.center();
    let s = Stroke::new(1.6, if resp.hovered() { theme.text } else { theme.text_muted });
    ui.painter().line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], s);
    ui.painter().line_segment([c + vec2(5.0, -5.0), c + vec2(-5.0, 5.0)], s);
    resp.on_hover_text("Close (Esc)")
}

/// A small uppercase heading that starts a group of fields.
pub fn section(ui: &mut Ui, theme: &Theme, title: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(title.to_uppercase()).size(10.5).strong().color(theme.text_muted));
    ui.add_space(2.0);
}

/// A label, the control, and an optional line of help under it.
pub fn field<R>(ui: &mut Ui, theme: &Theme, label: &str, help: Option<&str>, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.label(RichText::new(label).size(13.0).color(theme.text));
    let r = add(ui);
    if let Some(h) = help {
        ui.label(RichText::new(h).size(11.5).color(theme.text_muted));
    }
    ui.add_space(10.0);
    r
}

/// A full-width text box with comfortable padding.
pub fn text_input(ui: &mut Ui, value: &mut String, hint: &str, password: bool) -> Response {
    ui.add(crate::inputs::TextLine::new(value).hint(hint).password(password))
}

pub fn primary_button(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    let ink = if theme.dark { Color32::from_rgb(0, 24, 28) } else { Color32::WHITE };
    ui.add(
        egui::Button::new(RichText::new(text).strong().color(ink))
            .fill(theme.accent)
            .stroke(Stroke::NONE)
            .corner_radius(7)
            .min_size(vec2(96.0, 32.0)),
    )
}

/// A small framed button, for secondary actions inside a row (where a text
/// link was used before: it is easier to see and to hit with a finger).
pub fn small_button(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(11.5).color(theme.text))
            .fill(theme.panel)
            .stroke(Stroke::new(1.0, theme.node_border))
            .corner_radius(6)
            .min_size(vec2(0.0, 26.0)),
    )
}

/// A framed square button showing an icon (or `fallback` text when the icon is
/// missing), the same height as a text input. `tip` is shown on hover.
pub fn icon_button(ui: &mut Ui, theme: &Theme, icon: Option<&Path>, fallback: &str, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
    let fill = if resp.is_pointer_button_down_on() {
        mix(theme.panel, theme.text, 0.14)
    } else if resp.hovered() {
        mix(theme.panel, theme.text, 0.07)
    } else {
        theme.panel
    };
    let ink = if ui.is_enabled() { theme.text } else { theme.text_muted };
    ui.painter().rect(rect, 7.0, fill, Stroke::new(1.0, theme.node_border), egui::StrokeKind::Inside);
    match icon {
        Some(path) => egui::Image::from_uri(file_uri(path)).tint(ink).paint_at(ui, rect.shrink(9.0)),
        None => {
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, fallback, egui::FontId::proportional(13.0), ink);
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(tip)
}

pub fn secondary_button(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(theme.text))
            .fill(theme.panel)
            .stroke(Stroke::new(1.0, theme.node_border))
            .corner_radius(7)
            .min_size(vec2(84.0, 32.0)),
    )
}

/// A card with a title, a description and a switch on the right.
pub fn toggle_row(ui: &mut Ui, theme: &Theme, on: &mut bool, title: &str, description: &str) -> Response {
    let mut resp = None;
    Frame::new()
        .fill(theme.panel_header)
        .stroke(Stroke::new(1.0, theme.line))
        .corner_radius(9)
        .inner_margin(Margin::symmetric(14, 11))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width() - 48.0);
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.label(RichText::new(title).size(13.0).color(theme.text));
                    ui.label(RichText::new(description).size(11.5).color(theme.text_muted));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    resp = Some(crate::widgets::toggle(ui, on, theme));
                });
            });
        });
    ui.add_space(8.0);
    resp.unwrap()
}

/// A rounded tag such as "Saved" or "Not set".
pub fn pill(ui: &mut Ui, text: &str, color: Color32) {
    Frame::new().fill(with_alpha(color, 40)).corner_radius(99).inner_margin(Margin::symmetric(8, 2)).show(ui, |ui| {
        ui.label(RichText::new(text).size(11.0).strong().color(color));
    });
}

/// A quiet box for read-only text such as a folder path.
pub fn readout(ui: &mut Ui, theme: &Theme, text: &str) {
    Frame::new().fill(theme.field).corner_radius(6).inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(RichText::new(text).monospace().size(12.0).color(theme.text)).truncate());
    });
}
