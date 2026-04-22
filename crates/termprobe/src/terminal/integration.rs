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
    }

    pub fn last_command(&self) -> Option<&str> {
        self.state.last_command.as_deref()
    }

    pub fn last_output(&self) -> Option<&str> {
        self.state.last_output.as_deref()
    }
}

impl TrackerState {
    fn command_marker(&mut self, marker: &str, exit: Option<&str>) {
        match marker {
            "A" => {
                self.started = true;
                self.prompt_active = false;
                self.region = Region::None;
            }
            "B" => {
                self.started = true;
                self.prompt_active = true;
                self.region = Region::Command;
                self.command_buf.clear();
            }
            "C" => {
                self.prompt_active = false;
                self.last_exit = None;
                let cmd = clean(&self.command_buf);
                let cmd = cmd.strip_prefix("> ").unwrap_or(&cmd).to_string();
                self.last_command = Some(cmd);
                self.region = Region::Output;
                self.output_buf.clear();
                self.started_count += 1;
            }
            "D" => {
                self.last_output = Some(clean(&self.output_buf));
                self.prompt_active = false;
                self.region = Region::None;
                self.last_exit = exit.and_then(|s| s.trim().parse::<i32>().ok());
                self.finished_count += 1;
            }
            _ => {}
        }
    }
}
