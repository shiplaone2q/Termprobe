//! A single managed terminal session: a PTY feeding an emulator and command
//! tracker, with a background reader thread.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::diagnostics::{elapsed_ms, DiagnosticRetentionOptions, ScreenHistory};
use crate::event::BellTracker;
use crate::logger::Logger;
use crate::profile::Profile;
use crate::record::{self, CaptureError, Recorder, StartRecording};
#[cfg(feature = "recording-raster")]
use crate::render::raster::GridRenderer;
use crate::shell::{self, Shell};
use crate::terminal::backend::Backend;
use crate::terminal::emu::{Emulator, MouseModeTracker};
use crate::terminal::integration::CommandTracker;
use crate::terminal::pty::{validate_cwd, validate_size, Pty, SpawnOptions};

#[derive(Debug, Clone)]
pub(crate) struct TextHighlight {
    /// Match cells in full-grid coordinates.
    pub cells: Vec<(usize, usize)>,
    /// Full-grid row where the viewport began when the highlight was created.
    pub viewport_offset: usize,
}

pub(crate) struct ManualRecordingOptions {
    pub path: String,
    pub format: Option<crate::api::RecordingFormat>,
    pub fps: Option<u8>,
    pub speed: Option<f64>,
    pub idle_time_limit: Option<f64>,
    pub zoom: Option<f64>,
    pub background: Option<crate::api::CaptureBackground>,
}

pub struct TermState {
    pub emu: Box<dyn Emulator>,
    /// The profile this session started with, kept so a palette entry a
    /// program overrode can be told apart from one it never touched.
    pub(crate) profile: Profile,
    /// How this session's screenshots and recordings are drawn.
    pub(crate) style: crate::render::style::Style,
    #[cfg(feature = "recording-raster")]
    recording_frames: Option<record::frames::FrameCapture>,
    /// Shell-integration state, derived from the raw PTY stream rather than
    /// the emulator, so it is identical across backends.
    pub tracker: CommandTracker,
    pub(crate) mouse_mode: MouseModeTracker,
    pub observed_clipboard_revision: u64,
    pub started_at: Instant,
    pub visual_revision: u64,
    pub screen_history: ScreenHistory,
    pub screen_dirty: bool,
    pub last_screen_sample: Instant,
    pub last_visual_change_ms: u64,
    pub diagnostic_error: Option<String>,
    /// Time of the last observed visual change, not the last raw PTY output.
    pub last_change: Instant,
    pub awaiting_start: Option<u64>,
    pub exited: Option<i32>,
    pub exit_signal: Option<String>,
    pub exit_error: Option<String>,
    pub highlight: Option<TextHighlight>,
}

impl TermState {
    pub(crate) fn awaiting_command_start(&self) -> bool {
        self.awaiting_start
            .is_some_and(|seen| self.tracker.started_count() == seen)
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.exited.is_none()
            && self.exit_error.is_none()
            && !self.awaiting_command_start()
            && self.tracker.is_ready()
    }

    pub(crate) fn command_exit_code(&self) -> Option<i32> {
        if self.awaiting_command_start() || self.tracker.executing() {
            None
        } else {
            self.tracker.last_exit()
        }
    }
}

pub struct Session {
    pub shell: Option<Shell>,
    pub backend: Backend,
    pub child_pid: Option<u32>,
    /// Per-class timeout defaults for the lifetime of this session.
    pub timeouts: crate::api::Timeouts,
    pub pty: Arc<Pty>,
    pub state: Arc<Mutex<TermState>>,
    /// Serialize viewport movement with input without blocking the output reader.
    pub(crate) viewport_input: Arc<Mutex<()>>,
    pub cancelled: Arc<AtomicBool>,
    pub(crate) bells: BellTracker,
    recorder: Recorder,
    logger: Arc<Logger>,
    reader: Option<JoinHandle<()>>,
    _process_watcher: JoinHandle<()>,
}

