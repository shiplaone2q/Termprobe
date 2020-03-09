use crate::{finish_trace_and_close, Result};
use termprobe::api::*;
use termprobe::runtime::global_registry;
use termprobe::{
    DiagnosticRetentionOptions, ExecutionContext, FailureArtifactMode, FailureArtifactOptions,
    TraceMode, TraceOptions,
};

include!("session_generated.rs");

fn artifact_mode(mode: &str) -> Result<FailureArtifactMode> {
    serde_json::from_value(mode.into()).map_err(|_| {
        TermprobeError::usage(format!(
            "unknown failure artifact mode {mode:?}; expected all, html, text, or none"
        ))
    })
}
fn trace_mode(mode: &str) -> Result<TraceMode> {
    serde_json::from_value(mode.into()).map_err(|_| {
