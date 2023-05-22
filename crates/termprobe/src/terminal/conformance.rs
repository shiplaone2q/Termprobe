//! Backend-agnostic conformance suite for [`crate::terminal::emu::Emulator`].
//!
//! Every backend must produce the same grid for the same bytes, otherwise
//! swapping emulators silently changes what `expect`, `snapshot`, and the SVG
//! renderer see. Backends opt in with one line:
//!
//! ```ignore
//! crate::emulator_conformance_tests!(|c, r, s| Box::new(AlacrittyEmu::new(c, r, s)));
//! ```
//!
//! Each case becomes a separate `#[test]` in the calling module, so a failure
//! names the exact part of the contract that broke.
//!
//! Assertions here encode the *contract*, not one implementation. Where real
//! emulators legitimately diverge (reflow when shrinking below content width)
//! the test pins only the part that is universal and says why it stops short.

/// One part of the contract a backend cannot express, declared where it opts in.
///
/// This is deliberately not a general escape hatch. A variant earns its place
/// only when the limitation is in the emulator itself rather than in the
/// mapping onto it, so that adding one is a visible claim a reviewer can
/// check rather than a way to quiet a failing case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divergence {
    /// The backend records an underline color only on a cell that also has an
    /// underline style, so `SGR 58` on its own leaves nothing to read back.
    ///
    /// Nothing renders differently: a cell with no underline draws no
    /// underline color either way. What is lost is the color surviving in the
    /// cell vocabulary across an `SGR 24` that turns the underline off.
    UnderlineColorNeedsAStyle,

    /// Clipboard access is unavailable.
    ClipboardUnsupported,

    /// The backend reports a hyperlink's URI but not its `id=` parameter.
    ///
    /// Nothing about where the link points is lost. What is lost is being
    /// able to tell that two runs of cells belong to the same link when they
    /// are not adjacent.
    HyperlinkHasNoId,
    /// The backend does not implement the Kitty keyboard protocol at all, so
    /// it can never report a mode the child pushed.
    ///
    /// Unlike the other variants this one is visible to a user: key input
    /// falls back to legacy encodings even for a child that asked for `CSI u`.
    /// It belongs here rather than in a profile default because it is the
    /// emulator's own limit, not a setting.
    NoKittyKeyboard,
    /// `RIS` does not put `DECTCEM` back, so a hidden cursor stays hidden
    /// through a reset that restores every other mode.
    ///
    /// Verified rather than inferred: on the same emulator `RIS` does clear
    /// bracketed paste, so this is the terminal's own state and not the
    /// mapping onto it.
    RisDoesNotResetCursorVisibility,

    /// `CSI ?47 h` and `CSI ?1047 h` are ignored, so only `CSI ?1049 h`
    /// reaches the alternate screen.
    ///
    /// The two older sequences predate the cursor save that `?1049` bundles
    /// in. A backend that drops them stays on the primary screen and reports
    /// `alternate_screen` false, which is an honest account of what it did —
    /// the divergence is that it did nothing at all.
    ///
    /// Nothing in practice depends on it: every terminal here honors `?1049`,
    /// and that is what a full-screen program sends.
    NoLegacyAlternateScreen,
}

