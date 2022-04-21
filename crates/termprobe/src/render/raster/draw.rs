use tiny_skia::{FillRule, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, Transform};

use super::font::GlyphOutline;
use crate::profile::Rgb;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_glyph(
    pixmap: &mut Pixmap,
    glyph: &GlyphOutline,
    origin_x: f32,
    origin_y: f32,
    cell_width: f32,
    cell_height: f32,
    baseline: f32,
    color: Rgb,
    font_size: f32,
    output_scale: f32,
) {
    let bounds_width = f32::from(glyph.bounds.x_max - glyph.bounds.x_min).max(1.0);
    let bounds_height = f32::from(glyph.bounds.y_max - glyph.bounds.y_min).max(1.0);
    let transform = if glyph.powerline {
        let scale_x = cell_width / bounds_width;
        let scale_y = cell_height / bounds_height;
        Transform::from_row(
            scale_x,
            0.0,
            0.0,
            -scale_y,
            origin_x - f32::from(glyph.bounds.x_min) * scale_x,
            origin_y + f32::from(glyph.bounds.y_max) * scale_y,
        )
    } else {
        let scale_y = font_size * output_scale / f32::from(glyph.units_per_em);
        let (scale_x, x) = if glyph.advance == 0 {
            let rendered_width = bounds_width * scale_y;
            (
                scale_y,
                origin_x + (cell_width - rendered_width) / 2.0
                    - f32::from(glyph.bounds.x_min) * scale_y,
            )
        } else {
            (cell_width / f32::from(glyph.advance), origin_x)
        };
        let shear = if glyph.synthetic_italic {
            0.2 * scale_y
        } else {
            0.0
        };
        Transform::from_row(scale_x, 0.0, shear, -scale_y, x, baseline)
    };

    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, 255);
    pixmap.fill_path(&glyph.path, &paint, FillRule::Winding, transform, None);
    if glyph.synthetic_bold {
        let mut bold = transform;
        bold.tx += 0.65 * output_scale;
        pixmap.fill_path(&glyph.path, &paint, FillRule::Winding, bold, None);
    }
}

pub(super) fn fill_antialiased_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: Rgb,
) {
    let Some(rect) = Rect::from_xywh(x, y, width, height) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, 255);
    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
}

pub(super) fn fill_pixel_rect(
    pixmap: &mut Pixmap,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
