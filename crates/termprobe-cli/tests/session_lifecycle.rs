//! End-to-end coverage for session lifecycle over the real cli + daemon.

use std::io::{BufRead, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericFilePath, GenericNamespaced};
use termprobe::Backend;

#[allow(dead_code)]
#[path = "../src/config.rs"]
mod cli_config;

const BIN: &str = env!("CARGO_BIN_EXE_termprobe");

#[cfg(unix)]
#[test]
fn native_scrollback_moves_the_view_without_changing_full_text() {
    let sandbox = Sandbox::new("native-scrollback");
    sandbox.ok(&[
        "run", "--backend", "alacritty", "--rows", "5", "--cols", "40", "--",
        "sh", "-c",
        "i=0; while [ \"$i\" -lt 40 ]; do printf 'history-%s\\n' \"$i\"; i=$((i+1)); done; read line",
    ]);
    sandbox.wait_for_text("history-39", "5000");
    let full = sandbox.ok(&["text", "--full"]);
    let live = sandbox.ok(&["text"]);
    let viewport = || -> serde_json::Value {
        let output = sandbox.ok(&["get", "viewport", "--json"]);
        serde_json::from_str::<serde_json::Value>(&output).unwrap()["data"]["value"].clone()
    };
    assert_eq!(viewport()["offset"], 0);
    assert!(viewport()["history"].as_u64().unwrap() > 0);
    sandbox.ok(&["scroll", "up", "2"]);
    assert_eq!(viewport()["offset"], 2);
    sandbox.ok(&["scroll", "down"]);
    assert_eq!(viewport()["offset"], 1);
    sandbox.ok(&["scroll", "top"]);
    let top = viewport();
    assert_eq!(top["offset"], top["history"]);
    assert!(sandbox.ok(&["text"]).contains("history-0"));
    assert_eq!(sandbox.ok(&["text", "--full"]), full);
    assert_eq!(
        sandbox.run(&["mouse", "click", "0", "0"]).status.code(),
        Some(2)
    );
    assert_eq!(
        sandbox.run(&["click", "text", "history-0"]).status.code(),
        Some(1)
    );
    sandbox.ok(&["scroll", "bottom"]);
    assert_eq!(sandbox.ok(&["text"]), live);
    sandbox.ok(&["scroll", "top"]);
    sandbox.ok(&["type", "a"]);
    assert_eq!(viewport()["offset"], 0);
    for invalid in ["-1", "1.5", "2147483648"] {
        assert_eq!(
            sandbox.run(&["scroll", "up", invalid]).status.code(),
            Some(2)
        );
    }
}

#[test]
fn invalid_capture_backgrounds_fail_before_starting_a_session() {
    let sandbox = Sandbox::new("invalid-capture-background");
    let screenshot = sandbox.home.join("screen.svg");
    let recording = sandbox.home.join("recording.gif");
    for value in [
        "",
        "#12",
        "#ff00zz",
        "#12345678",
        "#12é34",
        "256,0,0",
        "rgb(-1,0,0)",
    ] {
        let background = format!("--background={value}");
        for args in [
            vec!["screenshot", screenshot.to_str().unwrap(), &background],
            vec!["record", "start", recording.to_str().unwrap(), &background],
        ] {
            let output = sandbox.run(&args);
            assert_eq!(output.status.code(), Some(2), "{args:?}");
            let message = String::from_utf8_lossy(&output.stderr);
            assert!(message.contains("color"), "{args:?}: {message}");
        }
    }
    assert!(!screenshot.exists());
    assert!(!recording.exists());
    assert_eq!(sandbox.ok(&["sessions"]).trim(), "no active sessions");
}

