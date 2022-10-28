use std::time::{Duration, Instant};

use crate::render::svg::RenderState;
use crate::terminal::cell::EmuCell;
use crate::terminal::emu::Emulator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub grid: Vec<Vec<EmuCell>>,
    pub title: Option<String>,
    pub duration: Duration,
    pub(crate) render_state: RenderState,
    pub(crate) cursor: Option<(u16, usize)>,
}

impl Frame {
    pub(crate) fn dimensions(&self) -> anyhow::Result<(u16, usize)> {
        let rows = self.grid.len();
        let cols = self.grid.first().map_or(0, Vec::len);
        if rows == 0 || cols == 0 {
            anyhow::bail!("recording frame dimensions must be non-zero");
        }
        if self.grid.iter().any(|row| row.len() != cols) {
            anyhow::bail!("recording frame rows have inconsistent widths");
        }
        Ok((
            cols.try_into()
                .map_err(|_| anyhow::anyhow!("recording frame width exceeds u16"))?,
            rows,
        ))
    }

    fn same_visual_state(&self, other: &Self) -> bool {
        self.grid == other.grid
            && self.title == other.title
            && self.render_state == other.render_state
            && self.cursor == other.cursor
    }
}

pub(crate) fn max_dimensions(frames: &[Frame]) -> anyhow::Result<(u16, usize)> {
    if frames.is_empty() {
        anyhow::bail!("recording timeline contains no frames");
    }
    let mut max_cols = 0;
    let mut max_rows = 0;
    for frame in frames {
        let (cols, rows) = frame.dimensions()?;
        max_cols = max_cols.max(cols);
        max_rows = max_rows.max(rows);
    }
    Ok((max_cols, max_rows))
}
#[derive(Debug, Clone)]
pub(crate) struct TimelineOptions {
    pub fps: u8,
    pub speed: f64,
    pub idle_time_limit: Duration,
    pub last_frame_duration: Duration,
}

impl Default for TimelineOptions {
    fn default() -> Self {
        Self {
            fps: 30,
            speed: 1.0,
            idle_time_limit: Duration::from_secs(5),
            last_frame_duration: Duration::from_secs(3),
        }
    }
}

pub(crate) struct FrameCapture {
    started: Instant,
    options: TimelineOptions,
    clock: TimelineClock,
    pending: TimedGrid,
    collector: FrameCollector,
    error: Option<anyhow::Error>,
}

impl FrameCapture {
    pub(crate) fn new(
        emulator: &dyn Emulator,
        options: TimelineOptions,
        started: Instant,
    ) -> anyhow::Result<Self> {
        validate_options(&options)?;
        let mut clock = TimelineClock::default();
        clock.advance(0.0, &options)?;
        Ok(Self {
            started,
            collector: FrameCollector::new(Duration::from_secs_f64(1.0 / f64::from(options.fps))),
            options,
            clock,
            pending: TimedGrid::capture(emulator, Duration::ZERO)?,
            error: None,
        })
    }

    pub(crate) fn capture(&mut self, emulator: &dyn Emulator, at: Instant) {
        if self.error.is_none() {
            if let Err(error) = self.push(emulator, at) {
                self.error = Some(error);
            }
        }
    }

    fn push(&mut self, emulator: &dyn Emulator, at: Instant) -> anyhow::Result<()> {
        let at = self.clock.advance(
            at.saturating_duration_since(self.started).as_secs_f64(),
