//! [`Emulator`] backend built on `alacritty_terminal`. Translates the
//! alacritty grid into termprobe's neutral cell vocabulary, plus a capture
//! proxy that queues the terminal's replies so the reader can forward them
//! back to the PTY.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags as AlacFlags;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{
    ClipboardType as AlacClipboardType, Config as AlacConfig, Osc52, Term, TermMode,
};
use alacritty_terminal::vte::ansi;
use alacritty_terminal::vte::ansi::CursorShape as AlacCursorShape;
use alacritty_terminal::vte::ansi::NamedColor;
use alacritty_terminal::vte::ansi::Rgb as AlacRgb;

use compact_str::{CompactString, ToCompactString};

use crate::event::BellTracker;
use crate::profile::{xterm_color, ColorSlot, Profile, Rgb};
use crate::terminal::cell::{
    Attrs, Color, EmuCell, Hyperlink, LinkCache, UnderlineStyle, CONTINUATION,
};
use crate::terminal::emu::{
    Clipboard, ClipboardType, ClipboardValidator, CursorShape, Emulator, KeyboardMode, TerminalMode,
};

fn clipboard_type(clipboard: AlacClipboardType) -> ClipboardType {
    match clipboard {
        AlacClipboardType::Clipboard => ClipboardType::Clipboard,
        AlacClipboardType::Selection => ClipboardType::Selection,
    }
}

/// Alacritty's palette colors arrive either as a `Named` variant or an index;
/// both funnel through [`Color::from_index`] so a given slot always yields the
/// same value. `Foreground`/`Background` are the terminal defaults, and every
/// other `NamedColor` (`Cursor`, the `Dim*` and `Bright*` aliases) is a UI role
/// with no cell representation, so it falls back to the default too.
fn color_from_alac(c: ansi::Color) -> Option<Color> {
    match c {
        ansi::Color::Named(named) => match named {
            ansi::NamedColor::Black
            | ansi::NamedColor::Red
            | ansi::NamedColor::Green
            | ansi::NamedColor::Yellow
            | ansi::NamedColor::Blue
            | ansi::NamedColor::Magenta
            | ansi::NamedColor::Cyan
            | ansi::NamedColor::White
            | ansi::NamedColor::BrightBlack
            | ansi::NamedColor::BrightRed
            | ansi::NamedColor::BrightGreen
            | ansi::NamedColor::BrightYellow
            | ansi::NamedColor::BrightBlue
            | ansi::NamedColor::BrightMagenta
            | ansi::NamedColor::BrightCyan
            | ansi::NamedColor::BrightWhite => Some(Color::from_index(named as u8)),
            _ => None,
        },
        ansi::Color::Spec(rgb) => Some(Color::Rgb(rgb.r, rgb.g, rgb.b)),
        ansi::Color::Indexed(i) => Some(Color::from_index(i)),
    }
}

fn underline_from_alac(c: &alacritty_terminal::term::cell::Cell) -> UnderlineStyle {
    let flags = c.flags;
    // Ordered widest-to-narrowest: alacritty clears the other underline bits on
    // each SGR, so at most one is ever set.
    if flags.contains(AlacFlags::DOUBLE_UNDERLINE) {
        UnderlineStyle::Double
    } else if flags.contains(AlacFlags::UNDERCURL) {
        UnderlineStyle::Curly
    } else if flags.contains(AlacFlags::DOTTED_UNDERLINE) {
        UnderlineStyle::Dotted
    } else if flags.contains(AlacFlags::DASHED_UNDERLINE) {
        UnderlineStyle::Dashed
    } else if flags.contains(AlacFlags::UNDERLINE) {
        UnderlineStyle::Single
    } else {
        UnderlineStyle::None
    }
}

/// alacritty invents an id for a link that arrived without one, appending
/// `_alacritty` to a process-wide counter. That is a rendering aid, not
/// something the child sent, and letting it through would put a value in
/// snapshots that changes with the order links were parsed in and that no
/// other backend produces. A link with no `id=` reports `None`.
///
/// A child that really sent `id=7_alacritty` is indistinguishable from a
/// synthesized one and reports `None` too. Telling them apart would mean
/// parsing OSC 8 ourselves, which is a lot of machinery for a collision
/// nobody will hit.
fn hyperlink_from_alac(
    link: &alacritty_terminal::term::cell::Hyperlink,
    links: &mut LinkCache,
) -> Arc<Hyperlink> {
    let id = link.id();
    links.get((!id.ends_with("_alacritty")).then_some(id), link.uri())
}

