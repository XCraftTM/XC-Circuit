//! The UI kit: every piece of interface the app is made of — buttons,
//! inputs, numbers, sliders, switches, checkboxes, dropdowns, colours, rows,
//! cards, notes, headings, menu items — drawn from the theme at one height,
//! so every page looks and behaves the same. Pages, builders, Bot › Settings
//! (and the elements plugins describe for it) and plugins' panels all use it;
//! draw with these instead of egui's own widgets.

use std::path::Path;

use eframe::egui::{self, Align, Color32, ComboBox, CursorIcon, FontId, Frame, Layout, Margin, Rect, Response, RichText, Role, Sense, Stroke, Ui, pos2, vec2};

use crate::editor::file_uri;
pub use crate::ui::inputs::{TextArea, TextLine, input_height};
use crate::ui::theme::{Theme, mix, with_alpha};

// ── From ui/dialog.rs ────────────────────────────────────────────────

/// A small uppercase heading that starts a group of fields. Its label can
/// name a control under it ([`labelled`]).
pub fn section(ui: &mut Ui, theme: &Theme, title: &str) -> Response {
    ui.add_space(4.0);
    let label = ui.label(RichText::new(title.to_uppercase()).size(10.5).strong().color(theme.text_muted));
    ui.add_space(2.0);
    label
}

/// A label, the control, and an optional line of help under it.
pub fn field<R>(ui: &mut Ui, theme: &Theme, label: &str, help: Option<&str>, add: impl FnOnce(&mut Ui) -> R) -> R {
    let label = ui.label(RichText::new(label).size(13.0).color(theme.text));
    let r = labelled(ui, label.id, add);
    if let Some(h) = help {
        ui.label(RichText::new(h).size(11.5).color(theme.text_muted));
    }
    ui.add_space(10.0);
    r
}

/// A full-width text box with comfortable padding.
pub fn text_input(ui: &mut Ui, value: &mut String, hint: &str, hidden: bool) -> Response {
    ui.add(crate::ui::inputs::TextLine::new(value).hint(hint).hidden(hidden))
}

/// Adds a button with its own fill and border that still reacts to the
/// pointer: lighter (or darker, in light themes) on hover and while pressed.
/// egui's `Button::fill` and `stroke` would turn those effects off.
pub fn styled_button(ui: &mut Ui, theme: &Theme, button: egui::Button, fill: Color32, stroke: Stroke) -> Response {
    // Styled on `ui` itself and put back afterwards, not in a ui.scope: a
    // scope is placed in the room left on the line, so in a wrapping row a
    // button that doesn't fit would stick out instead of starting a new line.
    let before = ui.visuals().widgets.clone();
    let w = &mut ui.visuals_mut().widgets;
    let border = |t: f32| if stroke.width > 0.0 { Stroke::new(stroke.width, mix(stroke.color, theme.text, t)) } else { Stroke::NONE };
    w.inactive.weak_bg_fill = fill;
    w.inactive.bg_stroke = stroke;
    w.hovered.weak_bg_fill = mix(fill, theme.text, 0.10);
    w.hovered.bg_stroke = border(0.35);
    w.active.weak_bg_fill = mix(fill, theme.text, 0.20);
    w.active.bg_stroke = border(0.5);
    let resp = ui.add(button);
    ui.visuals_mut().widgets = before;
    resp
}

pub fn primary_button(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    let ink = if theme.dark { Color32::from_rgb(0, 24, 28) } else { Color32::WHITE };
    let button = egui::Button::new(RichText::new(text).strong().color(ink)).corner_radius(7).min_size(vec2(96.0, ui.spacing().interact_size.y.max(32.0)));
    styled_button(ui, theme, button, theme.accent, Stroke::NONE)
}

