//! Typed start functions copy borrowed inputs on the caller thread. Only owned
//! operations reach worker threads, so waits cannot exhaust a foreign worker pool.
use super::*;
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, TryRecvError};

type Completion = Result<OperationResult>;

thread_local! {
    static START: RefCell<Option<Option<Receiver<Completion>>>> = const { RefCell::new(None) };
    static TASKS: RefCell<ThreadTasks> = RefCell::new(ThreadTasks::default());
}

#[derive(Default)]
struct ThreadTasks(HashSet<*mut TuiTask>);

impl Drop for ThreadTasks {
    fn drop(&mut self) {
        for task in self.0.drain() {
            drop(unsafe { Box::from_raw(task) });
        }
    }
}

/// A thread-owned asynchronous call. Start, poll, and free it on the same thread.
/// Thread exit frees any remaining tasks, including their unclaimed results.
/// Freeing a pending task discards its eventual result without canceling the call.
pub struct TuiTask {
    receiver: Option<Receiver<Completion>>,
    immediate: *mut TuiResult,
}

impl Drop for TuiTask {
    fn drop(&mut self) {
        unsafe { output::free(self.immediate) };
    }
}

pub(super) fn execute(work: impl FnOnce() -> Completion + Send + 'static) -> *mut TuiResult {
    if !START.with(|slot| slot.borrow().is_some()) {
        return boundary(work);
    }
    let (sender, receiver) = mpsc::channel();
    START.with(|slot| *slot.borrow_mut() = Some(Some(receiver)));
    match std::thread::Builder::new()