#[test]
fn cli_startup_fits_the_default_process_stack() {
    for argument in ["--help", "--version"] {
        let output = Command::new(BIN).arg(argument).output().unwrap();
        assert!(
            output.status.success(),
            "{argument} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

const CALL_TIMEOUT: Duration = Duration::from_secs(60);

static SANDBOX_SEQ: AtomicU32 = AtomicU32::new(0);

fn contains_rgba(pixels: &[u8], expected: [u8; 4]) -> bool {
    pixels.chunks(4).any(|pixel| pixel == expected)
}

struct Sandbox {
    label: &'static str,
    home: PathBuf,
    session: String,
}

impl Sandbox {
    fn new(label: &'static str) -> Self {
        let id = format!(
            "{:x}-{:x}",
            std::process::id(),
            SANDBOX_SEQ.fetch_add(1, Ordering::Relaxed)
        );
        let home = std::env::temp_dir().join(format!("su{id}"));
        std::fs::create_dir_all(&home).expect("create sandbox home");
        Sandbox {
            label,
            session: format!("s{id}"),
            home,
        }
    }

    /// Captures output to catch daemon-inherited stdout pipe hangs.
    fn try_run(&self, args: &[&str]) -> Option<Output> {
        self.try_run_in(None, args)
    }

    fn spawn(&self, args: &[&str]) -> Child {
        Command::new(BIN)
            .args(["--session", &self.session])
            .args(args)
            .env("TERMPROBE__HOME", &self.home)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn concurrent client")
    }

    fn wait_for_logged_operation(&self, marker: &str) {
        let log = self.home.join(format!("{}.log", self.session));
        let started = Instant::now();
        while !std::fs::read_to_string(&log).is_ok_and(|log| log.contains(marker)) {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{marker} never reached the engine"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn try_run_in(&self, cwd: Option<PathBuf>, args: &[&str]) -> Option<Output> {
        let session = self.session.clone();
        let home = self.home.clone();
        let owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut command = Command::new(BIN);
            command
                .args(["--session", &session])
                .args(&owned)
                .env("TERMPROBE__HOME", &home);
            if let Some(cwd) = cwd {
                command.current_dir(cwd);
            }
            let out = command.output();
            let _ = tx.send(out);
        });
        match rx.recv_timeout(CALL_TIMEOUT) {
            Ok(Ok(out)) => Some(out),
            Ok(Err(e)) => {
                eprintln!("could not spawn `termprobe {}`: {e}", args.join(" "));
                None
            }
            Err(_) => None,
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_in(None, args)
    }

    fn run_in(&self, cwd: Option<&std::path::Path>, args: &[&str]) -> Output {
        self.try_run_in(cwd.map(std::path::Path::to_path_buf), args)
            .unwrap_or_else(|| {
                panic!(
                    "[{}] `termprobe {}` produced no result within {:?}. Either it could not be \
                 spawned (see stderr above), or the cli process exited but left its stdout pipe \
                 open, which happens when the detached daemon inherits the cli's standard handles.",
                    self.label,
                    args.join(" "),
                    CALL_TIMEOUT
                )
            })
    }

    /// Run without any explicit or environment session target.
    fn run_untargeted(&self, args: &[&str]) -> Output {
        Command::new(BIN)
            .args(args)
            .env("TERMPROBE__HOME", &self.home)
            .env_remove("TERMPROBE__SESSION")
            .output()
            .expect("spawn termprobe")
    }

    fn run_as(&self, suffix: &str, args: &[&str]) -> Output {
        Command::new(BIN)
            .args(["--session", &format!("{}-{suffix}", self.session)])
            .args(args)
            .env("TERMPROBE__HOME", &self.home)
            .env_remove("TERMPROBE__SESSION")
            .output()
            .expect("spawn termprobe")
    }

    fn ok_as(&self, suffix: &str, args: &[&str]) -> String {
        let out = self.run_as(suffix, args);
        assert!(
            out.status.success(),
            "[{}] `termprobe {}` (session {suffix}) failed with {:?}\nstderr: {}",
            self.label,
            args.join(" "),
            out.status.code(),
            String::from_utf8_lossy(&out.stderr),
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn ok(&self, args: &[&str]) -> String {
        self.ok_in(None, args)
    }

    fn wait_for_text(&self, text: &str, timeout: &str) {
        self.ok(&[
            "expect",
            "text",
            text,
            "--match",
            "first",
            "--timeout",
            timeout,
        ]);
    }

    fn ok_in(&self, cwd: Option<&std::path::Path>, args: &[&str]) -> String {
        let out = self.run_in(cwd, args);
        assert!(
            out.status.success(),
            "[{}] `termprobe {}` failed with {:?}\nstdout: {}\nstderr: {}",
            self.label,
            args.join(" "),
            out.status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }
}

impl Drop for Sandbox {
    /// Best-effort teardown must not double-panic during unwinding.
    fn drop(&mut self) {
        let _ = self.try_run(&["close"]);
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

/// Connect to the session socket and send one request line, leaving the stream
/// open for whatever the request streams next.
fn monitor_stream(sandbox: &Sandbox, request: &str) -> interprocess::local_socket::Stream {
    let raw = cli_config::socket_name_in(&sandbox.home, &sandbox.session);
    let name = if cfg!(windows) {
        raw.to_ns_name::<GenericNamespaced>()
    } else {
        raw.to_fs_name::<GenericFilePath>()
    }
    .expect("valid session socket name");
    let mut stream =
        interprocess::local_socket::Stream::connect(name).expect("connect session socket");
    stream
        .write_all(request.as_bytes())
        .expect("send monitor request");
    stream.flush().expect("flush monitor request");
    if request.contains("\"monitor_input_stream\"") {
        let mut response = String::new();
        std::io::BufReader::new(&mut stream)
            .read_line(&mut response)
            .expect("read monitor input handshake");
        let response: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["ok"], true);
        assert!(response["data"]["initial_frame"].is_array());
    }
    stream
}

fn write_monitor_input(stream: &mut interprocess::local_socket::Stream, data: &[u8]) {
    serde_json::to_writer(
        &mut *stream,
        &serde_json::json!({ "kind": "write", "data": data }),
    )
    .expect("encode monitor input");
    stream.write_all(b"\n").expect("write monitor input");
}

#[test]
fn monitor_read_only_viewer_resizes_and_detaches() {
    let target = Sandbox::new("monitor-read-only-target");
    target.ok(&["open"]);
    target.ok(&["submit", "echo monitor-read-only-marker"]);
    target.wait_for_text("monitor-read-only-marker", "10000");

    let viewer = Sandbox::new("monitor-read-only-viewer");
    let home = format!("TERMPROBE__HOME={}", target.home.display());
    viewer.ok(&[
        "run",
        "--cols",
        "100",
        "--rows",
        "36",
        "--env",
        &home,
        "--",
        BIN,
        "--session",
        &target.session,
        "monitor",
    ]);
    viewer.wait_for_text("monitor-read-only-marker", "10000");
    viewer.ok(&["resize", "90", "34"]);
    viewer.wait_for_text("q quit", "10000");
    viewer.ok(&["key", "press", "q"]);
    viewer.ok(&["wait", "exit", "--timeout", "10000"]);
    target.ok(&["daemon", "status"]);
}

#[cfg(windows)]
#[test]
fn monitor_windows_console_forwards_ctrl_z_and_unicode_without_detaching() {
    let target = Sandbox::new("monitor-console-target");
    target.ok(&[
        "run", "--", "powershell", "-NoProfile", "-Command",
        "Write-Output CONSOLE_READY; while ($true) { $k = [Console]::ReadKey($true); [Console]::WriteLine('KEY:' + [int]$k.KeyChar) }",
    ]);
    target.wait_for_text("CONSOLE_READY", "10000");
    let viewer = Sandbox::new("monitor-console-viewer");
    let home = format!("TERMPROBE__HOME={}", target.home.display());
    viewer.ok(&[
        "run",
        "--cols",
        "100",
        "--rows",
        "36",
        "--env",
        &home,
        "--",
        BIN,
        "--session",
        &target.session,
        "monitor",
        "--interactive",
    ]);
    viewer.wait_for_text("Ctrl+] detach", "10000");
    viewer.ok(&["write", "\u{1a}"]);
    target.wait_for_text("KEY:26", "10000");
    viewer.ok(&["resize", "90", "34"]);
    viewer.ok(&["write", "a\u{e9}\u{1f680}"]);
    for code in [97, 233, 55357, 56960] {
        target.wait_for_text(&format!("KEY:{code}"), "10000");
    }
    let state: serde_json::Value = serde_json::from_str(&viewer.ok(&["--json", "state"])).unwrap();
    assert!(state["exited"].is_null());
    viewer.ok(&["write", "\u{1d}"]);
    viewer.ok(&["wait", "exit", "--timeout", "10000"]);
    target.ok(&["daemon", "status"]);
}

#[test]
fn screenshots_dispatch_by_extension_without_changing_svg_output() {
    let sandbox = Sandbox::new("screenshot-formats");
    let program = r#"printf "\033[41m \033[0m\033[38;2;0;255;0mX\033[0m\033]12;#ff00ff\007\033[1;4H"; sleep 30"#;
    sandbox.ok(&[
        "run", "--cols", "4", "--rows", "3", "--", "bash", "--norc", "-c", program,
    ]);
    sandbox.wait_for_text("X", "5000");

    let svg = sandbox.home.join("screen.svg");
    let extensionless = sandbox.home.join("screen");
    sandbox.ok(&["screenshot", svg.to_str().unwrap()]);
    sandbox.ok(&["screenshot", extensionless.to_str().unwrap()]);
    let svg_bytes = std::fs::read(&svg).unwrap();
    assert!(svg_bytes.starts_with(b"<svg "));
    assert_eq!(svg_bytes, std::fs::read(&extensionless).unwrap());

    let zoomed_svg = sandbox.home.join("screen-zoomed.svg");
    sandbox.ok(&["screenshot", zoomed_svg.to_str().unwrap(), "--zoom", "2"]);
    let zoomed_svg = std::fs::read_to_string(zoomed_svg).unwrap();
    assert!(
        // Follows the default style; update deliberately if a gap moves.
        zoomed_svg.contains(r#"width="236" height="334" viewBox="0 0 118 167""#),
        "unexpected zoomed SVG dimensions: {zoomed_svg}"
    );

    let png = sandbox.home.join("screen.PNG");
    sandbox.ok(&["screenshot", png.to_str().unwrap(), "--zoom", "2"]);
    let png_bytes = std::fs::read(&png).unwrap();
    assert_eq!(&png_bytes[..8], b"\x89PNG\r\n\x1a\n");
    let decoder = png::Decoder::new(std::io::Cursor::new(&png_bytes));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (236, 334));
    let pixels = &pixels[..info.buffer_size()];
    assert!(
        contains_rgba(pixels, [128, 0, 0, 255]),
        "styled red background cell was not rendered"
    );
    assert!(
        contains_rgba(pixels, [0, 255, 0, 255]),
        "non-empty green glyph was not rendered"
    );
    assert!(
        contains_rgba(pixels, [255, 0, 255, 255]),
        "magenta cursor was not rendered"
    );

    let unsupported = sandbox.home.join("screen.gif");
    let output = sandbox.run(&["screenshot", unsupported.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unsupported screenshot extension '.gif'")
    );
    assert!(!unsupported.exists());
}

#[test]
fn sandbox_paths_fit_in_a_unix_socket_address() {
    const SUN_PATH_MAX: usize = 103;
    const MACOS_TMPDIR: usize = 49;

    let sandbox = Sandbox::new("path-budget");
    let home = sandbox
        .home
        .file_name()
        .expect("sandbox home has a name")
        .to_string_lossy()
        .len();
    let socket = format!("{}-three.sock", sandbox.session).len();
    let total = MACOS_TMPDIR + home + 1 + socket;
    assert!(
        total <= SUN_PATH_MAX,
        "a sandbox socket would be {total} bytes on macOS; shorten the naming"
    );
}

/// Repeats `close` to catch the final-response drain race.
#[test]
fn close_always_reports_success() {
    let sandbox = Sandbox::new("close");
    for attempt in 0..5 {
        sandbox.ok(&["open"]);
        let out = sandbox.run(&["close"]);
        assert!(
            out.status.success(),
            "close #{attempt} failed with {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

/// Auto-started daemons must not inherit stdout and keep captures waiting for EOF.
#[test]
fn capturing_output_terminates_after_the_daemon_starts() {
    let sandbox = Sandbox::new("capture");
    let stdout = sandbox.ok(&["open"]);
    assert!(
        stdout.contains("\"session\""),
        "expected the open payload on stdout, got: {stdout}"
    );
    sandbox.ok(&["text"]);
}

/// One monitor holds two streams open: rendered frames out and input messages in.
/// Neither needs a target to exist, and the input stream outlives a restart.
#[test]
fn monitor_frames_and_input_outlive_the_target() {
    let sandbox = Sandbox::new("monitor-input");
    sandbox.ok(&["--verbose", "daemon", "start"]);

    let mut frames = monitor_stream(
        &sandbox,
        "{\"kind\":\"monitor\",\"cols\":80,\"rows\":24,\"interactive\":true}\n",
    );
    let (frame_tx, frame_rx) = std::sync::mpsc::channel();
    let (detach_tx, detach_rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        let read = frames.read(&mut buffer).expect("read monitor frame");
        frame_tx.send(read).expect("report monitor frame");
        let _ = detach_rx.recv();
    });
    assert!(
        frame_rx.recv_timeout(Duration::from_secs(5)).unwrap_or(0) > 0,
        "monitor did not receive a frame"
    );

    let mut input = monitor_stream(
        &sandbox,
        "{\"kind\":\"monitor_input_stream\",\"cols\":80,\"rows\":30}\n",
    );
    write_monitor_input(&mut input, b"ignored");
    input.flush().expect("flush without target");
    std::thread::sleep(Duration::from_millis(100));

    let secret = "human-secret-monitor-input";
    sandbox.ok(&["open"]);
    write_monitor_input(&mut input, format!("echo {secret}\r").as_bytes());
    input.flush().expect("flush first target input");
    sandbox.wait_for_text(secret, "5000");

    sandbox.ok(&["open", "--restart"]);
    input
        .write_all(b"{\"kind\":\"resize\",\"cols\":100,\"rows\":40}\n")
        .expect("resize monitor input");
