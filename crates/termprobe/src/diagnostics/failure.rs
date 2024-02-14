use serde::{Deserialize, Serialize};

use super::strings::truncate_diagnostic_value;
use super::FailureObservation;
use super::{
    CellMismatch, ComparisonDiagnostics, FailureReason, FailureReport, LocatorFailureReason,
    FAILURE_SCHEMA_VERSION,
};
use crate::api::{ErrorKind, TextPosition, TermprobeError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureDetails {
    pub schema_version: u32,
    pub operation: String,
    pub reason: FailureReason,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locator: Option<LocatorFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comparison: Option<ComparisonDiagnostics>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocatorFailure {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<LocatorFailureReason>,
    /// Selector descriptions leading to the failing stage.
    pub selectors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_index: Option<usize>,
    /// Sample candidate starts in terminal grid coordinates.
    pub locations: Vec<TextPosition>,
    pub mismatches: Vec<CellMismatch>,
}

impl FailureDetails {
    pub fn new(
        operation: impl AsRef<str>,
        reason: FailureReason,
        summary: impl AsRef<str>,
    ) -> Self {
        let mut truncated = false;
        Self {
            schema_version: FAILURE_SCHEMA_VERSION,
            operation: bounded(operation.as_ref(), 256, &mut truncated),
            reason,
            summary: bounded(summary.as_ref(), 4096, &mut truncated),
            locator: None,
            comparison: None,
            truncated,
        }
    }
}

impl FailureReport {
    pub(crate) fn failure_details(&self) -> FailureDetails {
        let mut details = FailureDetails::new(&self.operation.name, self.reason, &self.summary);
        let truncated = &mut details.truncated;
        *truncated |= self.truncated;
        details.comparison = self
            .comparison
            .as_ref()
            .map(|comparison| bounded_comparison(comparison, 1024, truncated));
        if let Some(comparison) = &details.comparison {
            if comparison.kind == "snapshot" {
                if let (Some(expected), Some(actual)) = (&comparison.expected, &comparison.actual) {
                    details.summary = format!(
                        "snapshot mismatch\n--- expected ---\n{expected}\n--- actual ---\n{actual}"
                    );
                }
            }
        }
        details.locator = self.locator.as_ref().map(|locator| {
            let stage = locator
                .failure_stage
                .and_then(|index| {
                    locator
                        .stages
                        .iter()
                        .find(|stage| stage.stage_index == index)
                })
                .or_else(|| locator.stages.last());
            let candidates = stage.map_or(locator.selected.as_slice(), |stage| &stage.candidates);
            let mismatches = stage.map_or(&[][..], |stage| stage.mismatches.as_slice());
            *truncated |= candidates.len() > 4
                || mismatches.len() > 4
                || stage
                    .is_some_and(|stage| stage.candidates_truncated || stage.mismatches_truncated);
            let mut selectors = Vec::new();
            for entry in &locator.stages {
                if let Some(selector) = &entry.selector {
                    if selectors.len() == 8 {
                        *truncated = true;
                        break;
