//! The color picker: a swatch button that opens a panel with a
//! saturation/brightness square, a hue bar, hex and RGB inputs, Discord's
//! colors and recently used ones.

use eframe::egui::{self, Color32, CursorIcon, Id, Mesh, Rect, Sense, Stroke, StrokeKind, Ui, ecolor::Hsva, pos2, vec2};

use crate::ui::theme::{Theme, mix};

const PANEL_W: f32 = 248.0;
const SQUARE_H: f32 = 150.0;
const HUE_H: f32 = 14.0;
const SWATCH: f32 = 22.0;
const RECENT_MAX: usize = 9;

/// Discord's brand colors, then a few useful neutrals.
const PRESETS: [(&str, &str); 9] = [
    ("Blurple", "#5865F2"),
    ("Green", "#57F287"),
    ("Yellow", "#FEE75C"),
    ("Fuchsia", "#EB459E"),
    ("Red", "#ED4245"),
    ("Aqua", "#00D8F0"),
    ("White", "#FFFFFF"),
    ("Gray", "#99AAB5"),
    ("Black", "#23272A"),
];

/// `#RRGGBB` for a color.
pub fn to_hex(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

/// Parses `#RRGGBB`, `RRGGBB`, `#RGB` or `RGB`.
pub fn parse_hex(s: &str) -> Option<Color32> {
    let h = s.trim().trim_start_matches('#');
    let digits: Vec<u8> = h.chars().map(|c| c.to_digit(16).map(|d| d as u8)).collect::<Option<_>>()?;
    match digits.as_slice() {
        [r, g, b] => Some(Color32::from_rgb(r * 17, g * 17, b * 17)),
        [r1, r2, g1, g2, b1, b2] => Some(Color32::from_rgb(r1 * 16 + r2, g1 * 16 + g2, b1 * 16 + b2)),
        _ => None,
    }
}

/// What the picker remembers while it is open.
#[derive(Clone)]
struct Open {
    /// Kept as HSV so the hue survives dragging to gray or black.
    hsva: Hsva,
    hex: String,
    fresh: bool,
}

/// A swatch with the hex code that opens the picker. Returns true when the color changed.
pub fn color_button(ui: &mut Ui, id: Id, color: &mut Color32, theme: &Theme, width: Option<f32>) -> bool {
    let hex = to_hex(*color);
    let font = egui::FontId::monospace(12.0);
    let galley = ui.painter().layout_no_wrap(hex.clone(), font, theme.text);
    let w = width.unwrap_or(galley.size().x + 40.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, crate::ui::kit::control_height(ui)), Sense::click());
    let open_id = id.with("open");
    let mut open: Option<Open> = ui.data(|d| d.get_temp(open_id));
    let fill = if resp.hovered() || open.is_some() { mix(theme.field, theme.text, 0.08) } else { theme.field };
    let p = ui.painter();
    p.rect(rect, 5.0, fill, Stroke::new(1.0, if open.is_some() { theme.accent } else { theme.line }), StrokeKind::Inside);
    let sw = Rect::from_min_size(rect.left_center() + vec2(5.0, -7.0), vec2(14.0, 14.0));
    p.rect(sw, 3.0, *color, Stroke::new(1.0, mix(*color, Color32::BLACK, 0.3)), StrokeKind::Inside);
    p.galley(pos2(sw.right() + 7.0, rect.center().y - galley.size().y / 2.0), galley, theme.text);
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(t!("Pick a color"));

    if resp.clicked() {
        open = match open {
            Some(_) => None,
            None => Some(Open { hsva: Hsva::from(*color), hex: hex.clone(), fresh: true }),
        };
    }
    let mut changed = false;
    if let Some(mut state) = open.take() {
        let (keep, c) = panel(ui.ctx(), id, rect, &mut state, theme);
        if let Some(c) = c
            && c != *color
        {
            *color = c;
            changed = true;
        }
        if keep {
            state.fresh = false;
            open = Some(state);
        } else {
            remember(ui.ctx(), *color);
        }
    }
    ui.data_mut(|d| match open {
        Some(s) => {
            d.insert_temp(open_id, s);
        }
        None => d.remove::<Open>(open_id),
    });
    changed
}

