use std::collections::HashMap;
use std::io::Write as IoWrite;
use std::io::{self, Read};
use std::path::Path;

use base64::Engine as _;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    ArtifactFile, FailureObservation, FailureReport, ScreenSnapshotDetails, TIMELINE_LIMIT,
};
use crate::api::CellColor;
use crate::render::svg::{self, RenderColors};
use crate::terminal::cell::{Attrs, EmuCell, CONTINUATION};

use super::markdown::{explanation, render as markdown, Explanation};

#[derive(Serialize)]
pub(super) struct Timeline {
    schema_version: u32,
    failure_screen_sequence: u64,
    geometry: Geometry,
    frames: Vec<Frame>,
    truncated: bool,
}

impl Timeline {
    pub(super) fn is_truncated(&self) -> bool {
        self.truncated
    }
}

#[derive(Serialize)]
struct Geometry {
    grid_x: f32,
    grid_y: f32,
    cell_width: f32,
    cell_height: f32,
}

#[derive(Serialize)]
struct Frame {
    #[serde(flatten)]
    screen: ScreenSnapshotDetails,
    /// Each grid entry indexes this dictionary, including blank/continuation cells.
    cells: Vec<FrameCell>,
    grid: Vec<Vec<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    svg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    omission: Option<String>,
}

#[derive(Serialize)]
struct FrameCell {
    char: String,
    width: u8,
    fg: CellColor,
    bg: CellColor,
    underline_color: CellColor,
    underline_style: &'static str,
    flags: Vec<&'static str>,
    resolved_fg: String,
    resolved_bg: String,
    resolved_underline_color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    link_id: Option<String>,
}

impl FrameCell {
    fn capture(cell: &EmuCell, width: u8, colors: &dyn RenderColors) -> Self {
        let style = svg::cell_paint(cell, colors);
        Self {
            char: cell.ch.to_string(),
            link: cell.uri().map(str::to_string),
            link_id: cell
                .hyperlink
                .as_ref()
                .and_then(|link| link.id.as_ref())
                .map(ToString::to_string),
            width,
            fg: crate::engine::cell_color(cell.fg),
            bg: crate::engine::cell_color(cell.bg),
            underline_color: crate::engine::cell_color(cell.underline_color),
            underline_style: cell.underline.name(),
            flags: [
                (Attrs::BOLD, "bold"),
                (Attrs::DIM, "dim"),
                (Attrs::ITALIC, "italic"),
                (Attrs::INVERSE, "inverse"),
                (Attrs::INVISIBLE, "invisible"),
                (Attrs::STRIKE, "strike"),
                (Attrs::BLINK, "blink"),
            ]
            .into_iter()
            .filter_map(|(flag, name)| cell.has(flag).then_some(name))
            .collect(),
            resolved_fg: style.fg.to_hex(),
            resolved_bg: svg::bg_of(cell, colors).to_hex(),
            resolved_underline_color: cell
                .underline_color
                .map_or(style.fg, |color| colors.resolve(Some(color), true))
                .to_hex(),
        }
    }
}

pub(super) fn timeline(observation: &FailureObservation) -> Result<Timeline, serde_json::Error> {
    timeline_with_limit(observation, TIMELINE_LIMIT)
}

fn timeline_with_limit(
    observation: &FailureObservation,
    limit: usize,
) -> Result<Timeline, serde_json::Error> {
    let sources = observation.history.retained();
    let mut timeline = Timeline {
        schema_version: 1,
        failure_screen_sequence: observation.screen_sequence,
        geometry: {
            let style = &observation.style;
            Geometry {
                grid_x: style.canvas_left() as f32 + style.content_left(),
                grid_y: style.canvas_top() as f32 + style.header_height() + style.content_top(),
                cell_width: style.cell_width(),
                cell_height: style.cell_height(),
            }
        },
        frames: sources
            .values()
            .map(|source| Frame {
                screen: source.details.clone(),
                cells: Vec::new(),
                grid: Vec::new(),
                svg: None,
                omission: Some(
                    if source.rows.iter().map(Vec::len).sum::<usize>() > 100_000 {
                        "Cell grid and SVG omitted: frame exceeds 100,000 cells."
                    } else {
                        "Cell grid and SVG omitted: timeline byte limit."
                    }
                    .into(),
                ),
            })
            .collect(),
        truncated: false,
    };
    // Omitted frames still retain text and metadata. Reserve their actual encoded
    // size before giving the newest evidence the remaining grid/SVG budget.
    let metadata_bytes = serde_json::to_vec(&timeline)?.len();
    if metadata_bytes > limit {
        return Err(serde::ser::Error::custom(
            "frame text and metadata exceed the timeline byte limit",
        ));
    }
    let mut remaining = limit.saturating_sub(metadata_bytes.max(64 * 1024));
    for (frame, source) in timeline.frames.iter_mut().zip(sources.values()).rev() {
        let reserved = serde_json::to_vec(&frame)?.len();
        if source.rows.iter().map(Vec::len).sum::<usize>() <= 100_000 {
            frame.omission =
                Some("SVG omitted: timeline byte limit. Cell metadata is retained.".into());
            let mut dictionary = HashMap::new();
            for row in source.rows.iter() {
                let mut indices = Vec::with_capacity(row.len());
                for (x, cell) in row.iter().enumerate() {
                    let width = if cell.ch == CONTINUATION {
                        0
                    } else if row.get(x + 1).is_some_and(|next| next.ch == CONTINUATION) {
                        2
                    } else {
                        1
                    };
                    let value = FrameCell::capture(cell, width, &source.render_state);
                    let key = serde_json::to_string(&value)?;
                    let index = *dictionary.entry(key).or_insert_with(|| {
                        let index = frame.cells.len();
                        frame.cells.push(value);
                        index
                    });
                    indices.push(index);
                }
                frame.grid.push(indices);
            }