/// A framed button for actions inside a row or a panel. Every such button
/// has this size: as tall as a text box, so buttons, boxes and dropdowns in a
/// row line up, and easy to hit with a finger.
/// A rounded chip for filtering a list (the block picker's kinds, the
/// console's levels): filled in `color` when chosen.
pub fn filter_chip(ui: &mut Ui, theme: &Theme, label: &str, color: Color32, on: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::proportional(12.0), if on { color } else { theme.text_muted });
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(20.0, 8.0), Sense::click());
    let fill = if on {
        with_alpha(color, 40)
    } else if resp.hovered() {
        theme.panel_header
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect(rect, 99.0, fill, Stroke::new(1.0, if on { with_alpha(color, 120) } else { theme.line }), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, if on || resp.hovered() { color } else { theme.text_muted });
    describe_selected(&resp, Role::Button, on, label);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn button(ui: &mut Ui, theme: &Theme, text: impl Into<RichText>) -> Response {
    framed(ui, theme, text.into().color(theme.text))
}

/// [`button`] for removing or deleting something.
pub fn danger_button(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    framed(ui, theme, RichText::new(text).color(theme.error))
}

pub fn framed(ui: &mut Ui, theme: &Theme, text: RichText) -> Response {
    let h = crate::ui::inputs::input_height(ui).max(ui.spacing().interact_size.y);
    let button = egui::Button::new(text.size(13.0)).wrap_mode(egui::TextWrapMode::Extend).corner_radius(7).min_size(vec2(h, h));
    styled_button(ui, theme, button, theme.panel, Stroke::new(1.0, theme.node_border))
}

/// A framed square button showing an icon (or `fallback` text when the icon is
/// missing), the same height as a text input. `tip` is shown on hover.
pub fn icon_button(ui: &mut Ui, theme: &Theme, icon: Option<&Path>, fallback: &str, tip: &str) -> Response {
    let side = ui.spacing().interact_size.y.max(34.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click());
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
    describe(&resp, Role::Button, tip);
    resp.on_hover_text(tip)
}

/// A small icon button without a frame, for what can be done to a row of a
/// list (edit, delete). `fallback` is shown when the icon is missing.
pub fn row_icon(ui: &mut Ui, theme: &Theme, icon: Option<&Path>, fallback: &str, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    let enabled = ui.is_enabled();
    if resp.hovered() && enabled {
        ui.painter().rect_filled(rect, 5.0, mix(theme.panel, theme.text, 0.1));
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let ink = if !enabled { with_alpha(theme.text_muted, 110) } else if resp.hovered() { theme.text } else { theme.text_muted };
    match icon {
        Some(path) => egui::Image::from_uri(file_uri(path)).tint(ink).paint_at(ui, rect.shrink(5.0)),
        None => {
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, fallback, FontId::proportional(13.0), ink);
        }
    }
    describe(&resp, Role::Button, tip);
    resp.on_hover_text(tip)
}

/// A button that looks like a link: accent-coloured text, underlined on
/// hover, no taller than a line of `font`. For actions inside lines of text.
pub fn link(ui: &mut Ui, theme: &Theme, text: &str, font: FontId) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, theme.accent);
    let (rect, resp) = ui.allocate_exact_size(galley.size(), Sense::click());
    describe(&resp, Role::Link, text);
    let color = if resp.hovered() { theme.text } else { theme.accent };
    ui.painter().galley(rect.min, galley, color);
    if resp.hovered() {
        ui.painter().hline(rect.x_range(), rect.bottom() - 1.0, Stroke::new(1.0, color));
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp
}

pub fn secondary_button(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    styled_button(ui, theme, egui::Button::new(RichText::new(text).color(theme.text)).corner_radius(7).min_size(vec2(84.0, ui.spacing().interact_size.y.max(32.0))), theme.panel, Stroke::new(1.0, theme.node_border))
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
                let name = ui
                    .vertical(|ui| {
                        ui.set_width(ui.available_width() - 48.0);
                        ui.spacing_mut().item_spacing.y = 3.0;
                        let name = ui.label(RichText::new(title).size(13.0).color(theme.text));
                        ui.label(RichText::new(description).size(11.5).color(theme.text_muted));
                        name.id
                    })
                    .inner;
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    resp = Some(labelled(ui, name, |ui| toggle(ui, on, theme)));
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

// ── From ui/widgets.rs ───────────────────────────────────────────────

/// A row with a text box and a control after it (a button): the control takes
/// what it needs, in any language, and the text box the rest of the width.
/// The row is as tall as a text box.
/// A drawn check mark: the interface font has no ✓.
pub fn check_mark(painter: &egui::Painter, center: egui::Pos2, size: f32, color: Color32) {
    let s = size / 2.0;
    painter.line(vec![center + egui::vec2(-s, 0.0), center + egui::vec2(-s * 0.3, s * 0.65), center + egui::vec2(s, -s * 0.6)], egui::Stroke::new(size / 6.0 + 0.6, color));
}

/// A check mark and `text`, in `color`: something that worked.
pub fn done_label(ui: &mut Ui, text: impl Into<egui::RichText>, color: Color32) -> Response {
    let text = text.into().color(color);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let size = ui.text_style_height(&egui::TextStyle::Body);
        let (r, _) = ui.allocate_exact_size(egui::vec2(size * 0.8, size), Sense::hover());
        check_mark(ui.painter(), r.center(), size * 0.62, color);
        ui.label(text);
    })
    .response
}

