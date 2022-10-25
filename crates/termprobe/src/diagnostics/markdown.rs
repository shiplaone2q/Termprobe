use super::{
    ArtifactFile, ArtifactFileStatus, FailureReason, FailureReport, ScreenSnapshotDetails,
    TraceOutcome,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Write;

#[derive(Serialize)]
pub(super) struct Explanation {
    title: String,
    expected: String,
    actual: String,
    note: String,
}

fn style_value(property: &str, value: &str) -> String {
    const ANSI: [&str; 16] = [
        "black",
        "red",
        "green",
        "yellow",
        "blue",
        "magenta",
        "cyan",
        "white",
        "bright black",
        "bright red",
        "bright green",
        "bright yellow",
        "bright blue",
        "bright magenta",
        "bright cyan",
        "bright white",
    ];
    if matches!(property, "foreground" | "background" | "underline_color") {
        if let Ok(index) = value.parse::<u8>() {
            return match ANSI.get(usize::from(index)) {
                Some(name) => format!("ANSI {index} ({name} slot)"),
                None => format!("ANSI {index}"),
            };
        }
    }
    value.to_string()
}

pub(super) fn explanation(details: &FailureReport) -> Explanation {
    use super::LocatorFailureReason;
    if let Some(locator) = &details.locator {
        let stage = locator
            .stages
            .iter()
            .find(|stage| Some(stage.stage_index) == locator.failure_stage)
            .or_else(|| {
                (!locator.stages_truncated)
                    .then(|| locator.stages.last())
                    .flatten()
            });
        if let Some(stage) = stage {
            let selector = stage.selector.as_ref().map_or_else(
                || stage.expression_path.clone(),
                |selector| format!("{:?}", selector.description()),
            );
            if matches!(
                locator.failure_reason,
                Some(
                    LocatorFailureReason::StyleFilterRemovedAll
                        | LocatorFailureReason::LinkFilterRemovedAll
                )
            ) {
                if let Some(mismatch) = stage.mismatches.first() {
                    let property = mismatch.property.replace('_', " ");
                    let mut actual = style_value(&mismatch.property, &mismatch.actual);
                    if let Some(resolved) = &mismatch.resolved {
                        actual.push_str(&format!("; resolved {resolved}"));
                    }
                    return Explanation {
                        title: match mismatch.property.as_str() {
                            "foreground" => "Foreground mismatch",
                            "background" => "Background mismatch",
                            "link" => "Hyperlink mismatch",
                            _ => "Cell style mismatch",
                        }
                        .into(),
                        expected: format!(
                            "{selector}: {property} {} {}",
                            mismatch.operator,
                            style_value(&mismatch.property, &mismatch.expected)
                        ),
                        actual: format!("The selector matched cells, but {property} was {actual}."),
                        note: format!(
                            "{} captured mismatches at {} (stage {}).",
                            stage.mismatches.len(),
                            stage.expression_path,
                            stage.stage_index
                        ),
                    };
                }
            }
            let (title, expected) = match locator.failure_reason {
                Some(LocatorFailureReason::IntersectionEmpty) => (
                    "Empty intersection",
                    "Cells selected by both operands to overlap".into(),
                ),
                Some(LocatorFailureReason::UnionEmpty) => (
                    "Empty union",
                    "At least one operand to select terminal cells".into(),
                ),
                Some(LocatorFailureReason::FilterRemovedAll) => (
                    "Containment filter rejected every candidate",
                    "A candidate to contain has matches and no has_not matches".into(),
                ),
                Some(LocatorFailureReason::AnchorNotFound) => {
                    ("Anchor not found", format!("An anchor matching {selector}"))
                }
                Some(LocatorFailureReason::AnchorAmbiguous | LocatorFailureReason::Ambiguous) => (
                    "Ambiguous locator",
                    format!("One unambiguous match for {selector}"),
                ),
                Some(LocatorFailureReason::OutsideViewport) => (
                    "Match outside viewport",
                    format!("{selector} inside the visible terminal"),
                ),
                _ if details.reason == FailureReason::UnexpectedMatch => (
                    "Unexpected match",
                    format!("No visible matches for {selector}"),
                ),
                _ => (
                    "Locator assertion failed",
                    format!(
                        "{selector} to satisfy the locator's text, style and occurrence filters"
                    ),
                ),
            };
            return Explanation {
                title: title.into(),
                expected,
                actual: format!(
                    "{} candidates before occurrence selection; stage {} had {} selector matches and {} style matches.",
                    locator.final_candidate_count,
                    stage.stage_index,
                    stage.raw_candidate_count,
                    stage.style_candidate_count
                ),
                note: format!("Expression: {}. {}", stage.expression_path, details.summary),
            };
        }
    }
    Explanation {
        title: match details.reason {
            FailureReason::Completed => "Session completed",
            FailureReason::TestFailed => "Test failed",
            FailureReason::Cancelled => "Operation cancelled",
            FailureReason::SessionExited => "Session exited",
            FailureReason::SnapshotMismatch => "Snapshot mismatch",
            FailureReason::EmulatorFault => "Emulator fault",
            FailureReason::TimedOut => "Operation timed out",
            _ => "Assertion failed",
        }
        .into(),
