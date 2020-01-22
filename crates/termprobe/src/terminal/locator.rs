//! Text/regex search over the terminal grid, including scoped and normalized
//! selectors. Match offsets are mapped back to terminal cells.

use regex::Regex;
use std::cell::OnceCell;

use crate::api::{
    LocatorDirection, LocatorQuery, LocatorSelector, MatchOccurrence, StyleSelector, TextAnchor,
    TextMatch, TextPosition, TextSelector, TextSpan, TextStyle, WhitespaceMode,
};

use super::cell::EmuCell;
use crate::diagnostics::{
    CellMismatch, CellStyleEvaluation, LocatorDiagnostics, LocatorFailureReason,
};

mod trace;
use trace::{NodeStats, Trace};

pub(crate) struct LocatorEvaluation {
    pub matches: Vec<LocatedMatch>,
    pub diagnostics: LocatorDiagnostics,
}

#[derive(Debug)]
struct SelectionFailure {
    reason: LocatorFailureReason,
    count: usize,
    message: String,
}
impl std::fmt::Display for SelectionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for SelectionFailure {}

#[derive(Debug)]
struct EvaluationFailure {
    stage: Option<usize>,
    reason: LocatorFailureReason,
    message: String,
}
impl std::fmt::Display for EvaluationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for EvaluationFailure {}

struct NodeOutcome {
    matches: Vec<LocatedMatch>,
    failure: Option<(Option<usize>, LocatorFailureReason)>,
    candidate_count: usize,
}

pub(crate) fn evaluate_query<F>(
    rows: &[Vec<EmuCell>],
    query: &LocatorQuery,
    require_one: bool,
    style: &mut F,
) -> anyhow::Result<LocatorEvaluation>
where
    F: FnMut(&EmuCell, &TextStyle, usize, usize, usize) -> CellStyleEvaluation,
{
    let mut trace = Trace {
        enabled: true,
        ..Trace::default()
    };
    let outcome = evaluate_node(
        &QueryGrid::new(rows),
        query,
        None,
        style,
        &mut trace,
        "root",
        require_one,
    );
    let (matches, failure, final_candidate_count, error) = match outcome {
        Ok(value) => (value.matches, value.failure, value.candidate_count, None),
        Err(error) => {
            let Some(failure) = error.downcast_ref::<EvaluationFailure>() else {
                return Err(error);
            };
            let count = failure
                .stage
                .and_then(|i| trace.stages.get(i))
                .map_or(0, |stage| stage.style_candidate_count);
            let mut message = error.to_string();
            trace.truncated |= trace::truncate(&mut message);
            (
                Vec::new(),
                Some((failure.stage, failure.reason)),
                count,
                Some(message),
            )
        }
    };
    Ok(LocatorEvaluation {
        diagnostics: LocatorDiagnostics {
            search_scope: if query.uses_full_grid() {
                "full_grid"
            } else {
                "viewport"
            }
            .into(),
            viewport_origin_y: 0,
            final_candidate_count,
            selected: trace::sample(&matches),
            stages: trace.stages,
            stages_truncated: trace.truncated,
            failure_stage: failure.and_then(|(stage, _)| stage),
            failure_reason: failure.map(|(_, reason)| reason),
            evaluation_error: error,
        },
        matches,
    })
}

pub enum Pattern {
    Text(String),
    Regex(Regex),
}

impl Pattern {
    pub fn new(text: &str, is_regex: bool) -> anyhow::Result<Self> {
        if is_regex {
            Ok(Pattern::Regex(Regex::new(text)?))
        } else {
            Ok(Pattern::Text(text.to_string()))
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Pattern::Text(text) => text.clone(),
            Pattern::Regex(regex) => regex.as_str().to_string(),
        }
    }

    pub fn matches(&self, haystack: &str) -> bool {
        match self {
            Pattern::Text(text) => haystack.contains(text.as_str()),
            Pattern::Regex(regex) => regex.is_match(haystack),
        }
    }