/// A row of a label and controls, all in its middle. egui centres each item
/// in the height the row has when the item is added, so a label put before
/// a taller control (a dropdown, a slider, a field) would sit at the top:
/// the row gets its full height first. (It starts at that height too: in
/// another row, a shorter start would be centred there and then grow down.)
///
/// It takes only as much width as its content did last time (the whole
/// width the first time), so in a row laid out from the right it sits on
/// the right instead of filling the row.
pub fn line<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let h = crate::ui::inputs::input_height(ui).max(ui.spacing().interact_size.y);
    let id = ui.scope_id().with(("kit-line", ui.next_auto_id()));
    let full = ui.available_size_before_wrap().x;
    let width = ui.data(|d| d.get_temp::<f32>(id)).map_or(full, |w| w.min(full));
    let r = ui.allocate_ui_with_layout(egui::vec2(width, h), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_height(h);
        add(ui)
    });
    let used = r.response.rect.width();
    if (used - width).abs() > 0.5 {
        ui.data_mut(|d| d.insert_temp(id, used));
        ui.ctx().request_repaint();
    }
    r.inner
}

pub fn field_with_button(ui: &mut Ui, field: impl FnOnce(&mut Ui), button: impl FnOnce(&mut Ui)) {
    let size = egui::vec2(ui.available_width(), crate::ui::inputs::input_height(ui).max(ui.spacing().interact_size.y));
    ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        button(ui);
        field(ui);
    });
}

/// A dropdown as tall as a text box, so the two line up in a row.
/// [`dropdown`] that stays open while something inside it is clicked (a
/// search field, say); `ui.close()` in `contents` closes it.
pub fn search_dropdown<R>(ui: &mut Ui, id_salt: egui::Id, width: f32, selected: impl Into<egui::WidgetText>, contents: impl FnOnce(&mut Ui) -> R) -> Response {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = crate::ui::inputs::input_height(ui);
        let resp = ComboBox::from_id_salt(id_salt).width(width).height(420.0).selected_text(selected).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show_ui(ui, contents).response;
        named(ui, resp)
    })
    .inner
}

pub fn dropdown<R>(ui: &mut Ui, id_salt: egui::Id, width: f32, selected: impl Into<egui::WidgetText>, contents: impl FnOnce(&mut Ui) -> R) -> Response {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = crate::ui::inputs::input_height(ui);
        let resp = ComboBox::from_id_salt(id_salt).width(width).selected_text(selected).show_ui(ui, contents).response;
        named(ui, resp)
    })
    .inner
}

/// A dropdown with a checkbox per option, for picking any number of them, as
/// tall as a text box. Closed, it names the first one picked (and how many
/// more), or says `none`. Options are (key, English label). Returns whether
/// the picks changed.
pub fn multi_select<K: PartialEq + Clone>(ui: &mut Ui, theme: &Theme, id_salt: egui::Id, width: f32, options: &[(K, &'static str)], picked: &mut Vec<K>, none: &str) -> bool {
    let name = |k: &K| options.iter().find(|(o, _)| o == k).map(|(_, l)| crate::i18n::t(l)).unwrap_or_default().to_string();
    let summary = match picked.len() {
        0 => none.to_string(),
        1 => name(&picked[0]),
        n => tf!("{first} + {more} more", first = name(&picked[0]), more = n - 1),
    };
    let mut changed = false;
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = crate::ui::inputs::input_height(ui);
        let resp = ComboBox::from_id_salt(id_salt).width(width).selected_text(summary).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show_ui(ui, |ui| {
            for (key, label) in options {
                let mut on = picked.contains(key);
                if checkbox(ui, theme, &mut on, crate::i18n::t(label)).changed() {
                    if on {
                        picked.push(key.clone());
                    } else {
                        picked.retain(|k| k != key);
                    }
                    changed = true;
                }
            }
        });
        named(ui, resp.response);
    });
    changed
}

