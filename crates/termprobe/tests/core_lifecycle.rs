use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use termprobe::{
    AutomaticRecording, AutomaticRecordingMode, Engine, ErrorKind, ExecutionContext, KeyAction,
    LocatorQuery, MouseAction, OpenOptions, Operation, OperationResult, RunOptions, Session,
    SessionRegistry, Timeouts,
};

const FIXTURE: &str = "TERMPROBE__CORE_LIFECYCLE_FIXTURE";

fn emit(bytes: &[u8]) {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(bytes).unwrap();
    stdout.flush().unwrap();
}

fn prompt() {
    emit(b"\x1b]133;A\x1b\\fixture> \x1b]133;B\x1b\\");
}

#[test]
fn child_fixture() {
    let Ok(mode) = std::env::var(FIXTURE) else {
        return;
    };
    match mode.as_str() {
        "commands" => {
            prompt();
            for line in std::io::stdin().lock().lines() {
                let line = line.unwrap();
                if line.trim() == "exit-without-start" {
                    std::process::exit(7);
                }
                if line.trim() == "delayed" {
                    std::thread::sleep(Duration::from_millis(400));
                }
                emit(b"\x1b]133;C\x1b\\");
                if line.trim() == "exit-with-start" {
                    std::process::exit(7);
                }
                if line.trim() == "emit-marker" {
                    emit(b"second-client-marker\r\n");
                }
                emit(b"command complete\r\n\x1b]133;D;0\x1b\\");
                prompt();
            }
        }
        "ready-exit" => {
            prompt();
            emit(b"ready-after-marker\r\n");
            std::process::exit(7);
        }
        "nonvisual" => {
            emit(b"fixture-ready\r\n");
            for _ in 0..1500 {
                emit(b"\x07\x1b[31m");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        "repaint" => {
            emit(b"fixture-repaint-ready\r\n");
            for index in 0..1500 {
                emit(format!("\x1b[2;1H{}", index % 10).as_bytes());
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        "quiet" => {
            emit(b"fixture-ready\r\n");
            std::thread::sleep(Duration::from_secs(30));
        }
        #[cfg(windows)]
        "exit-259" => {
            prompt();
            let mut line = String::new();
            std::io::stdin().read_line(&mut line).unwrap();
            std::process::exit(259);
        }
        _ => panic!("unknown fixture mode: {mode}"),
    }
}

fn options(mode: &str) -> RunOptions {
    let defaults = OpenOptions::default();
    RunOptions {
        program: std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        args: [
            "--exact",
            "child_fixture",
            "--nocapture",
            "--test-threads=1",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
        backend: defaults.backend,
        profile: defaults.profile,
        style: defaults.style,
        cols: 80,
        rows: 24,
        cwd: None,
        env: vec![(FIXTURE.to_string(), mode.to_string())],
        wait_ready: Some(false),
        restart: false,
        timeouts: Timeouts {
            ready: Some(5_000),
            ..Default::default()
        },
        recording: AutomaticRecording {
            mode: AutomaticRecordingMode::Disabled,
            directory: None,
        },
    }
}

fn test_directory(label: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("core-lifecycle-tests")
        .join(format!(
            "{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn wait_until(mut ready: impl FnMut() -> bool) {
    let started = Instant::now();
    while !ready() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "fixture did not reach the expected state"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_ready(
    session: &Session,
    timeout_ms: u64,
) -> Result<OperationResult, termprobe::TermprobeError> {
    session.execute(Operation::WaitReady {
        timeout_ms: Some(timeout_ms),
    })
}

fn wait_exit(session: &Session) {
    session
        .execute(Operation::WaitExit {
            timeout_ms: Some(5_000),
        })
        .unwrap();
}

fn expect_marker(timeout_ms: u64) -> Operation {
    Operation::WaitLocator {
        query: LocatorQuery::text("second-client-marker"),
        not: false,
        timeout_ms: Some(timeout_ms),
    }
}

#[test]
fn invalid_capture_style_is_rejected_before_session_creation() {
    let session = Session::new("invalid-capture-style");
    let mut run = options("quiet");
    run.style.font_size = f32::NAN;
    let result = session.run(run);
    session.close().unwrap();
    let error = result.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Usage);
    assert!(error.message.contains("font_size"), "{error}");

    let mut open = OpenOptions {
        wait_ready: Some(false),
        ..OpenOptions::default()
    };
    open.style.border.radius = -1.0;
    let result = session.open(open);
    session.close().unwrap();
    let error = result.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Usage);
    assert!(error.message.contains("border.radius"), "{error}");
}

#[test]
fn named_wait_allows_concurrent_input_resize_and_submit() {
    let registry = SessionRegistry::default();
    let session = registry.session("concurrent-input");
    let mut run = options("commands");
    run.wait_ready = Some(true);
    session.run(run).unwrap();
    let waiting = session.clone();
    let (finished_tx, finished_rx) = mpsc::channel();
    let waiter = std::thread::spawn(move || {
        finished_tx
            .send(waiting.execute(expect_marker(5_000)))
            .unwrap();
    });
    assert!(finished_rx
        .recv_timeout(Duration::from_millis(150))
        .is_err());

    for operation in [
        Operation::Write {
            data: "input".into(),
        },
        Operation::Key {
            keys: vec!["Enter".into()],
            action: KeyAction::Press,
        },
        Operation::Mouse {
            action: MouseAction::Move { x: 1, y: 1 },
        },
        Operation::Submit { data: None },
        Operation::Resize { cols: 90, rows: 26 },
        Operation::State,
    ] {
        let started = Instant::now();
        session.execute(operation).unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
    }
    assert!(finished_rx.try_recv().is_err());
    session
        .execute(Operation::Submit {
            data: Some("emit-marker".into()),
        })
        .unwrap();
    finished_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    waiter.join().unwrap();
    session.close().unwrap();
}
