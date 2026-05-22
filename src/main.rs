#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod codegen;
mod panels;
mod project;
mod settings;
mod svg_import;
mod widgets;

use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _};
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Rohkai")
            .with_icon(Arc::new(generate_icon())),
        ..Default::default()
    };
    eframe::run_native(
        "Rohkai",
        options,
        Box::new(|cc| Ok(Box::new(app::RohKaiApp::new(cc)))),
    )
}

/// Rasterise a bold rho/kai maker's mark into a 256x256 RGBA icon buffer.
fn generate_icon() -> egui::viewport::IconData {
    const SIZE: u32 = 256;
    const MARK: &str = "\u{03C1}\u{03D7}";
    const BACKGROUND: [u8; 3] = [0x10, 0x13, 0x16];
    const BORDER: [u8; 3] = [0x34, 0xD3, 0x99];
    const SHADOW: [u8; 3] = [0x03, 0x05, 0x07];
    const MARK_COLOR: [u8; 3] = [0x5A, 0xF0, 0xB8];

    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    draw_rounded_tile(&mut rgba, SIZE, BACKGROUND, BORDER);

    let font = FontRef::try_from_slice(include_bytes!("../assets/fonts/NotoSans-Regular.ttf"))
        .expect("embedded NotoSans bytes are valid");

    let scale = PxScale::from(198.0);
    let scaled = font.as_scaled(scale);

    let mut glyphs = Vec::new();
    let mut bounds: Option<ab_glyph::Rect> = None;
    let mut pen_x = 0.0f32;
    let mut prev_id = None;
    for ch in MARK.chars() {
        let gid = scaled.glyph_id(ch);
        if let Some(prev) = prev_id {
            pen_x += scaled.kern(prev, gid);
        }

        let glyph = gid.with_scale_and_position(scale, ab_glyph::point(pen_x, 0.0));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let glyph_bounds = outlined.px_bounds();
            bounds = Some(match bounds {
                Some(current) => ab_glyph::Rect {
                    min: ab_glyph::point(
                        current.min.x.min(glyph_bounds.min.x),
                        current.min.y.min(glyph_bounds.min.y),
                    ),
                    max: ab_glyph::point(
                        current.max.x.max(glyph_bounds.max.x),
                        current.max.y.max(glyph_bounds.max.y),
                    ),
                },
                None => glyph_bounds,
            });
        }

        glyphs.push((gid, pen_x));
        pen_x += scaled.h_advance(gid);
        prev_id = Some(gid);
    }

    if let Some(bounds) = bounds {
        let mark_width = bounds.max.x - bounds.min.x;
        let mark_height = bounds.max.y - bounds.min.y;
        let dx = (SIZE as f32 - mark_width) / 2.0 - bounds.min.x;
        let dy = (SIZE as f32 - mark_height) / 2.0 - bounds.min.y - 2.0;

        draw_glyph_run(
            &mut rgba,
            SIZE,
            &font,
            scale,
            &glyphs,
            (dx + 5.0, dy + 6.0),
            GlyphPaint {
                color: SHADOW,
                alpha: 0.38,
            },
        );

        // Small-window icons need extra weight, so draw the exact letters with
        // a tight offset cluster instead of depending on a thin font cut.
        for (ox, oy) in [
            (0.0, 0.0),
            (-3.0, 0.0),
            (3.0, 0.0),
            (0.0, -3.0),
            (0.0, 3.0),
            (-2.0, -2.0),
            (2.0, -2.0),
            (-2.0, 2.0),
            (2.0, 2.0),
        ] {
            draw_glyph_run(
                &mut rgba,
                SIZE,
                &font,
                scale,
                &glyphs,
                (dx + ox, dy + oy),
                GlyphPaint {
                    color: MARK_COLOR,
                    alpha: 0.86,
                },
            );
        }
    }

    egui::viewport::IconData {
        rgba,
        width: SIZE,
        height: SIZE,
    }
}

#[derive(Clone, Copy)]
struct GlyphPaint {
    color: [u8; 3],
    alpha: f32,
}

fn draw_rounded_tile(rgba: &mut [u8], size: u32, background: [u8; 3], border: [u8; 3]) {
    let rect_min = 12.0;
    let rect_max = size as f32 - 12.0;
    let radius = 46.0;

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let dist = rounded_rect_sdf(px, py, rect_min, rect_min, rect_max, rect_max, radius);
            let fill_alpha = (0.5 - dist).clamp(0.0, 1.0);
            let border_alpha = (2.3 - dist.abs()).clamp(0.0, 1.0) * 0.22;
            let i = ((y * size + x) * 4) as usize;

            if fill_alpha > 0.0 {
                blend_pixel(rgba, i, background, fill_alpha);
            }
            if border_alpha > 0.0 {
                blend_pixel(rgba, i, border, border_alpha);
            }
        }
    }
}

fn draw_glyph_run(
    rgba: &mut [u8],
    size: u32,
    font: &FontRef<'_>,
    scale: PxScale,
    glyphs: &[(ab_glyph::GlyphId, f32)],
    offset: (f32, f32),
    paint: GlyphPaint,
) {
    let (dx, dy) = offset;

    for &(gid, pen_x) in glyphs {
        let glyph = gid.with_scale_and_position(scale, ab_glyph::point(pen_x + dx, dy));

        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|rx, ry, cov| {
                let px = bounds.min.x as i32 + rx as i32;
                let py = bounds.min.y as i32 + ry as i32;
                if px >= 0 && py >= 0 && (px as u32) < size && (py as u32) < size {
                    let i = ((py as u32 * size + px as u32) * 4) as usize;
                    blend_pixel(rgba, i, paint.color, cov * paint.alpha);
                }
            });
        }
    }
}

fn rounded_rect_sdf(
    x: f32,
    y: f32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
) -> f32 {
    let center_x = (left + right) / 2.0;
    let center_y = (top + bottom) / 2.0;
    let half_width = (right - left) / 2.0 - radius;
    let half_height = (bottom - top) / 2.0 - radius;
    let qx = (x - center_x).abs() - half_width;
    let qy = (y - center_y).abs() - half_height;
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    let inside = qx.max(qy).min(0.0);

    outside + inside - radius
}

fn blend_pixel(rgba: &mut [u8], i: usize, color: [u8; 3], alpha: f32) {
    let src_alpha = alpha.clamp(0.0, 1.0);
    let dst_alpha = rgba[i + 3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

    if out_alpha <= f32::EPSILON {
        return;
    }

    for channel in 0..3 {
        let src = color[channel] as f32 / 255.0;
        let dst = rgba[i + channel] as f32 / 255.0;
        let out = (src * src_alpha + dst * dst_alpha * (1.0 - src_alpha)) / out_alpha;
        rgba[i + channel] = (out * 255.0).round() as u8;
    }
    rgba[i + 3] = (out_alpha * 255.0).round() as u8;
}
