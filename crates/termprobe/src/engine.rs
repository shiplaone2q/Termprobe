//! Reusable in-process terminal engine.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};
use std::time::{Duration, Instant};

use crate::api::{
    AutomaticRecording, AutomaticRecordingMode, Cell, CellColor, ClipboardPattern, Cursor,
    EffectiveTimeouts, ErrorKind, LocatorQuery, LocatorSelector, OpenOptions, OpenResult,
    Operation, OperationResult, PackedScreen, RunOptions, RuntimeStatus, ScreenshotResult,
    ScrollTarget, Size, SnapshotResult, TextAnchor, TextMatch, TextSelector, TextStyle,
    TermprobeError,
};
use crate::assert::color::{self, Expected};
use crate::assert::snapshot::{self, SnapshotStatus};
use crate::config::{self, POLL_DELAY_MS};
use crate::diagnostics::strings::{
    base_error_message, capture_error_message, diagnostic_hints, diagnostic_operation_name,
    format_timeout, locator_failure_message, operation_timeout, safe_operation_summary,
    timeout_message, title_timeout_message_from_actual, truncate_diagnostic_value,
};
use crate::diagnostics::{
    allocate_artifact_directory, allocate_trace_directory, elapsed_ms, failure_reason,
    recording_temp_path, write_failure_artifact, ArtifactInputs, CellMismatch, CellStyleEvaluation,
    ExecutionContext, FailureArtifactRef, FailureArtifactStatus, FailureObservation, FailureReason,
    FailureReport, InputDetails, LocatorFailureReason, OperationEvent, OperationExpectation,
    OperationHistory, PreparedRecording, ProcessDiagnostics, RecordingDiagnostics, RecordingStatus,
    RuntimeDiagnostics, TraceMode, TraceOptions, TraceOutcome, RECORDING_COPY_LIMIT,
};
use crate::diagnostics::{comparison_failure, merge_failure_details};
use crate::input::{keys, mouse};
use crate::logger::Logger;
use crate::session::{
    capture_visual_state, scroll_emulator_and_record, try_capture_visual_state,
    Session as TerminalSession, TermState, TextHighlight,
};
use crate::terminal::cell::{rows_to_strings, Attrs, Color, EmuCell};
use crate::terminal::emu::{
    ClipboardType, CursorShape, Emulator, KeyboardMode, MouseMode, TerminalMode,
};
use crate::terminal::locator::{self, Pattern};

pub struct Engine {
    name: String,
    operations: Mutex<()>,
    lifecycle: RwLock<()>,
    pending_lifecycle: AtomicUsize,
    session: RwLock<Option<TerminalSession>>,
    spawn_spec: Mutex<Option<SpawnSpec>>,
    live: Arc<Mutex<Option<LiveTarget>>>,
    interrupt: Mutex<Option<InterruptTarget>>,
    interrupt_generation: AtomicU64,
    logger: Arc<Logger>,
    default_recording_path: PathBuf,
    recording: Mutex<RecordingState>,
    trace: Mutex<TraceState>,
    operation_history: Mutex<OperationHistory>,
}

struct LifecycleRequest<'a>(&'a AtomicUsize);

impl<'a> LifecycleRequest<'a> {
    fn new(pending: &'a AtomicUsize) -> Self {
        pending.fetch_add(1, Ordering::AcqRel);
        Self(pending)
    }
}

impl Drop for LifecycleRequest<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

struct WaitContext<'a> {
    session: &'a TerminalSession,
    request_cancelled: &'a dyn Fn() -> bool,
}

impl WaitContext<'_> {
    fn check_cancelled(&self) -> Result<(), TermprobeError> {
        if (self.request_cancelled)() {
            Err(TermprobeError::assertion("operation was cancelled"))
        } else {
            Ok(())
        }
    }
}

impl std::ops::Deref for WaitContext<'_> {
    type Target = TerminalSession;

    fn deref(&self) -> &Self::Target {
        self.session
    }
}

