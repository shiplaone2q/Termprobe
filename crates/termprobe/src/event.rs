use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::api::BellEvent;

const MAX_BELL_EVENTS: usize = 1024;

#[derive(Clone)]
pub(crate) struct BellTracker {
    inner: Arc<BellTrackerInner>,
}

struct BellTrackerInner {
    start: Instant,
    published_sequence: AtomicU64,
    state: Mutex<BellState>,
}

#[derive(Default)]
struct BellState {
    count: u64,
    sequence: u64,
    events: VecDeque<BellEvent>,
}

pub(crate) struct BellSnapshot {
    pub count: u64,
    pub events: Vec<BellEvent>,
}

impl Default for BellTracker {
    fn default() -> Self {
        Self::new(Instant::now())
    }
}

impl BellTracker {
    pub fn new(start: Instant) -> Self {
        Self {
            inner: Arc::new(BellTrackerInner {
                start,
                published_sequence: AtomicU64::new(0),
                state: Mutex::new(BellState::default()),
            }),
        }
    }

    pub fn count(&self) -> u64 {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .count
    }

    pub fn sequence(&self) -> u64 {
