//! Terminal profiles: the settings a session runs with.
//!
//! A profile is chosen when a session opens and fixed for its lifetime. It is
//! read from a TOML file so a project can commit the terminal its tests expect,
//! rather than depending on whatever the machine happens to default to.
//!
//! # Colors are resolved here, not by the emulator
//!
//! A terminal grid stores color *indices*, not colors: a cell painted with
//! `SGR 31` records palette slot 1, and what that looks like is the viewer's
//! choice. Nothing in the emulator needs a palette — xterm.js's `theme` option
//! is inert in a headless terminal, and alacritty has no palette at all.
//!
//! termprobe has to make that choice twice: once to draw a screenshot, and once
//! to answer `expect --fg "#rrggbb"`. Those answers have to agree. They used to
//! come from two separate hardcoded tables that disagreed on all sixteen ANSI
//! slots, so `expect --fg "#800000"` passed on a cell the screenshot painted
//! `#e88388`. [`Colors`] is the single table both now read.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::terminal::cell::NamedColor;

/// Rows of scrollback a profile retains when it does not say otherwise.
///
/// The emulators do not agree on their own defaults (alacritty 10,000,
/// xterm.js 1,000), so this is always set explicitly rather than inherited.
pub const DEFAULT_SCROLLBACK: usize = 10_000;

/// The file a profile is read from, under the config directory.
pub const CONFIG_FILE: &str = "termprobe.toml";

/// The profile used when none is named.
pub const DEFAULT_PROFILE: &str = "default";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Parse `#rgb` or `#rrggbb`. The leading `#` is optional so a TOML value
    /// that lost it to a stray quote still reads sensibly.
    pub fn parse(s: &str) -> Result<Self, String> {
        let trimmed = s.trim();
        let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
        let digit = |byte: u8| -> Option<u8> {
            match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                b'A'..=b'F' => Some(byte - b'A' + 10),
                _ => None,
            }
        };
        let digits = hex
            .bytes()
            .map(digit)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| format!("invalid hex color {s:?}"))?;
        match digits.as_slice() {
            [r, g, b] => Ok(Rgb::new(r * 17, g * 17, b * 17)),
            [r1, r2, g1, g2, b1, b2] => Ok(Rgb::new(r1 * 16 + r2, g1 * 16 + g2, b1 * 16 + b2)),
            _ => Err(format!("color must be #rgb or #rrggbb (got {s:?})")),
        }
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Rgb::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// The colors a session paints with.
///
/// Only the sixteen ANSI slots are configurable. Indices 16-255 are the xterm
/// color cube and gray ramp, which are defined by the spec rather than by a
/// theme, so [`Colors::rgb`] computes them instead of storing them. A config
/// that could override them would let two sessions disagree about what
/// `--fg 196` means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Colors {
    /// The color text takes when a cell set none of its own.
    pub foreground: Rgb,
    /// The color an unpainted cell takes.
    pub background: Rgb,
    /// The color used to draw the cursor.
    pub cursor: Rgb,

    pub black: Rgb,
    pub red: Rgb,
    pub green: Rgb,
    pub yellow: Rgb,
    pub blue: Rgb,
    pub magenta: Rgb,
    pub cyan: Rgb,
    pub white: Rgb,
    pub bright_black: Rgb,
    pub bright_red: Rgb,
    pub bright_green: Rgb,
    pub bright_yellow: Rgb,
    pub bright_blue: Rgb,
    pub bright_magenta: Rgb,
    pub bright_cyan: Rgb,
    pub bright_white: Rgb,
}

impl Default for Colors {
    /// The classic VGA/xterm palette, which is what `TERM=xterm-256color`
    /// promises and what the assertion layer already compared against.
    fn default() -> Self {
        Colors {
            foreground: Rgb::new(192, 192, 192),
            background: Rgb::new(0, 0, 0),
            cursor: Rgb::new(192, 192, 192),

            black: Rgb::new(0, 0, 0),
            red: Rgb::new(128, 0, 0),
            green: Rgb::new(0, 128, 0),
            yellow: Rgb::new(128, 128, 0),
            blue: Rgb::new(0, 0, 128),
            magenta: Rgb::new(128, 0, 128),
            cyan: Rgb::new(0, 128, 128),
            white: Rgb::new(192, 192, 192),
            bright_black: Rgb::new(128, 128, 128),
            bright_red: Rgb::new(255, 0, 0),
            bright_green: Rgb::new(0, 255, 0),
            bright_yellow: Rgb::new(255, 255, 0),
            bright_blue: Rgb::new(0, 0, 255),
            bright_magenta: Rgb::new(255, 0, 255),
            bright_cyan: Rgb::new(0, 255, 255),
            bright_white: Rgb::new(255, 255, 255),
        }
    }
}

