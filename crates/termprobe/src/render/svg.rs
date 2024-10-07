//! Crisp, full-color SVG screenshot of a terminal grid, styled after
//! `svg-term-cli`: a rounded window panel with macOS-style controls.
//!
//! Vector output renders sharply at any zoom. The viewer supplies a monospace
//! face for text, while Nerd Font icons are emitted from a bundled symbols font
//! as SVG paths. Colors, bold/italic/underline/strike, inverse, and dim are all
//! preserved. Each run of same-styled cells is forced to its exact column width
//! via `textLength`, so alignment is independent of the rendering font's
//! metrics.

use std::fmt::Write;

use super::nerd_font::NerdFont;
use crate::api::CaptureBackground;
use crate::profile::{ColorSlot, Profile, Rgb};
use crate::render::style::Style;
use crate::terminal::cell::{truncate_to_columns, Attrs, Color, EmuCell, CONTINUATION};
use crate::terminal::emu::{CursorShape, Emulator};

pub(crate) const DOT_R: f32 = 7.0;
pub(crate) const RED_DOT_R: f32 = 2.5;
pub(crate) const RED_DOT_COLOR: Rgb = Rgb::new(105, 17, 10);
fn dots_right(style: &Style) -> f32 {
    style.content_left() + 5.0 + 2.0 * 20.0 + DOT_R
}

fn hex(c: Rgb) -> String {
    c.to_hex()
}

fn svg_dimension(value: f64) -> String {
    let mut output = format!("{value:.4}");
    while output.contains('.') && output.ends_with('0') {
        output.pop();
    }
    if output.ends_with('.') {
        output.pop();
    }
    output
}

fn dim(c: Rgb) -> Rgb {
    let s = |v: u8| (v as f32 * 0.6) as u8;
    Rgb::new(s(c.r), s(c.g), s(c.b))
}

static BLANK: EmuCell = EmuCell::blank();

fn cell_at(row: &[EmuCell], x: usize) -> &EmuCell {
    row.get(x).unwrap_or(&BLANK)
}

pub(crate) trait RenderColors {
    fn color(&self, slot: ColorSlot) -> Rgb;
    fn cursor_shape(&self) -> CursorShape;

    fn resolve(&self, color: Option<Color>, is_fg: bool) -> Rgb {
        match color {
            None => self.color(if is_fg {
                ColorSlot::Foreground
            } else {
                ColorSlot::Background
            }),
            Some(Color::Named(n)) => self.color(ColorSlot::Indexed(n.index())),
            Some(Color::Idx(i)) => self.color(ColorSlot::Indexed(i)),
            Some(Color::Rgb(r, g, b)) => Rgb::new(r, g, b),
        }
    }
}

impl<T: Emulator + ?Sized> RenderColors for T {
    fn color(&self, slot: ColorSlot) -> Rgb {
        Emulator::color(self, slot)
    }

    fn cursor_shape(&self) -> CursorShape {
        Emulator::cursor_shape(self)
    }
}

impl RenderColors for Profile {
    fn color(&self, slot: ColorSlot) -> Rgb {
        match slot {
            ColorSlot::Indexed(index) => self.colors.rgb(index),
            ColorSlot::Foreground => self.colors.foreground,
            ColorSlot::Background => self.colors.background,
            ColorSlot::Cursor => self.colors.cursor,
        }
    }

    fn cursor_shape(&self) -> CursorShape {
        CursorShape::Block
    }
}

/// The renderer-owned terminal state captured together with the grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderState {
    indexed: [Rgb; 256],
    foreground: Rgb,
    background: Rgb,
    cursor: Rgb,
    cursor_shape: CursorShape,
}

impl RenderState {
    pub(crate) fn capture(emu: &dyn Emulator) -> Self {
        Self {
            indexed: std::array::from_fn(|index| emu.color(ColorSlot::Indexed(index as u8))),
            foreground: emu.color(ColorSlot::Foreground),
            background: emu.color(ColorSlot::Background),
            cursor: emu.color(ColorSlot::Cursor),
            cursor_shape: emu.cursor_shape(),
        }
    }
}

impl RenderColors for RenderState {
    fn color(&self, slot: ColorSlot) -> Rgb {
        match slot {
            ColorSlot::Indexed(index) => self.indexed[index as usize],
            ColorSlot::Foreground => self.foreground,
            ColorSlot::Background => self.background,
            ColorSlot::Cursor => self.cursor,
        }
    }

