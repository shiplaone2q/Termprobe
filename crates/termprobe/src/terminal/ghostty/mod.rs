//! [`Emulator`] backend built on the `ghostty-vt` dependency.
//!
//! Ghostty handles are deliberately `!Send`, so the native terminal lives
//! on one worker thread. [`GhosttyEmu`] is the `Send` channel handle used by
//! the rest of termprobe.

use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use alacritty_terminal::vte::{Params, Parser, Perform};
use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use crate::event::BellTracker;
use crate::input::keys::KeyPress;
use crate::profile::{ColorSlot, Profile, Rgb};
use crate::terminal::cell::EmuCell;
use crate::terminal::emu::{
    ClipboardType, ClipboardValidator, ColorTable, CursorShape, Emulator, KeyboardMode,
    TerminalMode,
};

use self::core::GhosttyCore;

mod core;

#[derive(Debug)]
enum ClipboardOperation {
    Store {
        clipboard: ClipboardType,
        text: String,
    },
    Query {
        clipboard: ClipboardType,
        selector: u8,
        bell_terminated: bool,
    },
}

#[derive(Default)]
struct SequenceState {
    current: Option<String>,
    stack: Vec<Option<String>>,
    clipboard_operations: Vec<ClipboardOperation>,
    screen_exit: bool,
}

impl Perform for SequenceState {
    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        match params.first().copied() {
            Some(b"0" | b"2") => {
                let title = params[1..]
                    .iter()
                    .map(|part| String::from_utf8_lossy(part))
                    .collect::<Vec<_>>()
                    .join(";");
                self.current = (!title.is_empty()).then_some(title);
            }
            Some(b"52") => {
                let (clipboard, selector) = match params.get(1).copied() {
                    Some(b"c") => (ClipboardType::Clipboard, b'c'),
                    Some(b"p") => (ClipboardType::Selection, b'p'),
                    Some(b"s") => (ClipboardType::Selection, b's'),
                    _ => return,
                };
                match params.get(2).copied() {
                    Some(b"?") => self.clipboard_operations.push(ClipboardOperation::Query {
                        clipboard,
                        selector,
                        bell_terminated,
                    }),
                    Some(encoded) => {
                        let Ok(bytes) = BASE64.decode(encoded) else {
                            return;
                        };
                        let Ok(text) = String::from_utf8(bytes) else {
                            return;
                        };
                        self.clipboard_operations
                            .push(ClipboardOperation::Store { clipboard, text });
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        if !ignore && intermediates == b"?" && action == 'l' {
            self.screen_exit |= params
                .iter()
                .any(|param| matches!(param, [47 | 1047 | 1049]));
        }
        if ignore || action != 't' || !intermediates.is_empty() {
            return;
        }
        let operation = params
            .iter()
            .next()
            .and_then(|param| param.first())
            .copied()
            .unwrap_or_default();
        match operation {
            22 => self.stack.push(self.current.clone()),
            23 => {
                if let Some(title) = self.stack.pop() {
                    self.current = title;
                }
            }
            _ => {}
        }
    }
}

struct SequenceTracker {
    parser: Parser,
    state: SequenceState,
}

impl SequenceTracker {
    fn new() -> Self {
        Self {
            parser: Parser::new(),
            state: SequenceState::default(),
        }
    }

    fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.state, bytes);
    }

    fn take_clipboard_operations(&mut self) -> Vec<ClipboardOperation> {
        std::mem::take(&mut self.state.clipboard_operations)
    }
}

type Job = Box<dyn FnOnce(&mut GhosttyCore) + Send + 'static>;

pub struct GhosttyEmu {
    jobs: Option<Sender<Job>>,
    worker: Option<JoinHandle<()>>,
    sequences: SequenceTracker,
    clipboard_validator: ClipboardValidator,
}

impl GhosttyEmu {
    pub fn new(cols: u16, rows: u16, profile: &Profile) -> Result<Self> {
        Self::with_bell_tracker(cols, rows, profile, BellTracker::default())
    }

    pub(crate) fn with_bell_tracker(
        cols: u16,
        rows: u16,
        profile: &Profile,
        bells: BellTracker,
    ) -> Result<Self> {
        let (jobs, receiver) = mpsc::channel::<Job>();
        let (ready, started) = mpsc::sync_channel(1);
        let profile = *profile;
        let worker = thread::Builder::new()
            .name("termprobe-ghostty".to_string())
            .spawn(move || match GhosttyCore::new(cols, rows, profile, bells) {
                Ok(mut core) => {
                    let _ = ready.send(Ok(()));
                    while let Ok(job) = receiver.recv() {
                        job(&mut core);
                    }
                }
                Err(error) => {
                    let _ = ready.send(Err(format!("{error:#}")));
                }
            })
            .context("spawning Ghostty worker")?;

        match started.recv().context("starting Ghostty worker")? {
            Ok(()) => Ok(Self {
                jobs: Some(jobs),
                worker: Some(worker),
                sequences: SequenceTracker::new(),
                clipboard_validator: ClipboardValidator::new(),
            }),
            Err(message) => {
                let _ = worker.join();
                Err(anyhow!(message))
            }
        }
    }

    fn call<T, F>(&self, operation: &'static str, job: F) -> T
    where
        T: Send + 'static,
        F: FnOnce(&mut GhosttyCore) -> T + Send + 'static,
    {
        let (reply, response) = mpsc::sync_channel(1);
        self.jobs
            .as_ref()
            .expect("Ghostty worker is shutting down")
            .send(Box::new(move |core| {
                let _ = reply.send(job(core));
            }))
            .unwrap_or_else(|_| panic!("Ghostty worker stopped during {operation}"));
        response
            .recv()
            .unwrap_or_else(|_| panic!("Ghostty worker stopped during {operation}"))
    }

    fn call_result<T, F>(&self, operation: &'static str, job: F) -> T
    where
        T: Send + 'static,
        F: FnOnce(&mut GhosttyCore) -> Result<T> + Send + 'static,
    {
        self.call(operation, job)
            .unwrap_or_else(|error| panic!("Ghostty {operation} failed: {error:#}"))
    }
}

impl Drop for GhosttyEmu {
    fn drop(&mut self) {
        self.jobs.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Emulator for GhosttyEmu {
    fn process(&mut self, bytes: &[u8]) {
        self.clipboard_validator.process(bytes);
        let mut start = 0;
        for index in 0..bytes.len() {
            self.sequences.feed(&bytes[index..=index]);
            let operations = self.sequences.take_clipboard_operations();
            // Normalize the restored viewport before later bytes add output.
            let screen_exit = std::mem::take(&mut self.sequences.state.screen_exit);
            if operations.is_empty() && !screen_exit {
                continue;
            }

            let owned = bytes[start..=index].to_vec();
            self.call_result("processing output", move |core| {
                core.process(&owned)?;
                for operation in operations {
