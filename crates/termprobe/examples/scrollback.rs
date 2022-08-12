//! Run with `cargo run -p termprobe-rs --example scrollback --all-features`.

use std::io::{self, BufRead, Write};

use anyhow::{bail, ensure, Result};
use termprobe::{
    AutomaticRecording, AutomaticRecordingMode, Backend, OpenOptions, Operation, OperationResult,
    RunOptions, ScrollTarget, Session,
};

fn text(session: &Session, full: bool) -> Result<String> {
    match session.execute(Operation::Text { full })? {
        OperationResult::Text(text) => Ok(text),
        _ => bail!("unexpected text result"),
    }
}

fn demonstrate(backend: Backend) -> Result<()> {
    let session = Session::new(format!("scrollback-example-{}", backend.as_str()));
    let defaults = OpenOptions::default();
    session.run(RunOptions {
        backend,
        program: std::env::current_exe()?.to_string_lossy().into_owned(),
        args: vec!["--child".into()],
        cols: 30,
        rows: 5,
        profile: defaults.profile,
        style: defaults.style,
        cwd: None,
        env: Vec::new(),
        wait_ready: Some(false),
        restart: false,
        timeouts: defaults.timeouts,
        recording: AutomaticRecording {
            mode: AutomaticRecordingMode::Disabled,
            directory: None,
        },
    })?;
    let result = (|| -> Result<()> {
        session
            .get_by_text("READY")
            .wait_with_timeout(Some(5_000))?;
        let full = text(&session, true)?;