/// A pill-shaped on/off switch in the theme's Action green, one size
/// everywhere: big enough to hit with a finger.
pub fn toggle(ui: &mut Ui, on: &mut bool, theme: &Theme) -> Response {
    let size = vec2(44.0, 24.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    describe_selected(&response, Role::Switch, *on, "");
    let response = named(ui, response);
    let t = ui.ctx().animate_bool_responsive(response.id, *on);
    let bg = crate::ui::theme::mix(theme.field, theme.action, t);
    let painter = ui.painter();
    painter.rect_filled(rect, rect.height() / 2.0, bg);
    let r = rect.height() / 2.0 - 2.5;
    let x = egui::lerp((rect.left() + r + 2.5)..=(rect.right() - r - 2.5), t);
    let knob = if *on { Color32::from_rgb(4, 33, 13) } else { theme.text_muted };
    painter.circle_filled(egui::pos2(x, rect.center().y), r, knob);
    response
}

// ── From pages/settings.rs ───────────────────────────────────────────

/// One entry of a settings list on the left (here and in Bot › Settings):
/// its icon and name, and a count or mark on the right when there is one.
pub fn nav_item(ui: &mut Ui, theme: &Theme, icon: Option<&std::path::Path>, name: &str, selected: bool, badge: Option<(String, Color32)>) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    describe_selected(&resp, Role::Button, selected, name);
    if selected {
        ui.painter().rect_filled(rect, 6.0, with_alpha(theme.accent, 38));
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 6.0, theme.panel_header);
    }
    let tint = if selected { theme.accent } else { theme.text_muted };
    if let Some(path) = icon {
        egui::Image::from_uri(file_uri(path)).tint(tint).paint_at(ui, Rect::from_center_size(rect.left_center() + vec2(18.0, 0.0), vec2(16.0, 16.0)));
    }
    let mut right = rect.right() - 8.0;
    if let Some((text, color)) = &badge {
        let galley = ui.painter().layout_no_wrap(text.clone(), FontId::proportional(11.0), theme.panel);
        let size = vec2((galley.size().x + 10.0).max(18.0), 18.0);
        let pill = Rect::from_center_size(pos2(right - size.x / 2.0, rect.center().y), size);
        ui.painter().rect_filled(pill, 9.0, *color);
        ui.painter().galley(pill.center() - galley.size() / 2.0, galley, theme.panel);
        right = pill.left() - 6.0;
    }
    // A name too long for the list is cut short; the whole name shows on hover.
    let font = FontId::proportional(13.5);
    let color = if selected { theme.text } else { theme.text_muted };
    let galley = ui.painter().layout(name.to_string(), font, color, f32::INFINITY);
    let room = right - (rect.left() + 36.0);
    let resp = if galley.size().x > room {
        let job = egui::text::LayoutJob { wrap: egui::text::TextWrapping::truncate_at_width(room), ..egui::text::LayoutJob::simple_singleline(name.to_string(), FontId::proportional(13.5), color) };
        let cut = ui.painter().layout_job(job);
        ui.painter().galley(pos2(rect.left() + 36.0, rect.center().y - cut.size().y / 2.0), cut, color);
        resp.on_hover_text(name)
    } else {
        ui.painter().galley(pos2(rect.left() + 36.0, rect.center().y - galley.size().y / 2.0), galley, color);
        resp
    };
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// A group heading.
pub fn group(ui: &mut Ui, theme: &Theme, title: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title.to_uppercase()).size(11.0).strong().color(theme.text_muted));
    ui.add_space(6.0);
}

/// One setting: its name and description on the left, the control on the right.
pub fn setting_row(ui: &mut Ui, theme: &Theme, title: &str, description: &str, control: impl FnOnce(&mut Ui)) {
    egui::Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(8).inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
        let inner = ui.available_width();
        ui.set_width(inner);
        // The control's width as drawn last time: the text takes the rest. When
        // that leaves too little for the text, the control goes under it, so
        // a row is never wider than the page.
        let measured = ui.scope_id().with(("settings-control", title));
        let control_w: f32 = ui.data(|d| d.get_temp(measured)).unwrap_or(0.0);
        let stacked = inner - control_w - 16.0 < 180.0;
        // The title names the control too, for screen readers.
        let text = |ui: &mut Ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            let name = ui.label(RichText::new(title).size(13.5).color(theme.text));
            if !description.is_empty() {
                ui.label(RichText::new(description).size(11.5).color(theme.text_muted));
            }
            name.id
        };
        let drawn = if stacked {
            let name = text(ui);
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(inner);
                labelled(ui, name, control);
            })
            .response
            .rect
        } else {
            ui.horizontal(|ui| {
                let w = inner - control_w - 16.0;
                let name = if description.is_empty() {
                    // A name alone sits in the middle of the control's height (34: a field's).
                    let h = crate::ui::inputs::input_height(ui).max(ui.spacing().interact_size.y).max(34.0);
                    ui.allocate_ui_with_layout(vec2(w, h), Layout::left_to_right(Align::Center), |ui| {
                        ui.set_width(w);
                        text(ui)
                    })
                    .inner
                } else {
                    ui.vertical(|ui| {
                        ui.set_width(w);
                        text(ui)
                    })
                    .inner
                };
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| labelled(ui, name, control)).response.rect
            })
            .inner
        };
        // Stacked, the control's own width is what it took on its line.
        let width = if stacked { drawn.width().min(inner) } else { drawn.width() };
        if (width - control_w).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(measured, width));
            ui.ctx().request_repaint();
        }
    });
    ui.add_space(8.0);
}

// ── From bot/settings.rs ─────────────────────────────────────────────

/// A setting's box: its name, description and control stacked.
pub fn setting_card(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui)) {
    Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(8).inner_margin(Margin::symmetric(16, 12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 3.0;
        // A setting_title in it names the control under it.
        with_name(ui, Name::Later, add);
    });
    ui.add_space(8.0);
}

