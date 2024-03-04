use super::*;
use crate::diagnostics::{
    CellMismatch, LocatorStageDiagnostics, LocatorStageMode, OccurrenceSource,
};

const MAX_STAGES: usize = 128;
const MAX_SAMPLE: usize = 64;
const MAX_STAGE_BYTES: usize = 8 * 1024;

fn bound_stage(stage: &mut LocatorStageDiagnostics) -> bool {
    let mut truncated = false;
    while serde_json::to_vec(stage)
        .expect("diagnostic stage contains only serializable values")
        .len()
        > MAX_STAGE_BYTES
    {
        truncated = true;
        if !stage.candidates.is_empty() {
            stage.candidates.pop();
            stage.candidates_truncated = true;
        } else if !stage.mismatches.is_empty() {
            stage.mismatches.pop();
            stage.mismatches_truncated = true;
        } else {
            stage.selector = None;
            break;
        }
    }
    truncated
}

#[derive(Default)]
pub(super) struct Trace {
    pub enabled: bool,
    pub stages: Vec<LocatorStageDiagnostics>,
    pub truncated: bool,
}

#[derive(Default)]
pub(super) struct NodeStats {
    pub input: usize,
    pub raw: usize,
    pub styled: usize,
    pub capture_candidates: bool,
    pub candidate_count: usize,
    pub candidates: Vec<TextMatch>,
    pub selected_count: usize,
    pub mismatches: Vec<CellMismatch>,
    pub mismatches_truncated: bool,
    pub mismatch_limit: usize,
    pub reason: Option<LocatorFailureReason>,
}

impl NodeStats {
    pub fn samples_ambiguity(&self, occurrence: &MatchOccurrence, count: usize) -> bool {
        self.capture_candidates && *occurrence == MatchOccurrence::Unique && count > 1
    }

    pub fn sample_ambiguous_ranges(
        &mut self,
        rows: &[Vec<EmuCell>],
        flat: &FlatGrid,
        ranges: &[(usize, usize)],
        occurrence: &MatchOccurrence,
    ) {
        if self.samples_ambiguity(occurrence, ranges.len()) {
            self.candidate_count = ranges.len();
            self.candidates = sample_values(
                ranges
                    .iter()
                    .filter_map(|range| materialize(rows, flat, *range))
                    .map(|matched| matched.value),
            );
        }
    }

    pub fn mismatch_budget(&self) -> usize {
        self.mismatch_limit.saturating_sub(self.mismatches.len())
    }

    pub fn mismatch(&mut self, mut mismatch: CellMismatch) {
        for value in [
            &mut mismatch.grapheme,
            &mut mismatch.expected,
            &mut mismatch.actual,
        ] {
            self.mismatches_truncated |= truncate(value);
        }
        if self.mismatch_budget() > 0 {
            self.mismatches.push(mismatch);
        } else {
            self.mismatches_truncated = true;
        }
    }
}

impl Trace {
    pub fn mismatch_budget(&self, path: &str) -> usize {
        if !self.enabled {
            return 0;
        }
        self.stages
            .iter()
            .find(|stage| stage.expression_path == path)
            .map_or(MAX_SAMPLE, |stage| {
                if stage.mismatches_truncated {
                    0
                } else {
                    MAX_SAMPLE.saturating_sub(stage.mismatches.len())
                }
            })
    }

    pub fn record(
        &mut self,
        query: &LocatorQuery,
        path: &str,
        require_one: bool,
        stats: NodeStats,
    ) -> Option<usize> {
        if !self.enabled {
            return None;
        }
        if let Some(stage) = self
            .stages
            .iter_mut()
            .find(|stage| stage.expression_path == path)
        {
            stage.evaluations += 1;
            stage.input_candidate_count = stage.input_candidate_count.saturating_add(stats.input);
            stage.raw_candidate_count = stage.raw_candidate_count.saturating_add(stats.raw);
            stage.style_candidate_count = stage.style_candidate_count.saturating_add(stats.styled);
            stage.selected_count = stage.selected_count.saturating_add(stats.selected_count);
            stage.failure_reason = stats.reason;
            stage.candidates.extend(stats.candidates);
