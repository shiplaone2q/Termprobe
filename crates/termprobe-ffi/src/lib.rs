//! Typed C boundary shared by all language adapters.
//! C ABI input buffers must remain valid for the call. The adapter copies inputs
//! before execution; C output storage belongs to Rust until tui_result_free.
mod input;
mod output;
mod session;
mod tasks;
mod types;
use std::panic::{catch_unwind, AssertUnwindSafe};
pub use tasks::TuiTask;
use termprobe::api::*;
use termprobe::runtime::global_registry;
pub use types::*;

mod tasks_generated;
pub use tasks_generated::*;

pub const ABI_VERSION: u32 = 5;

type Result<T> = std::result::Result<T, TermprobeError>;
fn boundary(f: impl FnOnce() -> Result<OperationResult>) -> *mut TuiResult {
    match catch_unwind(AssertUnwindSafe(|| f().map(output::operation))) {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => output::error(error),
        Err(_) => output::error(TermprobeError::internal("native engine adapter panicked")),
    }
}
unsafe fn execute(
    session: TuiString,
    operation_name: &'static str,
    operation: impl FnOnce() -> Result<Operation>,
) -> *mut TuiResult {
    let prepared = catch_unwind(AssertUnwindSafe(|| {
        let (session, context) = unsafe { input::session(session)? };
        Ok((session, context, operation()?))
    }));
    match prepared {
        Ok(Ok((session, context, operation))) => {
            tasks::execute(move || session::execute(session, context, operation_name, operation))
        }
        Ok(Err(error)) => output::error(error),
        Err(_) => output::error(TermprobeError::internal("native engine adapter panicked")),
    }
}

fn finish_trace_and_close(
    finish_trace: impl FnOnce() -> Result<OperationResult>,
    close: impl FnOnce() -> Result<OperationResult>,
) -> Result<OperationResult> {
    let trace = finish_trace();
    let closed = close();
    if let Err(error) = trace {
        if error.kind != ErrorKind::NoSession {
            return Err(error);
        }
    }
    closed
}
#[no_mangle]
pub extern "C" fn tui_abi_version() -> u32 {
    ABI_VERSION
}
#[no_mangle]
/// # Safety
/// The session buffer must remain readable throughout this call.
pub unsafe extern "C" fn tui_get_viewport(session: TuiString) -> *mut TuiResult {
    unsafe { execute(session, "getViewport", || Ok(Operation::GetViewport)) }
}

#[no_mangle]
/// # Safety
/// The session buffer must remain readable throughout this call.
pub unsafe extern "C" fn tui_scroll(session: TuiString, lines: i32) -> *mut TuiResult {
    unsafe {
        execute(session, "scroll", || {
            Ok(Operation::Scroll {
                target: ScrollTarget::Lines(lines),
            })
        })
    }
}

#[no_mangle]
/// # Safety
/// The session buffer must remain readable throughout this call.
pub unsafe extern "C" fn tui_scroll_to_top(session: TuiString) -> *mut TuiResult {
    unsafe {
        execute(session, "scrollToTop", || {
            Ok(Operation::Scroll {
                target: ScrollTarget::Top,
            })
        })
    }
}

#[no_mangle]
/// # Safety
/// The session buffer must remain readable throughout this call.
pub unsafe extern "C" fn tui_scroll_to_bottom(session: TuiString) -> *mut TuiResult {
    unsafe {
        execute(session, "scrollToBottom", || {
            Ok(Operation::Scroll {
                target: ScrollTarget::Bottom,
            })
        })
    }
}
#[no_mangle]
/// # Safety
/// result must be NULL or a live result returned by this library, freed once.
pub unsafe extern "C" fn tui_result_free(result: *mut TuiResult) {
    unsafe { output::free(result) }
}
#[no_mangle]
/// # Safety
/// Borrowed input buffers must be valid and readable for their stated lengths
/// throughout this call; see TuiString and the input structure contracts.
pub unsafe extern "C" fn tui_open(session: TuiString, options: TuiOpenOptions) -> *mut TuiResult {
    unsafe {
        execute(session, "open", || {
            Ok(Operation::Open(input::open(options)?))
        })
    }
}
#[no_mangle]
/// Pointer form for foreign callers with limited by-value argument space.
///
/// # Safety
/// options must be NULL or point to a valid TuiOpenOptions for this call.
/// Its borrowed buffers follow the same contract as tui_open.
pub unsafe extern "C" fn tui_open_ptr(
    session: TuiString,
    options: *const TuiOpenOptions,
) -> *mut TuiResult {
    if options.is_null() {
        return boundary(|| Err(TermprobeError::usage("open options pointer is null")));
    }
    unsafe { tui_open(session, *options) }
}
#[no_mangle]
/// # Safety
/// Borrowed input buffers must be valid and readable for their stated lengths
/// throughout this call; see TuiString and the input structure contracts.
pub unsafe extern "C" fn tui_run(
    session: TuiString,
    options: TuiOpenOptions,
    program: TuiString,
    args: *const TuiString,
    args_len: usize,
) -> *mut TuiResult {
    unsafe {
        execute(session, "run", || {
            let o = input::open(options)?;
            let program = program.required()?;
            if program.is_empty() {
                return Err(TermprobeError::usage("program must not be empty"));
            }
            Ok(Operation::Run(RunOptions {
                program,
                args: input::strings(args, args_len)?,
                backend: o.backend,
                profile: o.profile,
                style: o.style,
                cols: o.cols,
                rows: o.rows,
                cwd: o.cwd,
                env: o.env,
                wait_ready: o.wait_ready,
                restart: o.restart,
                timeouts: o.timeouts,
                recording: o.recording,
            }))
        })
    }
}
#[no_mangle]
/// Pointer form for foreign callers with limited by-value argument space.
///
/// # Safety
/// options must be NULL or point to a valid TuiOpenOptions for this call.
/// All borrowed buffers follow the same contract as tui_run.
pub unsafe extern "C" fn tui_run_ptr(
    session: TuiString,
    options: *const TuiOpenOptions,
    program: TuiString,
    args: *const TuiString,
    args_len: usize,
) -> *mut TuiResult {
    if options.is_null() {
