use std::collections::HashMap;

use tiny_skia::{Path, PathBuilder};
use ttf_parser::{Face, OutlineBuilder};

use super::super::{font as catalog, nerd_font};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct GlyphKey {
    pub character: char,
    pub bold: bool,
    pub italic: bool,
}

pub(super) struct GlyphOutline {
    pub path: Path,
    pub bounds: ttf_parser::Rect,
    pub advance: u16,
    pub units_per_em: u16,
    pub synthetic_bold: bool,
    pub synthetic_italic: bool,
    pub powerline: bool,
}

pub(super) struct FontSystem {
    catalog: std::sync::Arc<catalog::Catalog>,
    glyphs: HashMap<GlyphKey, Option<GlyphOutline>>,
}

impl FontSystem {
    /// Built from the style's fonts, so a recording draws with the family the
    /// config named rather than whatever the catalog ranked first.
    pub fn new(font: &crate::render::style::FontFamilies) -> anyhow::Result<Self> {
        Ok(Self {
            catalog: catalog::catalog_for(font)?,
            glyphs: HashMap::new(),
        })
    }

    pub fn resolve(&mut self, key: GlyphKey) -> Option<&GlyphOutline> {
        if !self.glyphs.contains_key(&key) {
            let glyph = self.load(key);
            self.glyphs.insert(key, glyph);
        }
        self.glyphs.get(&key).and_then(Option::as_ref)
    }

