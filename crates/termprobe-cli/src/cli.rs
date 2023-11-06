use clap::{Args, Parser, Subcommand};

use termprobe::config::{DEFAULT_COLS, DEFAULT_ROWS};
use termprobe::shell::Shell;
use termprobe::{Backend, MouseButton, MouseOptions, RecordingFormat, Timeouts};

#[derive(Clone, Copy, clap::ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum BackendArg {
    Alacritty,
    Ghostty,
    Rio,
    Xtermjs,
}

impl From<BackendArg> for Backend {
    fn from(backend: BackendArg) -> Self {
        match backend {
            BackendArg::Alacritty => Backend::Alacritty,
            BackendArg::Ghostty => Backend::Ghostty,
            BackendArg::Rio => Backend::Rio,
            BackendArg::Xtermjs => Backend::Xtermjs,
        }
    }
}

#[derive(Clone, Copy, clap::ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum ShellArg {
    Bash,
    Powershell,
    Pwsh,
    Cmd,
    Fish,
    Zsh,
    Xonsh,
    Elvish,
    Nushell,
}

impl From<ShellArg> for Shell {
    fn from(shell: ShellArg) -> Self {
        match shell {
            ShellArg::Bash => Shell::Bash,
            ShellArg::Powershell => Shell::Powershell,
            ShellArg::Pwsh => Shell::Pwsh,
            ShellArg::Cmd => Shell::Cmd,
            ShellArg::Fish => Shell::Fish,
            ShellArg::Zsh => Shell::Zsh,
            ShellArg::Xonsh => Shell::Xonsh,
            ShellArg::Elvish => Shell::Elvish,
            ShellArg::Nushell => Shell::Nushell,
        }
    }
}

/// Which terminal profile a session runs with.
#[derive(Args, Clone, Default)]
pub struct ProfileArgs {
    /// Config file to read (default: ./termprobe.toml, then the platform config
    /// directory, then ~/.termprobe/termprobe.toml).
    #[arg(long, value_name = "PATH")]
    pub config: Option<std::path::PathBuf>,
    /// Named profile from the config file (default: `default`).
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,
}

impl ProfileArgs {
    /// Resolve to concrete settings. Done here, in the client, because the
    /// daemon is long-lived and shared and so has no working directory to
    /// resolve a project-local config against.
    pub fn resolve(&self) -> anyhow::Result<termprobe::profile::Settings> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        termprobe::profile::resolve_settings(self.config.as_deref(), self.profile.as_deref(), &cwd)
    }
}

#[derive(Clone, Copy, clap::ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum RecordingFormatArg {
    Apng,
    Gif,
    Mp4,
    Cast,
}

impl From<RecordingFormatArg> for RecordingFormat {
    fn from(format: RecordingFormatArg) -> Self {
        match format {
            RecordingFormatArg::Apng => RecordingFormat::Apng,
            RecordingFormatArg::Gif => RecordingFormat::Gif,
            RecordingFormatArg::Mp4 => RecordingFormat::Mp4,
            RecordingFormatArg::Cast => RecordingFormat::Cast,
        }
    }
}

/// Per-class default timeouts for a session, in milliseconds.
#[derive(Args, Clone, Copy, Default)]
pub struct TimeoutArgs {
    /// Default timeout for text actions and assertions (default 5000).
    #[arg(long = "timeout-text", value_name = "MS")]
    pub text: Option<u64>,
    /// Default timeout for `wait idle` (default 5000).
    #[arg(long = "timeout-idle", value_name = "MS")]
    pub idle: Option<u64>,
    /// Default timeout for `wait command` / `expect exit-code` (default 30000).
    #[arg(long = "timeout-command", value_name = "MS")]
    pub command: Option<u64>,
    /// Default timeout for `wait exit` (default 30000).
    #[arg(long = "timeout-exit", value_name = "MS")]
    pub exit: Option<u64>,
    /// Default timeout for `wait ready` (default 30000), and for the prompt
    /// wait `open` performs — which otherwise caps itself at 8000.
    #[arg(long = "timeout-ready", value_name = "MS")]
    pub ready: Option<u64>,
}

