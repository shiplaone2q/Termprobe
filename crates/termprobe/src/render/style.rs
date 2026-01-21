//! How a screenshot or recording is drawn.
//!
//! `[recording.style]` resolves through [`crate::profile::Settings`] into the
//! session. SVG and raster output share the same style.
//! Regenerate the default golden with `TERMPROBE__UPDATE_GOLDEN=1` after intentional changes.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::profile::Rgb;

/// The default CSS font stack for SVG and raster output.
pub const DEFAULT_FONT_FAMILY: &str =
    "'Cascadia Code','JetBrains Mono','Fira Code',Menlo,Consolas,'DejaVu Sans Mono',monospace";

const DEFAULT_FONT_SIZE: f32 = 17.0;
const MAX_FONT_SIZE: f32 = 1_000.0;
const MAX_LENGTH: f32 = 10_000.0;
const MAX_PADDING: u32 = 10_000;

const CELL_W_RATIO: f32 = 10.0 / DEFAULT_FONT_SIZE;
const CELL_H_RATIO: f32 = 21.0 / DEFAULT_FONT_SIZE;

/// Font families, with optional overrides for bold and italic text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FontFamilies {
    /// The family for unstyled text, and the fallback for the other three.
    pub family: String,
    pub bold: Option<String>,
    pub italic: Option<String>,
    pub bold_italic: Option<String>,
    /// Extra font files, relative to the config file.
    /// Unavailable fonts fall back to bundled and system fonts.
    /// Raster output loads each canonical path once, with a total 64 MiB limit.
    /// Font data above this limit causes an error.
    pub files: Vec<PathBuf>,
}

impl Default for FontFamilies {
    fn default() -> Self {
        Self {
            family: DEFAULT_FONT_FAMILY.to_string(),
            bold: None,
            italic: None,
            bold_italic: None,
            files: Vec::new(),
        }
    }
}

impl FontFamilies {
    /// Bold italic falls back through bold, italic, then the base family.
    pub fn resolve(&self, bold: bool, italic: bool) -> &str {
        let pick = match (bold, italic) {
            (true, true) => self
                .bold_italic
                .as_deref()
                .or(self.bold.as_deref())
                .or(self.italic.as_deref()),
            (true, false) => self.bold.as_deref(),
            (false, true) => self.italic.as_deref(),
            (false, false) => None,
        };
        pick.unwrap_or(&self.family)
    }

    pub fn named(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.family.as_str()).chain(
            [
                self.bold.as_deref(),
                self.italic.as_deref(),
                self.bold_italic.as_deref(),
            ]
            .into_iter()
            .flatten(),
        )
    }

    pub fn resolve_paths(&mut self, config_dir: &Path) {
        for file in &mut self.files {
            if file.is_relative() {
                *file = config_dir.join(&*file);
            }
        }
    }
}

/// The window chrome drawn around the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WindowStyle {
    /// Draw the title bar, divider, and controls above the grid.
    pub title_bar: bool,
    /// Draw the three controls when the title bar is visible.
    pub traffic_lights: bool,
    pub background: Rgb,
    pub foreground: Rgb,
    pub divider: Rgb,
}

impl WindowStyle {
    pub fn traffic_lights(&self) -> &[Rgb] {
        const LIGHTS: [Rgb; 3] = [
            Rgb::new(236, 106, 94),
            Rgb::new(244, 191, 79),
            Rgb::new(97, 197, 84),
        ];
        if self.title_bar && self.traffic_lights {
            &LIGHTS
        } else {
            &[]
        }
    }
}

impl Default for WindowStyle {
    fn default() -> Self {
        Self {
            title_bar: true,
            traffic_lights: true,
            background: Rgb::new(217, 217, 232),
            foreground: Rgb::new(65, 65, 69),
            divider: Rgb::new(0, 0, 0),
        }
    }
}

/// A panel border. A zero width disables it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BorderStyle {
    pub width: f32,
    pub color: Rgb,
    /// Corner radius of the panel, which the border follows.
    pub radius: f32,
}

impl Default for BorderStyle {
    fn default() -> Self {
        Self {
            width: 0.0,
            color: Rgb::new(0, 0, 0),
            radius: 8.0,
        }
    }
}

/// The drop shadow under the panel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShadowStyle {
    pub enabled: bool,
    pub color: Rgb,
    /// How far below the panel the shadow sits.
    pub offset: f32,
    /// How far beyond the panel's edges it reaches.
    pub spread: f32,
}

impl Default for ShadowStyle {
    fn default() -> Self {
        Self {
            enabled: true,
            color: Rgb::new(8, 8, 18),
            offset: 5.0,
            spread: 7.0,
        }
    }
}

impl ShadowStyle {
    /// `(spread, offset, alpha)` layers, largest first, shared by both renderers.
    fn layers(&self) -> [(f32, f32, u8); 4] {
        let spread_step = self.spread * 2.0 / 7.0;
        let offset_step = self.offset / 5.0;
        std::array::from_fn(|index| {
            let step = index as f32;
            (
                self.spread - step * spread_step,
                self.offset - step * offset_step,
                18 + index as u8 * 2,
            )
        })
    }
}

/// One number covers every side. A table sets individual sides.
/// Omitted sides inherit file settings, then the canvas or content defaults.
///
/// ```toml
/// canvas_padding = 24
///
/// # or
/// [recording.style.canvas_padding]
/// bottom = 48
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Padding {
    Uniform(u32),
    Sides(PaddingSides),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PaddingSides {
    pub top: Option<u32>,
    pub right: Option<u32>,
    pub bottom: Option<u32>,
    pub left: Option<u32>,
}

impl Padding {
    fn side(self, pick: fn(PaddingSides) -> Option<u32>, fallback: u32) -> u32 {
        match self {
            Self::Uniform(value) => value,
            Self::Sides(sides) => pick(sides).unwrap_or(fallback),
        }
    }

    fn each(self, fallback: Sides) -> [(&'static str, u32); 4] {
        [
            ("top", self.side(|s| s.top, fallback.top)),
            ("right", self.side(|s| s.right, fallback.right)),
            ("bottom", self.side(|s| s.bottom, fallback.bottom)),
            ("left", self.side(|s| s.left, fallback.left)),
        ]
    }
}
