//! Live session monitor: a human watches what an agent is driving.
//!
//! The daemon renders the live emulator grid into a framed, full-color ANSI
//! frame (see [`render_frame`]) and streams one every ~20fps over the session
//! socket. The client ([`run_client`]) takes over an alternate screen in raw
//! mode and blits those frames, so the viewer sees the session in real time
//! while the agent keeps driving it through the same daemon.

use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use termprobe::engine::LiveFrame as Frame;
use termprobe::terminal::cell::{Attrs, Color, EmuCell, UnderlineStyle};
use termprobe::terminal::emu::{KeyboardMode, MouseMode};

use crate::ansi;
#[cfg(windows)]
use crate::console_input::ConsoleInput;
use crate::monitor_input::{InputAction, InputEvent, InputParser};
use crate::protocol::MonitorInput;

/// The target's input modes as last announced to the viewer's terminal, so a
/// mode is only re-applied when the target changes it.
#[derive(Default)]
pub(crate) struct ModeMirror {
    keyboard: Option<KeyboardMode>,
    cursor_key_application: Option<bool>,
    bracketed_paste: Option<bool>,
    mouse: Option<MouseMode>,
}

/// Render a framed, full-color view of `frame` clipped to the `viewer` size.
///
/// `None` renders a "no active session" placeholder. The output positions
/// itself from the home cell and clears trailing cells/rows, so successive
/// frames repaint in place without flicker (no full screen clear).
pub fn render_frame(
    frame: Option<&Frame>,
    viewer: (u16, u16),
    session: &str,
    interactive: bool,
    modes: &mut ModeMirror,
) -> Vec<u8> {
    let vcols = viewer.0.max(8);
    let vrows = viewer.1.max(4);
    let inner_w = match frame {
        Some(f) => f.size.0.min(vcols - 2),
        None => vcols - 2,
    } as usize;
    let inner_h = match frame {
        Some(f) => f.size.1.min(vrows - 2),
        None => vrows - 2,
    } as usize;

    let mut out = String::with_capacity(inner_w * inner_h * 4);
    if interactive {
        let keyboard = frame.map_or_else(KeyboardMode::empty, |f| f.keyboard_mode);
        let cursor_key_application = frame.is_some_and(|f| f.cursor_key_application);
        let paste = frame.is_some_and(|f| f.bracketed_paste);
        let mouse = frame.map_or(MouseMode::None, |f| f.mouse_mode);
        if modes.keyboard != Some(keyboard) {
            out.push_str(&ansi::kitty_keyboard_mode(keyboard.bits()));
        }
        if modes.cursor_key_application != Some(cursor_key_application) {
            out.push_str(if cursor_key_application {
                ansi::APPLICATION_CURSOR_ENABLE
            } else {
                ansi::APPLICATION_CURSOR_DISABLE
            });
        }
        if modes.bracketed_paste != Some(paste) {
            out.push_str(if paste {
                ansi::BRACKETED_PASTE_ENABLE
            } else {
                ansi::BRACKETED_PASTE_DISABLE
            });
        }
        if modes.mouse != Some(mouse) {
            out.push_str(ansi::MOUSE_DISABLE);
            out.push_str(match mouse {
                MouseMode::None => "",
                MouseMode::Click => ansi::MOUSE_CLICK_ENABLE,
                MouseMode::Drag => ansi::MOUSE_DRAG_ENABLE,
                MouseMode::Motion => ansi::MOUSE_MOTION_ENABLE,
            });
        }
        modes.keyboard = Some(keyboard);
        modes.cursor_key_application = Some(cursor_key_application);
        modes.bracketed_paste = Some(paste);
        modes.mouse = Some(mouse);
    }
    out.push_str(ansi::HOME);
    header(&mut out, frame, session, inner_w);
    if let Some(f) = frame {
        content(&mut out, f, inner_w, inner_h);
    } else {
        placeholder(&mut out, inner_w, inner_h);
    }
    let detach_hint = if interactive {
        "┤ Ctrl+] detach ├"
    } else {
        "┤ q quit ├"
    };
    border_line(&mut out, '└', '┘', detach_hint, inner_w, false);
    out.push_str(ansi::ERASE_DISPLAY);
    out.into_bytes()
}

