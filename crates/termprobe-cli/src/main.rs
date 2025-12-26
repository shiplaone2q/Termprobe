mod agent_context;
mod ansi;
mod cli;
mod config;
#[cfg(windows)]
mod console_input;
mod daemon;
mod ipc;
mod monitor;
mod monitor_input;
mod protocol;
mod skill;

use std::ffi::OsString;
use std::path::Path;
use std::time::{Duration, Instant};

use clap::{CommandFactory, Parser};

use cli::{
    Cli, ClickCmd, Command, DaemonCmd, ExpectCmd, FindCmd, GetArg, HighlightCmd, KeyCmd, MatchArg,
    MouseCmd, RecordCmd, ScreenshotArgs, ScrollCmd, TextQueryArgs, TextSelectorArgs, TextStyleArgs,
    WaitCmd, WhitespaceArg,
};
use protocol::{GetField, MouseAction, Request, Response};
use termprobe::{
    CaptureBackground, ExecutionContext, FailureArtifactOptions, LocatorDirection, LocatorQuery,
    LocatorSelector, MatchOccurrence, MouseOptions, TextAnchor, TextScope, TextSelector, TextStyle,
    WhitespaceMode,
};

/// Agent skill router, installed as `SKILL.md`.
const SKILL_MD: &str = include_str!("../../../SKILL.md");
/// Version-matched references installed beside the skill router.
const SKILL_REFERENCES: &[(&str, &str)] = &[
    (
        "references/cli.md",
        include_str!("../../../references/cli.md"),
    ),
    (
        "references/python.md",
        include_str!("../../../references/python.md"),
    ),
    (
        "references/javascript.md",
        include_str!("../../../references/javascript.md"),
    ),
    (
        "references/rust.md",
        include_str!("../../../references/rust.md"),
    ),
    (
        "references/recipes.md",
        include_str!("../../../references/recipes.md"),
    ),
];

fn main() {
    let cli = Cli::parse_from(command_line_args());
    let session = config::session_name_from_env(cli.session.clone());
    let execution_context = match build_execution_context(&cli) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    let Some(command) = cli.command else {
        let _ = Cli::command().print_help();
        std::process::exit(0);
    };

    let code = match command {
        Command::InternalDaemon => {
            if let Err(e) = daemon::run(session, cli.verbose) {
                eprintln!("daemon error: {e}");
                std::process::exit(5);
            }
            0
        }
        Command::Usage => {
            print!("{}", usage_text());
            0
        }
        Command::AgentContext => {
            println!("{}", agent_context::render());
            0
        }
        Command::Skill { add: true } => skill::add(SKILL_MD, SKILL_REFERENCES),
        Command::Skill { add: false } => {
            print!("{}", skill::render(SKILL_MD, SKILL_REFERENCES));
            0
        }
        Command::GetRecording {
            session: target,
            config,
        } => get_recording(target.unwrap_or(session), config.as_deref()),
        Command::Sessions => list_sessions(cli.json),
        Command::Close { all } if all => close_all(cli.json),
        Command::Daemon {
            cmd: DaemonCmd::Start,
        } => daemon_start(&session, cli.verbose, cli.json),
        Command::Daemon {
            cmd: DaemonCmd::Status,
        } => daemon_status(&session, cli.json),
        Command::Daemon {
            cmd: DaemonCmd::Stop { all },
        } => daemon_stop(
            &session,
            all,
            config::session_was_specified(&cli.session),
            cli.json,
        ),
        Command::Monitor { interactive } => monitor::run_client(&session, interactive),
        command => run_remote(&session, command, cli.json, cli.verbose, execution_context),
    };
    std::process::exit(code);
}

fn command_line_args() -> Vec<OsString> {
    command_line_args_from(std::env::args_os())
}

fn command_line_args_from(args: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    let mut args = args.into_iter().collect::<Vec<_>>();
    let Some(separator) = args.iter().position(|arg| arg == "--") else {
        return args;
    };
    if has_command_before_separator(&args[1..separator]) {
        return args;
    }
    args.insert(separator, OsString::from("run"));
    args
}

fn has_command_before_separator(args: &[OsString]) -> bool {
    let mut skip_value = false;
    for arg in args {
        if skip_value {
            skip_value = false;
            continue;
        }
        let value = arg.to_string_lossy();
        if matches!(
            value.as_ref(),
            "--session"
                | "--failure-artifacts"
                | "--failure-artifact-mode"
                | "--diagnostic-context"
        ) {
            skip_value = true;
        } else if value == "--json"
            || value == "--verbose"
            || value == "-v"
            || value.starts_with("--session=")
        {
        } else if !value.starts_with('-') {
            return true;
        }
    }
    false
}