/// Colors picked recently, kept between sessions.
fn recent(ctx: &egui::Context) -> Vec<[u8; 3]> {
    ctx.data_mut(|d| d.get_persisted::<Vec<[u8; 3]>>(Id::unique("xc-recent-colors")).unwrap_or_default())
}

fn remember(ctx: &egui::Context, c: Color32) {
    let rgb = [c.r(), c.g(), c.b()];
    ctx.data_mut(|d| {
        let list = d.get_persisted_mut_or_default::<Vec<[u8; 3]>>(Id::unique("xc-recent-colors"));
        list.retain(|x| *x != rgb);
        list.insert(0, rgb);
        list.truncate(RECENT_MAX);
    });
}

/// Draws the open panel below (or above) the button. Returns whether it stays
/// open and the color picked this frame.
fn panel(ctx: &egui::Context, id: Id, button: Rect, state: &mut Open, theme: &Theme) -> (bool, Option<Color32>) {
    let screen = ctx.content_rect();
    let height = 330.0;
    let below = button.bottom() + 4.0 + height < screen.bottom();
    let pos = pos2(button.left().min(screen.right() - PANEL_W - 24.0), if below { button.bottom() + 4.0 } else { button.top() - 4.0 - height });
    let mut picked: Option<Color32> = None;
    let recent = recent(ctx);

    let area = egui::Area::new(id.with("panel")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::new()
            .fill(theme.panel)
            .stroke(Stroke::new(1.0, theme.node_border))
            .corner_radius(10)
            .inner_margin(10)
            .shadow(ctx.global_style().visuals.popup_shadow)
            .show(ui, |ui| {
                ui.set_width(PANEL_W);
                ui.spacing_mut().item_spacing = vec2(6.0, 8.0);

                // Saturation (x) and brightness (y) for the current hue.
                let (sq, sq_resp) = ui.allocate_exact_size(vec2(PANEL_W, SQUARE_H), Sense::click_and_drag());
                ui.painter().add(sv_mesh(sq, state.hsva.h));
                ui.painter().rect_stroke(sq, 1.0, Stroke::new(1.0, theme.line), StrokeKind::Inside);
                if let Some(p) = sq_resp.interact_pointer_pos().filter(|_| sq_resp.is_pointer_button_down_on() || sq_resp.clicked()) {
                    state.hsva.s = ((p.x - sq.left()) / sq.width()).clamp(0.0, 1.0);
                    state.hsva.v = 1.0 - ((p.y - sq.top()) / sq.height()).clamp(0.0, 1.0);
                    picked = Some(Color32::from(state.hsva));
                }
                let knob = pos2(sq.left() + state.hsva.s * sq.width(), sq.top() + (1.0 - state.hsva.v) * sq.height());
                ui.painter().circle_stroke(knob, 6.0, Stroke::new(2.0, Color32::WHITE));
                ui.painter().circle_stroke(knob, 7.5, Stroke::new(1.0, Color32::from_black_alpha(120)));

                // Hue.
                let (hue, hue_resp) = ui.allocate_exact_size(vec2(PANEL_W, HUE_H), Sense::click_and_drag());
                ui.painter().add(hue_mesh(hue));
                if let Some(p) = hue_resp.interact_pointer_pos().filter(|_| hue_resp.is_pointer_button_down_on() || hue_resp.clicked()) {
                    state.hsva.h = ((p.x - hue.left()) / hue.width()).clamp(0.0, 0.9999);
                    picked = Some(Color32::from(state.hsva));
                }
                let hx = hue.left() + state.hsva.h * hue.width();
                let marker = Rect::from_center_size(pos2(hx, hue.center().y), vec2(6.0, HUE_H + 4.0));
                ui.painter().rect(marker, 2.0, Color32::from(Hsva::new(state.hsva.h, 1.0, 1.0, 1.0)), Stroke::new(2.0, Color32::WHITE), StrokeKind::Outside);

                // Hex and RGB.
                let current = picked.unwrap_or(Color32::from(state.hsva));
                if picked.is_some() {
                    state.hex = to_hex(current);
                }
                // One height for the whole row, set before anything is placed, so
                // the swatch, boxes and labels share a center line: the number
                // boxes' own height (text and button padding), which they keep.
                let h = ui.spacing().interact_size.y.max(ui.text_style_height(&egui::TextStyle::Body) + 2.0 * ui.spacing().button_padding.y);
                ui.allocate_ui_with_layout(vec2(ui.available_width(), h), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let (sw, _) = ui.allocate_exact_size(vec2(h, h), Sense::hover());
                    ui.painter().rect(sw, 5.0, current, Stroke::new(1.0, theme.line), StrokeKind::Inside);
                    let hex = ui.add_sized([66.0, h], egui::TextEdit::singleline(&mut state.hex).font(egui::TextStyle::Monospace).char_limit(7).align(egui::Align2::LEFT_CENTER));
                    if hex.changed()
                        && let Some(c) = parse_hex(&state.hex)
                    {
                        state.hsva = Hsva::from(c);
                        picked = Some(c);
                    }
                    if hex.lost_focus() {
                        state.hex = to_hex(picked.unwrap_or(Color32::from(state.hsva)));
                    }
                    let mut rgb = [current.r(), current.g(), current.b()];
                    let mut edited = false;
                    for (i, label) in ["R", "G", "B"].into_iter().enumerate() {
                        ui.label(egui::RichText::new(label).size(10.5).color(theme.text_muted));
                        edited |= ui.add_sized([34.0, h], egui::DragValue::new(&mut rgb[i]).speed(1.0)).changed();
                    }
                    if edited {
                        let c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                        state.hsva = Hsva::from(c);
                        state.hex = to_hex(c);
                        picked = Some(c);
                    }
                });

                // Presets and recent colors.
                let swatches = |ui: &mut Ui, list: Vec<(String, Color32)>, picked: &mut Option<Color32>, state: &mut Open| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
                        for (name, c) in list {
                            let (r, resp) = ui.allocate_exact_size(vec2(SWATCH, SWATCH), Sense::click());
                            let selected = to_hex(c) == to_hex(current);
                            ui.painter().rect(r.shrink(1.0), 5.0, c, Stroke::new(1.0, theme.line), StrokeKind::Inside);
                            if selected || resp.hovered() {
                                ui.painter().rect_stroke(r, 6.0, Stroke::new(2.0, if selected { theme.accent } else { theme.text_muted }), StrokeKind::Outside);
                            }
                            if resp.on_hover_text(format!("{name}  {}", to_hex(c))).clicked() {
                                state.hsva = Hsva::from(c);
                                state.hex = to_hex(c);
                                *picked = Some(c);
                            }
                        }
                    });
                };
                ui.label(egui::RichText::new("DISCORD").size(10.0).color(theme.text_muted));
                swatches(ui, PRESETS.iter().filter_map(|(n, h)| Some((n.to_string(), parse_hex(h)?))).collect(), &mut picked, state);
                if !recent.is_empty() {
                    ui.label(egui::RichText::new(t!("Recent").to_uppercase()).size(10.0).color(theme.text_muted));
                    swatches(ui, recent.iter().map(|c| ("Recent".to_string(), Color32::from_rgb(c[0], c[1], c[2]))).collect(), &mut picked, state);
                }
            });
    });

    // Clicking outside or Escape closes it; the opening click doesn't count.
    let outside = !state.fresh
        && ctx.input(|i| i.pointer.any_pressed())
        && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area.response.rect.contains(p) && !button.contains(p));
    let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    (!(outside || escape), picked)
}

