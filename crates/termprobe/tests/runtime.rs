#[cfg(unix)]
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use termprobe::{
    global_registry, AutomaticRecording, AutomaticRecordingMode, ErrorKind, ExecutionContext,
    FailureArtifactMode, FailureArtifactOptions, FailureReason, LocatorDirection,
    LocatorExpectOptions, LocatorFilterOptions, LocatorQuery, MatchOccurrence, OccurrenceSource,
    OpenOptions, Operation, OperationResult, RunOptions, Session, SessionRegistry, TextSelector,
    TextStyle, Timeouts,
};

fn failure_report(error: &termprobe::TermprobeError) -> termprobe::FailureReport {
    let path = error.artifact.as_ref().unwrap().manifest.as_ref().unwrap();
    let manifest: termprobe::FailureArtifactManifest =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    manifest.details
}

fn report_session(name: &str) -> (Session, std::path::PathBuf) {
    let name = format!("{name}-{}", std::process::id());
    let directory = std::env::temp_dir().join(&name);
    let session = Session::new(name).with_execution_context(ExecutionContext {
        artifact: Some(FailureArtifactOptions {
            directory: directory.clone(),
            mode: FailureArtifactMode::Text,
            include_recording: false,
        }),
        ..Default::default()
    });
    (session, directory)
}

fn run_options(program: &str, args: &[&str]) -> RunOptions {
    let defaults = OpenOptions::default();
    RunOptions {
        backend: defaults.backend,
        program: program.to_string(),
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        profile: defaults.profile,
        style: defaults.style,
        cols: defaults.cols,
        rows: defaults.rows,
        cwd: defaults.cwd,
        env: defaults.env,
        wait_ready: Some(false),
        restart: defaults.restart,
        timeouts: defaults.timeouts,
        recording: defaults.recording,
    }
}

