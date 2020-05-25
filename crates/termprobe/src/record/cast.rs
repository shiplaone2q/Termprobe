use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::profile::ColorSlot;
use crate::terminal::cell::{Attrs, Color, EmuCell, UnderlineStyle, CONTINUATION};
use crate::terminal::emu::{CursorShape, Emulator};

const WIN32_INPUT_MODE: &[u8] = b"\x1b[?9001h";

pub(crate) struct CastWriter {
    start: Instant,
    path: PathBuf,
    sink: BufWriter<File>,
    last_committed: Option<Duration>,
}

impl CastWriter {
    pub fn create(
        path: &Path,
        cols: u16,
        rows: u16,
        env: &[(String, String)],
        start: Instant,
    ) -> io::Result<Self> {
        Self::create_guarded(path, cols, rows, env, start, None)
    }

    pub(super) fn create_guarded(
        path: &Path,
        cols: u16,
        rows: u16,
        env: &[(String, String)],
        start: Instant,
        protected: Option<&super::path::ProtectedFile>,
    ) -> io::Result<Self> {
        if let Some(protected) = protected {
            protected.check_path(path)?;
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let sink = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        // Check the opened file too, before truncation, so a path alias cannot
        // destroy the automatic stream even if it changes during the open.
        if let Some(protected) = protected {
            protected.check_file(path, &sink)?;
        }
        sink.set_len(0)?;
        let mut writer = Self {
            start,
            path: path.to_path_buf(),
            sink: BufWriter::new(sink),
            last_committed: None,
        };
        writer.write_header(cols, rows, env)?;
        Ok(writer)
    }

    pub fn write_output(&mut self, at: Instant, data: &str) -> io::Result<()> {
        self.write_event(at, "o", data)
    }

    pub fn write_resize(&mut self, at: Instant, cols: u16, rows: u16) -> io::Result<()> {
        self.write_event(at, "r", &format!("{cols}x{rows}"))
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.sink.flush()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn protect(&self) -> io::Result<super::path::ProtectedFile> {
        super::path::ProtectedFile::new(&self.path, self.sink.get_ref())
    }

    pub fn last_committed(&self) -> Option<Duration> {
        self.last_committed
    }

    fn write_header(&mut self, cols: u16, rows: u16, env: &[(String, String)]) -> io::Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let env = env
            .iter()
            .map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
            .collect::<serde_json::Map<String, serde_json::Value>>();
        serde_json::to_writer(
            &mut self.sink,
            &serde_json::json!({
                "version": 2,
                "width": cols,
                "height": rows,
                "timestamp": timestamp,
                "env": env,
            }),
        )
        .map_err(json_error)?;
        self.sink.write_all(b"\n")?;
        self.sink.flush()
    }

    fn write_event(&mut self, at: Instant, code: &str, data: &str) -> io::Result<()> {
        let elapsed = at.saturating_duration_since(self.start);
        serde_json::to_writer(&mut self.sink, &(elapsed.as_secs_f64(), code, data))
            .map_err(json_error)?;