/// A setting's name and description, above its control. In a
/// [`setting_card`], the name names the control too, for screen readers.
pub fn setting_title(ui: &mut Ui, theme: &Theme, label: &str, description: &str) {
    let name = ui.label(RichText::new(label).size(13.5).color(theme.text));
    // Only inside a card (or another named place), so it never names what follows the card.
    let key = egui::Id::unique(NAMED_BY);
    if ui.data(|d| d.get_temp::<Name>(key)).is_some() {
        ui.data_mut(|d| d.insert_temp(key, Name::Label(name.id)));
    }
    if !description.is_empty() {
        ui.label(RichText::new(description).size(11.5).color(theme.text_muted));
    }
    ui.add_space(5.0);
}

/// What's wrong with a value, under it.
pub fn problem_text(ui: &mut Ui, theme: &Theme, problem: Option<String>) {
    if let Some(p) = problem {
        ui.add_space(2.0);
        ui.label(RichText::new(p).size(11.5).color(theme.error));
    }
}

/// A box of text in a colour for its `tone`: info, warning or success.
pub fn note(ui: &mut Ui, icons: &std::collections::HashMap<String, std::path::PathBuf>, theme: &Theme, tone: &str, title: &str, text: &str) {
    use crate::ui::theme::with_alpha;
    let (color, icon) = match tone {
        "warning" => (theme.warning, "alert"),
        "success" => (theme.action, "check"),
        _ => (theme.accent, "info"),
    };
    Frame::new().fill(with_alpha(color, 22)).stroke(Stroke::new(1.0, with_alpha(color, 90))).corner_radius(8).inner_margin(Margin::symmetric(14, 11)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::hover());
            if let Some(path) = icons.get(icon) {
                egui::Image::from_uri(crate::editor::file_uri(path)).tint(color).paint_at(ui, r);
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if !title.is_empty() {
                    ui.label(RichText::new(title).size(13.5).strong().color(theme.text));
                }
                if !text.is_empty() {
                    ui.label(RichText::new(text).size(12.5).color(theme.text));
                }
            });
        });
    });
    ui.add_space(8.0);
}

// ── Names for screen readers ─────────────────────────────────────────────

/// Where the name for the controls being drawn is kept (see [`labelled`]).
const NAMED_BY: &str = "xc-kit-named-by";

/// What names the controls being drawn.
#[derive(Clone)]
enum Name {
    /// A label on screen.
    Label(egui::Id),
    /// A name not shown as a label of its own (a block's field, drawn on the canvas).
    Text(String),
    /// Nothing yet: a [`setting_card`] before its [`setting_title`].
    Later,
}

/// Runs `add` with `label` naming the controls in it that show no name of
/// their own (text boxes, dropdowns, numbers, sliders, switches, colours),
/// for screen readers and for tests that find controls by name. The kit's
/// rows and fields do this with their titles.
pub fn labelled<R>(ui: &mut Ui, label: egui::Id, add: impl FnOnce(&mut Ui) -> R) -> R {
    with_name(ui, Name::Label(label), add)
}

/// [`labelled`] by a name that isn't a label on screen.
pub fn called<R>(ui: &mut Ui, name: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    with_name(ui, Name::Text(name.to_string()), add)
}

/// Runs `add` with `name` for its controls, then puts back the name as it was.
fn with_name<R>(ui: &mut Ui, name: Name, add: impl FnOnce(&mut Ui) -> R) -> R {
    let key = egui::Id::unique(NAMED_BY);
    let before: Option<Name> = ui.data(|d| d.get_temp(key));
    ui.data_mut(|d| d.insert_temp(key, name));
    let r = add(ui);
    ui.data_mut(|d| match before {
        Some(b) => {
            d.insert_temp(key, b);
        }
        None => {
            d.remove::<Name>(key);
        }
    });
    r
}

/// Names a control by the label around it ([`labelled`], [`called`]), when there is one.
pub fn named(ui: &Ui, resp: Response) -> Response {
    match ui.data(|d| d.get_temp::<Name>(egui::Id::unique(NAMED_BY))) {
        Some(Name::Label(label)) => resp.labelled_by(label),
        Some(Name::Text(name)) => resp.accessible_name(name),
        Some(Name::Later) | None => resp,
    }
}

/// Tells screen readers what a control drawn by hand is (a button, a link)
/// and what it is called.
pub fn describe(resp: &Response, role: Role, name: &str) {
    resp.widget_info(|| egui::WidgetInfo::labeled(role, resp.enabled(), name));
}

/// [`describe`] for something that is on or off, chosen or not.
pub fn describe_selected(resp: &Response, role: Role, selected: bool, name: &str) {
    resp.widget_info(|| egui::WidgetInfo::selected(role, resp.enabled(), selected, name));
}

