//! The terminal-emulator seam.
//!
//! termprobe drives a PTY and reads back a cell grid. [`Emulator`] is the
//! entire contract between the two, so an emulator backend can be swapped
//! without touching render, assert, monitor, or the daemon.
//!
//! Command/exit/cwd tracking is deliberately *not* part of this trait: it is
//! derived from the raw PTY byte stream by [`crate::terminal::integration`],
//! which is backend-independent. Keeping it out means every backend reports
//! identical shell-integration behavior by construction rather than by
//! reimplementation.

use alacritty_terminal::vte::{Params, Parser, Perform};

use crate::profile::{ColorSlot, Rgb};
use crate::terminal::cell::{Color, EmuCell};

/// The shape a terminal draws its cursor as, set with `DECSCUSR` (`CSI Ps SP q`).
///
/// The specification defines three, each in a blinking and a steady form. The
/// blink is not represented: a screenshot is a single moment, and a blinking
/// cursor is drawn in the half of that cycle where it is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    #[default]
    Block,
    Underline,
    Bar,
}

impl CursorShape {
    /// The name this shape goes by on the wire.
    pub const fn name(self) -> &'static str {
        match self {
            CursorShape::Block => "block",
            CursorShape::Underline => "underline",
            CursorShape::Bar => "bar",
        }
    }

    /// Parse a wire name, or `None` when it is not a shape.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "block" => Some(CursorShape::Block),
            "underline" => Some(CursorShape::Underline),
            "bar" => Some(CursorShape::Bar),
            _ => None,
        }
    }
}

bitflags::bitflags! {
    /// Kitty keyboard protocol flags currently requested by the child.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct KeyboardMode: u8 {
        const DISAMBIGUATE_ESC_CODES = 1;
        const REPORT_EVENT_TYPES = 1 << 1;
        const REPORT_ALTERNATE_KEYS = 1 << 2;
        const REPORT_ALL_KEYS_AS_ESC = 1 << 3;
        const REPORT_ASSOCIATED_TEXT = 1 << 4;
    }
}

/// A terminal mode a test can ask about.
///
/// Deliberately a closed set rather than a mode number, because the point is
/// that every backend answers the same question the same way. A variant earns
/// its place only when all four backends can report it: alacritty and rio from
/// their `Mode` bitflags, ghostty from `Terminal::mode`, and xterm.js from its
/// `modes` object, whose fixed set is the binding constraint. Modes that only
/// some backends track are left out rather than reported as `false`, which
/// would be a wrong answer dressed as a real one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalMode {
    /// `DECCKM` (`CSI ?1 h`): cursor keys send `SS3` instead of `CSI`.
    ApplicationCursorKeys,
    /// `DECKPAM` (`ESC =`): the keypad sends application sequences.
    ///
    /// Driven by `DECKPAM`/`DECKPNM` rather than `CSI ?66 h`, which alacritty
    /// and rio do not implement: the escape form is the one all four honor.
    ApplicationKeypad,
    /// `DECOM` (`CSI ?6 h`): the cursor is confined to the scroll region.
    Origin,
    /// `DECAWM` (`CSI ?7 h`): text wraps at the right margin.
    Wraparound,
    /// `IRM` (`CSI 4 h`): printed text shifts the rest of the line right.
    Insert,
    /// `CSI ?1004 h`: the child is told when the terminal gains or loses focus.
    FocusEvents,
    /// `CSI ?2004 h`: pasted text is wrapped in `ESC [200~` and `ESC [201~`.
    BracketedPaste,
    /// The alternate screen is showing.
    ///
    /// Reached by `CSI ?1049 h`, and also by the older `CSI ?47 h` and
    /// `CSI ?1047 h` on the backends that honor them. This reports the screen
    /// itself rather than any one of those, so it is true however a program
    /// got there.
    AlternateScreen,
    /// `DECTCEM` (`CSI ?25 h`): the cursor is drawn.
    ///
    /// Not in xterm.js's `modes`, but every backend already had to answer it
    /// for the renderer, so it is reported from the same place
    /// [`Emulator::cursor_visible`] always was.
    CursorVisible,
}

impl TerminalMode {
    /// Every mode, so a caller can report the whole set without listing it.
    pub const ALL: [TerminalMode; 9] = [
        TerminalMode::ApplicationCursorKeys,
        TerminalMode::ApplicationKeypad,
        TerminalMode::Origin,
        TerminalMode::Wraparound,
        TerminalMode::Insert,
        TerminalMode::FocusEvents,
        TerminalMode::BracketedPaste,
        TerminalMode::AlternateScreen,
        TerminalMode::CursorVisible,
    ];

    /// The name this mode goes by on the wire.
    pub const fn name(self) -> &'static str {
        match self {
            TerminalMode::ApplicationCursorKeys => "application_cursor_keys",
            TerminalMode::ApplicationKeypad => "application_keypad",
            TerminalMode::Origin => "origin",
            TerminalMode::Wraparound => "wraparound",
            TerminalMode::Insert => "insert",
            TerminalMode::FocusEvents => "focus_events",
            TerminalMode::BracketedPaste => "bracketed_paste",
            TerminalMode::AlternateScreen => "alternate_screen",
            TerminalMode::CursorVisible => "cursor_visible",
        }
    }

    /// The sequence that turns this mode on, for tests and documentation.
    ///
    /// One sequence per mode, not every sequence that reaches it: the
    /// alternate screen also answers to `CSI ?47 h` and `CSI ?1047 h`, and
    /// `?1049` is named here because it is what a full-screen program sends
    /// and the only one every backend honors.
    pub const fn set_sequence(self) -> &'static [u8] {
        match self {
            TerminalMode::ApplicationCursorKeys => b"\x1b[?1h",
            TerminalMode::ApplicationKeypad => b"\x1b=",
            TerminalMode::Origin => b"\x1b[?6h",
            TerminalMode::Wraparound => b"\x1b[?7h",
            TerminalMode::Insert => b"\x1b[4h",
            TerminalMode::FocusEvents => b"\x1b[?1004h",
            TerminalMode::BracketedPaste => b"\x1b[?2004h",
            TerminalMode::AlternateScreen => b"\x1b[?1049h",
            TerminalMode::CursorVisible => b"\x1b[?25h",
        }
    }

    /// The sequence that turns this mode off.
    ///
    /// The counterpart to [`Self::set_sequence`]. `CSI ?1049 l` leaves the
    /// alternate screen whichever sequence entered it, since the three share
    /// one screen.
    pub const fn reset_sequence(self) -> &'static [u8] {
        match self {
            TerminalMode::ApplicationCursorKeys => b"\x1b[?1l",
            TerminalMode::ApplicationKeypad => b"\x1b>",
            TerminalMode::Origin => b"\x1b[?6l",
            TerminalMode::Wraparound => b"\x1b[?7l",
            TerminalMode::Insert => b"\x1b[4l",
