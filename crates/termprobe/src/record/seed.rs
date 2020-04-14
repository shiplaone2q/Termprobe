//! State needed after painting a recording's initial viewport. A picture alone
//! cannot reproduce the rendition, pending wrap or scrolling behavior of the
//! next output. Keep a bounded shadow terminal rather than retaining PTY history.

use std::fmt::Write;

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::{Cursor, Dimensions, Grid};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{self, CharsetIndex, Handler, StandardCharset};
use alacritty_terminal::vte::{Params, Parser, Perform};

pub(super) struct State {
    term: Term<VoidListener>,
    parser: ansi::Processor,
    controls: Controls,
    control_parser: ansi::Processor,
    boundary: SequenceBoundary,
}

impl State {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            term: Term::new(
                Config {
                    scrolling_history: 0,
                    ..Config::default()
                },
                &TermSize::new(usize::from(cols), usize::from(rows)),
                VoidListener,
            ),
            parser: ansi::Processor::new(),
            controls: Controls::new(usize::from(rows)),
            control_parser: ansi::Processor::new(),
            boundary: SequenceBoundary::default(),
        }
    }

    pub fn process(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
        self.control_parser.advance(&mut self.controls, bytes);
        self.boundary.process(bytes);
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let size = TermSize::new(usize::from(cols), usize::from(rows));
        if self.term.columns() != size.columns || self.term.screen_lines() != size.screen_lines {
            self.term.resize(size);
            self.controls.rows = usize::from(rows);
            self.controls.margins = (1, usize::from(rows));
        }
    }

    pub fn snapshot(&mut self, viewport: &str) -> String {
        let mut output = String::new();
        let modes = *self.term.mode();
        if modes.contains(TermMode::ALT_SCREEN) {
            // Preserve the hidden primary screen as well: leaving the alternate
            // screen during playback must not reveal an empty terminal.
            let active = self.term.grid().clone();
            self.term.swap_alt();
            paint_grid(&mut output, self.term.grid());
            restore_cursor(&mut output, self.term.grid(), &self.term.grid().cursor, 0);
            output.push_str("\x1b[?1049h");
            self.term.swap_alt();
            *self.term.grid_mut() = active;
        }
        output.push_str(viewport);
        restore_wraps(&mut output, self.term.grid());
        let (top, bottom) = self.controls.margins;
        let _ = write!(output, "\x1b[{top};{bottom}r");
        let grid = self.term.grid();
        restore_cursor(&mut output, grid, &grid.saved_cursor, 0);
        if self.controls.saved_blink {
            output.push_str("\x1b[5m");
        }
        output.push_str("\x1b7");

        let origin = if modes.contains(TermMode::ORIGIN) {
            output.push_str("\x1b[?6h");
            top - 1
        } else {
            0