/// Tells screen readers whether something that opens and closes (a folder) is open.
pub fn accesskit_expanded(resp: &Response, open: bool) {
    resp.ctx.accesskit_node_builder(resp.id, |node| node.set_expanded(open));
}

// ── The theme ────────────────────────────────────────────────────────────

/// Where the app leaves the icon set in egui's memory, for code that has
/// only a `ui` (a plugin panel's note).
pub const KIT_ICONS: &str = "xc-kit-icons";

/// The theme in use, for code that has only a `ui` (plugins' panels).
pub fn theme_of(ui: &Ui) -> Theme {
    ui.ctx().data(|d| d.get_temp::<Theme>(egui::Id::unique(crate::ui::theme::KIT_THEME))).unwrap_or_else(Theme::fallback)
}

// ── Sizes ────────────────────────────────────────────────────────────────

/// The height every control in a row has: a text input's. Buttons, numbers,
/// dropdowns, colours and paths are all this tall, so a row lines up.
pub fn control_height(ui: &Ui) -> f32 {
    input_height(ui).max(ui.spacing().interact_size.y)
}

/// Runs `add` with the kit's look for egui's own widgets (fields filled like
/// a text input, the control height), then puts the style back. On `ui`
/// itself, not a scope, so wrapping rows still wrap.
fn styled<R>(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui) -> R) -> R {
    let before = ui.style().clone();
    let h = control_height(ui);
    {
        let style = ui.style_mut();
        style.spacing.interact_size.y = h;
        style.spacing.button_padding.y = ((h - 14.0) / 2.0).max(2.0);
        let w = &mut style.visuals.widgets;
        for state in [&mut w.inactive, &mut w.hovered, &mut w.active] {
            state.corner_radius = egui::CornerRadius::same(6);
        }
        w.inactive.bg_fill = theme.field;
        w.inactive.weak_bg_fill = theme.field;
        w.inactive.bg_stroke = Stroke::new(1.0, theme.line);
        w.hovered.bg_fill = mix(theme.field, theme.text, 0.06);
        w.hovered.weak_bg_fill = mix(theme.field, theme.text, 0.06);
        w.hovered.bg_stroke = Stroke::new(1.0, mix(theme.line, theme.text, 0.3));
        w.active.bg_fill = mix(theme.field, theme.text, 0.1);
        w.active.weak_bg_fill = mix(theme.field, theme.text, 0.1);
        w.active.bg_stroke = Stroke::new(1.0, theme.accent);
    }
    let r = add(ui);
    ui.set_style(before);
    r
}

// ── Numbers and sliders ──────────────────────────────────────────────────

/// A number to type or drag left and right, as tall as the other controls:
/// `Number::new(&mut v).range(0.0..=100.0).suffix(" %").show(ui, theme)`.
pub struct Number<'a, N: egui::emath::Numeric> {
    value: &'a mut N,
    range: Option<std::ops::RangeInclusive<N>>,
    suffix: String,
    speed: f64,
    decimals: Option<usize>,
    width: f32,
}

impl<'a, N: egui::emath::Numeric> Number<'a, N> {
    pub fn new(value: &'a mut N) -> Self {
        Number { value, range: None, suffix: String::new(), speed: 1.0, decimals: None, width: 56.0 }
    }
    pub fn range(mut self, range: std::ops::RangeInclusive<N>) -> Self {
        self.range = Some(range);
        self
    }
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }
    /// How much a pixel of dragging changes it.
    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = speed;
        self
    }
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = Some(decimals);
        self
    }
    /// The narrowest it gets.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
    pub fn show(self, ui: &mut Ui, theme: &Theme) -> Response {
        styled(ui, theme, |ui| {
            ui.spacing_mut().interact_size.x = self.width;
            let mut drag = egui::DragValue::new(self.value).speed(self.speed).suffix(self.suffix);
            if let Some(r) = self.range {
                drag = drag.range(r);
            }
            if let Some(d) = self.decimals {
                drag = drag.fixed_decimals(d);
            }
            let resp = ui.add(drag);
            named(ui, resp)
        })
    }
}

/// A slider with its number beside it to type: `Slider::new(&mut v, 0.0..=100.0).show(ui, theme)`.
pub struct Slider<'a, N: egui::emath::Numeric> {
    value: &'a mut N,
    range: std::ops::RangeInclusive<N>,
    step: Option<f64>,
    suffix: String,
    decimals: Option<usize>,
    width: Option<f32>,
}