impl From<TimeoutArgs> for Timeouts {
    fn from(args: TimeoutArgs) -> Self {
        Timeouts {
            text: args.text,
            idle: args.idle,
            command: args.command,
            exit: args.exit,
            ready: args.ready,
        }
    }
}

#[derive(Args, Clone, Copy)]
pub struct DiagnosticRetentionArgs {
    /// Number of distinct recent screens retained for failure diagnostics.
    #[arg(long, value_name = "COUNT")]
    pub screen_history_limit: Option<u16>,
}

#[derive(Parser)]
#[command(name = "termprobe", version, about = "Headless terminal cli + daemon")]
pub struct Cli {
    /// Target a named session (env: TERMPROBE__SESSION).
    #[arg(long, global = true)]
    pub session: Option<String>,

    /// Emit machine-readable JSON.
    #[arg(long, global = true)]
    pub json: bool,

    /// Start the daemon with a verbose data log at ~/.termprobe/<session>.log.
    /// Records all PTY input/output. Only takes effect when the daemon starts.
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    /// Write a structured failure artifact under this directory.
    #[arg(long, global = true, value_name = "DIR")]
    pub failure_artifacts: Option<std::path::PathBuf>,

    /// Failure artifact contents.
    #[arg(
        long,
        global = true,
        value_enum,
        default_value_t = FailureArtifactModeArg::All
    )]
    pub failure_artifact_mode: FailureArtifactModeArg,

    /// Copy the automatic asciicast into failure artifacts.
    #[arg(long, global = true, requires = "failure_artifacts")]
    pub failure_artifact_recording: bool,

    /// Add a safe KEY=VALUE field to structured failure diagnostics.
    #[arg(long, global = true, value_name = "KEY=VALUE")]
    pub diagnostic_context: Vec<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum FailureArtifactModeArg {
    #[default]
    All,
    Html,
    Text,
    None,
}

impl From<FailureArtifactModeArg> for termprobe::FailureArtifactMode {
    fn from(value: FailureArtifactModeArg) -> Self {
        match value {
            FailureArtifactModeArg::All => Self::All,
            FailureArtifactModeArg::Html => Self::Html,
            FailureArtifactModeArg::Text => Self::Text,
            FailureArtifactModeArg::None => Self::None,
        }
    }
}

