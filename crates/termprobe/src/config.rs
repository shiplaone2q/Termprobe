use std::path::PathBuf;

pub const DEFAULT_COLS: u16 = 80;
pub const DEFAULT_ROWS: u16 = 30;
pub const POLL_DELAY_MS: u64 = 50;
/// Cap for `open`'s implicit prompt wait when no `ready` budget is configured.
pub const OPEN_READY_CAP_MS: u64 = 8_000;

/// The kind of thing a wait or assertion is blocking on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutClass {
    /// Text queries, actions, and assertions.
    Text,
    /// The screen going quiet: `wait idle`.
    Idle,
    /// A foreground command finishing: `wait command`, `expect exit-code`.
    Command,
    /// The session's program exiting: `wait exit`.
    Exit,
    /// The shell reporting a prompt: `wait ready`, and `open`'s implicit wait.
    Ready,
}

impl TimeoutClass {
    /// The built-in budget when nothing else is configured.
    pub fn built_in_ms(self) -> u64 {
        match self {
            TimeoutClass::Text | TimeoutClass::Idle => 5_000,
            TimeoutClass::Command | TimeoutClass::Exit | TimeoutClass::Ready => 30_000,
        }
    }

    /// The environment variable consulted for this class.
    pub fn env_var(self) -> &'static str {
        match self {
            TimeoutClass::Text => "TERMPROBE__TIMEOUT_TEXT_MS",
            TimeoutClass::Idle => "TERMPROBE__TIMEOUT_IDLE_MS",
            TimeoutClass::Command => "TERMPROBE__TIMEOUT_COMMAND_MS",
            TimeoutClass::Exit => "TERMPROBE__TIMEOUT_EXIT_MS",
            TimeoutClass::Ready => "TERMPROBE__TIMEOUT_READY_MS",
        }
    }

    /// The environment override, read at call time so tests can vary it.