/// Build the request for a daemon-backed command, then send it.
fn run_remote(
    session: &str,
    command: Command,
    json: bool,
    verbose: bool,
    context: ExecutionContext,
) -> i32 {
    let request = match build_request(command) {
        Ok(request) => request.with_context(context),
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };

    // Closing must remain available when the running daemon is from an older
    // client; every other command requires matching protocol behavior.
    let allow_incompatible = request.is_close();
    let socket = config::socket_name(session);
    if allow_incompatible && !ipc::is_running(&socket) {
        if json {
            println!("{}", serde_json::json!({ "ok": true }));
        }
        return 0;
    }
    let conn = match connect_to_daemon(session, verbose, allow_incompatible) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return 4;
        }
    };
    let response = if allow_incompatible {
        ipc::exchange_with_timeout(conn, &request, DAEMON_STOP_TIMEOUT)
    } else {
        ipc::exchange(conn, &request)
    };
    match response {
        Ok(resp) => print_response(&resp, json),
        Err(_) if allow_incompatible && !ipc::is_running(&socket) => {
            if json {
                println!("{}", serde_json::json!({ "ok": true }));
            }
            0
        }
        Err(e) => {
            eprintln!("request failed: {e}");
            4
        }
    }
}

fn build_execution_context(cli: &Cli) -> anyhow::Result<ExecutionContext> {
    let artifact = match cli.failure_artifacts.as_ref() {
        Some(directory) => {
            let directory = std::path::absolute(directory)?;
            Some(FailureArtifactOptions {
                directory,
                mode: cli.failure_artifact_mode.into(),
                include_recording: cli.failure_artifact_recording,
            })
        }
        None => None,
    };
    let mut diagnostic_context = std::collections::BTreeMap::new();
    for pair in &cli.diagnostic_context {
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("--diagnostic-context must use KEY=VALUE"))?;
        if key.trim().is_empty() {
            anyhow::bail!("--diagnostic-context key must not be empty");
        }
        diagnostic_context.insert(key.to_string(), value.to_string());
    }
    Ok(ExecutionContext {
        operation_name: None,
        artifact,
        diagnostic_context,
        retention: Default::default(),
        trace: None,
    })
}

