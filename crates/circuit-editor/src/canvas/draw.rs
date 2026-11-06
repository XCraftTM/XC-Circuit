//! Smaller drawing pieces: the grid, toolbar buttons, tooltips and sharp text at any zoom.

use super::*;

pub(super) fn draw_grid(painter: &egui::Painter, visible: Rect, theme: &Theme, zoom: f32) {
    let step = if zoom < 0.5 { GRID * 2.0 } else { GRID };
    let x0 = (visible.left() / step).floor() as i32;
    let y0 = (visible.top() / step).floor() as i32;
    let cols = (visible.width() / step).ceil() as i32 + 1;
    let rows = (visible.height() / step).ceil() as i32 + 1;
    if cols * rows > 40_000 {
        return;
    }
    let r = 1.3 / zoom.max(0.5);
    for i in 0..cols {
        for j in 0..rows {
            painter.circle_filled(pos2((x0 + i) as f32 * step, (y0 + j) as f32 * step), r, theme.canvas_dots);
        }
    }
}

/// A framed button for the canvas toolbars: an icon and a label.
pub(super) fn tool_button(ui: &mut Ui, theme: &Theme, icon: Option<&std::path::Path>, text: &str) -> Response {
    let text = RichText::new(text).size(12.5).color(theme.text);
    let button = match icon {
        Some(p) => egui::Button::image_and_text(egui::Image::from_uri(crate::editor::file_uri(p)).tint(theme.text).fit_to_exact_size(vec2(14.0, 14.0)), text),
        None => egui::Button::new(text),
    };
    let h = ui.spacing().interact_size.y.max(32.0);
    crate::ui::kit::styled_button(ui, theme, button.corner_radius(7).min_size(vec2(h, h)), theme.panel, Stroke::new(1.0, theme.node_border))
}

pub fn block_tooltip(ui: &mut Ui, def: &BlockDef, theme: &Theme) {
    ui.set_max_width(280.0);
    ui.horizontal(|ui| {
        ui.strong(&def.name);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format!("v{}", def.version)).small().color(theme.text_muted));
        });
    });
    ui.label(egui::RichText::new(format!("by {} · {}", def.author, def.category.as_deref().unwrap_or("Other"))).small().color(theme.text_muted));
    ui.label(&def.description);
}

pub(super) fn pin_tooltip(ui: &mut Ui, spot: &PinSpot, reg: &Registry, theme: &Theme) {
    ui.set_max_width(260.0);
    let color = theme.pin_color(&spot.ty, reg);
    ui.horizontal(|ui| {
        ui.strong(&spot.label);
        let chip = egui::RichText::new(spot.ty.display(reg)).small().color(mix(color, theme.text, 0.25));
        egui::Frame::new().fill(with_alpha(color, 40)).corner_radius(99).inner_margin(egui::Margin::symmetric(7, 1)).show(ui, |ui| ui.label(chip));
    });
    if let Some(d) = &spot.description {
        ui.label(egui::RichText::new(d).color(theme.text_muted));
    }
    if spot.faint {
        ui.label(egui::RichText::new(t!("Click to turn this field into a pin, or drop a wire on it.")).small().italics().color(theme.text_muted));
    }
}

/// Below this size on screen (in points), canvas text gets help to stay
/// readable: a heavier weight, more contrast and (below [`MIN_TEXT`]) a little more size.
const SMALL_TEXT: f32 = 9.5;
/// The smallest canvas text on screen, in points, as far as [`MAX_GROW`] allows.
const MIN_TEXT: f32 = 8.0;
/// How much bigger than its place small text may get.
const MAX_GROW: f32 = 1.3;
/// How much wider than it was small text may get, bold and bigger: wider
/// still, and it would run into what's next to it.
const MAX_WIDER: f32 = 1.15;

/// A text laid out again, its size on screen in points, how much brighter
/// grey goes (0 to 1), and the scale left over from rounding its size.
type Redone = (std::sync::Arc<egui::Galley>, f32, f32, f32);