#[derive(Args)]
pub struct ScreenshotArgs {
    /// Write an SVG or PNG image to this path (alias for --out).
    pub path: Option<String>,
    /// Write an SVG or PNG image to this path.
    #[arg(short, long)]
    pub out: Option<String>,
    /// Include scrollback, not just the visible viewport.
    #[arg(long)]
    pub full: bool,
    /// Scale image dimensions while keeping the same terminal cells.
    #[arg(long)]
    pub zoom: Option<f64>,
    /// Canvas background color (#rgb or #rrggbb).
    #[arg(long, conflicts_with = "transparent")]
    pub background: Option<String>,
    /// Leave the image canvas transparent.
    #[arg(long)]
    pub transparent: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Spawn a shell session (auto-starts the daemon).
    Open {
        /// Shell to launch (defaults to the platform shell).
        #[arg(long, value_enum)]
        shell: Option<ShellArg>,
        /// Terminal emulator to use (defaults to alacritty).
        #[arg(long, value_enum)]
        backend: Option<BackendArg>,
        /// Terminal width in columns.
        #[arg(long, default_value_t = DEFAULT_COLS)]
        cols: u16,
        /// Terminal height in rows.
        #[arg(long, default_value_t = DEFAULT_ROWS)]
        rows: u16,
        /// Working directory for the session.
        #[arg(long)]
        cwd: Option<String>,
        /// Environment overrides as KEY=VALUE (repeatable).
        #[arg(long = "env")]
        env: Vec<String>,
        /// Block until the shell reports a ready prompt (the default), or
        /// return as soon as it is spawned with --no-wait-ready.
        #[arg(long)]
        wait_ready: bool,
        /// Return as soon as the shell is spawned, without waiting for a prompt.
        #[arg(long, conflicts_with = "wait_ready")]
        no_wait_ready: bool,
        /// Replace a live session instead of reusing it.
        #[arg(long, visible_alias = "force")]
        restart: bool,
        #[command(flatten)]
        profile: ProfileArgs,
        #[command(flatten)]
        timeouts: TimeoutArgs,
        #[command(flatten)]
        diagnostics: DiagnosticRetentionArgs,
    },
    /// Spawn a session running a program directly.
    Run {
        /// Program to run.
        program: String,
        /// Arguments passed to the program.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
        /// Terminal emulator to use (defaults to alacritty).
        #[arg(long, value_enum)]
        backend: Option<BackendArg>,
        /// Terminal width in columns.
        #[arg(long, default_value_t = DEFAULT_COLS)]
        cols: u16,
        /// Terminal height in rows.
        #[arg(long, default_value_t = DEFAULT_ROWS)]
        rows: u16,
        /// Working directory for the session.
        #[arg(long)]
        cwd: Option<String>,
        /// Environment overrides as KEY=VALUE (repeatable).
        #[arg(long = "env")]
        env: Vec<String>,
        /// Block until the program reports a ready prompt (off by default;
        /// only meaningful for programs with shell integration).
        #[arg(long)]
        wait_ready: bool,
        /// Return as soon as the program is spawned (the default).
        #[arg(long, conflicts_with = "wait_ready")]
        no_wait_ready: bool,
        /// Replace a live session instead of reusing it.
        #[arg(long, visible_alias = "force")]
        restart: bool,
        #[command(flatten)]
        profile: ProfileArgs,
        #[command(flatten)]
        timeouts: TimeoutArgs,
        #[command(flatten)]
        diagnostics: DiagnosticRetentionArgs,
    },
    /// Gracefully stop and recreate the session from its last successful open or run.
    Restart {
        /// Time to wait after Ctrl-C/SIGINT before forcibly killing the child.
        #[arg(long, value_name = "MS", default_value_t = 5_000)]
        graceful_timeout: u64,
    },
    /// Close the current session (or all sessions).
    Close {
        /// Close every session, not just the current one.
        #[arg(long)]
        all: bool,
    },
    /// List active sessions.
    Sessions,
    /// Start, inspect, or stop a session's daemon.
    Daemon {
        #[command(subcommand)]
        cmd: DaemonCmd,
    },
    /// Print cwd, size, cursor, last command/exit, and a text snapshot.
    State,
    /// Print the terminal text.
    Text {
        /// Include scrollback, not just the visible viewport.
        #[arg(long)]
        full: bool,
    },
    /// Capture a screenshot: terminal text to stdout, or a full-color SVG/PNG
    /// image selected by the output extension (SVG is the default).
    Screenshot(ScreenshotArgs),
    /// Start or stop an animated terminal recording.
    Record {
        #[command(subcommand)]
        cmd: RecordCmd,
    },
    /// Dump cell attributes for a region.
    Cells {
        /// Left column, 0-based.
        x: u16,
        /// Top row, 0-based.
        y: u16,
        /// Width in cells.
        #[arg(default_value_t = 1)]
        w: u16,
        /// Height in cells.
        #[arg(default_value_t = 1)]
        h: u16,
    },
    /// Get a structured field.
    Get {
        /// Field to print.
        #[arg(value_enum)]
        field: GetArg,
    },
    /// Type literal text.
    Type {
        /// Literal text to type.
        text: String,
    },
    /// Type text then submit with the shell return key.
    Submit {
        /// Text to type before the return key (optional).
        text: Option<String>,
    },
    /// Keyboard input.
    Key {
        #[command(subcommand)]
        action: KeyCmd,
    },
    /// Compatibility alias for `key press`.
    #[command(hide = true)]
    Press {
        /// Key names or combos to press in sequence.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        keys: Vec<String>,
    },
    /// Mouse control.
    Mouse {
        #[command(subcommand)]
        action: MouseCmd,
    },
    /// Move the native history viewport without sending mouse input.
    Scroll {
        #[command(subcommand)]
        target: ScrollCmd,
    },
    /// Resize the PTY and emulator.
    Resize {
        /// New width in columns.
        cols: u16,
        /// New height in rows.
        rows: u16,
    },
    /// Write raw bytes (no return key).
    Write {
        /// Raw bytes to write.
        data: String,
    },
    /// Send a signal to the session's child process.
    Signal {
        /// Signal to send.
        #[arg(value_enum)]
        name: SignalArg,
    },
    /// Kill the session's child process.
    Kill,
    /// Block until the title changes, the screen is idle, a command finishes,
    /// the shell is ready, a bell rings, or the session exits.
    Wait {
        #[command(subcommand)]
        what: WaitCmd,
    },
    /// Assert a condition (exit 0 pass / 1 fail).
    Expect {
        #[command(subcommand)]
        what: ExpectCmd,
    },
    /// Find current terminal matches and return their locations.
    Find {
        #[command(subcommand)]
        what: FindCmd,
    },
    /// Wait for a text match, then click its middle cell.
    Click {
        #[command(subcommand)]
        what: ClickCmd,
    },
    /// Highlight text matching the query and optional style constraints.
    Highlight {
        #[command(subcommand)]
        what: HighlightCmd,
    },
    /// Print the session's recording (asciinema v2 cast) to stdout.
    ///
    /// Redirect to a `.cast` file for playback in the asciicast ecosystem.
    GetRecording {
        /// Session to read (defaults to --session / the default session).
        session: Option<String>,
        /// Config file used to resolve a custom recording directory.
        #[arg(long, value_name = "PATH")]
        config: Option<std::path::PathBuf>,
    },
    /// Watch a session live in another terminal (full-color, framed).
    ///
    /// Takes over an alternate screen and streams the session as the agent
    /// drives it. By default, press `q`, `Esc`, or `Ctrl-C` to detach.
    Monitor {
        /// Forward keyboard and paste input to the session; press Ctrl+] to detach.
        #[arg(long)]
        interactive: bool,
    },
    /// Print a compact command cheatsheet for agents.
    Usage,
    /// Print a machine-readable description of the full cli surface (JSON).
    ///
    /// Versioned via `schema_version`; lists every command, flag, type, enum,
    /// default, and the exit-code taxonomy. Generated from the cli definition.
    AgentContext,
    /// Print the complete agent guide or install its routed, multi-file skill.
    Skill {
        /// Interactively install the skill by choosing its scope and agent directory.
        #[arg(long)]
        add: bool,
    },
    /// Internal: run the session daemon.
    #[command(name = "__daemon", hide = true)]
    InternalDaemon,
}

