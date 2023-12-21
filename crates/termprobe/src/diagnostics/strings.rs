use super::{DiagnosticHint, FailureReason, FailureReport, LocatorFailureReason};
use crate::api::{LocatorQuery, Operation};

pub(crate) fn format_timeout(timeout_ms: u64) -> String {
    if timeout_ms.is_multiple_of(1_000) {
        format!("{}s", timeout_ms / 1_000)
    } else {
        format!("{timeout_ms}ms")
    }
}

pub(crate) fn diagnostic_hints(details: &FailureReport) -> Vec<DiagnosticHint> {
    let mut hints = Vec::new();
    match details.reason {
        FailureReason::LocatorAmbiguous => hints.push(DiagnosticHint {
            code: "choose_occurrence".to_string(),
            message:
                "Narrow the locator or choose first(), last(), or nth() when multiple matches are expected."
                    .to_string(),
        }),
        FailureReason::LocatorNoMatch => {
            if let Some(locator) = &details.locator {
                if locator.failure_reason == Some(LocatorFailureReason::StyleFilterRemovedAll) {
                    hints.push(DiagnosticHint {
                        code: "inspect_style_mismatch".to_string(),
                        message:
                            "The selector matched candidates, but their styles did not match."
                                .to_string(),
                    });
                } else if let Some(stage) = locator.failure_stage {
                    hints.push(DiagnosticHint {
                        code: "inspect_locator_stage".to_string(),
                        message: format!(
                            "Inspect locator stage {stage}; it produced no selected candidates."
                        ),
                    });
                }
            }
        }
        FailureReason::MatchNotActionable => hints.push(DiagnosticHint {
            code: "make_match_visible".to_string(),
            message:
                "The locator matched, but the result was not actionable in the visible viewport."
                    .to_string(),
        }),
        FailureReason::SessionExited => hints.push(DiagnosticHint {
            code: "inspect_process_exit".to_string(),
            message: "Inspect the process exit code and the final recording output.".to_string(),
        }),
        FailureReason::TimedOut => hints.push(DiagnosticHint {
            code: "inspect_last_change".to_string(),
            message:
                "Inspect the retained screen transitions and the last successful operation before the timeout."
                    .to_string(),
        }),
        _ => {}
    }
    hints
}

pub(crate) fn base_error_message(message: &str) -> String {
    message
        .split_once("\n\nTerminal content:\n")
        .map_or(message, |(base, _)| base)
        .to_string()
}

pub(crate) fn diagnostic_operation_name(operation: &Operation) -> &'static str {
    match operation {
        Operation::Open(_) => "open",
        Operation::Run(_) => "run",
        Operation::Restart { .. } => "restart",
        Operation::Close => "close",
        Operation::FinishTrace { .. } => "trace.finish",
        Operation::State => "state",
        Operation::Text { .. } => "text",
        Operation::PackedScreen { .. } => "packed_screen",
        Operation::Cells { .. } => "cells",
        Operation::GetCommand => "get.command",
        Operation::GetOutput => "get.output",
        Operation::GetExitCode => "get.exit_code",
        Operation::GetCwd => "get.cwd",