    fn cursor_shape(&self) -> CursorShape {
        self.cursor_shape
    }
}

/// Resolved background color for a cell (honoring inverse).
pub(crate) fn bg_of(cell: &EmuCell, colors: &dyn RenderColors) -> Rgb {
    let bg = colors.resolve(cell.bg, false);
    let fg = colors.resolve(cell.fg, true);
    if cell.has(Attrs::INVERSE) {
        fg
    } else {
        bg
    }
}

/// Cell colors and attributes after inverse and dim are resolved.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct CellPaint {
    pub fg: Rgb,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub invisible: bool,
}

pub(crate) fn cell_paint(cell: &EmuCell, colors: &dyn RenderColors) -> CellPaint {
    let mut fg = colors.resolve(cell.fg, true);
    let bg = colors.resolve(cell.bg, false);
    if cell.has(Attrs::INVERSE) {
        fg = bg;
    }
    if cell.has(Attrs::DIM) {
        fg = dim(fg);
    }
    CellPaint {
        fg,
        bold: cell.has(Attrs::BOLD),
        italic: cell.has(Attrs::ITALIC),
        underline: cell.underline.is_underlined(),
        strike: cell.has(Attrs::STRIKE),
        invisible: cell.has(Attrs::INVISIBLE),
    }
}

