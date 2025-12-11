use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::api::RecordingFormat;
use crate::profile::Profile;
use crate::record::frames::Frame;
use crate::render::svg::RenderState;
use crate::terminal::alacritty::AlacrittyEmu;
use crate::terminal::cell::{Attrs, Color, EmuCell, NamedColor, UnderlineStyle};

use super::encode;
use super::raster::GridRenderer;

const COLS: u16 = 28;
const ROWS: usize = 5;
const UPDATE_ENV: &str = "UPDATE_TERMPROBE__RENDER_SNAPSHOTS";

#[test]
fn single_frame_gif_and_png_renders_match_snapshots() {
    for case in cases() {
        for (format, extension) in [
            (RecordingFormat::Gif, "gif"),
            (RecordingFormat::Apng, "png"),
        ] {
            let output = temporary_output(case.name, extension);
            let frame = Frame {
                grid: case.grid.clone(),
                title: None,
                duration: Duration::from_millis(250),
                render_state: RenderState::capture(&AlacrittyEmu::new(
                    COLS,
                    ROWS as u16,
                    &Profile::default(),
                )),
                cursor: case.cursor,
            };
            let mut renderer = GridRenderer::with_zoom(
                COLS,
                ROWS,
                case.zoom,
                crate::render::style::Style::default(),
            )
            .unwrap();
            encode::encode(&output, format, &[frame], &mut renderer, 30, None).unwrap();
            let actual = std::fs::read(&output).unwrap();
            std::fs::remove_file(output).unwrap();
            assert_snapshot(case.name, extension, &actual);
        }
    }
}

struct SnapshotCase {
    name: &'static str,
    grid: Vec<Vec<EmuCell>>,
    cursor: Option<(u16, usize)>,
    zoom: f64,
}

fn cases() -> [SnapshotCase; 4] {
    [
        SnapshotCase {
            name: "regular",
            grid: regular_grid(),
            cursor: None,
            zoom: 2.0,
        },
        SnapshotCase {
            name: "styles",
            grid: styles_grid(),
            cursor: None,
            zoom: 2.0,
        },
        SnapshotCase {
            name: "nerd-fonts",
            grid: nerd_font_grid(),
            cursor: None,
            zoom: 2.0,
        },
        SnapshotCase {
            name: "pixel-alignment",
            grid: pixel_alignment_grid(),
            cursor: Some((27, 1)),
            zoom: 1.25,
        },
    ]
}

fn regular_grid() -> Vec<Vec<EmuCell>> {
    let mut grid = blank_grid();
    write_text(
        &mut grid,
        0,
        0,
        "termprobe render snapshot",
        CellStyle::default(),
    );
    write_text(
        &mut grid,
        1,
        0,
        "ANSI color palette",
        CellStyle {
            fg: Some(Color::Named(NamedColor::BrightCyan)),
            ..CellStyle::default()
        },
    );
    write_text(
        &mut grid,
        2,
        0,
        "truecolor background",
        CellStyle {
            fg: Some(Color::Rgb(250, 245, 235)),
            bg: Some(Color::Rgb(96, 48, 128)),
            ..CellStyle::default()
        },
    );
    write_text(&mut grid, 3, 0, "0123456789 -> [] {}", CellStyle::default());
    write_text(
        &mut grid,
        4,
        0,
        "full glyphs: \u{03bb} \u{0416} \u{2260}",
        CellStyle::default(),
    );
    grid
