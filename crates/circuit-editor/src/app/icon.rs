// The app icon, drawn in code: the window icon at run time, and (included by
// build.rs) the .exe's icon on Windows. No dependencies, so build.rs can use it.

/// RGBA pixels of the icon at `size`×`size`, with smoothed edges (4×4 samples
/// per pixel).
pub fn icon_rgba(size: u32) -> Vec<u8> {
    const SAMPLES: u32 = 4;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let mut sum = [0f32; 4];
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    // Sample in the 200×200 design space.
                    let px = (x as f32 + (sx as f32 + 0.5) / SAMPLES as f32) * 200.0 / size as f32;
                    let py = (y as f32 + (sy as f32 + 0.5) / SAMPLES as f32) * 200.0 / size as f32;
                    if let Some(c) = icon_color(px, py) {
                        for k in 0..3 {
                            sum[k] += c[k] as f32;
                        }
                        sum[3] += 255.0;
                    }
                }
            }
            let n = (SAMPLES * SAMPLES) as f32;
            let i = ((y * size + x) * 4) as usize;
            if sum[3] > 0.0 {
                // Colors averaged over the covered samples, coverage as alpha.
                let covered = sum[3] / 255.0;
                for k in 0..3 {
                    rgba[i + k] = (sum[k] / covered).round() as u8;
                }
                rgba[i + 3] = (sum[3] / n).round() as u8;
            }
        }
    }
    rgba
}

/// The icon's color at a point of the 200×200 design, or None outside it.
fn icon_color(px: f32, py: f32) -> Option<[u8; 3]> {
    let aqua = [0x00, 0xE5, 0xFF];
    let ink = [0xE4, 0xF4, 0xF7];
    let bg = [0x0A, 0x16, 0x1B];
    let inside_tile = {
        let (cx, cy) = (px.clamp(50.0, 150.0), py.clamp(50.0, 150.0));
        ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() <= 46.0
    };
    if !inside_tile {
        return None;
    }
    let mut color = bg;
    let (dx, dy) = (px - 100.0, py - 100.0);
    let r = (dx * dx + dy * dy).sqrt();
    let angle = dy.atan2(dx).to_degrees();
    if (r - 54.0).abs() <= 8.0 && !(-45.0..=45.0).contains(&angle) {
        color = aqua;
    }
    let on_x = |ax: f32, ay: f32, bx: f32, by: f32| {
        let (vx, vy) = (bx - ax, by - ay);
        let t = (((px - ax) * vx + (py - ay) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
        ((px - ax - t * vx).powi(2) + (py - ay - t * vy).powi(2)).sqrt() <= 6.5
    };
    if on_x(82.0, 82.0, 118.0, 118.0) || on_x(118.0, 82.0, 82.0, 118.0) {
        color = ink;
    }
    for (cx, cy) in [(138.2, 61.8), (138.2, 138.2)] {
        let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
        if d <= 16.5 {
            color = if d >= 9.5 { aqua } else { [0x07, 0x10, 0x14] };
        }
    }
    Some(color)
}