fn header(out: &mut String, frame: Option<&Frame>, session: &str, inner_w: usize) {
    let title = match frame {
        Some(f) => {
            let shell = f.shell.map(|s| format!("{s} · ")).unwrap_or_default();
            let status = match f.exited {
                Some(code) => format!("exited {code}"),
                None => "live".to_string(),
            };
            format!("┤ {shell}{}×{} · {status} ├", f.size.0, f.size.1)
        }
        None => format!("┤ {session} · no session ├"),
    };
    border_line(out, '┌', '┐', &title, inner_w, true);
}

fn content(out: &mut String, f: &Frame, inner_w: usize, inner_h: usize) {
    let (cx, cy) = f.cursor;
    let show_cursor = f.exited.is_none() && f.cursor_visible;
    for y in 0..inner_h {
        out.push_str(ansi::BORDER);
        out.push('│');
        out.push_str(ansi::RESET);
        let row = f.grid.get(y);
        let mut last: Option<Style> = None;
        for x in 0..inner_w {
            let mut cell = row.and_then(|r| r.get(x)).cloned().unwrap_or_default();
            if show_cursor && x as u16 == cx && y as u16 == cy {
                cell.attrs.toggle(Attrs::INVERSE);
            }
            let style = Style::from(&cell);
            if last.as_ref() != Some(&style) {
                out.push_str(&style.sgr());
                last = Some(style);
            }
            out.push_str(&cell.ch);
        }
        out.push_str(ansi::RESET);
        out.push_str(ansi::BORDER);
        out.push('│');
        out.push_str(ansi::RESET);
        out.push_str(ansi::ERASE_LINE);
        out.push_str("\r\n");
    }
}

fn placeholder(out: &mut String, inner_w: usize, inner_h: usize) {
    let msg = "no active session, run `termprobe open`";
    for y in 0..inner_h {
        out.push_str(ansi::BORDER);
        out.push('│');
        out.push_str(ansi::RESET);
        if y == inner_h / 2 {
            let shown: String = msg.chars().take(inner_w).collect();
            let count = shown.chars().count();
            let pad = inner_w.saturating_sub(count) / 2;
            out.push_str(&" ".repeat(pad));
            out.push_str(&shown);
            out.push_str(&" ".repeat(inner_w.saturating_sub(pad + count)));
        } else {
            out.push_str(&" ".repeat(inner_w));
        }
        out.push_str(ansi::BORDER);
        out.push('│');
        out.push_str(ansi::RESET);
        out.push_str(ansi::ERASE_LINE);
        out.push_str("\r\n");
    }
}

fn border_line(out: &mut String, left: char, right: char, title: &str, inner_w: usize, nl: bool) {
    out.push_str(ansi::BORDER);
    out.push(left);
    let tlen = title.chars().count();
    if tlen + 1 >= inner_w {
        out.extend(title.chars().take(inner_w));
    } else {
        out.push('─');
        out.push_str(title);
        for _ in 0..(inner_w - 1 - tlen) {
            out.push('─');
        }
    }
    out.push(right);
    out.push_str(ansi::RESET);
    out.push_str(ansi::ERASE_LINE);
    if nl {
        out.push_str("\r\n");
    }
}

#[derive(PartialEq, Clone)]
struct Style {
    fg: Option<Color>,
    bg: Option<Color>,
    underline: UnderlineStyle,
    underline_color: Option<Color>,
    attrs: Attrs,
}

impl Style {
    fn from(c: &EmuCell) -> Self {
        Style {
            fg: c.fg,
            bg: c.bg,
            underline: c.underline,
            underline_color: c.underline_color,
            attrs: c.attrs,
        }
    }

    fn sgr(&self) -> String {
        let mut s = String::from(ansi::SGR_START);
        for (attr, code) in [
            (Attrs::BOLD, "1"),
            (Attrs::DIM, "2"),
            (Attrs::ITALIC, "3"),
            (Attrs::BLINK, "5"),
            (Attrs::INVERSE, "7"),
            (Attrs::INVISIBLE, "8"),
            (Attrs::STRIKE, "9"),
        ] {
            if self.attrs.contains(attr) {
                s.push(';');
                s.push_str(code);
