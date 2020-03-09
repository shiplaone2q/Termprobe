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