impl<'a, N: egui::emath::Numeric> Slider<'a, N> {
    pub fn new(value: &'a mut N, range: std::ops::RangeInclusive<N>) -> Self {
        Slider { value, range, step: None, suffix: String::new(), decimals: None, width: None }
    }
    pub fn step(mut self, step: f64) -> Self {
        self.step = Some(step);
        self
    }
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = Some(decimals);
        self
    }
    /// The slider's own width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    /// The slider, then the number: true when either changed it.
    pub fn show(self, ui: &mut Ui, theme: &Theme) -> bool {
        let Slider { value, range, step, suffix, decimals, width } = self;
        line(ui, |ui| {
            if let Some(w) = width {
                ui.spacing_mut().slider_width = w;
            }
            let mut slider = egui::Slider::new(&mut *value, range.clone()).show_value(false);
            if let Some(s) = step {
                slider = slider.step_by(s);
            }
            let slider = ui.add(slider);
            let slid = named(ui, slider).changed();
            let mut number = Number::new(value).range(range).suffix(suffix).speed(step.unwrap_or(1.0));
            if let Some(d) = decimals {
                number = number.decimals(d);
            }
            number.show(ui, theme).changed() || slid
        })
    }
}

// ── Choices ──────────────────────────────────────────────────────────────

/// A yes/no in a list: a square box with a drawn check mark, and its text.
/// (A setting on its own row is a [`toggle`].)
pub fn checkbox(ui: &mut Ui, theme: &Theme, on: &mut bool, text: impl Into<egui::WidgetText>) -> Response {
    checkbox_mixed(ui, theme, on, false, text)
}

/// [`checkbox`] that can show "some of them" (a dash) for a group.
pub fn checkbox_mixed(ui: &mut Ui, theme: &Theme, on: &mut bool, mixed: bool, text: impl Into<egui::WidgetText>) -> Response {
    let text: egui::WidgetText = text.into();
    let galley = text.into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body);
    let side = 18.0;
    let size = vec2(side + 8.0 + galley.size().x, galley.size().y.max(side) + 4.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    // "Some of them" is neither on nor off.
    let selected = (*on || !mixed).then_some(*on);
    resp.widget_info(|| egui::WidgetInfo { selected, ..egui::WidgetInfo::labeled(Role::CheckBox, ui.is_enabled(), galley.text()) });
    let enabled = ui.is_enabled();
    let b = Rect::from_center_size(pos2(rect.left() + side / 2.0, rect.center().y), vec2(side, side));
    let painter = ui.painter();
    let filled = *on || mixed;
    let fade = |c: Color32| if enabled { c } else { with_alpha(c, 110) };
    let fill = if filled { theme.accent } else { theme.field };
    let stroke = if filled {
        Stroke::NONE
    } else if resp.hovered() && enabled {
        Stroke::new(1.0, theme.text_muted)
    } else {
        Stroke::new(1.0, theme.line)
    };
    painter.rect(b, 4.0, fade(fill), stroke, egui::StrokeKind::Inside);
    let ink = if theme.dark { Color32::from_rgb(0, 24, 28) } else { Color32::WHITE };
    if mixed && !*on {
        painter.line_segment([b.left_center() + vec2(4.5, 0.0), b.right_center() - vec2(4.5, 0.0)], Stroke::new(2.0, ink));
    } else if *on {
        check_mark(painter, b.center(), 10.0, ink);
    }
    let color = fade(if enabled { theme.text } else { theme.text_muted });
    painter.galley(pos2(b.right() + 8.0, rect.center().y - galley.size().y / 2.0), galley, color);
    if enabled { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp }
}