/// The saturation/brightness square for a hue, as a shaded mesh.
fn sv_mesh(rect: Rect, hue: f32) -> Mesh {
    const N: usize = 12;
    let mut mesh = Mesh::default();
    for j in 0..=N {
        for i in 0..=N {
            let (s, v) = (i as f32 / N as f32, 1.0 - j as f32 / N as f32);
            let p = pos2(rect.left() + s * rect.width(), rect.top() + (1.0 - v) * rect.height());
            mesh.colored_vertex(p, Color32::from(Hsva::new(hue, s, v, 1.0)));
        }
    }
    grid_indices(&mut mesh, N, N);
    mesh
}

/// The rainbow hue bar.
fn hue_mesh(rect: Rect) -> Mesh {
    const N: usize = 36;
    let mut mesh = Mesh::default();
    for j in 0..=1 {
        for i in 0..=N {
            let h = i as f32 / N as f32;
            let p = pos2(rect.left() + h * rect.width(), rect.top() + j as f32 * rect.height());
            mesh.colored_vertex(p, Color32::from(Hsva::new(h.min(0.9999), 1.0, 1.0, 1.0)));
        }
    }
    grid_indices(&mut mesh, N, 1);
    mesh
}

fn grid_indices(mesh: &mut Mesh, cols: usize, rows: usize) {
    let w = cols as u32 + 1;
    for j in 0..rows as u32 {
        for i in 0..cols as u32 {
            let a = j * w + i;
            mesh.add_triangle(a, a + 1, a + w);
            mesh.add_triangle(a + 1, a + w + 1, a + w);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_from_the_square() {
        let ctx = egui::Context::default();
        let theme = Theme::fallback();
        let mut color = Color32::from_rgb(0x58, 0x65, 0xF2);
        // The swatch is as tall as the kit's controls.
        let height = std::cell::Cell::new(22.0);
        let run = |events: Vec<egui::Event>, color: &mut Color32| {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 600.0))), events, ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| {
                height.set(crate::ui::kit::control_height(ui));
                color_button(ui, Id::unique("c"), color, &theme, Some(120.0));
            });
            out.textures_delta.clear();
        };
        let click = |p: egui::Pos2| {
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
                egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
            ]
        };
        run(vec![], &mut color);
        run(click(pos2(20.0, 10.0)), &mut color);
        run(vec![], &mut color);
        assert!(ctx.data(|d| d.get_temp::<Open>(Id::unique("c").with("open"))).is_some(), "clicking the swatch opens the picker");

        // Near the bottom of the square everything is close to black, whatever the hue.
        let near_bottom_left = pos2(14.0, height.get() + 4.0 + 10.0 + SQUARE_H - 4.0);
        run(click(near_bottom_left), &mut color);
        assert!(color.r().max(color.g()).max(color.b()) < 60, "picked {color:?}");

        run(vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }], &mut color);
        run(vec![], &mut color);
        assert!(ctx.data(|d| d.get_temp::<Open>(Id::unique("c").with("open"))).is_none(), "Escape closes it");
        assert_eq!(recent(&ctx).first(), Some(&[color.r(), color.g(), color.b()]), "the picked color is remembered");
    }

    #[test]
    fn hex_parsing() {
        assert_eq!(parse_hex("#5865F2"), Some(Color32::from_rgb(0x58, 0x65, 0xF2)));
        assert_eq!(parse_hex("5865f2"), Some(Color32::from_rgb(0x58, 0x65, 0xF2)));
        assert_eq!(parse_hex("#fff"), Some(Color32::WHITE));
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("#GGGGGG"), None);
        assert_eq!(to_hex(Color32::from_rgb(0, 0xD8, 0xF0)), "#00D8F0");
    }
}