    fn ranges(&self, chars: &[char]) -> Vec<(usize, usize)> {
        match self {
            Pattern::Text(text) => {
                let needle: Vec<char> = text.chars().collect();
                text_ranges(chars, &needle)
            }
            Pattern::Regex(regex) => {
                let block: String = chars.iter().collect();
                let mut byte_offset = 0;
                let mut char_offset = 0;
                regex
                    .find_iter(&block)
                    .filter_map(|matched| {
                        if matched.is_empty() {
                            return None;
                        }
                        char_offset += block[byte_offset..matched.start()].chars().count();
                        let start = char_offset;
                        char_offset += matched.as_str().chars().count();
                        byte_offset = matched.end();
                        Some((start, char_offset))
                    })
                    .collect()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct MatchedCell {
    pub x: usize,
    pub y: usize,
    pub cell: EmuCell,
}

#[derive(Debug, Clone)]
pub struct LocatedMatch {
    pub value: TextMatch,
    pub cells: Vec<MatchedCell>,
    pub(crate) source_start: usize,
    pub(crate) source_end: usize,
}

struct FlatGrid {
    chars: Vec<char>,
    sources: Vec<usize>,
    source_ends: Vec<usize>,
    width: usize,
}

impl FlatGrid {
    fn within(&self, (start, end): (usize, usize)) -> (usize, usize) {
        (
            self.sources.partition_point(|source| *source < start),
            self.source_ends.partition_point(|source| *source <= end),
        )
    }
}

struct QueryGrid<'a> {
    rows: &'a [Vec<EmuCell>],
    width: usize,
    exact: OnceCell<FlatGrid>,
    normalized: OnceCell<FlatGrid>,
    #[cfg(test)]
    flattenings: std::cell::Cell<usize>,
    #[cfg(test)]
    scanned_cells: std::cell::Cell<usize>,
}

impl<'a> QueryGrid<'a> {
    fn new(rows: &'a [Vec<EmuCell>]) -> Self {
        Self {
            rows,
            width: rows.iter().map(Vec::len).max().unwrap_or(0),
            exact: OnceCell::new(),
            normalized: OnceCell::new(),
            #[cfg(test)]
            flattenings: std::cell::Cell::new(0),
            #[cfg(test)]
            scanned_cells: std::cell::Cell::new(0),
        }
    }

    fn flat(&self, whitespace: WhitespaceMode) -> &FlatGrid {
        let cache = match whitespace {
            WhitespaceMode::Exact => &self.exact,
            WhitespaceMode::Normalize => &self.normalized,
        };
        cache.get_or_init(|| {
            #[cfg(test)]
            self.flattenings.set(self.flattenings.get() + 1);
            flatten(self.rows, whitespace)
        })
    }
}

/// Locate the matches selected by `selector`.
pub fn locate(rows: &[Vec<EmuCell>], selector: &TextSelector) -> anyhow::Result<Vec<LocatedMatch>> {
    locate_text_within(&QueryGrid::new(rows), selector, None, None, None)
}

pub fn locate_query<F>(
    rows: &[Vec<EmuCell>],
    query: &LocatorQuery,
    style_matches: &mut F,
) -> anyhow::Result<Vec<LocatedMatch>>
where
    F: FnMut(&EmuCell, &TextStyle) -> bool,
{
    locate_query_within(&QueryGrid::new(rows), query, None, style_matches)
}

fn locate_query_within<F>(
    grid: &QueryGrid<'_>,
    query: &LocatorQuery,
    enclosing: Option<&[(usize, usize)]>,
    style_matches: &mut F,
) -> anyhow::Result<Vec<LocatedMatch>>
where
    F: FnMut(&EmuCell, &TextStyle) -> bool,
{
    evaluate_node(
        grid,
        query,
        enclosing,
        &mut |cell, style, _, _, _| CellStyleEvaluation {
            matched: style_matches(cell, style),
            mismatches: Vec::new(),
            mismatches_truncated: false,
        },
        &mut Trace::default(),
        "root",
        false,
    )
    .map(|outcome| outcome.matches)
}

#[allow(clippy::too_many_arguments)]
fn evaluate_node<F>(
    grid: &QueryGrid<'_>,
    query: &LocatorQuery,
    enclosing: Option<&[(usize, usize)]>,
    style_evaluate: &mut F,
    trace: &mut Trace,
    path: &str,
    require_one: bool,
) -> anyhow::Result<NodeOutcome>
where
    F: FnMut(&EmuCell, &TextStyle, usize, usize, usize) -> CellStyleEvaluation,
{
    let mut stats = NodeStats {
        capture_candidates: trace.enabled,
        mismatch_limit: trace.mismatch_budget(path),
        ..NodeStats::default()
    };
    let mut inherited_failure = None;
    let outcome = (|| -> anyhow::Result<Vec<LocatedMatch>> {
        let mut parents = match query.within.as_deref() {
            Some(parent) => {
                let parent = evaluate_node(
                    grid,
                    parent,
                    enclosing,
                    style_evaluate,
                    trace,
                    &format!("{path}.within"),
                    false,
                )?;
                inherited_failure = parent.failure;
                let mut parents = parent.matches;
                if parents.is_empty() {
                    stats.reason = Some(LocatorFailureReason::RelativeRegionNoMatch);
                    return Ok(Vec::new());
                }
                parents.sort_by_key(|matched| matched.source_start);
                Some(parents)
            }
            None => None,
        };
        stats.input = parents.as_ref().map_or(1, Vec::len);
        let relative = parents.as_ref().map(|parents| {
            relative_regions(
                parents,
                query.direction,
                grid.width.saturating_mul(grid.rows.len()),
            )
        });
        let allowed = match (relative, enclosing) {
            (Some(relative), Some(enclosing)) => Some(
                relative
                    .iter()
                    .flat_map(|region| intersect_ranges(std::slice::from_ref(region), enclosing))
                    .collect(),
            ),
            (Some(relative), None) => Some(relative),
            (None, Some(enclosing)) => Some(enclosing.to_vec()),
            (None, None) => None,
        };
        let occurrence = if require_one && query.occurrence == MatchOccurrence::Any {
            MatchOccurrence::Unique
        } else {
            query.occurrence.clone()
        };
        let select_early = query.style.is_empty();
        let mut selected_early = false;
        let mut matches = match &query.selector {
            LocatorSelector::Text(selector) => {
                selected_early = select_early;
                locate_text_observed(
                    grid,
                    selector,
                    allowed.as_deref(),
                    select_early.then_some(&occurrence),
                    enclosing,
                    &mut stats,
                )?
            }
            LocatorSelector::Style(selector)
                if query.direction == LocatorDirection::Within && parents.is_some() =>
            {
                let mut matches = parents.take().expect("parent matches are present");
                stats.raw = matches.len();
                matches.retain(|matched| {
                    evaluate_match_style(matched, &selector.style, style_evaluate, &mut stats)
                });
                if matches.is_empty() && stats.raw > 0 {
                    stats.reason = Some(LocatorFailureReason::StyleFilterRemovedAll);
                }
                matches
            }
            LocatorSelector::Style(selector) => {
                selected_early = select_early;
                locate_style_observed(
                    grid,
                    selector,
                    allowed.as_deref(),
                    select_early.then_some(&occurrence),
                    &mut |cell, style| style_evaluate(cell, style, 0, 0, 0).matched,
                    &mut stats,
                )?
            }
            LocatorSelector::Link(selector)
                if query.direction == LocatorDirection::Within && parents.is_some() =>
            {
                let mut matches = parents.take().expect("parent matches are present");
                stats.raw = matches.len();
                matches.retain(|matched| {
                    let mut matched_all = true;
                    for cell in &matched.cells {
                        if !matched_all && stats.mismatch_budget() == 0 {
                            stats.mismatches_truncated = true;
                            break;
                        }
                        if !cell_has_link(&cell.cell, &selector.uri) {
                            matched_all = false;
                            if stats.mismatch_budget() == 0 {
                                stats.mismatches_truncated = true;
                                break;
                            }
                            stats.mismatch(CellMismatch {