/// One choice of several (a theme, a mode): a round mark, filled when chosen.
pub fn radio(ui: &mut Ui, theme: &Theme, selected: bool, text: impl Into<egui::WidgetText>) -> Response {
    let text: egui::WidgetText = text.into();
    let galley = text.into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body);
    let side = 18.0;
    let size = vec2(side + 8.0 + galley.size().x, galley.size().y.max(side) + 4.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    describe_selected(&resp, Role::RadioButton, selected, galley.text());
    let c = pos2(rect.left() + side / 2.0, rect.center().y);
    let painter = ui.painter();
    if selected {
        painter.circle_filled(c, side / 2.0, theme.accent);
        painter.circle_filled(c, 3.5, if theme.dark { Color32::from_rgb(0, 24, 28) } else { Color32::WHITE });
    } else {
        painter.circle(c, side / 2.0 - 0.5, theme.field, Stroke::new(1.0, if resp.hovered() { theme.text_muted } else { theme.line }));
    }
    painter.galley(pos2(c.x + side / 2.0 + 8.0, rect.center().y - galley.size().y / 2.0), galley, theme.text);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// An entry of a dropdown's list.
pub fn option(ui: &mut Ui, selected: bool, text: impl Into<egui::WidgetText>) -> Response {
    ui.selectable_label(selected, text)
}

/// A dropdown that sets `value` to one of `choices` (value, what it shows).
/// True when it changed.
pub fn select<V: AsRef<str>, L: AsRef<str>>(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, width: f32, value: &mut String, choices: &[(V, L)]) -> bool {
    let shown = choices.iter().find(|(v, _)| v.as_ref() == value.as_str()).or(choices.first()).map(|(_, l)| l.as_ref().to_string()).unwrap_or_default();
    let mut changed = false;
    dropdown(ui, egui::Id::unique(id), width, shown, |ui| {
        for (v, l) in choices {
            if option(ui, value.as_str() == v.as_ref(), l.as_ref()).clicked() {
                *value = v.as_ref().to_string();
                changed = true;
            }
        }
    });
    changed
}

// ── Colours, files, lists ────────────────────────────────────────────────

/// A colour as #RRGGBB text, with the app's picker. True when it changed.
pub fn color(ui: &mut Ui, theme: &Theme, id: impl std::hash::Hash + std::fmt::Debug, hex: &mut String) -> bool {
    let mut c = crate::ui::theme::hex_color(hex.trim()).unwrap_or(Color32::WHITE);
    if crate::builders::color::color_button(ui, egui::Id::unique(id), &mut c, theme, Some(120.0)) {
        *hex = crate::builders::color::to_hex(c);
        return true;
    }
    false
}

/// A file (or folder) path to type, with Browse…. True when it changed.
pub fn path(ui: &mut Ui, theme: &Theme, value: &mut String, hint: &str, folder: bool, width: f32) -> bool {
    let mut browse = false;
    let mut changed = false;
    ui.allocate_ui(vec2(width, control_height(ui)), |ui| {
        field_with_button(ui, |ui| changed = text_input(ui, value, hint, false).changed(), |ui| browse = secondary_button(ui, theme, t!("Browse…")).clicked());
    });
    if browse {
        let picker = rfd::FileDialog::new();
        if let Some(p) = if folder { picker.pick_folder() } else { picker.pick_file() } {
            *value = p.display().to_string();
            changed = true;
        }
    }
    changed
}

/// A list of texts: a box and a remove button for each, + Add at the end
/// (the new row gets the cursor). True when it changed.
#[allow(clippy::too_many_arguments)]
pub fn text_list(ui: &mut Ui, theme: &Theme, icons: &std::collections::HashMap<String, std::path::PathBuf>, id: impl std::hash::Hash + std::fmt::Debug, items: &mut Vec<String>, placeholder: &str, width: f32) -> bool {
    let focus_id = egui::Id::unique(("kit-list-new", egui::Id::unique(id)));
    let focus: Option<usize> = ui.data_mut(|d| d.remove_temp(focus_id));
    let mut remove = None;
    let mut changed = false;
    for (i, item) in items.iter_mut().enumerate() {
        line(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let input = ui.allocate_ui(vec2(width - 46.0, control_height(ui)), |ui| text_input(ui, item, placeholder, false)).inner;
            changed |= input.changed();
            if focus == Some(i) {
                input.request_focus();
            }
            if icon_button(ui, theme, icons.get("trash").map(|p| p.as_path()), "×", t!("Remove")).clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        items.remove(i);
        changed = true;
    }
    if button(ui, theme, t!("+ Add")).clicked() {
        items.push(String::new());
        ui.data_mut(|d| d.insert_temp(focus_id, items.len() - 1));
        changed = true;
    }
    changed
}

// ── Text ─────────────────────────────────────────────────────────────────

/// A small grey name above or beside a control.
pub fn caption(ui: &mut Ui, theme: &Theme, text: impl Into<String>) -> Response {
    ui.label(RichText::new(text).size(11.5).color(theme.text_muted))
}

/// Help or a hint under or beside something: small and grey.
pub fn hint(ui: &mut Ui, theme: &Theme, text: impl Into<String>) -> Response {
    ui.label(RichText::new(text).size(11.5).color(theme.text_muted))
}

// ── Menus ────────────────────────────────────────────────────────────────

/// An entry of a menu with an icon before it.
pub fn menu_icon_item(ui: &mut Ui, icon: Option<&Path>, tint: Color32, text: impl Into<egui::WidgetText>) -> Response {
    match icon {
        Some(p) => ui.add(egui::Button::image_and_text(egui::Image::from_uri(file_uri(p)).fit_to_exact_size(vec2(14.0, 14.0)).tint(tint), text)),
        None => ui.add(egui::Button::new(text)),
    }
}

/// An entry of a menu, with its shortcut on the right.
pub fn menu_item(ui: &mut Ui, text: impl Into<egui::WidgetText>, shortcut: Option<String>, enabled: bool) -> Response {
    let mut b = egui::Button::new(text);
    if let Some(s) = shortcut {
        b = b.shortcut_text(s);
    }
    ui.add_enabled(enabled, b)
}