/// Generates the conformance tests for one backend. `$make` builds a boxed
/// emulator from `(cols, rows, &Profile)`.
///
/// A second argument lists the parts of the contract the backend cannot
/// express; see [`Divergence`].
///
/// The body is fully path-qualified because it expands into the caller's
/// module; it must not collide with whatever that module already imports.
#[macro_export]
macro_rules! emulator_conformance_tests {
    ($make:expr) => {
        $crate::emulator_conformance_tests!($make, &[]);
    };
    ($make:expr, $divergences:expr) => {
        const CONFORMANCE_DIVERGENCES: &[$crate::terminal::conformance::Divergence] = $divergences;

        fn conformance_emu(
            cols: u16,
            rows: u16,
            scrollback: usize,
        ) -> Box<dyn $crate::terminal::emu::Emulator> {
            conformance_emu_with(
                cols,
                rows,
                $crate::profile::Profile {
                    scrollback,
                    ..Default::default()
                },
            )
        }

        fn conformance_emu_with(
            cols: u16,
            rows: u16,
            profile: $crate::profile::Profile,
        ) -> Box<dyn $crate::terminal::emu::Emulator> {
            let make: fn(
                u16,
                u16,
                &$crate::profile::Profile,
            ) -> Box<dyn $crate::terminal::emu::Emulator> = $make;
            make(cols, rows, &profile)
        }

        /// Row text with trailing blanks removed, for readable assertions.
        fn conformance_text(rows: &[Vec<$crate::terminal::cell::EmuCell>]) -> Vec<String> {
            $crate::terminal::cell::rows_to_strings(rows)
                .into_iter()
                .map(|r| r.trim_end().to_string())
                .collect()
        }

        fn conformance_clipboard_unsupported() -> bool {
            CONFORMANCE_DIVERGENCES
                .contains(&$crate::terminal::conformance::Divergence::ClipboardUnsupported)
        }

        #[test]
        fn conformance_viewport_scrolls_without_input_or_history_changes() {
            use $crate::api::ScrollTarget;
            let mut e = conformance_emu(12, 3, 20);
            e.process(
                b"row0\r\nrow1\r\nrow2\r\n\x1b[1;31mrow3\x1b[0m\r\n\
                  \x1b]8;id=live;https://example.com\x1b\\row4\x1b]8;;\x1b\\\r\nrow5",
            );
            let full = e.full_rows();
            let cursor = e.cursor();
            assert_eq!(e.viewport(), $crate::api::Viewport { offset: 0, history: 3 });
            assert!(e.take_pending_writes().is_empty());
            for (target, offset) in [
                (ScrollTarget::Lines(1), 1),
                (ScrollTarget::Lines(1), 2),
                (ScrollTarget::Lines(-1), 1),
                (ScrollTarget::Lines(i32::MAX), 3),
                (ScrollTarget::Lines(i32::MAX), 3),
                (ScrollTarget::Lines(i32::MIN), 0),
                (ScrollTarget::Top, 3),
                (ScrollTarget::Lines(0), 3),
                (ScrollTarget::Bottom, 0),
            ] {
                e.scroll(target);
                assert_eq!(e.fault(), None);
                assert_eq!(e.viewport().offset, offset, "{target:?}");
                assert_eq!(e.viewable_rows(), full[3 - offset..6 - offset], "{target:?}");
                assert_eq!(e.active_rows(), full[3..], "{target:?}");
                assert_eq!(e.viewport().offset, offset, "active read moved the viewport");
                assert_eq!(e.full_rows(), full);
                assert_eq!(e.cursor(), cursor);
                assert!(e.cursor_visible());
                assert_eq!(e.viewport_cursor().is_some(), offset == 0);
                assert!(e.take_pending_writes().is_empty(), "scroll generated PTY input");
            }
        }

        #[test]
        fn conformance_viewport_preserves_output_anchor_and_clamps_eviction() {
            use $crate::api::ScrollTarget;
            let mut e = conformance_emu(12, 3, 8);
            e.process(b"row0\r\nrow1\r\nrow2\r\nrow3\r\nrow4\r\nrow5");
            e.scroll(ScrollTarget::Lines(2));
            let selected = e.viewable_rows();
            e.process(b"\r\nrow6");
            assert_eq!(e.viewable_rows(), selected);
            assert_eq!(e.viewport().offset, 3);
            for row in 7..20 {
                e.process(format!("\r\nrow{row}").as_bytes());
            }
            let viewport = e.viewport();
            assert_eq!(viewport.offset, viewport.history);
            assert_eq!(viewport.history, 8);
            assert_eq!(e.viewable_rows(), e.full_rows()[..3]);
            e.scroll(ScrollTarget::Bottom);
            e.process(b"\r\nnewest");
            assert_eq!(e.viewport().offset, 0);
            assert_eq!(conformance_text(&e.viewable_rows())[2], "newest");
            assert_eq!(e.fault(), None);
        }

        #[test]
        fn conformance_viewport_survives_resize_and_alternate_screen() {
            use $crate::api::ScrollTarget;
            let mut e = conformance_emu(12, 3, 20);
            e.process(b"row0\r\nrow1\r\nrow2\r\nrow3\r\nrow4\r\nrow5");
            e.scroll(ScrollTarget::Top);
            let selected = e.viewable_rows();
            e.process(b"\x1b[?1049halternate");
            for target in [ScrollTarget::Top, ScrollTarget::Lines(5), ScrollTarget::Bottom] {
                e.scroll(target);
                assert_eq!(e.viewport(), $crate::api::Viewport::default());
                assert_eq!(e.active_rows(), e.viewable_rows());
            }
            e.process(b"\x1b[?1049l");
            assert_eq!(e.viewable_rows(), selected);
            for (cols, rows) in [(12, 4), (8, 2), (16, 3)] {
                e.resize(cols, rows);
                let viewport = e.viewport();
                assert!(viewport.offset <= viewport.history);
                let full = e.full_rows();
                assert_eq!(e.viewable_rows(), full[viewport.origin()..viewport.origin() + rows as usize]);
            }
            assert_eq!(e.fault(), None);
        }

        #[test]
        fn conformance_viewport_follows_output_after_history_clear() {
            use $crate::api::ScrollTarget;
            for chunks in [
                vec![b"\x1b[3J\r\nrow6\r\nrow7".as_slice()],
                vec![b"\x1b[3".as_slice(), b"J", b"\r\nrow6\r\nrow7"],
            ] {
                let mut e = conformance_emu(12, 3, 20);
                e.process(b"row0\r\nrow1\r\nrow2\r\nrow3\r\nrow4\r\nrow5");
                e.scroll(ScrollTarget::Top);
                for chunk in chunks {
                    e.process(chunk);
                }
                assert_eq!(e.viewport().offset, 0);
                assert_eq!(conformance_text(&e.viewable_rows())[2], "row7");
                assert!(e.take_pending_writes().is_empty());
                assert_eq!(e.fault(), None);
            }
        }

        #[test]
        fn conformance_viewport_follows_output_after_resize_consumes_history() {
            use $crate::api::ScrollTarget;
            let mut e = conformance_emu(12, 3, 20);
            e.process(b"row0\r\nrow1\r\nrow2\r\nrow3\r\nrow4\r\nrow5");
            e.scroll(ScrollTarget::Top);
            e.resize(12, 6);
            assert_eq!(e.viewport(), $crate::api::Viewport::default());
            e.process(b"\r\nrow6\r\nrow7");
            assert_eq!(e.viewport().offset, 0);
            assert_eq!(conformance_text(&e.viewable_rows())[5], "row7");
            assert!(e.take_pending_writes().is_empty());
            assert_eq!(e.fault(), None);
        }

        #[test]
        fn conformance_viewport_follows_output_after_alternate_screen_resize() {
            use $crate::api::ScrollTarget;
            for mode in ["47", "1047", "1049", "25;1049"] {
                if matches!(mode, "47" | "1047") && CONFORMANCE_DIVERGENCES.contains(
                    &$crate::terminal::conformance::Divergence::NoLegacyAlternateScreen,
                ) {
                    continue;
                }
                let output = format!("\x1b[?{mode}l\x1b[6;1H\r\nrow6\r\nrow7");
                let output = output.as_bytes();
                for split in 0..=output.len() {
                    let mut e = conformance_emu(12, 3, 20);
                    e.process(b"row0\r\nrow1\r\nrow2\r\nrow3\r\nrow4\r\nrow5");
                    e.scroll(ScrollTarget::Top);
                    e.process(format!("\x1b[?{mode}h").as_bytes());
                    e.resize(12, 6);
                    e.process(&output[..split]);
                    e.process(&output[split..]);
                    assert_eq!(e.viewport().offset, 0, "mode {mode}, split at {split}");
                    assert_eq!(conformance_text(&e.viewable_rows())[5], "row7");
                    assert!(e.take_pending_writes().is_empty());
                    assert_eq!(e.fault(), None);
                }
            }
        }

        #[test]
        fn conformance_viewport_preserves_history_after_alternate_screen_resize() {
            use $crate::api::ScrollTarget;
            let mut e = conformance_emu(12, 3, 20);
            e.process(b"row0\r\nrow1\r\nrow2\r\nrow3\r\nrow4\r\nrow5");
            e.scroll(ScrollTarget::Top);
            let selected = e.viewable_rows();
            e.process(b"\x1b[?1049h");
            e.resize(12, 4);
            e.process(b"\x1b[?1049l\x1b[4;1H\r\nrow6\r\nrow7");
            assert!(e.viewport().offset > 0);
            assert_eq!(&e.viewable_rows()[..3], selected);
            assert_eq!(e.fault(), None);
        }

        #[test]
        fn conformance_viewport_preserves_history_after_partial_resize() {
            use $crate::api::ScrollTarget;
            let mut e = conformance_emu(12, 3, 20);
            e.process(b"row0\r\nrow1\r\nrow2\r\nrow3\r\nrow4\r\nrow5");
            e.scroll(ScrollTarget::Top);
            e.resize(12, 4);
            assert!(e.viewport().offset > 0);
            let selected = e.viewable_rows();
            e.process(b"\r\nrow6\r\nrow7");
            assert_eq!(e.viewable_rows(), selected);
            assert!(e.viewport().offset > 0);
            assert_eq!(e.fault(), None);
        }

        #[test]
        fn conformance_viewport_without_history_is_unchanged() {
            use $crate::api::ScrollTarget;
            let mut e = conformance_emu(12, 3, 0);
            e.process(b"row0\r\nrow1\r\nrow2\r\nrow3");
            let live = e.viewable_rows();
            for target in [ScrollTarget::Top, ScrollTarget::Lines(i32::MAX), ScrollTarget::Lines(i32::MIN), ScrollTarget::Bottom] {
                e.scroll(target);
                assert_eq!(e.viewport(), $crate::api::Viewport::default());
                assert_eq!(e.viewable_rows(), live);
            }
            assert_eq!(e.fault(), None);
        }

        /// A mode sequence still lands when a PTY read splits it, which is
        /// where an emulator that scanned for whole sequences would fail.
        /// Split one byte before the final letter, the last place the parser
        /// can still be holding parameters.
        #[test]
        fn conformance_terminal_modes_survive_a_split_read() {
            use $crate::terminal::emu::TerminalMode;
            for mode in TerminalMode::ALL {
                let set = mode.set_sequence();
                let (head, tail) = set.split_at(set.len() - 1);
                let mut e = conformance_emu(20, 4, 100);
                e.process(head);
                e.process(tail);
                assert!(e.mode(mode), "{} lost across a split read", mode.name());
            }
        }

        /// `RIS` puts every mode back to its default, which is how a program
        /// that leaves the terminal in a strange state is recovered from.
        #[test]
        fn conformance_ris_resets_every_terminal_mode() {
            use $crate::terminal::emu::TerminalMode;
            for mode in TerminalMode::ALL {
                let mut e = conformance_emu(20, 4, 100);
                // Drive it away from its default in whichever direction that is.
                let away = if mode.default_enabled() {
                    mode.reset_sequence()
                } else {
                    mode.set_sequence()
                };
                e.process(away);
                assert_eq!(e.mode(mode), !mode.default_enabled());

                e.process(b"\x1bc");
                if mode == TerminalMode::CursorVisible
                    && CONFORMANCE_DIVERGENCES.contains(
                        &$crate::terminal::conformance::Divergence::RisDoesNotResetCursorVisibility,
                    )
                {
                    continue;
                }
                assert_eq!(
                    e.mode(mode),
                    mode.default_enabled(),
                    "RIS did not restore {}",
                    mode.name()
                );
            }
        }

        /// The grid is always exactly `rows` x `cols`, regardless of content.
        #[test]
        fn conformance_grid_shape_is_exact() {
            let mut e = conformance_emu(10, 4, 100);
            e.process(b"hi");
            let rows = e.viewable_rows();
            assert_eq!(rows.len(), 4, "row count must equal terminal rows");
            for row in &rows {
                assert_eq!(row.len(), 10, "every row must be full width");
            }
            assert_eq!(e.size(), (10, 4));
        }

        /// `full_rows` is as rectangular as `viewable_rows`; ragged history
        /// rows would misalign the boxed snapshot output.
        #[test]
        fn conformance_full_rows_are_rectangular() {
            let mut e = conformance_emu(10, 3, 100);
            e.process(b"a\r\nbb\r\nccc\r\ndddd\r\neeeee");
            for row in e.full_rows() {
                assert_eq!(row.len(), 10, "history rows must be full width too");
            }
        }

        /// Printable text lands on the grid, and untouched cells are blank.
        ///
        /// A blank cell must be *fully* default: `expect --bg` and the SVG
        /// renderer read color off cells the shell never painted.
        #[test]
        fn conformance_text_and_blank_cells() {
            let mut e = conformance_emu(10, 2, 100);
            e.process(b"abc");
            let rows = e.viewable_rows();
            assert_eq!(rows[0][0].ch, "a");
            assert_eq!(rows[0][1].ch, "b");
            assert_eq!(rows[0][2].ch, "c");
            assert_eq!(
                rows[0][3],
                $crate::terminal::cell::EmuCell::default(),
                "an untouched cell must be blank with default colors and no attributes"
            );
            assert_eq!(
                $crate::terminal::cell::rows_to_strings(&rows)[0],
                "abc       "
            );
        }

        /// CR/LF moves to the next row rather than wrapping text together.
        #[test]
        fn conformance_newline_moves_row() {
            let mut e = conformance_emu(10, 3, 100);
            e.process(b"one\r\ntwo");
            let text = conformance_text(&e.viewable_rows());
            assert_eq!(text[0], "one");
            assert_eq!(text[1], "two");
        }

        /// A bare CR returns to column 0 so later bytes overwrite in place.
        /// Progress bars and readline redraws depend on this.
        #[test]
        fn conformance_carriage_return_overwrites() {
            let mut e = conformance_emu(10, 2, 100);
            e.process(b"12345\rab");
            assert_eq!(conformance_text(&e.viewable_rows())[0], "ab345");
        }

        /// Backspace moves the cursor back without erasing; the next byte
        /// overwrites in place.
        #[test]
        fn conformance_backspace_moves_cursor() {
            let mut e = conformance_emu(10, 2, 100);
            e.process(b"abc\x08X");
            assert_eq!(conformance_text(&e.viewable_rows())[0], "abX");
            assert_eq!(e.cursor(), (3, 0));
        }

        /// Text longer than the row wraps onto the next row. `expect text` and
        /// `locator::find` flatten the grid, so wrap placement is observable.
        #[test]
        fn conformance_autowrap_at_right_margin() {
            let mut e = conformance_emu(5, 3, 100);
            e.process(b"abcdefgh");
            let text = conformance_text(&e.viewable_rows());
            assert_eq!(text[0], "abcde", "first row fills to the margin");
            assert_eq!(text[1], "fgh", "the remainder continues on the next row");
        }

        /// A tab advances the cursor to the next 8-column tab stop.
        ///
        /// What a backend leaves *in* the skipped cells is genuinely
        /// divergent — alacritty stores a literal `\t` there, others store
        /// blanks — so this pins only cursor advance and landing column,
        /// which every emulator agrees on.
        #[test]
        fn conformance_tab_advances_to_stop() {
            let mut e = conformance_emu(20, 2, 100);
            e.process(b"a\tb");
            let rows = e.viewable_rows();
            assert_eq!(rows[0][0].ch, "a");
            assert_eq!(rows[0][8].ch, "b", "next glyph lands on the 8-column stop");
            assert_eq!(e.cursor(), (9, 0), "cursor sits just past the tab stop");
        }

        /// Every SGR attribute the cell vocabulary exposes round-trips.
        #[test]
        fn conformance_sgr_attributes() {
            let mut e = conformance_emu(20, 2, 100);
            // bold, dim, italic, underline, inverse, hidden, strikethrough
            e.process(b"\x1b[1;2;3;4;7;8;9mX\x1b[0mY");
            let rows = e.viewable_rows();
            use $crate::terminal::cell::Attrs;
            let set = Attrs::BOLD
                | Attrs::DIM
                | Attrs::ITALIC
                | Attrs::INVERSE
                | Attrs::INVISIBLE
