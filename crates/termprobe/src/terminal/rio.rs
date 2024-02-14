//! [`Emulator`] backend built on `rio-vt`.

use std::sync::{Arc, Mutex, PoisonError};

use compact_str::{CompactString, ToCompactString};
use rio_vt::ansi::CursorShape as RioCursorShape;
use rio_vt::clipboard::ClipboardType as RioClipboardType;
use rio_vt::config::colors::term::COUNT as COLOR_COUNT;
use rio_vt::config::colors::{AnsiColor, ColorRgb, NamedColor as RioNamedColor};
use rio_vt::crosswords::grid::ExtrasTable;
use rio_vt::crosswords::pos::Line;
use rio_vt::crosswords::square::{ContentTag, Hyperlink as RioHyperlink, Square, Wide};
use rio_vt::crosswords::style::{Style, StyleFlags, StyleSet};
use rio_vt::crosswords::{Crosswords, CrosswordsSize, Mode};
use rio_vt::event::{EventListener, RioEvent, WindowId};
use rio_vt::performer::handler::Processor;

use crate::event::BellTracker;
use crate::profile::{xterm_color, ColorSlot, Profile, Rgb};
use crate::terminal::cell::{
    Attrs, Color, EmuCell, Hyperlink, LinkCache, UnderlineStyle, CONTINUATION,
};
use crate::terminal::emu::{
    Clipboard, ClipboardType, ClipboardValidator, CursorShape, Emulator, KeyboardMode, TerminalMode,
};

fn clipboard_type(clipboard: RioClipboardType) -> ClipboardType {
    match clipboard {
        RioClipboardType::Clipboard => ClipboardType::Clipboard,
        RioClipboardType::Selection => ClipboardType::Selection,
    }
}

fn color_from_rio(color: AnsiColor) -> Option<Color> {
    match color {
        AnsiColor::Named(RioNamedColor::Foreground | RioNamedColor::Background) => None,
        AnsiColor::Named(named) if (named as usize) < 16 => Some(Color::from_index(named as u8)),
        AnsiColor::Named(_) => None,
        AnsiColor::Indexed(index) => Some(Color::from_index(index)),
        AnsiColor::Spec(rgb) => Some(Color::Rgb(rgb.r, rgb.g, rgb.b)),
    }
}

fn underline_from_rio(flags: StyleFlags) -> UnderlineStyle {
    if flags.contains(StyleFlags::DOUBLE_UNDERLINE) {
        UnderlineStyle::Double
    } else if flags.contains(StyleFlags::UNDERCURL) {
        UnderlineStyle::Curly
    } else if flags.contains(StyleFlags::DOTTED_UNDERLINE) {
        UnderlineStyle::Dotted
    } else if flags.contains(StyleFlags::DASHED_UNDERLINE) {
        UnderlineStyle::Dashed
    } else if flags.contains(StyleFlags::UNDERLINE) {
        UnderlineStyle::Single
    } else {
        UnderlineStyle::None
    }
}

/// rio invents an id for a link that arrived without one, appending `_rio` to
/// a process-wide counter, exactly as alacritty does with `_alacritty`. It is
/// dropped for the same reason: it is not what the child sent, it varies with
/// parse order, and no other backend produces it.
fn hyperlink_from_rio(link: &RioHyperlink, links: &mut LinkCache) -> Arc<Hyperlink> {
    let id = link.id();
    links.get((!id.ends_with("_rio")).then_some(id), link.uri())
}

fn styled_cell(
    square: Square,
    style: Style,
    extras: &ExtrasTable,
    links: &mut LinkCache,
) -> EmuCell {
    let ch = match square.wide() {
        Wide::Spacer => CompactString::const_new(CONTINUATION),
        Wide::LeadingSpacer => CompactString::const_new(" "),
        Wide::Narrow | Wide::Wide => {
            let c = square.c();
            if matches!(c, '\0' | '\t' | ' ') {
                CompactString::const_new(" ")
            } else {
                let mut text = c.to_compact_string();
                if let Some(extra) = square.extras_id().and_then(|id| extras.get(id)) {
                    for c in &extra.zerowidth {
                        text.push(*c);
                    }
                }
                text
            }
        }
    };

    let mut attrs = Attrs::empty();
    for (flag, attr) in [
        (StyleFlags::BOLD, Attrs::BOLD),
        (StyleFlags::DIM, Attrs::DIM),
        (StyleFlags::ITALIC, Attrs::ITALIC),
        (StyleFlags::INVERSE, Attrs::INVERSE),
        (StyleFlags::HIDDEN, Attrs::INVISIBLE),
        (StyleFlags::STRIKEOUT, Attrs::STRIKE),
    ] {
        attrs.set(attr, style.flags.contains(flag));
    }

    EmuCell {
        ch,
        fg: color_from_rio(style.fg),
        bg: color_from_rio(style.bg),
        underline: underline_from_rio(style.flags),
        underline_color: style.underline_color.and_then(color_from_rio),
        attrs,
        hyperlink: square
            .extras_id()
            .and_then(|id| extras.get(id))
            .and_then(|extra| extra.hyperlink.as_ref())
            .map(|link| hyperlink_from_rio(link, links)),
    }
}

fn cell_from_rio(
    square: Square,
    styles: &StyleSet,
    extras: &ExtrasTable,
    links: &mut LinkCache,
) -> EmuCell {
    match square.content_tag() {
        ContentTag::Codepoint => styled_cell(square, styles.get(square.style_id()), extras, links),
        ContentTag::BgPalette => EmuCell {
            bg: Some(Color::from_index(square.bg_palette_index())),
            ..EmuCell::blank()
        },
        ContentTag::BgRgb => {
            let (r, g, b) = square.bg_rgb();
            EmuCell {
                bg: Some(Color::Rgb(r, g, b)),
                ..EmuCell::blank()
            }
        }
    }
}

#[derive(Clone)]
struct ColorState {
    profile: Profile,
    overrides: Arc<Mutex<[Option<Rgb>; COLOR_COUNT]>>,
}

impl ColorState {
    fn new(profile: &Profile) -> Self {
        Self {
            profile: *profile,
            overrides: Arc::new(Mutex::new([None; COLOR_COUNT])),
        }
    }

    fn slot_index(slot: ColorSlot) -> usize {
        match slot {
            ColorSlot::Indexed(index) => index as usize,
            ColorSlot::Foreground => RioNamedColor::Foreground as usize,
            ColorSlot::Background => RioNamedColor::Background as usize,
            ColorSlot::Cursor => RioNamedColor::Cursor as usize,
        }
    }

    fn configured(&self, index: usize) -> Rgb {
        match index {
            0..=15 => self.profile.colors.ansi()[index],
