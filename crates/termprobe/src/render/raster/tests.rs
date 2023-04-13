#[cfg(feature = "recording-font-jetbrains-mono-styles")]
use super::font::{FontSystem, GlyphKey};
use super::{FrameRenderer, GridRenderer, RgbaFrame};
use crate::api::CaptureBackground;
use crate::profile::Profile;
use crate::record::frames::Frame;
use crate::render::style::{Padding, Style};
use crate::render::svg::{RenderColors, RenderState};
use crate::terminal::alacritty::AlacrittyEmu;
use crate::terminal::cell::{Attrs, Color, EmuCell, CONTINUATION};
use crate::terminal::emu::Emulator;
use std::time::Duration;

fn cell(character: &str, attrs: Attrs) -> EmuCell {
    EmuCell {
        ch: character.into(),
        fg: Some(Color::Rgb(220, 220, 220)),
        attrs,
        ..EmuCell::blank()
    }
}

fn frame(grid: Vec<Vec<EmuCell>>) -> Frame {
    frame_with_cursor(grid, None)
}

fn frame_with_cursor(grid: Vec<Vec<EmuCell>>, cursor: Option<(u16, usize)>) -> Frame {
    let emulator = AlacrittyEmu::new(
        grid.first().map_or(1, Vec::len) as u16,
        grid.len().max(1) as u16,
        &Profile::default(),
    );
    Frame {
        grid,
        title: None,
        duration: Duration::ZERO,
        render_state: RenderState::capture(&emulator),
        cursor,
    }
}

#[test]
fn repeated_renders_are_byte_identical() {
    let frame = frame(vec![vec![cell("x", Attrs::empty())]]);
    let mut renderer = GridRenderer::new(1, 1);
    let first = renderer.render(&frame).unwrap();
    let second = renderer.render(&frame).unwrap();
    assert_eq!(first.as_raw(), second.as_raw());
    assert_eq!(renderer.pixel_size().1 % 2, 0);
}

#[test]
fn scaled_renderers_multiply_output_dimensions() {
    let standard = GridRenderer::new(80, 30);
    let hidpi = GridRenderer::with_scale(80, 30, 2);
    assert_eq!(
        hidpi.pixel_size(),
        (standard.pixel_size().0 * 2, standard.pixel_size().1 * 2)
    );
}

#[test]
fn static_screenshots_keep_odd_svg_dimensions_at_zoom() {
    // Sizes follow the default style; update them deliberately if a default
    // gap moves. The point is the difference: a recording is rounded up to
    // even dimensions for the video encoders, a screenshot is not.
    for (zoom, recording_size, screenshot_size) in
        [(1.0, (88, 126), (88, 125)), (2.0, (176, 252), (176, 250))]
    {
        let recording = GridRenderer::with_zoom(1, 1, zoom, Style::default()).unwrap();
        let screenshot = GridRenderer::for_screenshot(1, 1, zoom, Style::default(), None).unwrap();

        assert_eq!(recording.pixel_size(), recording_size);
        assert_eq!(screenshot.pixel_size(), screenshot_size);
    }
}

#[test]
fn fractional_zoom_shrinks_output_without_changing_grid_dimensions() {
    let standard = GridRenderer::new(80, 30);
    let half = GridRenderer::with_zoom(80, 30, 0.5, Style::default()).unwrap();
    assert_eq!(
        half.pixel_size(),
        (
            standard.pixel_size().0.div_ceil(2),
            standard.pixel_size().1.div_ceil(2)
        )
    );

    let mut half = half;
    half.render(&frame(vec![vec![EmuCell::blank(); 80]; 30]))
        .unwrap();
}

#[test]
fn canvas_background_can_be_custom_or_transparent() {
    let frame = frame(vec![vec![cell("x", Attrs::empty())]]);
    let mut custom = GridRenderer::with_zoom_and_background(
        1,
        1,
        1.0,
        Style::default(),
        Some(CaptureBackground::Color(crate::profile::Rgb::new(1, 2, 3))),
    )
    .unwrap();
    let custom = custom.render(&frame).unwrap();
    assert_eq!(&custom.as_raw()[..4], &[1, 2, 3, 255]);

    let mut transparent = GridRenderer::with_zoom_and_background(
        1,
        1,
        1.0,
        Style::default(),
        Some(CaptureBackground::Transparent),
    )
    .unwrap();
    let transparent = transparent.render(&frame).unwrap();
    assert_eq!(&transparent.as_raw()[..4], &[0, 0, 0, 0]);
}