fn cell_from_alac(c: &alacritty_terminal::term::cell::Cell, links: &mut LinkCache) -> EmuCell {
    let flags = c.flags;
    // Only WIDE_CHAR_SPACER is a continuation: it is the second column of a
    // wide char on this row. LEADING_WIDE_CHAR_SPACER is the opposite, a filler
    // in the last column when a wide char did not fit and wrapped to the next
    // row, so it owns its column and has to render as a blank.
    let ch = if flags.contains(AlacFlags::WIDE_CHAR_SPACER) {
        CompactString::const_new(CONTINUATION)
    } else if flags.contains(AlacFlags::LEADING_WIDE_CHAR_SPACER) {
        CompactString::const_new(" ")
    } else {
        let mut s = c.c.to_compact_string();
        for zw in c.zerowidth().unwrap_or(&[]) {
            s.push(*zw);
        }
        s
    };

    let mut attrs = Attrs::empty();
    for (flag, attr) in [
        (AlacFlags::BOLD, Attrs::BOLD),
        (AlacFlags::DIM, Attrs::DIM),
        (AlacFlags::ITALIC, Attrs::ITALIC),
        (AlacFlags::INVERSE, Attrs::INVERSE),
        (AlacFlags::HIDDEN, Attrs::INVISIBLE),
        (AlacFlags::STRIKEOUT, Attrs::STRIKE),
    ] {
        attrs.set(attr, flags.contains(flag));
    }
    // `Attrs::BLINK` stays clear: alacritty_terminal parses SGR 5/6/25 and then
    // discards them, its `Flags` has no blink bit, so this backend has nothing
    // to read. The attribute is still part of the vocabulary because ghostty
    // (`Style.Flags.blink`) and xterm.js (`FgFlags.BLINK`) both track it.

    EmuCell {
        ch,
        fg: color_from_alac(c.fg),
        bg: color_from_alac(c.bg),
        underline: underline_from_alac(c),
        underline_color: c.underline_color().and_then(color_from_alac),
        attrs,
        hyperlink: c
            .hyperlink()
            .as_ref()
            .map(|link| hyperlink_from_alac(link, links)),
    }
}

/// Formats a color query's reply once the color is known. alacritty builds
/// this closure with the query's own prefix and terminator already captured,
/// so the answer echoes the form the program asked in.
type ReplyFormat = Arc<dyn Fn(AlacRgb) -> String + Send + Sync>;

/// One thing the terminal wants to say back to the PTY.
///
/// A color query cannot be answered inside the listener: the color lives in
/// the terminal that owns this listener. It is parked as a [`Reply::Color`]
/// until `process` regains access to the terminal at the query boundary.
enum Reply {
    Bytes(Vec<u8>),
    Color(usize, ReplyFormat),
}

/// Queues what the terminal wants to say back to the PTY, in the order it
/// decided to say it.
///
/// Answers and other replies share one queue because a program may pipeline
/// several requests in a single write and match the answers up by position.
/// The common idiom ends a batch of queries with a device attributes request,
/// whose reply every terminal sends, and treats that reply as the end of the
/// batch: an answer that arrived after it would look like the query went
/// unanswered, and would then be read as though the user had typed it.
#[derive(Default, Clone)]
struct CaptureProxy {
    pending: Arc<Mutex<Vec<Reply>>>,
    title: Arc<Mutex<Option<String>>>,
    clipboard: Arc<Mutex<Clipboard>>,
    bells: BellTracker,
    color_query_pending: Arc<AtomicBool>,
}

impl EventListener for CaptureProxy {
    fn send_event(&self, ev: Event) {
        match ev {
            Event::PtyWrite(bytes) => {
                self.push_reply(Reply::Bytes(bytes.as_bytes().to_vec()), false)
            }
            Event::ColorRequest(slot, format) => self.push_reply(Reply::Color(slot, format), true),
            Event::ClipboardStore(clipboard, text) => {
                self.clipboard
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .set(clipboard_type(clipboard), text);
            }
            Event::ClipboardLoad(clipboard, format) => {
                let text = self
                    .clipboard
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get(clipboard_type(clipboard))
                    .to_string();
                self.push_reply(Reply::Bytes(format(&text).into_bytes()), false);
            }
            // `OSC 0` and `OSC 2` both arrive here, and so does a pop of the
            // title stack (`CSI 23 t`), which alacritty implements by setting
            // the title it popped. An empty payload means the same as a reset:
            // the program is asking for no title rather than for a blank one.
            Event::Title(title) => self.set_title((!title.is_empty()).then_some(title)),
            Event::ResetTitle => self.set_title(None),
            Event::Bell => self.bells.ring(),
            _ => {}
        }
    }
}