fn connect_to_daemon(
    session: &str,
    verbose: bool,
    allow_incompatible: bool,
) -> anyhow::Result<ipc::Stream> {
    const ATTEMPTS: u32 = 3;
    let mut last = None;
    for attempt in 0..ATTEMPTS {
        let socket = config::socket_name(session);
        if !(allow_incompatible && ipc::is_running(&socket)) {
            let _ = ensure_daemon(session, verbose)
                .map_err(|e| anyhow::anyhow!("failed to start daemon: {e}"))?;
        }
        match ipc::connect(&config::socket_name(session)) {
            Ok(conn) => return Ok(conn),
            Err(e) => last = Some(e),
        }
        if attempt + 1 < ATTEMPTS {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let err = last.expect("at least one connect attempt");
    Err(anyhow::anyhow!("request failed: {err}"))
}

fn ready_flag(wait_ready: bool, no_wait_ready: bool) -> Option<bool> {
    match (wait_ready, no_wait_ready) {
        (true, _) => Some(true),
        (_, true) => Some(false),
        _ => None,
    }
}

fn build_request(command: Command) -> anyhow::Result<Request> {
    let req = match command {
        Command::Open {
            shell,
            backend,
            cols,
            rows,
            cwd,
            env,
            wait_ready,
            no_wait_ready,
            restart,
            profile,
            timeouts,
            diagnostics,
        } => {
            let settings = resolve_client_settings(&profile)?;
            Request::Open {
                shell: shell.map(Into::into),
                program: None,
                backend: backend.map(Into::into).unwrap_or_default(),
                profile: settings.profile,
                cols,
                rows,
                cwd: Some(resolve_client_path(cwd.unwrap_or_else(|| ".".into()))?),
                env: parse_env(&env)?,
                wait_ready: ready_flag(wait_ready, no_wait_ready),
                restart,
                timeouts: settings.timeouts.with_overrides(timeouts.into()),
                recording: Box::new(settings.recording),
                style: Box::new(settings.style.clone()),
                trace: settings.trace,
                diagnostics: termprobe::DiagnosticRetentionOptions {
                    screen_history_limit: diagnostics
                        .screen_history_limit
                        .unwrap_or(settings.diagnostics.screen_history_limit),
                },
            }
        }
        Command::Run {
            program,
            args,
            backend,
            cols,
            rows,
            cwd,
            env,
            wait_ready,
            no_wait_ready,
            restart,
            profile,
            timeouts,
            diagnostics,
        } => {
            let mut prog = vec![program];
            prog.extend(args);
            let settings = resolve_client_settings(&profile)?;
            Request::Open {
                shell: None,
                program: Some(prog),
                backend: backend.map(Into::into).unwrap_or_default(),
                profile: settings.profile,
                cols,
                rows,
                cwd: Some(resolve_client_path(cwd.unwrap_or_else(|| ".".into()))?),
                env: parse_env(&env)?,
                wait_ready: ready_flag(wait_ready, no_wait_ready),
                restart,
                timeouts: settings.timeouts.with_overrides(timeouts.into()),
                recording: Box::new(settings.recording),
                style: Box::new(settings.style.clone()),
                trace: settings.trace,
                diagnostics: termprobe::DiagnosticRetentionOptions {
                    screen_history_limit: diagnostics
                        .screen_history_limit
                        .unwrap_or(settings.diagnostics.screen_history_limit),
                },
            }
        }
        Command::Restart { graceful_timeout } => Request::Restart {
            graceful_timeout_ms: graceful_timeout,
        },
        Command::Close { .. } => Request::Close,
        // Every `daemon` subcommand is handled in `main`: they decide for
        // themselves whether to start a daemon, and requests built here always
        // do.
        Command::Daemon { .. } => {
            anyhow::bail!("internal: `daemon` must be handled before build_request")
        }
        Command::State => Request::State,
        Command::Text { full } => Request::Text { full },
        Command::Screenshot(ScreenshotArgs {
            path,
            out,
            full,
            zoom,
            background,
            transparent,
        }) => {
            let path = out.or(path).map(resolve_client_path).transpose()?;
            if (zoom.is_some() || background.is_some() || transparent) && path.is_none() {
                anyhow::bail!(
                    "screenshot --zoom, --background, and --transparent require --out or a path"
                );
            }
            Request::Screenshot {
                full,
                path,
                zoom,
                background: capture_background(background, transparent)?,
            }
        }
        Command::Record {
            cmd:
                RecordCmd::Start {
                    path,
                    format,
                    fps,
                    speed,
                    idle_time_limit,
                    zoom,
                    background,
                    transparent,
                },
        } => Request::StartRecording {
            path: resolve_client_path(path)?,
            format: format.map(Into::into),
            fps,
            speed,
            idle_time_limit,
            zoom,
            background: capture_background(background, transparent)?,
        },
        Command::Record {
            cmd: RecordCmd::Stop,
        } => Request::StopRecording,
        Command::Cells { x, y, w, h } => Request::Cells { x, y, w, h },
        Command::Get { field } => Request::Get {
            field: map_field(field),
        },
        Command::Type { text } => Request::Write { data: text },
        Command::Submit { text } => Request::Submit { data: text },
        Command::Key { action } => map_key(action),
        Command::Press { keys } => Request::Key {
            action: termprobe::KeyAction::Press,
            keys,
        },
        Command::Mouse { action } => Request::Mouse {
            action: map_mouse(action),
        },
        Command::Resize { cols, rows } => Request::Resize { cols, rows },
        Command::Scroll { target } => Request::Scroll {
            target: match target {
                ScrollCmd::Up { lines } => termprobe::ScrollTarget::Lines(lines),
                ScrollCmd::Down { lines } => termprobe::ScrollTarget::Lines(-lines),
                ScrollCmd::Top => termprobe::ScrollTarget::Top,
                ScrollCmd::Bottom => termprobe::ScrollTarget::Bottom,
            },
        },
        Command::Write { data } => Request::Write { data },
        Command::Signal { name } => Request::Signal {
            name: name.as_str().to_string(),
        },
        Command::Kill => Request::Signal {
            name: "KILL".to_string(),
        },
        Command::Wait { what } => map_wait(what),
        Command::Find { what } => map_find(what),
        Command::Click { what } => map_click(what),
        Command::Highlight { what } => map_highlight(what),
        Command::Expect { what } => map_expect(what),
        _ => anyhow::bail!("unsupported command"),
    };
    Ok(req)
}

fn capture_background(
    background: Option<String>,
    transparent: bool,
) -> anyhow::Result<Option<CaptureBackground>> {
    if transparent {
        return Ok(Some(CaptureBackground::Transparent));
    }
    background
        .map(|value| CaptureBackground::parse(&value).map_err(anyhow::Error::msg))
        .transpose()
}

fn resolve_client_path(path: String) -> anyhow::Result<String> {
    if path.trim().is_empty() {
        anyhow::bail!("path must not be empty");
    }
    let path = std::path::absolute(path)?;
    Ok(path.to_string_lossy().into_owned())
}

fn resolve_client_settings(
    profile: &cli::ProfileArgs,
) -> anyhow::Result<termprobe::profile::Settings> {
    let mut settings = profile.resolve()?;
    settings.trace.directory = std::path::absolute(&settings.trace.directory)?;
    if let Some(directory) = &mut settings.recording.directory {
        *directory = std::path::absolute(&*directory)?;
    }
    Ok(settings)
}

fn map_field(field: GetArg) -> GetField {
    match field {
        GetArg::Command => GetField::Command,
        GetArg::Output => GetField::Output,
        GetArg::ExitCode => GetField::ExitCode,
        GetArg::Cwd => GetField::Cwd,
        GetArg::Cursor => GetField::Cursor,
        GetArg::Viewport => GetField::Viewport,
        GetArg::Modes => GetField::Modes,
        GetArg::Colors => GetField::Colors,
        GetArg::Size => GetField::Size,
        GetArg::Title => GetField::Title,
        GetArg::Clipboard => GetField::Clipboard,
        GetArg::Bells => GetField::BellCount,
        GetArg::BellEvents => GetField::BellEvents,
    }
}

fn map_key(action: KeyCmd) -> Request {
    let (action, keys) = match action {
        KeyCmd::Press { keys } => (termprobe::KeyAction::Press, keys),
        KeyCmd::Down { keys } => (termprobe::KeyAction::Down, keys),
        KeyCmd::Repeat { keys } => (termprobe::KeyAction::Repeat, keys),
        KeyCmd::Up { keys } => (termprobe::KeyAction::Up, keys),
    };
    Request::Key { action, keys }
}

fn map_mouse(action: MouseCmd) -> MouseAction {
    match action {
        MouseCmd::Click {
            x,
            y,
            on_text,
            options,
            clicks,
        } => MouseAction::Click {
            x,
            y,
            on_text,
            options: options.into(),
            clicks,
        },
        MouseCmd::Move { x, y } => MouseAction::Move { x, y },
        MouseCmd::Down { x, y, options } => MouseAction::Down {
            x,
            y,
            options: options.into(),
        },
        MouseCmd::Up { x, y, options } => MouseAction::Up {
            x,
            y,
            options: options.into(),
        },
        MouseCmd::Drag {
            x1,
            y1,
            x2,
            y2,
            options,
        } => MouseAction::Drag {
            x1,
            y1,
            x2,
            y2,
            options: options.into(),
        },
        MouseCmd::Scroll { direction, amount } => MouseAction::Scroll {
            direction: direction.as_str().to_string(),
            amount,
        },
    }
}

fn map_wait(what: WaitCmd) -> Request {
    match what {
        WaitCmd::Title {
            text,
            regex,
            not,
            timeout,
        } => Request::WaitTitle {
            text,
            regex,
            timeout_ms: timeout,
            not,
        },
        WaitCmd::Clipboard {
            text,
            regex,
            timeout,
        } => Request::WaitClipboard {
            text,
            regex,
            timeout_ms: timeout,
        },
        WaitCmd::Idle { timeout } => Request::WaitIdle {
            timeout_ms: timeout,
        },
        WaitCmd::Command { timeout } => Request::WaitCommand {
            timeout_ms: timeout,
        },
        WaitCmd::Exit { timeout } => Request::WaitExit {
            timeout_ms: timeout,
        },
        WaitCmd::Ready { timeout } => Request::WaitReady {
            timeout_ms: timeout,
        },
        WaitCmd::Bell { timeout } => Request::WaitBell {
            timeout_ms: timeout,
        },
    }
}

fn map_occurrence(
    mode: Option<MatchArg>,
    nth: Option<usize>,
    default: MatchOccurrence,
) -> MatchOccurrence {
    if let Some(index) = nth {
        return MatchOccurrence::Nth(index);
    }
    match mode {
        Some(MatchArg::Any) => MatchOccurrence::Any,
        Some(MatchArg::Unique) => MatchOccurrence::Unique,
        Some(MatchArg::First) => MatchOccurrence::First,
        Some(MatchArg::Last) => MatchOccurrence::Last,
        None => default,
    }
}

fn map_anchor(
    text: Option<String>,
    regex: bool,
    mode: Option<MatchArg>,
    nth: Option<usize>,
) -> Option<TextAnchor> {
    text.map(|text| TextAnchor {
        text,
        regex,
        occurrence: map_occurrence(mode, nth, MatchOccurrence::Unique),
    })
}

fn map_selector(text: String, args: TextSelectorArgs) -> TextSelector {
    TextSelector {
        text,
        regex: args.regex,
        full: args.full,