/// Escape text and quotes for a double-quoted XML attribute.
fn escape_attribute(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

fn run_text(row: &[EmuCell], start: usize, end: usize) -> String {
    let mut text = String::with_capacity(end - start);
    for i in start..end {
        text.push_str(&cell_at(row, i).ch);
    }
    text
}

fn write_text_run(
    out: &mut String,
    row: &[EmuCell],
    run: std::ops::Range<usize>,
    y: usize,
    paint: CellPaint,
    nerd_font: &NerdFont,
    style: &Style,
) {
    if paint.invisible {
        return;
    }
    let (start, end) = (run.start, run.end);
    let cell_w = style.cell_width();
    let cell_h = style.cell_height();
    let header_h = style.header_height();
    let font_baseline = style.baseline();
    let fg = hex(paint.fg);
    let tx = style.content_left() + start as f32 * cell_w;
    let baseline = header_h + style.content_top() + y as f32 * cell_h + font_baseline;
    let width = (end - start) as f32 * cell_w;
    let original_text = run_text(row, start, end);
    let (text, run_x_adjust) = nerd_font.prepare_run(&original_text, width, cell_w);

    // Preserve decoration for runs containing only vector glyphs.
    if !original_text.trim().is_empty() {
        let weight = if paint.bold {
            r#" font-weight="bold""#
        } else {
            ""
        };
        let italic = if paint.italic {
            r#" font-style="italic""#
        } else {
            ""
        };
        let deco = match (paint.underline, paint.strike) {
            (true, true) => r#" text-decoration="underline line-through""#,
            (true, false) => r#" text-decoration="underline""#,
            (false, true) => r#" text-decoration="line-through""#,
            (false, false) => "",
        };
        let resolved = style.font.resolve(paint.bold, paint.italic);
        let family = if resolved == style.font.family {
            String::new()
        } else {
            format!(r#" font-family="{}""#, escape_attribute(resolved))
        };
        let _ = write!(
            out,
            r#"<text x="{tx:.2}" y="{baseline:.2}" fill="{fg}"{family}{weight}{italic}{deco} textLength="{width:.2}" lengthAdjust="spacingAndGlyphs" xml:space="preserve">{esc}</text>"#,
            esc = escape(&text)
        );
    }
    for i in start..end {
        for c in cell_at(row, i).ch.chars() {
            nerd_font.write_use(
                out,
                c,
                (
                    style.content_left() + i as f32 * cell_w,
                    header_h + style.content_top() + y as f32 * cell_h,
                ),
                (cell_w, cell_h),
                run_x_adjust,
                &fg,
            );
        }
    }
}

pub(crate) fn title_advance(style: &Style) -> f32 {
    style.title_font_size * (style.cell_width() / style.font_size)
}

pub(crate) fn media_title(title: Option<&str>, cols: u16, rows: usize, fits: usize) -> String {
    let base = title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .unwrap_or("termprobe capture");
    let suffix = format!(" - {cols}x{rows}");
    let full = format!("{base}{suffix}");
    if crate::terminal::cell::display_width(&full) <= fits {
        return full;
    }

    let suffix_width = crate::terminal::cell::display_width(&suffix);
    if suffix_width >= fits {
        return truncate_to_columns(&full, fits);
    }
    format!("{}{suffix}", truncate_to_columns(base, fits - suffix_width))
}

pub(crate) fn visible_title(
    title: Option<&str>,
    cols: u16,
    rows: usize,
    width: f32,
    style: &Style,
) -> Option<String> {
    const GAP: f32 = 8.0;
    let margin = if style.window.traffic_lights {
        dots_right(style) + GAP
    } else {
        style.content_left().max(style.content_right()).max(GAP)
    };
    let available = width - 2.0 * margin;
    let fits = (available / title_advance(style)).floor().max(0.0) as usize;
    if fits == 0 {
        return None;
    }
    Some(media_title(title, cols, rows, fits))
}

fn write_title(
    out: &mut String,
    title: Option<&str>,
    cols: u16,
    rows: usize,
    width: f32,
    style: &Style,
) {
    let Some(shown) = visible_title(title, cols, rows, width, style) else {
        return;
    };
    let header_h = style.header_height();
    let title_font_size = style.title_font_size;
    let title_fg = style.window.foreground;
    let resolved = style.font.resolve(true, false);
    let family = if resolved == style.font.family {
        String::new()
    } else {
        format!(r#" font-family="{}""#, escape_attribute(resolved))
    };
    let _ = write!(
        out,
        r#"<text x="{cx:.2}" y="{baseline:.2}" fill="{fill}" font-size="{title_font_size}px"{family} font-weight="bold" text-anchor="middle" xml:space="preserve">{esc}</text>"#,
        cx = width / 2.0,
        baseline = header_h / 2.0 + title_font_size * 0.35,
        fill = hex(title_fg),
        esc = escape(&shown),
    );
}

/// How much of a cell the thin cursor shapes cover.
const CURSOR_THICKNESS: f32 = 2.0;

/// Draw the cursor over the cell it sits on.
///
/// A block is filled and the character redrawn in the cell's background color,
/// which is how a terminal keeps the character under a block cursor readable.
/// It is drawn after the text pass so the block covers the first, normally
/// colored draw of that character.
fn write_cursor(
    out: &mut String,
    rows: &[Vec<EmuCell>],
    (cx, cy): (u16, usize),
    colors: &dyn RenderColors,
    nerd_font: &NerdFont,
    style: &Style,
) {
    let Some(row) = rows.get(cy) else {
        return;
    };
    let cell_w = style.cell_width();
    let cell_h = style.cell_height();
    let header_h = style.header_height();
    let cell = cell_at(row, cx as usize);
    // A double-width character stores its second half as a continuation cell,
    // so the cursor has to cover both or it clips the glyph down the middle.
    let span = if row
        .get(cx as usize + 1)
        .is_some_and(|next| next.ch == CONTINUATION)
    {
        2.0
    } else {
        1.0
    };
    let w = span * cell_w;
    let x = style.content_left() + cx as f32 * cell_w;
    let y = header_h + style.content_top() + cy as f32 * cell_h;
    let fill = hex(colors.color(ColorSlot::Cursor));

    let (rx, ry, rw, rh) = match colors.cursor_shape() {
        CursorShape::Block => (x, y, w, cell_h),
        CursorShape::Underline => (x, y + cell_h - CURSOR_THICKNESS, w, CURSOR_THICKNESS),
        CursorShape::Bar => (x, y, CURSOR_THICKNESS, cell_h),
    };
    let _ = write!(
        out,
        r#"<rect x="{rx:.2}" y="{ry:.2}" width="{rw:.2}" height="{rh:.2}" fill="{fill}"/>"#
    );

    if colors.cursor_shape() != CursorShape::Block {
        return;
    }
    let mut paint = cell_paint(cell, colors);
    paint.fg = bg_of(cell, colors);
    write_text_run(
        out,
        row,
        cx as usize..cx as usize + span as usize,
        cy,
        paint,
        nerd_font,
