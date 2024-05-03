use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use ttf_parser::{Face, OutlineBuilder};

use crate::terminal::cell::EmuCell;

// Nerd Fonts Symbols v3.4.0 is MIT-licensed; see the adjacent LICENSE.
pub(crate) const FONT_DATA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/nerd-fonts/SymbolsNerdFontMono-Regular.ttf"
));

struct Glyph {
    path: String,
    advance: u16,
    bounds: ttf_parser::Rect,
}

#[derive(Default)]
struct SvgPathBuilder {
    path: String,
}

impl OutlineBuilder for SvgPathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.path, "M{x:.1},{y:.1}");
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.path, "L{x:.1},{y:.1}");
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let _ = write!(self.path, "Q{x1:.1},{y1:.1},{x:.1},{y:.1}");
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let _ = write!(self.path, "C{x1:.1},{y1:.1},{x2:.1},{y2:.1},{x:.1},{y:.1}");
    }

    fn close(&mut self) {
        self.path.push('Z');
    }
}

struct GlyphTransform {
    x: f32,
    y: f32,
    scale_x: f32,
    scale_y: f32,
}

pub(super) struct NerdFont {
    glyphs: BTreeMap<char, Glyph>,
    scale: f32,
}

impl NerdFont {
    pub fn new(rows: &[Vec<EmuCell>], font_size: f32) -> Self {
        let chars: BTreeSet<char> = rows
            .iter()
            .flatten()
            .flat_map(|cell| cell.ch.chars())
            .filter(|c| is_private_use(*c))
            .collect();
        if chars.is_empty() {
            return Self {
                glyphs: BTreeMap::new(),
                scale: 1.0,
            };
        }

        let face = Face::parse(FONT_DATA, 0).expect("bundled Nerd Font must be valid");
        let mut glyphs = BTreeMap::new();
        for c in chars {
            let Some(glyph_id) = face.glyph_index(c) else {
                continue;
            };
            let mut builder = SvgPathBuilder::default();
            let Some(bounds) = face.outline_glyph(glyph_id, &mut builder) else {
                continue;
            };
            if builder.path.is_empty() {
                continue;
            }
            glyphs.insert(
                c,
                Glyph {
                    path: builder.path,
                    advance: face
                        .glyph_hor_advance(glyph_id)
                        .unwrap_or(face.units_per_em()),
                    bounds,
                },
            );
        }

        Self {
            glyphs,
            scale: font_size / face.units_per_em() as f32,
