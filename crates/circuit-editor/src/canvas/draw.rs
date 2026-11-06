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

/// egui draws text in a zoomed layer from glyphs rendered for 100% and then
/// scales them, which blurs them (at 65% the letters are shrunken bitmaps).
/// This lays every text in the canvas out again at the size it appears on
/// screen, shrinks it back so the layer's zoom brings it to exactly that size,
/// and snaps it to whole pixels.
pub(super) fn sharpen_text(ctx: &egui::Context, layer: LayerId, to_global: TSTransform) {
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
    let rescale = (zoom - 1.0).abs() > 0.001;
    // Laid out before touching the paint lists: both need the context's lock.
    let sharp: Vec<(usize, Pos2, Option<std::sync::Arc<egui::Galley>>)> = texts
        .into_iter()
        .map(|(i, pos, galley)| {
            let screen = to_global * pos;
            let snapped = to_global.inverse() * pos2((screen.x * ppp).round() / ppp, (screen.y * ppp).round() / ppp);
            let big = rescale.then(|| ctx.fonts_mut(|f| f.layout_job(scaled_job(&galley.job, zoom))));
            (i, snapped, big)
        })
        .collect();
    ctx.graphics_mut(|g| {
        let list = g.entry(layer);
        for (i, pos, big) in sharp {
            list.mutate_shape(egui::layers::ShapeIdx(i), |c| {
                if let egui::Shape::Text(t) = &mut c.shape {
                    if let Some(big) = big {
                        t.galley = big;
                        t.transform(TSTransform::from_scaling(1.0 / zoom));
                    }
                    t.pos = pos;
                }
            });
        }
    });
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
