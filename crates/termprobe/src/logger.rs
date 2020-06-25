//! Optional verbose logging of everything a terminal session reads and writes
//! the PTY, plus lifecycle events. Modeled on inshellisense's data log: every
//! record is timestamped and byte-escaped so control sequences are visible.
//!
//! Tags:
//!   READ   bytes read from the PTY (what the app draws on screen)
//!   WRITE  bytes written to the PTY (keystrokes / input)
//!   REPLY  bytes we wrote back to the PTY answering a capability query
//!   EVENT  lifecycle notes (open, resize, exit, requests)

use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Logger {
    sink: Option<Mutex<BufWriter<std::fs::File>>>,
}

impl Logger {
    /// A no-op logger.
    pub fn disabled() -> Self {
        Logger { sink: None }
    }

    /// Create a logger that truncates and writes to `path`.
    pub fn to_file(path: &Path) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
