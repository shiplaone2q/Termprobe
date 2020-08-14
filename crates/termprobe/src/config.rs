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