#[derive(Subcommand)]
pub enum RecordCmd {
    /// Start recording terminal output to APNG, GIF, MP4, or asciicast v2.
    Start {
        /// Output path. The extension selects APNG (.png/.apng), GIF, MP4, or cast.
        path: String,
        /// Override the format inferred from the output extension.
        #[arg(long, value_enum)]
        format: Option<RecordingFormatArg>,
        /// Maximum animation frame rate.
        #[arg(long)]
        fps: Option<u8>,
        /// Playback speed multiplier.
        #[arg(long)]
        speed: Option<f64>,
        /// Clamp idle gaps to this many seconds.
        #[arg(long)]
        idle_time_limit: Option<f64>,
        /// Scale image/video dimensions while keeping the same terminal cells.
        #[arg(long)]
        zoom: Option<f64>,
        /// Canvas background color (#rgb or #rrggbb).
        #[arg(long, conflicts_with = "transparent")]
        background: Option<String>,
        /// Leave the APNG or GIF canvas transparent.
        #[arg(long)]
        transparent: bool,
    },
    /// Stop the active recording and finish its output file.
    Stop,
}

/// Signals deliverable to a session's child process.
#[derive(Clone, Copy, clap::ValueEnum)]
#[clap(rename_all = "upper")]
pub enum SignalArg {
    /// Interrupt the foreground program (Ctrl-C).