#[derive(Clone)]
struct SpawnSpec {
    command: SpawnCommand,
    resolved_cwd: Option<PathBuf>,
    retention: crate::diagnostics::DiagnosticRetentionOptions,
    trace: Option<TraceOptions>,
}

#[derive(Clone)]
enum SpawnCommand {
    Open(OpenOptions),
    Run(RunOptions),
}

impl SpawnSpec {
    fn restart(mut self) -> Self {
        match &mut self.command {
            SpawnCommand::Open(options) => options.restart = true,
            SpawnCommand::Run(options) => options.restart = true,
        }
        self
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        match &mut self.command {
            SpawnCommand::Open(options) => {
                options.cols = cols;
                options.rows = rows;
            }
            SpawnCommand::Run(options) => {
                options.cols = cols;
                options.rows = rows;
            }
        }
    }
}

#[derive(Clone)]
struct RecordingState {
    path: Option<PathBuf>,
    mode: AutomaticRecordingMode,
    failed: bool,
}

#[derive(Default)]
struct TraceState {
    options: TraceOptions,
    artifact: Option<FailureArtifactRef>,
    context: std::collections::BTreeMap<String, String>,
    owned_directories: Vec<PathBuf>,
    pending_startup_outcome: bool,
}

impl TraceState {
    fn diagnostic_context(
        &self,
        context: &ExecutionContext,
    ) -> std::collections::BTreeMap<String, String> {
        let mut diagnostic_context = self.context.clone();
        diagnostic_context.extend(context.sanitized_context());
        ExecutionContext {
            diagnostic_context,
            ..Default::default()
        }
        .sanitized_context()
    }
}

#[derive(Clone)]
struct InterruptTarget {
    pty: Arc<crate::terminal::pty::Pty>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

struct LiveTarget {
    state: Arc<Mutex<TermState>>,
    viewport_input: Arc<Mutex<()>>,
    pty: Arc<crate::terminal::pty::Pty>,
    shell: Option<&'static str>,
}

#[derive(Clone)]
struct OperationMetadata {
    sequence: u64,
    interrupt_generation: u64,
    name: String,
    timeout_ms: Option<u64>,
    started_at: Instant,
    started_ms: u64,
    screen_before: u64,
    safe_summary: String,
    is_assertion: bool,
    expectation: Option<OperationExpectation>,
    input: Option<InputDetails>,
}

impl OperationMetadata {
    fn pending_event(&self, result: &str, screen_at_return: u64) -> OperationEvent {
        OperationEvent {
            sequence: self.sequence,
            name: self.name.clone(),
            started_ms: self.started_ms,
            ended_ms: self.started_ms.saturating_add(elapsed_ms(self.started_at)),
            result: result.into(),
            screen_before: self.screen_before,
            screen_at_return,
            safe_summary: self.safe_summary.clone(),
            is_assertion: self.is_assertion,
            expectation: self.expectation.clone(),
            input: self.input.clone(),
        }
    }
}

pub struct LiveFrame {
    pub grid: Vec<Vec<EmuCell>>,
    pub cursor: (u16, u16),
    pub cursor_visible: bool,
    pub size: (u16, u16),
    pub keyboard_mode: KeyboardMode,
    pub cursor_key_application: bool,
    pub bracketed_paste: bool,
    pub mouse_mode: MouseMode,
    pub exited: Option<i32>,
    pub shell: Option<&'static str>,
}

/// One-line operation description for the verbose log. Open and Run redact env
/// values (they may contain secrets) and report only the variable count.
fn operation_summary(operation: &Operation) -> String {
    match operation {
        Operation::Open(options) => format!(
            "Open {{ backend: {}, shell: {:?}, scrollback: {}, {}x{}, cwd: {:?}, wait_ready: {:?}, restart: {}, timeouts: {:?}, env: <{} vars> }}",
            options.backend.as_str(),
            options.shell,
            options.profile.scrollback,
            options.cols,
            options.rows,
            options.cwd,
            options.wait_ready,
            options.restart,
            options.timeouts,
            options.env.len()
        ),
        Operation::Run(options) => format!(
            "Run {{ backend: {}, program: {:?}, args: {:?}, scrollback: {}, {}x{}, cwd: {:?}, wait_ready: {:?}, restart: {}, timeouts: {:?}, env: <{} vars> }}",
            options.backend.as_str(),
            options.program,
            options.args,
            options.profile.scrollback,
            options.cols,
            options.rows,
            options.cwd,
            options.wait_ready,
            options.restart,
            options.timeouts,
            options.env.len()
        ),
        other => format!("{other:?}"),
    }
}

impl Engine {
    pub fn new(name: String, logger: Arc<Logger>, recording_path: PathBuf) -> Self {
        Self {
            name,
            operations: Mutex::new(()),
            lifecycle: RwLock::new(()),
            pending_lifecycle: AtomicUsize::new(0),
            session: RwLock::new(None),
            spawn_spec: Mutex::new(None),
            live: Arc::new(Mutex::new(None)),
            interrupt: Mutex::new(None),
            interrupt_generation: AtomicU64::new(0),
            logger,
            default_recording_path: recording_path.clone(),
            recording: Mutex::new(RecordingState {
                path: None,
                mode: AutomaticRecordingMode::Disabled,
                failed: false,
            }),
            trace: Mutex::new(TraceState::default()),
            operation_history: Mutex::new(OperationHistory::new()),
        }
    }

