//! Key-name and key-event to terminal input sequence mapping.

use crate::api::KeyAction;
use compact_str::{CompactString, ToCompactString};

use crate::terminal::emu::KeyboardMode;

/// The terminal state key encoding depends on.
///
/// Grouped rather than passed as separate arguments because encoding branches
/// on all of it at once and the set grows: DECCKM joined the Kitty flags here
/// once arrows had to be able to come out as `SS3 A`.
///
/// Public entry points take `impl Into<InputModes>`, so a caller that only has
/// Kitty flags can still pass them directly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputModes {
    pub keyboard: KeyboardMode,
    /// `DECCKM`: the child expects `SS3 A` from the up arrow, not `CSI A`.
    pub cursor_key_application: bool,
}

impl From<KeyboardMode> for InputModes {
    fn from(keyboard: KeyboardMode) -> Self {
        Self {
            keyboard,
            cursor_key_application: false,
        }
    }
}

const ESC: &str = "\u{1b}";
const CSI: &str = "\u{1b}[";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum KeyEventKind {
    #[default]
    Press,
    Repeat,
    Release,
}

impl KeyEventKind {
    fn code(self) -> Option<u8> {
        match self {
            KeyEventKind::Press => None,
            KeyEventKind::Repeat => Some(2),
            KeyEventKind::Release => Some(3),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
    pub hyper: bool,
    pub meta: bool,
}

impl Mods {
    fn any(self) -> bool {
        self.ctrl || self.alt || self.shift || self.super_key || self.hyper || self.meta
    }

    fn disambiguates_character(self) -> bool {
        self.ctrl || self.alt || self.super_key || self.hyper || self.meta
    }

    pub fn has_kitty_only_modifier(self) -> bool {
        self.super_key || self.hyper || self.meta
    }

    /// Kitty and xterm modifier parameters are one plus the modifier bitfield.
    fn param(self) -> u16 {
        1 + self.shift as u16
            + 2 * self.alt as u16
            + 4 * self.ctrl as u16
            + 8 * self.super_key as u16
            + 16 * self.hyper as u16
            + 32 * self.meta as u16
    }
}

struct ParsedToken<'a> {
    key: &'a str,
    mods: Mods,
}

fn parse_token(token: &str) -> anyhow::Result<ParsedToken<'_>> {
    if !token.contains('+') || token.len() == 1 {
        return Ok(ParsedToken {
            key: token,
            mods: Mods::default(),
        });
    }

    let parts: Vec<&str> = token.split('+').collect();
    let (key, prefixes) = parts
        .split_last()
        .ok_or_else(|| anyhow::anyhow!("invalid key: '{token}'"))?;
    if key.is_empty() {
        anyhow::bail!("invalid key: '{token}'");
    }

    let mut mods = Mods::default();
    for prefix in prefixes {
        match prefix.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods.ctrl = true,
            "alt" | "option" => mods.alt = true,
            "shift" => mods.shift = true,
            "super" | "command" | "cmd" | "win" | "windows" => mods.super_key = true,
            "hyper" => mods.hyper = true,
            "meta" => mods.meta = true,
            other => anyhow::bail!("unknown modifier: {other}"),
        }
    }

    Ok(ParsedToken { key, mods })
}

fn kitty_sequences(mode: InputModes) -> bool {
    mode.keyboard.intersects(
        KeyboardMode::DISAMBIGUATE_ESC_CODES
            | KeyboardMode::REPORT_EVENT_TYPES
            | KeyboardMode::REPORT_ALL_KEYS_AS_ESC,
    )
}

fn named(key: &str, mods: Mods, event: KeyEventKind, mode: InputModes) -> Option<String> {
    // `DECCKM` swaps the cursor keys' introducer from `CSI` to `SS3`, and it
    // reaches exactly the keys `CSI ... A-D`, `H`, and `F` spell: the arrows
    // plus Home and End. Only an unmodified press changes, which is already
    // what `ss3_unmodified` means for F1-F4, so it reuses that rather than
    // adding a second rule that could drift from it.
    let ss3 = mode.cursor_key_application;
    let sequence = match key.to_ascii_lowercase().as_str() {
        "home" => navigation(None, 'H', ss3, mods, event, mode),
        "end" => navigation(None, 'F', ss3, mods, event, mode),
        "up" => navigation(None, 'A', ss3, mods, event, mode),
        "down" => navigation(None, 'B', ss3, mods, event, mode),
        "right" => navigation(None, 'C', ss3, mods, event, mode),
        "left" => navigation(None, 'D', ss3, mods, event, mode),
        "pageup" => navigation(Some(5), '~', false, mods, event, mode),
        "pagedown" => navigation(Some(6), '~', false, mods, event, mode),
        "insert" => navigation(Some(2), '~', false, mods, event, mode),
        "delete" => navigation(Some(3), '~', false, mods, event, mode),
        "backspace" => control_key(ControlKey::Backspace, mods, event, mode),
        "tab" => control_key(ControlKey::Tab, mods, event, mode),
        "enter" | "return" => control_key(ControlKey::Enter, mods, event, mode),
        "space" => character(' ', mods, event, mode),
        "escape" | "esc" => control_key(ControlKey::Escape, mods, event, mode),
        "f1" => navigation(None, 'P', true, mods, event, mode),
        "f2" => navigation(None, 'Q', true, mods, event, mode),
        "f3" if kitty_sequences(mode) => navigation(Some(13), '~', false, mods, event, mode),
        "f3" => navigation(None, 'R', true, mods, event, mode),
        "f4" => navigation(None, 'S', true, mods, event, mode),
        "f5" => navigation(Some(15), '~', false, mods, event, mode),
        "f6" => navigation(Some(17), '~', false, mods, event, mode),
        "f7" => navigation(Some(18), '~', false, mods, event, mode),
        "f8" => navigation(Some(19), '~', false, mods, event, mode),
        "f9" => navigation(Some(20), '~', false, mods, event, mode),
        "f10" => navigation(Some(21), '~', false, mods, event, mode),
        "f11" => navigation(Some(23), '~', false, mods, event, mode),
        "f12" => navigation(Some(24), '~', false, mods, event, mode),
        _ => return None,
    };
    Some(sequence)
}

