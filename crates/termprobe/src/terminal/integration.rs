//! Command and output tracking from the PTY byte stream.

use alacritty_terminal::vte::{Parser, Perform};

const OSC_CMD: &[u8] = b"133";
const OSC_VSCODE: &[u8] = b"633";
const OSC_CWD: &[u8] = b"7";

#[derive(Default, PartialEq, Clone, Copy)]
enum Region {
    #[default]
    None,
    Command,
    Output,
}

#[derive(Default)]
struct TrackerState {
    region: Region,
    command_buf: String,
    output_buf: String,

    started: bool,
    prompt_active: bool,
    last_exit: Option<i32>,
    cwd: Option<String>,
    last_command: Option<String>,
    last_output: Option<String>,
    started_count: u64,
    finished_count: u64,
}

/// Tracks command boundaries, exit codes, and cwd from the PTY byte stream.
pub struct CommandTracker {
    parser: Parser,
    state: TrackerState,
}

impl Default for CommandTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandTracker {
    pub fn new() -> Self {
        CommandTracker {
            parser: Parser::new(),
            state: TrackerState::default(),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.state, bytes);
    }

    /// Whether the shell is sitting at an idle, ready prompt.
    pub fn is_ready(&self) -> bool {
        self.state.prompt_active
    }

    /// Whether at least one prompt has been seen.
    pub fn started(&self) -> bool {
        self.state.started
    }

    pub fn started_count(&self) -> u64 {
        self.state.started_count
    }

    pub fn executing(&self) -> bool {
        self.state.region == Region::Output
    }

    pub fn finished_count(&self) -> u64 {
        self.state.finished_count
    }

    /// The last reported command status, cleared when a new command starts.
    pub fn last_exit(&self) -> Option<i32> {
        self.state.last_exit
    }

    pub fn cwd(&self) -> Option<&str> {
        self.state.cwd.as_deref()
