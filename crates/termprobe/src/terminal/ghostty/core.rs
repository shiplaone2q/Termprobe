//! Single-threaded Ghostty terminal and neutral-grid translation.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use compact_str::CompactString;
use ghostty_vt::error::Error as GhosttyError;
use ghostty_vt::key::{
    Action as GhosttyKeyAction, Encoder, Event as GhosttyKeyEvent, Key, KittyKeyFlags,
    Mods as GhosttyMods, OptionAsAlt,
};
use ghostty_vt::render::{CellIterator, CursorVisualStyle, RowIterator};
use ghostty_vt::screen::{Cell as GhosttyCell, CellContentTag, CellWide, GridRef, Screen};
use ghostty_vt::style::{Palette, PaletteIndex, RgbColor, Style, StyleColor, Underline};
use ghostty_vt::terminal::{
    ConformanceLevel, DeviceAttributeFeature, DeviceAttributes, DeviceType, Mode, Point,
    PointCoordinate, PointSpace, PrimaryDeviceAttributes, ScrollViewport,
    SecondaryDeviceAttributes, TertiaryDeviceAttributes,
};
use ghostty_vt::{RenderState, Terminal};

use crate::event::BellTracker;
use crate::input::keys::{KeyEventKind, KeyPress, Mods};
use crate::profile::{xterm_color, ColorSlot, Profile, Rgb};
use crate::terminal::cell::{
    Attrs, Color, EmuCell, Hyperlink, LinkCache, UnderlineStyle, CONTINUATION,
};
use crate::terminal::emu::{
    Clipboard, ClipboardType, ColorTable, CursorShape, KeyboardMode, TerminalMode,
};

fn to_ghostty_rgb(color: Rgb) -> RgbColor {
    RgbColor {
        r: color.r,
        g: color.g,
        b: color.b,
    }
}

fn from_ghostty_rgb(color: RgbColor) -> Rgb {
    Rgb::new(color.r, color.g, color.b)
}

fn cell_color(color: StyleColor) -> Option<Color> {
    match color {
        StyleColor::None => None,
        StyleColor::Palette(index) => Some(Color::from_index(index.0)),
        StyleColor::Rgb(color) => Some(Color::Rgb(color.r, color.g, color.b)),
    }
}

fn underline(style: Underline) -> UnderlineStyle {
    match style {
        Underline::None => UnderlineStyle::None,
        Underline::Single => UnderlineStyle::Single,
        Underline::Double => UnderlineStyle::Double,
        Underline::Curly => UnderlineStyle::Curly,
        Underline::Dotted => UnderlineStyle::Dotted,
        Underline::Dashed => UnderlineStyle::Dashed,
        _ => UnderlineStyle::Single,
    }
}

/// Read a cell's OSC 8 URI off a grid reference.
///
/// A link spanning N cells reuses one `Arc` through `links` rather than
/// building N copies of the same URI.
///
/// Ghostty's FFI exposes the URI and nothing else, so the `id=` parameter is
/// not recoverable here and every ghostty link reports `id: None`. That is
/// declared as a conformance divergence rather than worked around.
fn grid_hyperlink(grid: &GridRef<'_>, links: &mut LinkCache) -> Result<Option<Arc<Hyperlink>>> {
    let mut inline = [0u8; 128];
    let uri = match grid.hyperlink_uri(&mut inline) {
        Ok(0) => return Ok(None),
        Ok(len) => {
            CompactString::from_utf8(&inline[..len]).context("hyperlink URI is not UTF-8")?
        }
        Err(GhosttyError::OutOfSpace { required }) => {
            let mut buf = vec![0u8; required];
            let len = grid
                .hyperlink_uri(&mut buf)
                .context("reading hyperlink URI")?;
            CompactString::from_utf8(&buf[..len]).context("hyperlink URI is not UTF-8")?
        }
        Err(error) => return Err(error).context("reading hyperlink URI"),
    };
    Ok(Some(links.get(None, &uri)))
}

