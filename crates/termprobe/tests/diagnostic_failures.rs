use termprobe::{
    AutomaticRecording, AutomaticRecordingMode, FailureReason, OpenOptions, Operation, RunOptions,
    Session,
};

fn run_options(program: &str, args: &[&str]) -> RunOptions {
    let defaults = OpenOptions::default();
    RunOptions {
        backend: defaults.backend,
        program: program.into(),
        args: args.iter().map(|arg| (*arg).into()).collect(),
        profile: defaults.profile,
        style: defaults.style,
        cols: 80,
        rows: 30,
        cwd: None,
        env: Vec::new(),
        wait_ready: Some(false),
        restart: false,
        timeouts: defaults.timeouts,
        recording: AutomaticRecording {
            mode: AutomaticRecordingMode::Disabled,
            directory: None,
        },
    }
}

fn snapshot(root: &std::path::Path, update: bool) -> Operation {
    Operation::Snapshot {
        name: "diagnostic".into(),
        update,
        include_style: false,
        include_title: false,
        cwd: Some(root.to_string_lossy().into_owned()),
    }
}

#[test]
fn completed_processes_preserve_comparison_and_internal_failure_causes() {
    let root =
        std::env::temp_dir().join(format!("termprobe-exit-diagnostic-{}", std::process::id()));
    let session = Session::new(format!("exit-diagnostic-{}", std::process::id()));
    let options = if cfg!(windows) {
        run_options("cmd.exe", &["/d", "/c", "exit 0"])
    } else {
        run_options("sh", &["-c", "exit 0"])
    };
    session.run(options).unwrap();
    session
        .execute(Operation::WaitExit {
            timeout_ms: Some(15_000),
        })
        .unwrap();
    std::fs::create_dir_all(root.join("__snapshots__")).unwrap();
    std::fs::write(
        root.join("__snapshots__").join("diagnostic.snap"),
        "different baseline\n",
    )
    .unwrap();
    let mismatch = session.execute(snapshot(&root, false)).unwrap_err();
    let scalar = session
        .execute(Operation::ExpectOutput {
            text: "missing output".into(),
            regex: false,
        })
        .unwrap_err();
    let internal = session
        .execute(Operation::Screenshot {
            full: false,
            path: Some(
                root.join("missing")
                    .join("screen.svg")
                    .to_string_lossy()
                    .into_owned(),
            ),
            zoom: None,
            background: None,
        })
        .unwrap_err();
    let stopped = session
        .get_by_text("missing marker")
        .wait_with_timeout(Some(0))
        .unwrap_err();