impl Colors {
    /// The sixteen ANSI slots, in palette order.
    pub fn ansi(&self) -> [Rgb; 16] {
        [
            self.black,
            self.red,
            self.green,
            self.yellow,
            self.blue,
            self.magenta,
            self.cyan,
            self.white,
            self.bright_black,
            self.bright_red,
            self.bright_green,
            self.bright_yellow,
            self.bright_blue,
            self.bright_magenta,
            self.bright_cyan,
            self.bright_white,
        ]
    }

    /// The name a slot goes by in the config file.
    pub fn slot_name(index: u8) -> Option<&'static str> {
        Some(match NamedColor::from_index(index)? {
            NamedColor::Black => "black",
            NamedColor::Red => "red",
            NamedColor::Green => "green",
            NamedColor::Yellow => "yellow",
            NamedColor::Blue => "blue",
            NamedColor::Magenta => "magenta",
            NamedColor::Cyan => "cyan",
            NamedColor::White => "white",
            NamedColor::BrightBlack => "bright_black",
            NamedColor::BrightRed => "bright_red",
            NamedColor::BrightGreen => "bright_green",
            NamedColor::BrightYellow => "bright_yellow",
            NamedColor::BrightBlue => "bright_blue",
            NamedColor::BrightMagenta => "bright_magenta",
            NamedColor::BrightCyan => "bright_cyan",
            NamedColor::BrightWhite => "bright_white",
        })
    }

    /// Set one color by the name used in config files and language bindings.
    pub fn set_named(&mut self, name: &str, value: Rgb) -> bool {
        let target = match name {
            "foreground" => &mut self.foreground,
            "background" => &mut self.background,
            "cursor" => &mut self.cursor,
            "black" => &mut self.black,
            "red" => &mut self.red,
            "green" => &mut self.green,
            "yellow" => &mut self.yellow,
            "blue" => &mut self.blue,
            "magenta" => &mut self.magenta,
            "cyan" => &mut self.cyan,
            "white" => &mut self.white,
            "bright_black" => &mut self.bright_black,
            "bright_red" => &mut self.bright_red,
            "bright_green" => &mut self.bright_green,
            "bright_yellow" => &mut self.bright_yellow,
            "bright_blue" => &mut self.bright_blue,
            "bright_magenta" => &mut self.bright_magenta,
            "bright_cyan" => &mut self.bright_cyan,
            "bright_white" => &mut self.bright_white,
            _ => return false,
        };
        *target = value;
        true
    }

    /// Resolve any 256-color index.
    ///
    /// Slots 0-15 come from the profile; everything above comes from the
    /// xterm table, which no profile can move.
    pub fn rgb(&self, index: u8) -> Rgb {
        match index {
            0..=15 => self.ansi()[index as usize],
            _ => xterm_color(index),
        }
    }
}

/// A color a program can address.
///
/// `OSC 4` names a palette entry and `OSC 10/11/12` name the three defaults.
/// Emulators number these however they like internally, so each backend
/// translates its own layout and that numbering never reaches here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSlot {
    Indexed(u8),
    Foreground,
    Background,
    Cursor,
}

/// The xterm 256-color table, which is the same in every terminal.
///
/// Slots 0-15 here are the classic VGA colors, and a profile overrides them.
/// The rest is the 6x6x6 color cube and the 24-step gray ramp, which the
/// specification fixes and no profile can move: `--fg 196` has to mean the
/// same thing in every session.
static XTERM_256: [Rgb; 256] = build_xterm_256();

const fn build_xterm_256() -> [Rgb; 256] {
    let mut table = [Rgb::new(0, 0, 0); 256];

    // 0-15: VGA.
    let vga = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    let mut i = 0;
    while i < 16 {
        table[i] = Rgb::new(vga[i].0, vga[i].1, vga[i].2);
        i += 1;
    }

    // 16-231: a 6x6x6 cube whose levels step 0, 95, 135, 175, 215, 255.
    let levels = [0u8, 95, 135, 175, 215, 255];
    while i < 232 {
        let n = i - 16;
        table[i] = Rgb::new(levels[(n / 36) % 6], levels[(n / 6) % 6], levels[n % 6]);
        i += 1;
    }

    // 232-255: a gray ramp from 8 to 238 in steps of 10.
    while i < 256 {
        let v = (i - 232) as u8 * 10 + 8;
        table[i] = Rgb::new(v, v, v);
        i += 1;
    }
    table
}

/// The color a slot has when nothing has overridden it.
pub fn xterm_color(index: u8) -> Rgb {
    XTERM_256[index as usize]
}

/// The settings a session runs with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    /// Rows retained beyond the visible screen.
    pub scrollback: usize,
    pub colors: Colors,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            scrollback: DEFAULT_SCROLLBACK,
            colors: Colors::default(),
        }
    }
}

/// Recording settings. Profiles inherit omitted keys from the file.