/// egui draws text in a zoomed layer from glyphs rendered for 100% and then
/// scales them, which blurs them (at 65% the letters are shrunken bitmaps).
/// This lays every text in the canvas out again at the size it appears on
/// screen, shrinks it back so the layer's zoom brings it to exactly that size,
/// and snaps it to whole pixels.
///
/// Zoomed out, text that would be tiny is also made readable: bold instead of
/// Ubuntu Light, grey text brighter (towards `text`), and the smallest a
/// little bigger, centred on where it was.
pub(super) fn sharpen_text(ctx: &egui::Context, layer: LayerId, to_global: TSTransform, text: Color32) {
    let zoom = to_global.scaling;
    let ppp = ctx.pixels_per_point();
    let texts: Vec<(usize, Pos2, std::sync::Arc<egui::Galley>)> = ctx.graphics(|g| {
        g.get(layer)
            .map(|list| list.all_entries().enumerate().filter_map(|(i, c)| if let egui::Shape::Text(t) = &c.shape { Some((i, t.pos, t.galley.clone())) } else { None }).collect())
            .unwrap_or_default()
    });
    if texts.is_empty() {
        return;
    }
    let bold = ctx.fonts(|f| f.definitions().families.contains_key(&crate::ui::fonts::bold(1.0).family));
    // Laid out before touching the paint lists: both need the context's lock.
    let sharp: Vec<(usize, Pos2, Option<Redone>)> = texts
        .into_iter()
        .map(|(i, pos, galley)| {
            let size = galley.job.sections.iter().map(|s| s.format.font_id.size).fold(0.0, f32::max);
            let screen = size * zoom;
            let small = screen > 0.0 && screen < SMALL_TEXT;
            // Only a single line that doesn't wrap may get bigger and bolder:
            // wrapped text (a message box, the embed preview) has to fit its box.
            let free = galley.rows.len() <= 1 && !galley.job.wrap.max_width.is_finite();
            let grow = if small && free && screen < MIN_TEXT { (MIN_TEXT / screen).min(MAX_GROW) } else { 1.0 };
            let redo = (zoom - 1.0).abs() > 0.001 || small;
            // In steps, like the size below: a zoom in progress then reuses
            // the layouts egui keeps, instead of laying everything out anew.
            let how = (((SMALL_TEXT - screen) / (SMALL_TEXT - 5.0)).clamp(0.0, 1.0) * 8.0).round() / 8.0;
            // Laid out at `k` times its size, rounded to a quarter point; the
            // returned scale (within a percent or two of 1) makes up the rest.
            let lay = |k: f32| {
                let kq = if size > 0.0 { ((size * k * 4.0).round() / 4.0).max(0.25) / size } else { k };
                let mut job = scaled_job(&galley.job, kq);
                if small {
                    readable(&mut job, how, text, bold && free);
                }
                (ctx.fonts_mut(|f| f.layout_job(job)), k / kq)
            };
            let big = redo.then(|| {
                let (mut g, mut fix) = lay(zoom * grow);
                let wider = g.size().x * fix / zoom / galley.size().x.max(0.1);
                if free && small && wider > MAX_WIDER {
                    (g, fix) = lay(zoom * grow * MAX_WIDER / wider);
                }
                (g, screen, how, fix)
            });
            // Bigger (or bolder, so wider) text stays centred on its place.
            let pos = match &big {
                Some((g, _, _, fix)) if free => pos - (g.size() * *fix / zoom - galley.size()) / 2.0,
                _ => pos,
            };
            let screen_pos = to_global * pos;
            let snapped = to_global.inverse() * pos2((screen_pos.x * ppp).round() / ppp, (screen_pos.y * ppp).round() / ppp);
            (i, snapped, big)
        })
        .collect();
    ctx.graphics_mut(|g| {
        let list = g.entry(layer);
        for (i, pos, big) in sharp {
            list.mutate_shape(egui::layers::ShapeIdx(i), |c| {
                if let egui::Shape::Text(t) = &mut c.shape {
                    if let Some((big, screen, how, fix)) = big {
                        t.galley = big;
                        t.transform(TSTransform::from_scaling(fix / zoom));
                        if screen < SMALL_TEXT {
                            t.fallback_color = brighter(t.fallback_color, text, how);
                        }
                    }
                    t.pos = pos;
                }
            });
        }
    });
}

/// Small text made readable: bold instead of the light regular font (when
/// the bold font is installed), grey colours brighter by `how` (0 to 1).
fn readable(job: &mut egui::text::LayoutJob, how: f32, text: Color32, bold: bool) {
    for s in &mut job.sections {
        let f = &mut s.format;
        if bold && f.font_id.family == egui::FontFamily::Proportional {
            f.font_id = crate::ui::fonts::bold(f.font_id.size);
        }
        f.color = brighter(f.color, text, how);
    }
}

/// A grey `color` moved towards `text` by up to 60% (`how` from 0 to 1);
/// colours (accents, errors) and the placeholder stay as they are.
fn brighter(color: Color32, text: Color32, how: f32) -> Color32 {
    if color == Color32::PLACEHOLDER {
        return color;
    }
    let [r, g, b, a] = color.to_array();
    let grey = r.max(g).max(b) - r.min(g).min(b) < 32;
    if !grey {
        return color;
    }
    let mixed = crate::ui::theme::mix(color, text, how.clamp(0.0, 1.0) * 0.6);
    Color32::from_rgba_unmultiplied(mixed.r(), mixed.g(), mixed.b(), a)
}

/// A copy of a layout job with every size multiplied by `k`.
pub(super) fn scaled_job(job: &egui::text::LayoutJob, k: f32) -> egui::text::LayoutJob {
    let mut job = job.clone();
    if job.wrap.max_width.is_finite() {
        job.wrap.max_width *= k;
    }
    job.first_row_min_height *= k;
    for s in &mut job.sections {
        s.leading_space *= k;
        let f = &mut s.format;
        f.font_id.size *= k;
        f.extra_letter_spacing *= k;
        f.line_height = f.line_height.map(|h| h * k);
        f.expand_bg *= k;
        f.underline.width *= k;
        f.strikethrough.width *= k;
    }
    job
}