fn navigation(
    base: Option<u16>,
    terminator: char,
    ss3_unmodified: bool,
    mods: Mods,
    event: KeyEventKind,
    mode: InputModes,
) -> String {
    let report_events = mode.keyboard.contains(KeyboardMode::REPORT_EVENT_TYPES);
    if event == KeyEventKind::Release && !report_events {
        return String::new();
    }

    let event_code = report_events.then(|| event.code()).flatten();
    let details = mods.any() || event_code.is_some();
    if ss3_unmodified && !kitty_sequences(mode) && !details {
        return format!("{ESC}O{terminator}");
    }

    let mut sequence = String::from(CSI);
    match base {
        Some(base) => sequence.push_str(&base.to_string()),
        None if details => sequence.push('1'),
        None => {}
    }
    if details {
        sequence.push(';');
        sequence.push_str(&mods.param().to_string());
        if let Some(event_code) = event_code {
            sequence.push(':');
            sequence.push_str(&event_code.to_string());
        }
    }
    sequence.push(terminator);
    sequence
}

#[derive(Clone, Copy)]
enum ControlKey {
    Enter,
    Escape,
    Tab,
    Backspace,
}

impl ControlKey {
    fn codepoint(self) -> u32 {
        match self {
            ControlKey::Enter => 13,
            ControlKey::Escape => 27,
            ControlKey::Tab => 9,
            ControlKey::Backspace => 127,
        }
    }

    fn is_safety_key(self) -> bool {
        !matches!(self, ControlKey::Escape)
    }
}

fn control_key(key: ControlKey, mods: Mods, event: KeyEventKind, mode: InputModes) -> String {
    let report_events = mode.keyboard.contains(KeyboardMode::REPORT_EVENT_TYPES);
    let report_all = mode.keyboard.contains(KeyboardMode::REPORT_ALL_KEYS_AS_ESC);
    if event == KeyEventKind::Release && !report_events {
        return String::new();
    }

    let disambiguate = mode.keyboard.contains(KeyboardMode::DISAMBIGUATE_ESC_CODES);
    if matches!(key, ControlKey::Escape)
        && event == KeyEventKind::Press
        && !mods.any()
        && !disambiguate
        && !report_all
    {
        return ESC.to_string();
    }

    let legacy_mode = !kitty_sequences(mode);
    if legacy_mode && !mods.has_kitty_only_modifier() {
        return if event == KeyEventKind::Release {
            String::new()
        } else {
            legacy_control_key(key, mods)
        };
    }

    // Enter, Tab and Backspace report no release until report-all is on, so
    // that a program leaving event reporting set does not stop the user
    // typing `reset` at a shell prompt. Kitty gates this on the chord rather
    // than the key, so a modified one is still reported: `shift+tab` releases
    // as `CSI 9;2:3u` while `tab` releases as nothing. Ghostty exempts the
    // key whatever the modifiers, which the oracle records as a divergence.
    let safety_path = key.is_safety_key() && !report_all && !mods.any();
    if safety_path {
        return if event == KeyEventKind::Release {
            String::new()
        } else {
            legacy_control_key(key, mods)
        };
    }

    kitty_u(key.codepoint().to_string(), mods, event, mode, None)
}

fn legacy_control_key(key: ControlKey, mods: Mods) -> String {
    let mut sequence = String::new();
    match key {
        ControlKey::Enter => {
            if mods.alt {
                sequence.push_str(ESC);
            }
            sequence.push('\r');
        }
        ControlKey::Escape => {
            if mods.alt {
                sequence.push_str(ESC);
            }
            sequence.push_str(ESC);
        }
        ControlKey::Tab if mods.shift => {
            if mods.alt {
                sequence.push_str(ESC);
            }
            sequence.push_str("\u{1b}[Z");
        }
        ControlKey::Tab => {
