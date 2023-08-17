use std::collections::BTreeSet;

use tiny_skia::Pixmap;

use crate::api::CaptureBackground;
use crate::profile::ColorSlot;
use crate::record::frames::Frame;
use crate::terminal::cell::{EmuCell, CONTINUATION};
use crate::terminal::emu::CursorShape;

use super::svg;
use super::svg::RenderColors;

mod draw;
mod font;

use draw::{
    draw_glyph, fill_antialiased_rect, fill_circle, fill_pixel_rect, fill_rounded_rect,
    fill_rounded_rect_alpha, fill_top_rounded_rect, format_glyph_sequence, is_default_ignorable,
    stroke_rounded_rect, unpremultiply, unsupported_grapheme,
};
use font::{FontSystem, GlyphKey};

use crate::render::style::Style;

/// Limit the RGBA buffer to 400 MB.
const MAX_PIXELS: u64 = 100_000_000;

#[derive(Debug)]
pub struct RgbaFrame {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RgbaFrame {
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn as_raw(&self) -> &[u8] {
        &self.pixels
    }

    pub fn into_raw(self) -> Vec<u8> {
        self.pixels
    }
}

pub trait FrameRenderer {
    fn render(&mut self, frame: &Frame) -> anyhow::Result<RgbaFrame>;
    fn pixel_size(&self) -> (u32, u32);
}

pub struct GridRenderer {
    max_cols: u16,
    max_rows: usize,
    exact_size: bool,
    scale: f32,
    width: u32,
    height: u32,
    pixmap: Pixmap,
    fonts: FontSystem,
    style: Style,
    /// Overrides the style's canvas background for one capture.
    background: Option<CaptureBackground>,
}

impl GridRenderer {
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn new(cols: u16, rows: usize) -> Self {
        Self::with_scale(cols, rows, 1)
    }

    pub fn with_scale(cols: u16, rows: usize, scale: u32) -> Self {
        Self::with_zoom(cols, rows, f64::from(scale), Style::default())
            .expect("recording raster scale must fit output dimensions")
    }

    pub fn with_zoom(cols: u16, rows: usize, zoom: f64, style: Style) -> anyhow::Result<Self> {
        Self::with_zoom_background_and_size(cols, rows, zoom, style, None, false)
    }

    pub fn with_zoom_and_background(
        cols: u16,
        rows: usize,
        zoom: f64,
        style: Style,
        background: Option<CaptureBackground>,
    ) -> anyhow::Result<Self> {
        Self::with_zoom_background_and_size(cols, rows, zoom, style, background, false)
    }

    pub(crate) fn for_screenshot(
        cols: u16,
        rows: usize,
        zoom: f64,
        style: Style,
        background: Option<CaptureBackground>,
    ) -> anyhow::Result<Self> {
        Self::with_zoom_background_and_size(cols, rows, zoom, style, background, true)
    }

    #[allow(clippy::too_many_arguments)]
    fn with_zoom_background_and_size(
        cols: u16,
        rows: usize,
        zoom: f64,
        style: Style,
        background: Option<CaptureBackground>,
        exact_size: bool,
    ) -> anyhow::Result<Self> {
        if !zoom.is_finite() || zoom <= 0.0 || zoom > f64::from(f32::MAX) {
            anyhow::bail!("recording zoom must be finite and greater than zero");
        }
        style.validate().map_err(|error| anyhow::anyhow!(error))?;
        let (base_width, base_height) = if exact_size {
            svg::exact_pixel_size(cols, rows, &style)
        } else {
            svg::pixel_size(cols, rows, &style)
        };
        let horizontal = style
            .canvas_horizontal()
            .ok_or_else(|| anyhow::anyhow!("recording canvas padding must fit in u32"))?;
        let vertical = style
            .canvas_vertical()
            .ok_or_else(|| anyhow::anyhow!("recording canvas padding must fit in u32"))?;
        let width = base_width
            .checked_add(horizontal)
            .ok_or_else(|| anyhow::anyhow!("recording width must fit in u32"))?;
        let height = base_height
            .checked_add(vertical)
            .ok_or_else(|| anyhow::anyhow!("recording height must fit in u32"))?;
        let width = scaled_dimension(width, zoom, "width")?;
        let height = scaled_dimension(height, zoom, "height")?;
        let pixels = u64::from(width) * u64::from(height);
        if pixels > MAX_PIXELS {
            anyhow::bail!(
                "recording is {width}x{height}, which is {pixels} pixels; the limit is {MAX_PIXELS}"
            );
        }
        Ok(Self {
            max_cols: cols,
            max_rows: rows,
            exact_size,
            scale: zoom as f32,
            width,
            height,
            pixmap: Pixmap::new(width, height).ok_or_else(|| {
                anyhow::anyhow!("terminal recording dimensions must fit a pixmap")
            })?,
            fonts: FontSystem::new(&style.font)?,
            style,
            background,
        })
    }
}

impl FrameRenderer for GridRenderer {
    fn render(&mut self, frame: &Frame) -> anyhow::Result<RgbaFrame> {
        let grid = &frame.grid;
        let (cols, rows) = frame.dimensions()?;
        if cols > self.max_cols || rows > self.max_rows {
            anyhow::bail!(
                "recording frame dimensions {cols}x{rows} exceed canvas dimensions {}x{}",
                self.max_cols,
                self.max_rows
            );
        }
        if grid.iter().any(|row| row.len() > usize::from(cols)) {
            anyhow::bail!("recording frame row exceeds its declared width");
        }

        let scale = self.scale;
        let colors = &frame.render_state;
        let style = &self.style;
        let (base_width, base_height) = if self.exact_size {
            svg::exact_pixel_size(cols, rows, style)
        } else {
            svg::pixel_size(cols, rows, style)
        };
        let panel_width = scaled_dimension(base_width, f64::from(self.scale), "frame width")?;
        let panel_height = scaled_dimension(base_height, f64::from(self.scale), "frame height")?;
        // Center smaller frames inside the configured padding after a resize.
        let pad_left = style.canvas_left() as f32 * scale;
        let pad_top = style.canvas_top() as f32 * scale;
        let pad_right = style.canvas_right() as f32 * scale;
        let pad_bottom = style.canvas_bottom() as f32 * scale;
