//! Backend-neutral grid vocabulary.
//!
//! Every consumer of the terminal grid (render, assert, monitor, locator)
//! speaks these types and nothing else, so swapping the emulator backend
//! behind [`crate::terminal::emu::Emulator`] is invisible to them. Nothing in
//! this module may depend on a specific emulator crate.

use bitflags::bitflags;
use compact_str::CompactString;
use std::sync::Arc;
use unicode_width::UnicodeWidthStr;

/// The 16 themeable palette slots (ANSI 0-15).
///
/// Split out from [`Color::Idx`] by *numeric range*, not by how the escape
/// sequence spelled it, so `SGR 31` and `SGR 38;5;1` both land here.
///
/// The backends do not agree on whether that spelling survives parsing:
/// alacritty keeps it (`Named` vs `Indexed`) and so does xterm.js (`CM_P16`
/// vs `CM_P256`), but ghostty flattens both into one `.palette` value. A model
/// that preserved the distinction would therefore be unimplementable on
/// ghostty. The usual reason to want it, painting bold text bright, does not
/// need it either: ghostty keys that off the index (`bold && idx < 8 =>
/// palette[idx + 8]`), which applies to `38;5;1` just as much as to `31`.
///
/// What every supported backend agrees on is that 0-15 are the slots a theme
/// may override, which is the only distinction any consumer here acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NamedColor {
    Black = 0,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}

impl NamedColor {
    pub const ALL: [NamedColor; 16] = [
        NamedColor::Black,
        NamedColor::Red,
        NamedColor::Green,
        NamedColor::Yellow,
        NamedColor::Blue,
        NamedColor::Magenta,
        NamedColor::Cyan,
        NamedColor::White,
        NamedColor::BrightBlack,
        NamedColor::BrightRed,
        NamedColor::BrightGreen,
        NamedColor::BrightYellow,
        NamedColor::BrightBlue,
        NamedColor::BrightMagenta,
        NamedColor::BrightCyan,
        NamedColor::BrightWhite,
    ];

    /// The palette slot this name occupies (0-15).
    pub fn index(self) -> u8 {
        self as u8
    }

    /// The name for a palette slot, or `None` outside 0-15.
    pub fn from_index(i: u8) -> Option<Self> {
        Self::ALL.get(i as usize).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// A themeable palette slot, ANSI 0-15.
    Named(NamedColor),
    /// A fixed 256-color palette index, 16-255.
    Idx(u8),
    Rgb(u8, u8, u8),
}

impl Color {
    /// Build from a 256-color index, routing 0-15 to [`Color::Named`] so the
    /// same index yields the same value no matter which backend produced it.
    pub fn from_index(i: u8) -> Self {
        match NamedColor::from_index(i) {
            Some(named) => Color::Named(named),
            None => Color::Idx(i),
        }
    }

    /// The 256-color index for this color, approximating RGB.
    pub fn to_index(self) -> u8 {
        match self {
            Color::Named(n) => n.index(),
            Color::Idx(i) => i,
            Color::Rgb(r, g, b) => crate::assert::color::rgb_to_ansi256(r, g, b),
        }
    }
}

/// The shape of a cell's underline.
///
/// [`UnderlineStyle::None`] is a value, not an absence: it is the shape an
/// un-underlined cell has. Wrapping this in an `Option` would give two ways to
/// spell "not underlined" and force every reader through a `map` to reach the
/// shape, which is why the style and its color sit flat on the cell rather
