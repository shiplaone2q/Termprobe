use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::api::{
    AutomaticRecordingMode, LocatorDirection, LocatorSelector, MatchOccurrence, Size, TextMatch,
    TextPosition,
};
use crate::render::svg::RenderState;
use crate::terminal::cell::EmuCell;
use crate::terminal::emu::CursorShape;

mod expectation;
mod failure;
mod html;
mod input;
mod markdown;
pub(crate) mod strings;
#[cfg(test)]
mod test_fixture;
pub use expectation::{LocatorExpectation, OperationExpectation};
pub(crate) use failure::{comparison_failure, failure_reason, merge_failure_details};
pub use failure::{FailureDetails, LocatorFailure};
pub use input::{InputArguments, InputDetails, MouseTarget};

pub const FAILURE_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_SCREEN_HISTORY_LIMIT: u16 = 10;
pub const MAX_SCREEN_HISTORY_LIMIT: u16 = 50;

const FAILURE_JSON_LIMIT: usize = 2 * 1024 * 1024;
const REPORT_LIMIT: usize = 1024 * 1024;
const TIMELINE_LIMIT: usize = 8 * 1024 * 1024;
const HTML_LIMIT: usize = 128 * 1024 * 1024;
const SCREEN_TEXT_LIMIT: usize = 1024 * 1024;
const SCREEN_SVG_LIMIT: usize = 8 * 1024 * 1024;
pub(crate) const RECORDING_COPY_LIMIT: u64 = 64 * 1024 * 1024;
const ARTIFACT_TOTAL_LIMIT: u64 = 256 * 1024 * 1024;
const MAX_HISTORY_BYTES: usize = 512 * 1024;
const MAX_CHECKPOINT_BYTES: usize = 8 * 1024 * 1024;
const MAX_CONTEXT_ENTRIES: usize = 16;
const MAX_CONTEXT_KEY_BYTES: usize = 64;
const MAX_CONTEXT_VALUE_BYTES: usize = 256;
const MAX_CANDIDATES: usize = 64;
const MAX_MISMATCHES: usize = 64;
const MAX_OPERATION_HISTORY: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureReason {
    TimedOut,
    SessionExited,
    Cancelled,
    LocatorNoMatch,
    LocatorAmbiguous,
    UnexpectedMatch,
    MatchNotActionable,
    ScalarMismatch,
    SnapshotMismatch,
    EmulatorFault,
    InternalFailure,
    Completed,
    TestFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocatorFailureReason {
    AnchorNotFound,
    AnchorAmbiguous,
    RelativeRegionNoMatch,
    StyleFilterRemovedAll,
    LinkFilterRemovedAll,
    IntersectionEmpty,
    UnionEmpty,
    FilterRemovedAll,
    NthOutOfRange,
    OutsideViewport,
    MatchedNoCells,
    NoMatch,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OccurrenceSource {
    Explicit,
    ActionDefault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocatorStageMode {
    Text,
    ContiguousStyleRuns,
    ParentStyleFilter,
    ContiguousLinkRuns,
    ParentLinkFilter,
    Intersection,
    Union,
    ContainmentFilter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationDiagnostics {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    pub elapsed_ms: u64,
    pub started_screen_sequence: u64,
    pub failed_screen_sequence: u64,
}

impl OperationDiagnostics {
    pub fn pending(name: impl Into<String>, timeout_ms: Option<u64>) -> Self {
        Self {
            name: name.into(),
            timeout_ms,
            elapsed_ms: 0,
            started_screen_sequence: 0,
            failed_screen_sequence: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocatorStageDiagnostics {
    pub stage_index: usize,
    #[serde(default)]
    pub expression_path: String,
    #[serde(default)]
    pub evaluations: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<LocatorFailureReason>,
    pub mode: LocatorStageMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<LocatorSelector>,
    pub direction: LocatorDirection,
    pub requested_occurrence: MatchOccurrence,
    /// Action operations may require a unique match.
    pub effective_occurrence: MatchOccurrence,
    pub occurrence_source: OccurrenceSource,
    pub input_candidate_count: usize,
    pub raw_candidate_count: usize,
    /// Candidates remaining after applying the requested styles.
    pub style_candidate_count: usize,
    pub selected_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<TextMatch>,
    pub candidates_truncated: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mismatches: Vec<CellMismatch>,
    pub mismatches_truncated: bool,
}

impl LocatorStageDiagnostics {
    pub(crate) fn truncate(&mut self) {
        if self.candidates.len() > MAX_CANDIDATES {
            self.candidates.truncate(MAX_CANDIDATES);
            self.candidates_truncated = true;
        }
        if self.mismatches.len() > MAX_MISMATCHES {
            self.mismatches.truncate(MAX_MISMATCHES);
            self.mismatches_truncated = true;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellMismatch {
    pub location: TextPosition,
    pub grapheme: String,
    pub property: String,
    pub operator: String,
    pub expected: String,
    pub actual: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CellStyleEvaluation {
    pub matched: bool,
    pub mismatches: Vec<CellMismatch>,
    pub mismatches_truncated: bool,
}

impl CellStyleEvaluation {
    pub(crate) fn reject(&mut self, limit: usize, capture: impl FnOnce() -> CellMismatch) -> bool {
        self.matched = false;
        if self.mismatches.len() == limit {
            self.mismatches_truncated = true;
            return false;
        }
        self.mismatches.push(capture());
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocatorDiagnostics {
    /// Whether matching searched viewport or retained scrollback.
    pub search_scope: String,
    /// Absolute grid row where the viewport begins.
    pub viewport_origin_y: u32,
    pub stages: Vec<LocatorStageDiagnostics>,
    /// Candidates before the final occurrence is selected.
    pub final_candidate_count: usize,
    #[serde(default)]
    pub stages_truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation_error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected: Vec<TextMatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_stage: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<LocatorFailureReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationTransition {
    pub elapsed_ms: u64,
    pub screen_sequence: u64,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_index: Option<usize>,
    pub stage_counts: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationEvent {
    pub sequence: u64,
    pub name: String,
    pub started_ms: u64,
    pub ended_ms: u64,
    pub result: String,
    pub screen_before: u64,
    pub screen_at_return: u64,
    pub safe_summary: String,
    #[serde(default)]
    pub is_assertion: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expectation: Option<OperationExpectation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<InputDetails>,
}

#[derive(Debug)]
pub(crate) struct OperationHistory {
    next_sequence: u64,
    generation: u64,
    entries: VecDeque<OperationEvent>,
}

#[derive(Debug)]
pub(crate) struct PendingOperation {
    sequence: u64,
    name: String,
    started_at: Instant,
    started_ms: u64,
    screen_before: u64,
    safe_summary: String,
    is_assertion: bool,
    expectation: Option<OperationExpectation>,
    generation: u64,
}

impl PendingOperation {
    pub(crate) fn sequence(&self) -> u64 {
        self.sequence
    }
}

impl OperationHistory {
    pub(crate) fn new() -> Self {
        Self {
            next_sequence: 1,
            generation: 0,
            entries: VecDeque::new(),
        }
    }

    pub(crate) fn begin(
        &mut self,
        name: String,
        started_ms: u64,
        screen_before: u64,
        safe_summary: String,
        is_assertion: bool,
        expectation: Option<OperationExpectation>,
    ) -> PendingOperation {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1).max(1);
        PendingOperation {
            sequence,
            name,
            started_at: Instant::now(),
            started_ms,
            screen_before,
            safe_summary,
            is_assertion,
            expectation,
            generation: self.generation,
        }
    }

    pub(crate) fn reset_session(&mut self) {
        self.entries.clear();
        self.generation = self.generation.wrapping_add(1);
    }

    pub(crate) fn finish(
        &mut self,
        pending: PendingOperation,
        ended_ms: Option<u64>,
        screen_at_return: u64,
        result: impl Into<String>,
        input: Option<InputDetails>,
    ) {
        let started_ms = if pending.generation == self.generation {
            pending.started_ms
        } else {
            0
        };
        self.entries.push_back(OperationEvent {
            sequence: pending.sequence,
            name: pending.name,
            started_ms,
            ended_ms: ended_ms
                .unwrap_or_else(|| started_ms.saturating_add(elapsed_ms(pending.started_at))),
            result: result.into(),
            screen_before: if pending.generation == self.generation {
                pending.screen_before
            } else {
                0
            },
            screen_at_return,
            safe_summary: pending.safe_summary,
            is_assertion: pending.is_assertion,
            expectation: pending.expectation,
            input,
        });
        while self.entries.len() > MAX_OPERATION_HISTORY {
            self.entries.pop_front();
        }
    }

    pub(crate) fn snapshot(&self) -> Vec<OperationEvent> {
        self.entries.iter().cloned().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorDiagnostics {
    pub column: u16,
    pub row: u16,
    pub visible: bool,
    pub shape: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenSnapshotDetails {
    pub sequence: u64,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub repeat_count: u64,
    pub changes: Vec<String>,
    pub size: Size,
    pub cursor: CursorDiagnostics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenHistoryDetails {
    pub limit: u16,
    pub dropped_screen_count: u64,
    pub dropped_row_count: u64,
    #[serde(default)]
    pub dropped_checkpoint_count: u64,
    pub screens: Vec<ScreenSnapshotDetails>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoints: Vec<ScreenSnapshotDetails>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalDiagnostics {
    pub size: Size,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub cursor: CursorDiagnostics,
    pub last_visual_change_ms: u64,
    pub unchanged_for_ms: u64,
    pub screen_history: ScreenHistoryDetails,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessDiagnostics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_error: Option<String>,
    pub cancelled: bool,
    pub ready: bool,
    pub command_running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_command_exit: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDiagnostics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeouts: Option<crate::api::EffectiveTimeouts>,
    pub termprobe_version: String,
    pub backend: String,
    pub target_os: String,
    pub target_arch: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingStatus {
    Disabled,
    Unavailable,
    Live,
    Copied,
    Omitted,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingDiagnostics {
    pub mode: AutomaticRecordingMode,
    pub status: RecordingStatus,
    pub failure_offset_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_committed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub ephemeral: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticHint {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonDiagnostics {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
