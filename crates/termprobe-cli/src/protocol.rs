use serde::{Deserialize, Serialize};
use serde_json::json;

use termprobe::{
    AutomaticRecording, Backend, ClipboardPattern, DiagnosticRetentionOptions, Engine,
    ExecutionContext, FailureArtifactRef, FailureDetails, KeyAction, LocatorQuery, MouseOptions,
    OpenOptions, Operation, OperationResult, RecordingFormat, RunOptions, ScreenshotResult,
    TermprobeError,
};

pub use termprobe::{ErrorKind, MouseAction, Timeouts};

pub const PROTOCOL_VERSION: u32 = 7;

#[derive(Serialize, Deserialize)]
#[serde(remote = "termprobe::ScrollTarget")]
enum ScrollTarget {
    Lines(i32),
    Top,
    Bottom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    Scroll {
        #[serde(with = "ScrollTarget")]
        target: termprobe::ScrollTarget,
    },
    WithContext {
        request: Box<Request>,
        context: ExecutionContext,
    },
    Ping,
    Open {
        shell: Option<termprobe::shell::Shell>,
        program: Option<Vec<String>>,
        #[serde(default)]
        backend: Backend,
        /// Terminal settings, already resolved from the config file by the
        /// client. The daemon never reads that file: it is long-lived and
        /// shared, so it has no single working directory to resolve a
        /// project-local config against.
        #[serde(default)]
        profile: termprobe::profile::Profile,
        /// How this session's screenshots and recordings are drawn, resolved
        /// from the config file by the client for the same reason.
        #[serde(default)]
        style: Box<termprobe::render::style::Style>,
        cols: u16,
        rows: u16,
        cwd: Option<String>,
        env: Vec<(String, String)>,
        #[serde(default)]
        wait_ready: Option<bool>,
        #[serde(default)]
        restart: bool,
        #[serde(default)]
        timeouts: Timeouts,
        #[serde(default)]
        recording: Box<AutomaticRecording>,
        #[serde(default)]
        trace: termprobe::TraceOptions,
        #[serde(default)]
        diagnostics: DiagnosticRetentionOptions,
    },
    Restart {
        graceful_timeout_ms: u64,
    },
    Close,
    Status,
    State,
    Text {
        full: bool,
    },
    Cells {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
    },
    Get {
        field: GetField,
    },
    Write {
        data: String,
    },
    Submit {
        data: Option<String>,
    },
    Key {
        action: KeyAction,
        keys: Vec<String>,
    },
    /// Backward-compatible request from older clients.
    Press {
        keys: Vec<String>,
    },
    Mouse {
        action: MouseAction,
    },
    Resize {
        cols: u16,
        rows: u16,
    },
    Signal {
        name: String,
    },
    WaitTitle {
        text: String,
        regex: bool,
        #[serde(default)]
        timeout_ms: Option<u64>,
        not: bool,
    },
    WaitClipboard {
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        regex: bool,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    WaitIdle {
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    WaitCommand {
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    WaitExit {
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    WaitReady {
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    WaitBell {
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    FindLocator {
        query: LocatorQuery,
    },
    ClickLocator {
        query: LocatorQuery,
        button: u8,
        clicks: u8,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    HighlightLocator {
        query: LocatorQuery,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    ExpectLocator {
        query: LocatorQuery,
        not: bool,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    ExpectTitle {
        text: String,
        regex: bool,
        not: bool,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    ExpectExitCode {
        code: i32,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    ExpectOutput {
        text: String,
        regex: bool,
    },
    ExpectMode {
        mode: String,
        enabled: bool,
        timeout_ms: Option<u64>,
    },
    ExpectColors {
        foreground: Option<String>,
        background: Option<String>,
        cursor: Option<String>,
        #[serde(default)]
        palette: Vec<(u8, String)>,
        timeout_ms: Option<u64>,
    },
    ExpectCursor {
        visible: Option<bool>,
        shape: Option<String>,
        x: Option<u16>,
        y: Option<u16>,
        timeout_ms: Option<u64>,
    },
    ExpectBellCount {
        count: u64,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    Snapshot {
        name: String,
        update: bool,
        include_style: bool,
        #[serde(default)]
        include_title: bool,
        #[serde(default)]
        cwd: Option<String>,
    },
    Screenshot {
        full: bool,
        path: Option<String>,
        #[serde(default)]
        zoom: Option<f64>,
        #[serde(default)]
        background: Option<termprobe::CaptureBackground>,
    },
    StartRecording {
        path: String,
        format: Option<RecordingFormat>,
        fps: Option<u8>,
        speed: Option<f64>,
        idle_time_limit: Option<f64>,
        #[serde(default)]
        zoom: Option<f64>,
        #[serde(default)]
        background: Option<termprobe::CaptureBackground>,
    },
    StopRecording,
    FlushRecording,
    Monitor {
        cols: u16,
        rows: u16,
        #[serde(default)]
        interactive: bool,
    },
    MonitorInputStream {
        cols: u16,
        rows: u16,
    },
    Shutdown,
}

impl Request {
    pub fn execute(self, engine: &Engine) -> Response {
        self.execute_cancellable(engine, &|| false)
    }

    pub fn execute_cancellable(self, engine: &Engine, cancelled: &dyn Fn() -> bool) -> Response {
        if let Request::WithContext {
            request,
            mut context,
        } = self
        {
            if context.operation_name.is_none() {
                context.operation_name = Some(request.diagnostic_operation_name().to_string());
            }
            let request_retention = request.diagnostic_retention();
            if context.trace.is_none() {