fn cell_from_ghostty(
    cell: GhosttyCell,
    style: Style,
    graphemes: &[char],
    hyperlink: Option<Arc<Hyperlink>>,
) -> Result<EmuCell> {
    let ch = match cell.wide().context("reading cell width")? {
        CellWide::SpacerTail => CompactString::const_new(CONTINUATION),
        CellWide::SpacerHead => CompactString::const_new(" "),
        CellWide::Narrow | CellWide::Wide if graphemes.is_empty() => CompactString::const_new(" "),
        CellWide::Narrow | CellWide::Wide => graphemes.iter().collect::<String>().into(),
    };

    let bg = match cell.content_tag().context("reading cell content")? {
        CellContentTag::BgColorPalette => Some(Color::from_index(
            cell.bg_color_palette()
                .context("reading cell palette background")?
                .0,
        )),
        CellContentTag::BgColorRgb => {
            let color = cell.bg_color_rgb().context("reading cell RGB background")?;
            Some(Color::Rgb(color.r, color.g, color.b))
        }
        CellContentTag::Codepoint | CellContentTag::CodepointGrapheme => cell_color(style.bg_color),
    };

    let mut attrs = Attrs::empty();
    for (enabled, attr) in [
        (style.bold, Attrs::BOLD),
        (style.faint, Attrs::DIM),
        (style.italic, Attrs::ITALIC),
        (style.inverse, Attrs::INVERSE),
        (style.invisible, Attrs::INVISIBLE),
        (style.strikethrough, Attrs::STRIKE),
        (style.blink, Attrs::BLINK),
    ] {
        attrs.set(attr, enabled);
    }

    Ok(EmuCell {
        ch,
        fg: cell_color(style.fg_color),
        bg,
        underline: underline(style.underline),
        underline_color: cell_color(style.underline_color),
        attrs,
        hyperlink,
    })
}

fn grid_graphemes(grid: &GridRef<'_>) -> Result<Vec<char>> {
    let mut inline = ['\0'; 8];
    match grid.graphemes(&mut inline) {
        Ok(len) => Ok(inline[..len].to_vec()),
        Err(GhosttyError::OutOfSpace { required }) => {
            let mut chars = vec!['\0'; required];
            let len = grid
                .graphemes(&mut chars)
                .context("reading cell graphemes")?;
            chars.truncate(len);
            Ok(chars)
        }
        Err(error) => Err(error).context("reading cell graphemes"),
    }
}

fn cell_from_grid(grid: &GridRef<'_>, links: &mut LinkCache) -> Result<EmuCell> {
    let cell = grid.cell().context("reading scrollback cell value")?;
    let graphemes = if matches!(
        cell.wide().context("reading scrollback cell width")?,
        CellWide::SpacerHead | CellWide::SpacerTail
    ) {
        Vec::new()
    } else {
        grid_graphemes(grid)?
    };
    let hyperlink = grid_hyperlink(grid, links)?;
    cell_from_ghostty(
        cell,
        grid.style().context("reading scrollback cell style")?,
        &graphemes,
        hyperlink,
    )
}

#[derive(Clone)]
pub(super) struct Frame {
    pub(super) rows: Vec<Vec<EmuCell>>,
    pub(super) cursor: (u16, u16),
    pub(super) cursor_shape: CursorShape,
}

pub(super) struct GhosttyCore {
    terminal: Terminal<'static, 'static>,
    render: RenderState<'static>,
    row_iter: RowIterator<'static>,
    cell_iter: CellIterator<'static>,
    pending: Rc<RefCell<Vec<u8>>>,
    clipboard: Clipboard,
    profile: Profile,
    frame: Option<Frame>,
}

impl GhosttyCore {
    pub(super) fn new(cols: u16, rows: u16, profile: Profile, bells: BellTracker) -> Result<Self> {
        let mut terminal = Terminal::new(cols.max(1), rows.max(1)).context("creating terminal")?;
        terminal
            .resize(cols.max(1), rows.max(1), 1, 1)
            .context("initializing terminal cell size")?;
        terminal
            .set_scrollback_max_lines(Some(profile.scrollback))
            .context("setting scrollback limit")?;

        terminal