#[test]
fn adjacent_background_cells_are_seamless_at_fractional_zoom() {
    let backgrounds = [
        Color::Rgb(125, 86, 244),
        Color::Rgb(125, 86, 244),
        Color::Rgb(125, 86, 244),
        Color::Rgb(236, 106, 94),
        Color::Rgb(236, 106, 94),
        Color::Rgb(236, 106, 94),
        Color::Rgb(244, 191, 79),
        Color::Rgb(244, 191, 79),
        Color::Rgb(244, 191, 79),
        Color::Rgb(97, 197, 84),
        Color::Rgb(97, 197, 84),
        Color::Rgb(97, 197, 84),
    ];
    let rows = 5;
    let grid = vec![
        backgrounds
            .iter()
            .copied()
            .map(|background| EmuCell {
                bg: Some(background),
                ..EmuCell::blank()
            })
            .collect::<Vec<_>>();
        rows
    ];

    for zoom in [1.02, 1.25] {
        let mut renderer =
            GridRenderer::with_zoom(backgrounds.len() as u16, rows, zoom, Style::default())
                .unwrap();
        let image = renderer.render(&frame(grid.clone())).unwrap();
        let (panel_width, panel_height) =
            crate::render::svg::pixel_size(backgrounds.len() as u16, rows, &Style::default());
        let panel_width = super::scaled_dimension(panel_width, zoom, "test width").unwrap();
        let panel_height = super::scaled_dimension(panel_height, zoom, "test height").unwrap();
        let origin_x = (image.dimensions().0 - panel_width) as f32 / 2.0;
        let origin_y = (image.dimensions().1 - panel_height) as f32 / 2.0;
        let scale = zoom as f32;

        for row in 0..rows {
            let top = grid_y(origin_y, row, scale);
            let bottom = grid_y(origin_y, row + 1, scale);
            for (column, background) in backgrounds.iter().copied().enumerate() {
                let left = grid_x(origin_x, column, scale);
                let right = grid_x(origin_x, column + 1, scale);
                let expected = color_to_pixel(background);
                for y in top..bottom {
                    for x in left..right {
                        assert_eq!(
                            pixel_at(&image, x, y),
                            expected,
                            "unexpected pixel at cell ({column}, {row}), ({x}, {y}), zoom {zoom}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn block_cursor_is_aligned_with_background_cells_at_fractional_zoom() {
    let background = Color::Rgb(97, 197, 84);
    let mut row = vec![
        EmuCell {
            bg: Some(background),
            ..EmuCell::blank()
        };
        3
    ];
    row.push(EmuCell::blank());

    for zoom in [1.02, 1.25] {
        let content = frame_with_cursor(vec![row.clone()], Some((3, 0)));
        let cursor = content
            .render_state
            .color(crate::profile::ColorSlot::Cursor);
        let mut renderer = GridRenderer::with_zoom(4, 1, zoom, Style::default()).unwrap();
        let image = renderer.render(&content).unwrap();
        let (panel_width, panel_height) = crate::render::svg::pixel_size(4, 1, &Style::default());
        let panel_width = super::scaled_dimension(panel_width, zoom, "test width").unwrap();
        let panel_height = super::scaled_dimension(panel_height, zoom, "test height").unwrap();
        let origin_x = (image.dimensions().0 - panel_width) as f32 / 2.0;
        let origin_y = (image.dimensions().1 - panel_height) as f32 / 2.0;
        let scale = zoom as f32;
        let top = grid_y(origin_y, 0, scale);
        let bottom = grid_y(origin_y, 1, scale);
        let background_left = grid_x(origin_x, 2, scale);
        let cursor_left = grid_x(origin_x, 3, scale);
        let cursor_right = grid_x(origin_x, 4, scale);

        for y in top..bottom {
            for x in background_left..cursor_left {
                assert_eq!(
                    pixel_at(&image, x, y),
                    color_to_pixel(background),
                    "background gap before cursor at ({x}, {y}), zoom {zoom}"
                );
            }
            for x in cursor_left..cursor_right {
                assert_eq!(
                    pixel_at(&image, x, y),
                    [cursor.r, cursor.g, cursor.b, 255],
                    "misaligned cursor pixel at ({x}, {y}), zoom {zoom}"
                );