impl Session {
    /// The session default for `class`, else the environment, else the built-in.
    pub fn timeout_for(&self, class: crate::config::TimeoutClass) -> u64 {
        self.timeouts
            .get(class)
            .unwrap_or_else(|| class.default_ms())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn open(
        shell: Option<Shell>,
        program: Option<Vec<String>>,
        backend: Backend,
        profile: Profile,
        style: crate::render::style::Style,
        cols: u16,
        rows: u16,
        cwd: Option<PathBuf>,
        env: Vec<(String, String)>,
        timeouts: crate::api::Timeouts,
        diagnostics: DiagnosticRetentionOptions,
        logger: Arc<Logger>,
        recording_path: Option<PathBuf>,
        recording_required: bool,
    ) -> anyhow::Result<Self> {
        style.validate().map_err(anyhow::Error::msg)?;
        validate_size(cols, rows)?;
        if let Some(cwd) = &cwd {
            validate_cwd(cwd)?;
        }
        diagnostics.validate().map_err(anyhow::Error::msg)?;
        let started_at = Instant::now();
        let bells = BellTracker::new(started_at);
        let emu = backend.build_with_bells(cols, rows, &profile, bells.clone())?;
        let mut initial_state = TermState {
            emu,
            profile,
            style,
            #[cfg(feature = "recording-raster")]
            recording_frames: None,
            tracker: CommandTracker::new(),
            mouse_mode: MouseModeTracker::new(),
            observed_clipboard_revision: 0,
            started_at,
            visual_revision: 0,
            screen_history: ScreenHistory::new(diagnostics.screen_history_limit),
            screen_dirty: true,
            last_screen_sample: started_at,
            last_visual_change_ms: 0,
            diagnostic_error: None,
            last_change: Instant::now(),
            awaiting_start: None,
            exited: None,
            exit_signal: None,
            exit_error: None,
            highlight: None,
        };
        let _ = try_capture_visual_state(&mut initial_state, true);
        let state = Arc::new(Mutex::new(initial_state));

        let mut rec_env = vec![("TERM".to_string(), "xterm-256color".to_string())];
        if let Some(sh) = shell {
            rec_env.push(("SHELL".to_string(), sh.as_str().to_string()));
        }
        let recorder = Recorder::create_at(
            recording_path.clone(),
            cols,
            rows,
            &rec_env,
            recording_required,
            logger.clone(),
            started_at,
        )?;
        let spawned = (|| {
            if let Some(program) = &program {
                let (target, args) = program
                    .split_first()
                    .ok_or_else(|| anyhow::anyhow!("empty program"))?;
                let opts = SpawnOptions {
                    cols,
                    rows,
                    cwd: None,
                    env,
                };
                Pty::spawn_with_cwd(target, args, &opts, cwd.as_deref())
            } else {
                let sh = shell.unwrap_or_else(shell::default_shell);
                let mut launch = shell::shell_launch(sh)?;
                launch.env.extend(env);
                Pty::spawn_launch_with_cwd(&launch, cols, rows, cwd)
            }
        })();
        let (pty, reader) = match spawned {
            Ok(spawned) => spawned,
            Err(error) => {
                let remove = recorder.automatic_enabled();
                drop(recorder);
                if remove {
                    if let Some(path) = recording_path {
                        let _ = std::fs::remove_file(path);
                    }
                }
                return Err(error);
            }
        };

        let child_pid = pty.pid();
        let pty = Arc::new(pty);
        let cancelled = Arc::new(AtomicBool::new(false));

        let reader_state = state.clone();
        let reader_pty = pty.clone();
        let reader_logger = logger.clone();
        let reader_recorder = recorder.capture();
        let reader_finished = Arc::new(AtomicBool::new(false));
        let finished = reader_finished.clone();
        let mut reader = reader;
        let reader_handle = std::thread::spawn(move || {
            use std::io::Read;
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        reader_logger.event("pty stream reached EOF");
                        break;
                    }
                    Err(error) => {
                        reader_logger.event(&format!("pty read failed error={error}"));
                        break;
                    }
                    Ok(n) => {
                        reader_logger.read(&buf[..n]);
                        let pending = {
                            let mut st = reader_state
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            st.emu.process(&buf[..n]);
                            st.tracker.feed(&buf[..n]);
                            st.visual_revision = st.visual_revision.wrapping_add(1);
                            st.screen_dirty = true;
                            st.mouse_mode.process(&buf[..n]);
                            st.highlight = None;
                            reader_recorder.on_data(&buf[..n]);
                            #[cfg(feature = "recording-raster")]
                            capture_recording_frame(&mut st);
                            let _ = try_capture_visual_state(&mut st, false);
                            st.emu.take_pending_writes()
                        };
                        if !pending.is_empty() {
                            reader_logger.reply(&pending);
                            let _ = reader_pty.write(&pending);
                        }
                    }
                }
            }
            finished.store(true, Ordering::Release);
        });

        let watcher_state = state.clone();
        let watcher_pty = pty.clone();
        let watcher_logger = logger.clone();
        let process_watcher = std::thread::spawn(move || loop {
            let status = watcher_pty.try_wait();
            match status {
                Ok(Some(status)) => {
                    // Preserve trailing output before publishing exit, without
                    // waiting forever for descendants that inherited the PTY.
                    let draining = Instant::now();
                    while !reader_finished.load(Ordering::Acquire)
                        && draining.elapsed() < Duration::from_millis(250)
                    {
                        std::thread::sleep(Duration::from_millis(crate::config::POLL_DELAY_MS));
                    }
                    watcher_logger.event(&format!(
                        "process exited code={} signal={:?}",
                        status.code, status.signal
                    ));
                    let mut st = watcher_state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    st.exited = Some(status.code);
                    st.exit_signal = status.signal;
                    break;
                }
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(crate::config::POLL_DELAY_MS));
                }
                Err(error) => {
                    let error = error.to_string();
                    watcher_logger.event(&format!("process wait failed error={error}"));
                    let mut st = watcher_state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    st.exit_error = Some(error);
                    break;
                }
            }
        });

        logger.event(&format!(
            "session open shell={:?} program={:?} backend={} {}x{}",
            shell,
            program,
            backend.as_str(),
            cols,
            rows
        ));

        Ok(Session {
            shell,
            backend,
            child_pid,
            timeouts,
            pty,
            state,
            viewport_input: Arc::new(Mutex::new(())),
            cancelled,
            bells,
            recorder,
            logger,
            reader: Some(reader_handle),
            _process_watcher: process_watcher,