    pub fn execute(&self, operation: Operation) -> Result<OperationResult, TermprobeError> {
        self.execute_with_context(operation, ExecutionContext::default())
    }

    pub fn execute_with_context(
        &self,
        operation: Operation,
        context: ExecutionContext,
    ) -> Result<OperationResult, TermprobeError> {
        self.execute_with_context_cancellable(operation, context, &|| false)
    }

    /// Execute a request whose caller may disconnect. Cancellation is checked
    /// during waits and never terminates the session or cancels another caller.
    pub fn execute_with_context_cancellable(
        &self,
        operation: Operation,
        context: ExecutionContext,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<OperationResult, TermprobeError> {
        if let Some(artifact) = &context.artifact {
            artifact.validate().map_err(TermprobeError::usage)?;
        }
        if let Some(trace) = &context.trace {
            trace.validate().map_err(TermprobeError::usage)?;
        }
        self.interrupt_for_operation(&operation);
        let interrupt_generation = self.interrupt_generation.load(Ordering::Acquire);
        let waiting = operation.is_wait();
        let lifecycle = matches!(
            operation,
            Operation::Open(_)
                | Operation::Run(_)
                | Operation::Restart { .. }
                | Operation::Close
                | Operation::FinishTrace { .. }
        );
        // A wait pins its generation through diagnostics, but does not exclude
        // input or resize. Announce lifecycle writers before locking so even a
        // newly arriving waiter yields, independent of RwLock's fairness policy.
        let pending = lifecycle.then(|| LifecycleRequest::new(&self.pending_lifecycle));
        let cancelled =
            || cancelled() || (waiting && self.pending_lifecycle.load(Ordering::Acquire) != 0);
        let _shared = if lifecycle {
            None
        } else if waiting {
            Some(loop {
                if cancelled() {
                    return Err(TermprobeError::assertion("operation was cancelled"));
                }
                match self.lifecycle.try_read() {
                    Ok(guard) => break guard,
                    Err(TryLockError::Poisoned(error)) => break error.into_inner(),
                    Err(TryLockError::WouldBlock) => {
                        std::thread::sleep(Duration::from_millis(POLL_DELAY_MS));
                    }
                }
            })
        } else {
            Some(
                self.lifecycle
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            )
        };
        let _exclusive = lifecycle.then(|| {
            self.lifecycle
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        });
        drop(pending);
        let _operation = (!waiting).then(|| {
            self.operations
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        });
        if self.logger.enabled() {
            self.logger
                .event(&format!("operation {}", operation_summary(&operation)));
        }
        let name = context
            .operation_name
            .clone()
            .unwrap_or_else(|| diagnostic_operation_name(&operation).to_string());
        let screen_before = self.capture_current_screen_sequence(true, false);
        let canonical_name = diagnostic_operation_name(&operation);
        let is_assertion = canonical_name.starts_with("expect.")
            || canonical_name.starts_with("wait.")
            || matches!(canonical_name, "locator.wait" | "locator.resolve");
        let started_ms = self.current_session_elapsed_ms().unwrap_or(0);
        let mut metadata = OperationMetadata {
            sequence: 0,
            interrupt_generation,
            name: name.clone(),
            timeout_ms: operation_timeout(&operation),
            started_at: Instant::now(),
            started_ms,
            screen_before,
            safe_summary: safe_operation_summary(&operation),
            is_assertion,
            expectation: OperationExpectation::capture(&operation),
            input: InputDetails::capture(&operation),
        };
        let pending = self
            .operation_history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .begin(
                name,
                started_ms,
                screen_before,
                metadata.safe_summary.clone(),
                is_assertion,
                metadata.expectation.clone(),
            );
        metadata.sequence = pending.sequence();
        let mut result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result = self.execute_inner(operation, &context, &mut metadata, &cancelled);
            if waiting && cancelled() {
                let message = "operation was cancelled";
                Err(
                    TermprobeError::assertion(message).with_report(FailureReport::new(
                        metadata.name.clone(),
                        metadata.timeout_ms,
                        FailureReason::Cancelled,
                        message,
                    )),
                )
            } else {
                result
            }
        }))
        .unwrap_or_else(|payload| {
            Err(TermprobeError::internal(format!(
                "native terminal operation panicked: {}",
                panic_message(payload.as_ref())
            )))
        });
        let failed = result
            .as_ref()
            .is_err_and(|error| matches!(error.kind, ErrorKind::Assertion | ErrorKind::Internal));
        if failed {
            self.recording
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .failed = true;
        }
        if let Err(error) = &mut result {
            self.prepare_failure_observation(error);
        }
        let pin_checkpoint = is_assertion
            || (metadata.input.is_some()
                && self
                    .trace
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .options
                    .mode
                    != TraceMode::Off);
        let screen_at_return = result
            .as_ref()
            .err()
            .and_then(|error| error.observation.as_deref())
            .map_or_else(
                || self.capture_current_screen_sequence(true, pin_checkpoint),
                |observation| observation.screen_sequence,
            );
        let result_name = match &result {
            Ok(_) => "ok",
            Err(error) => error.kind.as_str(),
        };
        // Failure enrichment takes the session lock before the history lock.
        let ended_ms = result
            .as_ref()
            .err()
            .and_then(|error| error.observation.as_deref())
            .map_or_else(
                || self.current_session_elapsed_ms(),
                |observation| Some(observation.captured_ms),
            );
        self.operation_history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .finish(
                pending,
                ended_ms,
                screen_at_return,
                result_name,
                metadata.input.clone(),
            );
        if let Err(error) = &mut result {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.finalize_failure(error, &context, &metadata);
            }));
            error.observation = None;
            error.report = None;
        }
        result
    }

    fn execute_inner(
        &self,
        operation: Operation,
        context: &ExecutionContext,
        metadata: &mut OperationMetadata,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<OperationResult, TermprobeError> {
        match operation {
            Operation::FinishTrace { failed } => {
                self.finish_trace(context, Some(failed))?;
                Ok(OperationResult::Unit)
            }
            Operation::Open(options) => self
                .open(options, context, metadata)
                .map(OperationResult::Open),
            Operation::Run(options) => self
                .run(options, context, metadata)
                .map(OperationResult::Open),
            Operation::Restart {
                graceful_timeout_ms,
            } => self
                .restart(graceful_timeout_ms, context, metadata)
                .map(OperationResult::Open),
            Operation::Close => {
                let trace_result = self.finish_trace(context, None);
                *self
                    .live
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
                *self
                    .interrupt
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
                if let Some(session) = self.write_session().take() {
                    session.kill();
                    drop(session);
                }
                *self
                    .spawn_spec
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
                self.cleanup_recording();
                self.trace
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .pending_startup_outcome = false;
                trace_result?;
                Ok(OperationResult::Unit)
            }
            Operation::Resize { cols, rows } => {
                let result = self.with_session(|session| {
                    dispatch(
                        session,
                        Operation::Resize { cols, rows },
                        &mut metadata.input,
                        cancelled,
                    )
                });
                if result.is_ok() {
                    if let Some(spec) = self
                        .spawn_spec
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .as_mut()
                    {
                        spec.resize(cols, rows);
                    }
                }
                result
            }
            other => self
                .with_session(|session| dispatch(session, other, &mut metadata.input, cancelled)),
        }
    }

    fn open(
        &self,
        options: OpenOptions,
        context: &ExecutionContext,
        metadata: &OperationMetadata,
    ) -> Result<OpenResult, TermprobeError> {
        self.spawn(
            SpawnSpec {
                command: SpawnCommand::Open(options),
                resolved_cwd: None,
                retention: context.retention,
                trace: context.trace.clone(),
            },
            context,
            metadata,
        )
    }

    fn run(
        &self,
        options: RunOptions,
        context: &ExecutionContext,
        metadata: &OperationMetadata,
    ) -> Result<OpenResult, TermprobeError> {
        self.spawn(
            SpawnSpec {
                command: SpawnCommand::Run(options),
                resolved_cwd: None,
                retention: context.retention,
                trace: context.trace.clone(),
            },
            context,
            metadata,
        )
    }

    fn restart(
        &self,
        graceful_timeout_ms: u64,
        context: &ExecutionContext,
        metadata: &OperationMetadata,
    ) -> Result<OpenResult, TermprobeError> {
        let spec = self
            .spawn_spec
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .ok_or_else(TermprobeError::no_restart_metadata)?;

        if let Some(session) = self.lock_session().as_ref() {
            if session.is_alive()? {
                if let Err(error) = session.pty.signal("INT") {
                    self.logger
                        .event(&format!("restart interrupt failed error={error}"));
                }
                let start = Instant::now();
                let timeout = Duration::from_millis(graceful_timeout_ms);
                while session.is_alive()? && start.elapsed() < timeout {
                    std::thread::sleep(Duration::from_millis(POLL_DELAY_MS));
                }
            }
        }

        let mut context = context.clone();
        if context.trace.is_none() {
            context.trace = spec.trace.clone();
        }
        self.spawn(spec.restart(), &context, metadata)
    }

    fn spawn(
        &self,
        mut spec: SpawnSpec,
        context: &ExecutionContext,
        metadata: &OperationMetadata,
    ) -> Result<OpenResult, TermprobeError> {
        let diagnostics = spec.retention;
        let (
            shell,
            program,
            backend,
            profile,
            style,
            cols,
            rows,
            cwd,
            env,
            wait_ready,
            restart,
            timeouts,
            mut recording,
        ) = match &spec.command {
            SpawnCommand::Open(options) => (
                options.shell,
                None,
                options.backend,
                options.profile,
                options.style.clone(),
                options.cols,
                options.rows,
                options.cwd.clone(),
                options.env.clone(),
                options.wait_ready,
                options.restart,
                options.timeouts,
                options.recording.clone(),
            ),
            SpawnCommand::Run(options) => {
                let mut program = Vec::with_capacity(options.args.len() + 1);
                program.push(options.program.clone());
                program.extend(options.args.clone());
                (
                    None,
                    Some(program),
                    options.backend,
                    options.profile,
                    options.style.clone(),
                    options.cols,
                    options.rows,
                    options.cwd.clone(),
                    options.env.clone(),
                    options.wait_ready,
                    options.restart,
                    options.timeouts,
                    options.recording.clone(),
                )
            }
        };
        style.validate().map_err(TermprobeError::usage)?;
        crate::terminal::pty::validate_size(cols, rows)?;
        let cwd = match &spec.resolved_cwd {
            Some(cwd) => cwd.clone(),
            None => std::path::absolute(cwd.as_deref().unwrap_or(".")).map_err(|error| {
                TermprobeError::usage(format!("failed to resolve session cwd: {error}"))
            })?,
        };
        crate::terminal::pty::validate_cwd(&cwd)?;
        spec.resolved_cwd = Some(cwd.clone());
        recording.validate()?;
        let mut trace_options = context.trace.clone().unwrap_or_default();
        trace_options.directory =
            std::path::absolute(&trace_options.directory).map_err(|error| {
                TermprobeError::internal(format!("failed to resolve trace directory: {error}"))
            })?;
        spec.trace = context.trace.as_ref().map(|_| trace_options.clone());
        if context.trace.is_some() {
            recording.mode = match trace_options.mode {
                TraceMode::Off => AutomaticRecordingMode::Disabled,
                TraceMode::On => AutomaticRecordingMode::Always,
                TraceMode::OnFailure => AutomaticRecordingMode::OnFailure,
            };
        } else if context
            .artifact
            .as_ref()
            .is_some_and(|artifact| artifact.include_recording)
            && recording.mode == AutomaticRecordingMode::Disabled
        {
            recording.mode = AutomaticRecordingMode::OnFailure;
        }
        match &mut spec.command {
            SpawnCommand::Open(options) => options.recording = recording.clone(),
            SpawnCommand::Run(options) => options.recording = recording.clone(),
        }
        diagnostics.validate().map_err(TermprobeError::usage)?;
        let mut current = self.write_session();
        if let Some(previous) = current.as_ref() {
            if !restart && previous.is_alive()? {
                return Ok(OpenResult {
                    shell_pid: previous.pid(),
                    session: self.name.clone(),
                    ready: previous.is_ready(),
                    recording: self.recording_path_string(),
                });
            }
        }
        let recording_required = recording.directory.is_some();
        let recording_path = self.resolve_recording_path(&recording)?;

        *self
            .live
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        *self
            .interrupt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        if let Some(previous) = current.take() {
            self.finish_trace_with_session(&previous, context, None)?;
            previous.kill();
            drop(previous);
        }
        drop(current);
        self.operation_history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reset_session();
        self.discard_recording();
        *self
            .trace
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = TraceState {
            options: trace_options,
            artifact: None,
            context: context.sanitized_context(),
            owned_directories: Vec::new(),
            pending_startup_outcome: false,
        };
        *self
            .recording
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = RecordingState {
            path: None,
            mode: recording.mode,
            failed: false,
        };
        let session = TerminalSession::open(
            shell,
            program.clone(),
            backend,
            profile,
            style,
            cols,
            rows,
            Some(cwd),
            env,
            timeouts,
            diagnostics,
            self.logger.clone(),
            recording_path.clone(),
            recording_required,
        )
        .map_err(|error| TermprobeError::internal(format!("failed to open session: {error}")))?;
        {
            let mut interrupt = self
                .interrupt
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if self.interrupt_generation.load(Ordering::Acquire) != metadata.interrupt_generation {
                session.cancelled.store(true, Ordering::Release);
            }
            *interrupt = Some(InterruptTarget {
                pty: session.pty.clone(),
                cancelled: session.cancelled.clone(),
            });
        }
        if session.cancelled.load(Ordering::Acquire) {
            session.pty.kill();
        }
        self.recording
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .path = session
            .automatic_recording_enabled()
            .then_some(recording_path)
            .flatten();

        let shell_pid = session.pid();
        let ready_timeout = open_ready_timeout(&session);
        let ready = if wait_ready.unwrap_or(program.is_none()) {
            await_ready(&session, ready_timeout, &|| false)
        } else {
            session.is_ready()
        };
        let cancelled = session.cancelled.load(Ordering::Acquire);
        if cancelled || (wait_ready == Some(true) && !ready) {
            let mut error = if cancelled {
                TermprobeError::assertion("open: session startup was cancelled")
            } else {
                startup_readiness_error(&metadata.name, ready_timeout)
            };
            error.observation = Some(Box::new(capture_failure_observation(&session)));
            let metadata = OperationMetadata {
                started_ms: 0,
                screen_before: 0,
                ..metadata.clone()
            };
            self.finalize_failure_with_session(
                &mut error,
                context,
                &metadata,
                Some(&session),
                true,
            );
            self.trace
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .pending_startup_outcome = true;
            *self
                .interrupt
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            session.kill();
            return Err(error);
        }
        let live = LiveTarget {
            state: session.state.clone(),
            viewport_input: session.viewport_input.clone(),
            pty: session.pty.clone(),
            shell: session.shell.map(|value| value.as_str()),
        };
        *self.write_session() = Some(session);
        *self
            .live
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(live);
        *self
            .spawn_spec
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(spec);
        Ok(OpenResult {
            shell_pid,
            session: self.name.clone(),
            ready,
            recording: self.recording_path_string(),
        })
    }

    fn with_session<F>(&self, operation: F) -> Result<OperationResult, TermprobeError>
    where
        F: FnOnce(&TerminalSession) -> Result<OperationResult, TermprobeError>,
    {
        let guard = self.lock_session();
        let session = guard.as_ref().ok_or_else(TermprobeError::no_session)?;
        // The emulator is fed on the reader thread, where there is nobody to
        // return an error to, so a backend that failed to parse records it and
        // the next operation reports it. Checked before the operation runs:
        // once the grid has stopped tracking the bytes, every answer read out
        // of it is a guess, and a wrong answer is worse than a failure.
        if let Some(fault) = session.fault() {
            return Err(
                TermprobeError::internal(fault.clone()).with_report(FailureReport::new(
                    "terminal.operation",
                    None,
                    FailureReason::EmulatorFault,
                    fault,
                )),
            );
        }
        operation(session)
    }

    fn capture_current_screen_sequence(&self, force: bool, pin: bool) -> u64 {
        let guard = self.lock_session();
        let Some(session) = guard.as_ref() else {
            return 0;
        };
        let mut state = session
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let sequence = try_capture_visual_state(&mut state, force).unwrap_or(0);
        if pin && sequence != 0 {
            state.screen_history.pin_current();
        }
        sequence
    }

    fn current_session_elapsed_ms(&self) -> Option<u64> {
        let guard = self.lock_session();
        guard.as_ref().map(|session| {
            let state = session
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            elapsed_ms(state.started_at)
        })
    }

    fn prepare_failure_observation(&self, error: &mut TermprobeError) {
        if error.observation.is_some()
            || !matches!(error.kind, ErrorKind::Assertion | ErrorKind::Internal)
        {
            return;
        }
        let guard = self.lock_session();
        if let Some(session) = guard.as_ref() {
            error.observation = safe_capture_failure_observation(session).map(Box::new);
        }
    }

    fn finalize_failure(
        &self,
        error: &mut TermprobeError,
        context: &ExecutionContext,
        metadata: &OperationMetadata,
    ) {
        if !matches!(error.kind, ErrorKind::Assertion | ErrorKind::Internal) {
            return;
        }
        if error.details.is_some() {
            return;
        }
        let guard = self.lock_session();
        self.finalize_failure_with_session(error, context, metadata, guard.as_ref(), false);
    }

    fn finalize_failure_with_session(
        &self,
        error: &mut TermprobeError,
        context: &ExecutionContext,
        metadata: &OperationMetadata,
        session: Option<&TerminalSession>,
        include_pending_operation: bool,
    ) {
        if !matches!(error.kind, ErrorKind::Assertion | ErrorKind::Internal) {
            return;
        }
        if error.details.is_some() {
            return;
        }
        let captured = if error.observation.is_none() {
            session.and_then(safe_capture_failure_observation)
        } else {
            None
        };
        let observation = error.observation.as_deref().or(captured.as_ref());
        let (summary, summary_truncated) =
            truncate_diagnostic_value(base_error_message(&error.message), 64 * 1024);
        let mut details = FailureReport::new(
            metadata.name.clone(),
            metadata.timeout_ms,
            failure_reason(error, observation),
            summary,
        );
        details.truncated = summary_truncated;
        details.operation.elapsed_ms = metadata.started_at.elapsed().as_millis() as u64;
        details.operation.started_screen_sequence = metadata.screen_before;
        details.operation.failed_screen_sequence = observation
            .as_ref()
            .map_or(0, |value| value.screen_sequence);
        if let Some(existing) = error.report.take() {
            merge_failure_details(&mut details, *existing);
        }
        details.truncated |= details
            .locator
            .as_ref()
            .is_some_and(|locator| locator.stages_truncated);
        let trace = self
            .trace
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .options
            .clone();
        let trace_artifact = (trace.mode != TraceMode::Off).then(|| trace.artifact_options());
        if !context
            .artifact
            .iter()
            .chain(trace_artifact.iter())
            .any(|options| options.mode != crate::diagnostics::FailureArtifactMode::None)
        {
            let failure = details.failure_details();
            error.message = failure.summary.clone();
            error.details = Some(Box::new(failure));
            return;
        }
        details.context = self
            .trace
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .diagnostic_context(context);
        details.recent_operations = self
            .operation_history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .snapshot();
        if include_pending_operation {
            details.recent_operations.push(
                metadata.pending_event(
                    error.kind.as_str(),
                    observation
                        .as_ref()
                        .map_or(metadata.screen_before, |value| value.screen_sequence),
                ),
            );
        }

        details.truncated |= details.recent_operations.iter().any(|event| {
            matches!(
                event.expectation,
                Some(OperationExpectation::Unavailable { .. })
            ) || event
                .input
                .as_ref()
                .is_some_and(InputDetails::is_unavailable)
        });
        if let Some(observation) = &observation {
            details.terminal = Some(observation.terminal());
            details.process = Some(observation.process.clone());
            details.runtime = Some(RuntimeDiagnostics {
                session_name: Some(self.name.clone()),
                ..observation.runtime.clone()
            });
            details.recording = Some(self.recording_diagnostics(observation));
        }
        details.hints = diagnostic_hints(&details);

        details.finish_signature();

        let mut exports_truncated = false;
        for (is_trace, options) in [
            (false, context.artifact.as_ref()),
            (true, trace_artifact.as_ref()),
        ] {
            let (Some(options), Some(observation)) = (options, observation) else {
                continue;
            };
            if options.mode != crate::diagnostics::FailureArtifactMode::None {
                let mut export_details = details.clone();
                export_details.outcome = is_trace.then_some(TraceOutcome::Failed);
                let allocated = if is_trace {
                    allocate_trace_directory(&options.directory)
                } else {
                    allocate_artifact_directory(&options.directory)
                };
                let mut owned_directory = None;
                let artifact = match allocated {
                    Ok(directory) => {
                        if is_trace {
                            owned_directory = Some(directory.clone());
                        }
                        let prepared_recording = if options.include_recording {
                            session.and_then(|session| {
                                self.prepare_recording_artifact(
                                    session,
                                    observation,
                                    &directory,
                                    &mut export_details,
                                )
                            })
                        } else {
                            None
                        };
                        write_failure_artifact(
                            options,
                            ArtifactInputs {
                                details: &mut export_details,
                                observation,
                                recording: prepared_recording,
                            },
                            directory,
                        )
                    }
                    Err(error) => FailureArtifactRef {
                        status: FailureArtifactStatus::Failed,
                        directory: options.directory.to_string_lossy().into_owned(),
                        manifest: None,
                        report: None,
                        report_html: None,
                        timeline: None,
                        screen_text: None,
                        screen_svg: None,
                        recording: None,
                        errors: vec![format!(
                            "failed to allocate failure artifact directory: {error}"
                        )],
                    },
                };
                if is_trace {
                    let mut trace = self
                        .trace
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    trace.artifact = Some(artifact.clone());
                    trace.owned_directories.extend(owned_directory);
                }
                exports_truncated |= export_details.truncated;
                if is_trace && artifact.status != FailureArtifactStatus::Written {
                    if let Some(primary) = error.artifact.as_mut() {
                        if primary.status == FailureArtifactStatus::Written {
                            primary.status = FailureArtifactStatus::Partial;
                        }
                        primary.errors.push(format!(
                            "trace export was not fully written at {}",
                            artifact.directory
                        ));
                        primary.errors.extend(
                            artifact
                                .errors
                                .iter()
                                .map(|message| format!("trace: {message}")),
                        );