#[test]
fn trace_retention_writes_complete_bundles_only_for_selected_outcomes() {
    use termprobe::{TraceMode, TraceOptions};
    let root = std::env::temp_dir().join(format!("termprobe-trace-modes-{}", std::process::id()));
    for mode in [TraceMode::Off, TraceMode::On, TraceMode::OnFailure] {
        for failed in [false, true] {
            let directory = root.join(format!("{mode:?}-{failed}"));
            let session = Session::new(format!("trace-{mode:?}-{failed}-{}", std::process::id()))
                .with_execution_context(ExecutionContext {
                    trace: Some(TraceOptions {
                        mode,
                        directory: directory.clone(),
                    }),
                    ..Default::default()
                });
            let options = if cfg!(windows) {
                run_options(
                    "powershell.exe",
                    &[
                        "-NoProfile",
                        "-Command",
                        "[Console]::Write('TRACE_READY'); Start-Sleep -Seconds 30",
                    ],
                )
            } else {
                run_options("sh", &["-c", "printf TRACE_READY; sleep 30"])
            };
            let opened = session.run(options).unwrap();
            session
                .get_by_text("TRACE_READY")
                .wait_with_timeout(Some(5_000))
                .unwrap();
            if failed {
                session
                    .execute(Operation::WaitLocator {
                        query: LocatorQuery::text("missing trace marker"),
                        not: false,
                        timeout_ms: Some(0),
                    })
                    .unwrap_err();
            }
            session.close().unwrap();
            session.close().unwrap();
            let retained = mode == TraceMode::On || (mode == TraceMode::OnFailure && failed);
            if retained {
                let bundles: Vec<_> = std::fs::read_dir(&directory)
                    .unwrap()
                    .map(|item| item.unwrap().path())
                    .collect();
                assert_eq!(bundles.len(), 1);
                let bundle = &bundles[0];
                for name in [
                    "trace.json",
                    "trace.md",
                    "trace.html",
                    "session.cast",
                    "timeline.json",
                ] {
                    assert!(bundle.join(name).is_file(), "missing {name}: {bundle:?}");
                }
                let manifest: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(bundle.join("trace.json")).unwrap())
                        .unwrap();
                assert_eq!(
                    manifest["outcome"],
                    if failed { "failed" } else { "passed" }
                );
                assert_eq!(manifest["runtime"]["session_name"], session.name());
                assert!(std::fs::read_to_string(bundle.join("session.cast"))
                    .unwrap()
                    .contains("TRACE_READY"));
                let report = std::fs::read_to_string(bundle.join("trace.md")).unwrap();
                assert!(report.contains("session.cast"));
                let outcome = if failed { "failed" } else { "passed" };
                assert!(report.contains(&format!("| Outcome | <code>{outcome}</code> |")));
                assert!(report.contains(if failed {
                    "## Failure screen"
                } else {
                    "## Final screen"
                }));
            } else {
                assert!(!directory.exists());
            }
            if mode == TraceMode::Off {
                assert!(opened.recording.is_empty());
            } else if mode == TraceMode::OnFailure && !failed {
                assert!(!std::path::Path::new(&opened.recording).exists());
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn trace_retention_uses_the_final_test_outcome() {
    use termprobe::{TraceMode, TraceOptions};
    let root = std::env::temp_dir().join(format!("termprobe-trace-outcome-{}", std::process::id()));
    for (mode, failed, caught) in [
        (TraceMode::OnFailure, false, 2),
        (TraceMode::OnFailure, true, 0),
        (TraceMode::On, false, 2),
        (TraceMode::OnFailure, true, 2),
    ] {
        let directory = root.join(format!("{mode:?}-{failed}-{caught}"));
        let session = Session::new(format!("trace-outcome-{mode:?}-{failed}-{caught}"))
            .with_execution_context(ExecutionContext {
                trace: Some(TraceOptions {
                    mode,
                    directory: directory.clone(),
                }),
                ..Default::default()
            });
        let options = if cfg!(windows) {
            run_options(
                "powershell.exe",
                &[
                    "-NoProfile",
                    "-Command",
                    "[Console]::Write('TRACE_READY'); Start-Sleep -Seconds 30",
                ],
            )
        } else {
            run_options("sh", &["-c", "printf TRACE_READY; sleep 30"])
        };
        session.run(options).unwrap();
        session
            .get_by_text("TRACE_READY")
            .wait_with_timeout(Some(5_000))
            .unwrap();
        for _ in 0..caught {
            session
                .execute(Operation::WaitLocator {
                    query: LocatorQuery::text("missing expected marker"),
                    not: false,
                    timeout_ms: Some(0),
                })
                .unwrap_err();
        }
        session.execute(Operation::FinishTrace { failed }).unwrap();
        session.close().unwrap();
        let bundles: Vec<_> = std::fs::read_dir(&directory)
            .unwrap()
            .map(|item| item.unwrap().path())
            .collect();
        if mode == TraceMode::OnFailure && !failed {
            assert!(bundles.is_empty());
        } else {
            assert_eq!(bundles.len(), if failed { caught.max(1) } else { 1 });
            for bundle in bundles {
                let manifest: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(bundle.join("trace.json")).unwrap())
                        .unwrap();
                assert_eq!(
                    manifest["outcome"],
                    if failed { "failed" } else { "passed" }
                );
                if caught == 0 || !failed {
                    assert_eq!(
                        manifest["reason"],
                        if failed { "test_failed" } else { "completed" }
                    );
                }
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn enabled_traces_pin_input_return_screens() {
    use termprobe::{KeyAction, TraceMode, TraceOptions};
    let directory =
        std::env::temp_dir().join(format!("termprobe-input-checkpoint-{}", std::process::id()));
    let session = Session::new("input-checkpoint").with_execution_context(ExecutionContext {
        trace: Some(TraceOptions {
            mode: TraceMode::OnFailure,
            directory: directory.clone(),
        }),
        ..Default::default()
    });
    let options = if cfg!(windows) {
        run_options("powershell.exe", &["-NoProfile", "-Command", "[Console]::Write('INPUT_READY'); [void][Console]::ReadLine(); [Console]::Write('INPUT_DONE'); Start-Sleep -Seconds 30"])
    } else {
        run_options(
            "sh",
            &[
                "-c",
                "printf INPUT_READY; read line; printf '\\r\\nINPUT_DONE'; sleep 30",
            ],
        )
    };
    session.run(options).unwrap();
    let started = Instant::now();
    loop {
        let OperationResult::Text(text) = session.execute(Operation::Text { full: false }).unwrap()
        else {
            panic!("expected text");
        };
        if text.contains("INPUT_READY") {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "input fixture did not start"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    session
        .execute(Operation::Key {
            keys: vec!["Left".into()],
            action: KeyAction::Up,
        })
        .unwrap();
    session
        .execute(Operation::Submit {
            data: Some("go".into()),
        })
        .unwrap();
    session
        .get_by_text("INPUT_DONE")
        .wait_with_timeout(Some(5_000))
        .unwrap();
    let error = session
        .execute(Operation::WaitLocator {
            query: LocatorQuery::text("missing checkpoint marker"),
            not: false,
            timeout_ms: Some(0),
        })
        .unwrap_err();
    let details = failure_report(&error);
    let input = details
        .recent_operations
        .iter()
        .find(|op| op.name == "key")
        .unwrap();
    assert_ne!(
        input.screen_at_return,
        details.operation.failed_screen_sequence
    );
    assert!(details
        .terminal
        .as_ref()
        .unwrap()
        .screen_history
        .checkpoints
        .iter()
        .any(|frame| frame.sequence == input.screen_at_return));
    session.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn input_diagnostics_capture_written_bytes_key_phases_and_resolved_clicks() {
    use termprobe::{
        InputArguments, KeyAction, MouseAction, MouseOptions, MouseTarget, TextPosition,
    };
    let (session, directory) = report_session("input-diagnostics");
    let options = if cfg!(windows) {
        run_options(
            "powershell.exe",
            &[
                "-NoProfile",
                "-Command",
                "[Console]::Write(\"Save`r`n\"); Start-Sleep -Seconds 30",
            ],
        )
    } else {
        run_options("sh", &["-c", "printf 'Save\\r\\n'; sleep 30"])
    };
    session.run(options).unwrap();
    let result = (|| -> Result<(), termprobe::TermprobeError> {
        session.get_by_text("Save").wait_with_timeout(Some(5_000))?;
        session.execute(Operation::Mouse {
            action: MouseAction::Click {
                x: Some(8),
                y: Some(3),
                on_text: None,
                options: MouseOptions::default(),
                clicks: 1,
            },
        })?;
        session.execute(Operation::Mouse {
            action: MouseAction::Click {
                x: None,
                y: None,
                on_text: Some("Save".into()),
                options: MouseOptions::default(),
                clicks: 1,
            },
        })?;
        session.get_by_text("Save").click()?;
        session.execute(Operation::Write {
            data: "a\u{e9}\n".into(),
        })?;
        session.execute(Operation::Submit {
            data: Some("deploy".into()),
        })?;
        for action in [KeyAction::Down, KeyAction::Up, KeyAction::Press] {
            session.execute(Operation::Key {
                keys: vec!["Left".into()],
                action,
            })?;
        }
        let error = session
            .execute(Operation::WaitLocator {
                query: LocatorQuery::text("missing input diagnostic marker"),
                not: false,
                timeout_ms: Some(0),
            })
            .unwrap_err();
        let details = failure_report(&error);
        let inputs: Vec<_> = details
            .recent_operations
            .iter()
            .filter_map(|op| op.input.as_ref())
            .collect();
        assert_eq!(inputs.len(), 8);
        assert_eq!(
            inputs[0].mouse_position,
            Some(TextPosition { column: 8, row: 3 })
        );
        assert_eq!(
            inputs[1].mouse_position,
            Some(TextPosition { column: 2, row: 0 })
        );
        assert_eq!(inputs[2].mouse_position, inputs[1].mouse_position);
        assert!(matches!(
            inputs[1].arguments,
            InputArguments::MouseClick {
                target: MouseTarget::Text { .. },
                ..
            }
        ));
        assert!(matches!(
            inputs[2].arguments,
            InputArguments::MouseClick {
                target: MouseTarget::Locator,
                ..
            }
        ));
        assert_eq!(
            inputs[1].sent_bytes.as_deref(),
            Some(&b"\x1b[<0;3;1M\x1b[<0;3;1m"[..])
        );
        assert_eq!(inputs[3].sent_bytes.as_deref(), Some(&b"a\xc3\xa9\n"[..]));
        assert_eq!(inputs[4].sent_bytes.as_deref(), Some(&b"deploy\r"[..]));
        assert_eq!(inputs[5].sent_bytes.as_deref(), Some(&b"\x1b[D"[..]));
        assert_eq!(inputs[6].sent_bytes.as_deref(), Some(&b""[..]));
        assert_eq!(inputs[7].sent_bytes, inputs[5].sent_bytes);
        Ok(())
    })();
    session.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
    result.unwrap();
}

#[test]
fn locator_composition_is_owner_checked_and_immutable() {
    let session = Session::new("same-name");
    let other = Session::new("same-name");
    let text = session.get_by_text("Docs");
    let link = session.clone().get_by_link("test:link");
    assert!(text.and(&link).is_ok());
    assert!(text.or(&other.get_by_text("Docs")).is_err());
    assert!(text
        .filter(LocatorFilterOptions {
            has: Some(other.get_by_link("test:link")),
            has_not: None
        })
        .is_err());
    assert!(text.filter(LocatorFilterOptions::default()).is_err());
    let selected = text.and(&link).unwrap().first();
    assert_eq!(selected.query().occurrence, MatchOccurrence::First);
    assert_eq!(text.query().occurrence, MatchOccurrence::Any);
    let registry = SessionRegistry::default();
    let a = registry.session("a");
    assert!(a.get_by_text("x").and(&a.clone().get_by_link("")).is_ok());
    assert!(a
        .get_by_text("x")
        .and(&registry.session("b").get_by_link(""))
        .is_err());
}

#[test]
fn link_locator_expressions_drive_live_actions() {
    let session = Session::new(format!("native-link-locator-{}", std::process::id()));
    let options = if cfg!(windows) {
        run_options("powershell.exe", &["-NoProfile", "-Command",
            "[Console]::Write(([string][char]27+'[1mA'+[char]27+']8;;test:link'+[char]7+'B'+[char]27+'[22mC'+[char]27+']8;;'+[char]7)); Start-Sleep -Seconds 30"])
    } else {
        run_options(
            "sh",
            &[
                "-c",
                "printf '\\033[1mA\\033]8;;test:link\\007B\\033[22mC\\033]8;;\\007'; sleep 30",
            ],
        )
    };
    session.run(options).unwrap();
    let result = (|| -> Result<(), termprobe::TermprobeError> {
        let bold = session.get_by_style(TextStyle {
            bold: Some(true),
            ..Default::default()
        });
        let link = session.get_by_link("test:link");
        let intersection = bold.and(&link)?;
        intersection.wait_with_timeout(Some(5_000))?;
        assert_eq!(intersection.location()?.text, "B");
        assert_eq!(bold.or(&link)?.location()?.text, "ABC");
        let parent = session.get_by_text("ABC");
        assert_eq!(
            parent
                .filter(LocatorFilterOptions {
                    has: Some(link),
                    has_not: None
                })?
                .location()?
                .text,
            "ABC"
        );
        assert_eq!(parent.get_by_link("test:link").count()?, 0);
        intersection.click()?;
        intersection.highlight()?;
        Ok(())
    })();
    session.close().unwrap();
    result.unwrap();
}

fn wait_for_exit(session: &Session) {
    session
        .execute(Operation::WaitExit {
            timeout_ms: Some(5_000),
        })
        .expect("wait for process exit");
}

fn process_exit_code(session: &Session) -> Option<i32> {
    let OperationResult::State(state) = session.execute(Operation::State).expect("read state")
    else {
        panic!("unexpected state result");
    };
    state.exited
}

#[cfg(unix)]
fn process_exit_signal(session: &Session) -> Option<String> {
    let OperationResult::State(state) = session.execute(Operation::State).expect("read state")
    else {
        panic!("unexpected state result");
    };
    state.exit_signal
}

#[test]
fn named_handles_share_a_process_local_terminal() {
    let name = format!("native-runtime-{}", std::process::id());
    let registry = global_registry();
    let first = registry.session(name.clone());
    let second = registry.session(name.clone());

    first
        .open(OpenOptions {
            recording: AutomaticRecording {
                mode: AutomaticRecordingMode::Always,
                ..Default::default()
            },
            ..Default::default()
        })
        .expect("open terminal");
    second
        .execute(Operation::Submit {
            data: Some("echo native-runtime".to_string()),
        })
        .expect("submit command");
    first
        .execute(Operation::WaitCommand {
            timeout_ms: Some(30_000),
        })
        .expect("wait for command");
    second
        .execute(Operation::WaitLocator {
            query: LocatorQuery::text("native-runtime"),
            not: false,
            timeout_ms: Some(5_000),
        })
        .expect("find command output");
    assert!(second
        .recording()
        .expect("read active recording")
        .contains("native-runtime"));

    assert!(registry.sessions().contains(&name));
    first.close().expect("close terminal");
    assert!(!registry.sessions().contains(&name));
    assert!(second
        .recording()
        .expect("read recording")
        .contains("native-runtime"));

    second
        .open(OpenOptions {
            wait_ready: Some(false),
            ..OpenOptions::default()
        })
        .expect("open replacement");
    first
        .close()
        .expect("close replacement through first handle");
}

#[test]
fn automatic_recording_supports_disabled_and_custom_directory() {
    let registry = SessionRegistry::default();
    let root = std::env::temp_dir().join(format!("termprobe-recording-mode-{}", std::process::id()));

    let disabled = registry.session("recording-disabled");
    let opened = disabled
        .open(OpenOptions {
            wait_ready: Some(false),
            recording: AutomaticRecording {
                mode: AutomaticRecordingMode::Disabled,
                directory: Some(root.clone()),
            },
            ..OpenOptions::default()
        })
        .unwrap();
    assert!(opened.recording.is_empty());
    disabled.close().unwrap();
    assert!(disabled.recording().is_err());
    assert!(!root.exists());

    let always = registry.session("recording-always");
    let opened = always
        .open(OpenOptions {
            wait_ready: Some(false),
            recording: AutomaticRecording {
                mode: AutomaticRecordingMode::Always,
                directory: Some(root.clone()),
            },
            ..OpenOptions::default()
        })
        .unwrap();
    let path = std::path::PathBuf::from(opened.recording);
    assert!(path.starts_with(&root));
    always.close().unwrap();
    assert!(always.recording().is_ok());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn named_session_close_preserves_execution_context() {
    let root = std::env::current_dir()
        .unwrap()
        .join("target")
        .join(format!("close-context-{}", std::process::id()));
    let registry = SessionRegistry::default();
    let mut contexts = Vec::new();
    for method in [
        "handle_close",
        "handle_execute",
        "handle_execute_with_context",
        "registry_execute",
    ] {
        let directory = root.join(method);
        let session = registry.session(method);
        session
            .execute_with_context(
                Operation::Open(OpenOptions {
                    wait_ready: Some(false),
                    recording: AutomaticRecording {
                        directory: Some(directory.join("recordings")),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                ExecutionContext {
                    trace: Some(termprobe::TraceOptions {
                        mode: termprobe::TraceMode::On,
                        directory: directory.join("traces"),
                    }),
                    diagnostic_context: [("opened".to_string(), method.to_string())].into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let context = ExecutionContext {
            diagnostic_context: [("closed".to_string(), method.to_string())].into(),
            ..Default::default()
        };
        match method {
            "handle_close" => session.with_execution_context(context).close().unwrap(),
            "handle_execute" => {
                session
                    .with_execution_context(context)
                    .execute(Operation::Close)
                    .unwrap();
            }
            "handle_execute_with_context" => {
                session
                    .execute_with_context(Operation::Close, context)
                    .unwrap();
            }
            _ => {
                registry
                    .execute_with_context(method, Operation::Close, context)
                    .unwrap();
            }
        }
        assert!(registry.sessions().is_empty());
        assert!(session.recording().is_ok());
        let traces: Vec<_> = std::fs::read_dir(directory.join("traces"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(traces.len(), 1);
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(traces[0].join("trace.json")).unwrap()).unwrap();
        contexts.push((method, manifest["context"].clone()));
        session.close().unwrap();
    }
    std::fs::remove_dir_all(root).unwrap();
    for (method, context) in contexts {
        assert_eq!(context["opened"], method);
        assert_eq!(context["closed"], method);
    }
}

#[test]
fn restart_preserves_recording_without_a_trace_override_and_honors_an_explicit_override() {
    let registry = SessionRegistry::default();
    let root = std::env::temp_dir().join(format!("termprobe-restart-policy-{}", std::process::id()));
    let session = registry.session("restart-recording-policy");
    let opened = session
        .open(OpenOptions {
            wait_ready: Some(false),
            recording: AutomaticRecording {
                mode: AutomaticRecordingMode::Always,
                directory: Some(root.clone()),
            },
            ..Default::default()
        })
        .unwrap();
    let OperationResult::Open(restarted) = session
        .execute(Operation::Restart {
            graceful_timeout_ms: 0,
        })
        .unwrap()
    else {
        panic!("restart must return the new session");
    };
    assert!(!opened.recording.is_empty());
    assert!(!restarted.recording.is_empty());
    assert!(std::path::Path::new(&restarted.recording).is_file());
    let OperationResult::Open(disabled) = registry
        .execute_with_context(
            session.name(),
            Operation::Restart {
                graceful_timeout_ms: 0,
            },
            ExecutionContext {
                trace: Some(termprobe::TraceOptions {
                    mode: termprobe::TraceMode::Off,
                    directory: root.join("traces"),
                }),
