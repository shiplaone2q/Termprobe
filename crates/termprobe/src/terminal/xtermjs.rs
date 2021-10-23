//! [`Emulator`] backend built on `@xterm/headless` running in QuickJS.
//!
//! The bundle and its host shim are embedded in the binary and evaluated into
//! a fresh QuickJS context per session, so this backend adds no runtime
//! dependency on Node or on anything installed on the machine.
//!
//! # Why the grid crosses the boundary packed
//!
//! Reading a cell means a call into JS, and an 80x30 screen is 2,400 of them
//! with ten property reads each. Walking the grid that way costs milliseconds
//! per poll. Instead [`shim.js`](../../../assets/xtermjs/shim.js) flattens a row
//! span into one string and one integer array, so a whole screen crosses in
//! two values and this module's job is decoding rather than traversal.
//!
//! # Threading
//!
//! [`Emulator`] is `Send` and the daemon moves the emulator between its reader
//! and request threads. `rquickjs`'s `parallel` feature makes `Runtime` and
//! `Context` `Send + Sync`, which is what lets this type be `Send` without
//! confining the interpreter to a thread of its own. It is emphatically not
//! `Sync`-in-spirit: every entry point below takes `&mut self`, so the daemon's
//! existing mutex is still what serializes access.

use std::sync::{Arc, Mutex};

use compact_str::{CompactString, ToCompactString};
use rquickjs::{Context, Ctx, Function, Object, Runtime};

use crate::event::BellTracker;
use crate::profile::{ColorSlot, Profile, Rgb};
use crate::terminal::cell::{Attrs, Color, EmuCell, Hyperlink, UnderlineStyle, CONTINUATION};
use crate::terminal::emu::{ClipboardValidator, ColorTable, CursorShape, Emulator, TerminalMode};

const XTERM_BUNDLE: &str = include_str!("../../assets/xtermjs/xterm-headless.js");
const UNICODE11: &str = include_str!("../../assets/xtermjs/addon-unicode11.js");
const SHIM: &str = include_str!("../../assets/xtermjs/shim.js");

/// The unicode11 addon is UMD and publishes itself by *replacing*
/// `module.exports`, so it is lifted onto a global the shim can find. Reading
/// it back before `__boot` runs also leaves `exports.Terminal`, which the shim
/// set up, untouched.
const UNICODE11_CAPTURE: &str = "globalThis.__unicode11 = module.exports.Unicode11Addon;";

/// Ints per cell in the packed `meta` array, mirroring `pack()` in the shim.
const STRIDE: usize = 7;

/// Where the three dynamic colors sit in the flat table handed to the shim,
/// which continues past the 256 palette slots. The numbering is this
/// backend's own arrangement with its own shim and reaches no further.
const FOREGROUND: usize = 256;
const BACKGROUND: usize = 257;
const CURSOR: usize = 258;
const SLOTS: usize = 259;

/// Rows decoded per `pack` call. Bounds the size of the temporary JS array a
/// full-scrollback read builds; see [`XtermJsEmu::rows_in_range`].
const PACK_BATCH_ROWS: usize = 256;

/// Color-mode bits, packed alongside the SGR booleans in the `flags` int.
const FG_PALETTE: i32 = 256;
const FG_RGB: i32 = 512;
const BG_PALETTE: i32 = 1024;
const BG_RGB: i32 = 2048;
const UL_PALETTE: i32 = 4096;
const UL_RGB: i32 = 8192;

/// Decode one color slot. `mode` is the pair of bits that says how to read
/// `raw`; with neither set the cell uses the terminal default, which the cell
/// vocabulary spells as `None`.
fn color(raw: i32, flags: i32, palette_bit: i32, rgb_bit: i32) -> Option<Color> {
    if flags & palette_bit != 0 {
        Some(Color::from_index(raw as u8))
    } else if flags & rgb_bit != 0 {
        Some(Color::Rgb(
            ((raw >> 16) & 0xff) as u8,
            ((raw >> 8) & 0xff) as u8,
            (raw & 0xff) as u8,
        ))
    } else {
        None
    }
}

/// xterm.js's `UnderlineStyle`, which already folds "not underlined" into
/// `NONE` and a bare `SGR 4` into `SINGLE`, so no separate underline flag has
/// to be consulted here.
fn underline(raw: i32) -> UnderlineStyle {
    match raw {
        1 => UnderlineStyle::Single,
        2 => UnderlineStyle::Double,
        3 => UnderlineStyle::Curly,
        4 => UnderlineStyle::Dotted,
        5 => UnderlineStyle::Dashed,
        _ => UnderlineStyle::None,
    }
}

/// Pack a color into the `0xRRGGBB` int the shim speaks.
fn rgb_to_i32(c: Rgb) -> i32 {
    ((c.r as i32) << 16) | ((c.g as i32) << 8) | c.b as i32
}

fn attrs(flags: i32) -> Attrs {
    let mut a = Attrs::empty();
    for (bit, attr) in [
        (1, Attrs::BOLD),
        (2, Attrs::DIM),
        (4, Attrs::ITALIC),
        (8, Attrs::INVERSE),
        (16, Attrs::INVISIBLE),
        (32, Attrs::STRIKE),
        (64, Attrs::BLINK),
    ] {
        a.set(attr, flags & bit != 0);
    }
    a
}

pub struct XtermJsEmu {
    // Held to keep the interpreter alive for as long as the context that runs
    // in it; nothing calls through it directly.
    _runtime: Runtime,
    ctx: Context,
    /// The size xterm.js actually applied, which is not always the size that
    /// was asked for: it clamps to a 2x1 minimum. Caching the *requested* size
    /// sheared the grid, because `pack` emits `term.cols` cells per row while
    /// the decoder chunks by this value.
    cols: u16,
    rows: u16,
    /// The settings this session was opened with. A program can shadow a
    /// color at runtime but never reach this, so a reset always has a value
    /// to restore.
    profile: Profile,
    /// The first JS exception this emulator hit, if any.
    ///
    /// The first one wins: once the engine has thrown, what follows is a
    /// consequence rather than a new fact, and the earliest message names the
    /// cause. Behind a lock because the reads that can fault take `&self`.
    fault: Mutex<Option<String>>,
    bells: BellTracker,
    clipboard_validator: ClipboardValidator,
}

/// Render a failed JS call as a message worth reading.
///
/// rquickjs reports a thrown exception as a bare [`rquickjs::Error::Exception`]
/// and parks the value on the context, so the message and stack have to be
/// claimed with `catch` or the next call overwrites them.
fn describe(ctx: &Ctx<'_>, method: &str, error: rquickjs::Error) -> String {
    if !matches!(error, rquickjs::Error::Exception) {
        return format!("xterm.js: calling {method} failed: {error}");
    }
    let thrown = ctx.catch();
    let detail = match thrown.as_exception() {
        Some(exception) => match exception.stack() {
            Some(stack) if !stack.trim().is_empty() => {
                format!("{exception}\n{}", stack.trim_end())
            }
            _ => exception.to_string(),
        },
        None => format!("{thrown:?}"),
    };
    format!("xterm.js: {method} threw: {detail}")
}

impl XtermJsEmu {
    pub fn new(cols: u16, rows: u16, profile: &Profile) -> anyhow::Result<Self> {
        Self::with_bell_tracker(cols, rows, profile, BellTracker::default())
    }
