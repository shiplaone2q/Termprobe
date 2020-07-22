use super::*;
use std::time::{Duration, Instant};

unsafe fn completed(task: *mut TuiTask) -> *mut TuiResult {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let result = unsafe { tasks::tui_task_poll(task) };
        if !result.is_null() {
            assert!(unsafe { tasks::tui_task_poll(task) }.is_null());
            unsafe { tasks::tui_task_free(task) };
            return result;
        }
        assert!(
            Instant::now() < deadline,
            "asynchronous task did not complete"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn typed_start_copies_inputs_and_transfers_result_once() {
    let task = {
        let context = String::from(r#"{"name":"async-owned-input"}"#);
        unsafe { tui_state_start(text(&context)) }
    };
    let result = unsafe { completed(task) };
    assert_eq!(unsafe { error_kind(result) }, 3);
    assert!(unsafe { message(result) }.contains("no active session"));
    unsafe { tui_result_free(result) };
    let invalid = unsafe { tui_state_start(TuiString::default()) };
    let result = unsafe { completed(invalid) };
    assert_eq!(unsafe { error_kind(result) }, 2);
    unsafe { tui_result_free(result) };
}

#[test]
fn asynchronous_panics_are_contained() {
    let task = tasks::tui_panic_probe_start();
    let result = unsafe { completed(task) };
    assert_eq!(unsafe { error_kind(result) }, 5);
    assert!(unsafe { message(result) }.contains("intentional native panic probe"));
    unsafe { tui_result_free(result) };
}

#[test]
fn pending_tasks_can_be_abandoned_without_borrowing_foreign_storage() {
    let (sender, receiver) = std::sync::mpsc::channel();
    let task = tasks::start(|| {
        tasks::execute(move || {
            receiver.recv().unwrap();
            Ok(OperationResult::Text("owned discarded result".into()))
        })
    });
    unsafe { tasks::tui_task_free(task) };
    sender.send(()).unwrap();
    unsafe { tasks::tui_task_free(std::ptr::null_mut()) };
}

#[test]
fn viewport_result_preserves_native_count_width() {
    let result = output::operation(OperationResult::Viewport(Viewport {
        offset: usize::MAX - 1,
        history: usize::MAX,
    }));
    unsafe {
        assert_eq!((*result).viewport.offset, usize::MAX - 1);
        assert_eq!((*result).viewport.history, usize::MAX);
        tui_result_free(result);
    }
    assert_eq!(tui_abi_version(), 5);
}

fn text(value: &str) -> TuiString {
    TuiString {
        data: value.as_ptr(),
        len: value.len(),
    }
}
unsafe fn message(result: *mut TuiResult) -> String {
    let result = unsafe { result.as_ref() }.expect("native call returned a null result");
    unsafe { result.error_message.required().unwrap() }
}
unsafe fn error_kind(result: *mut TuiResult) -> u32 {
    unsafe { result.as_ref() }
        .expect("native call returned a null result")
        .error_kind
}

#[test]
fn missing_sessions_retain_structured_errors_and_owned_messages() {
    for _ in 0..128 {
        let result = unsafe { tui_state(text(r#"{"name":"go-abi-missing-session"}"#)) };
        assert_eq!(unsafe { error_kind(result) }, 3);
        assert!(unsafe { message(result) }.contains("no active session"));
        unsafe { tui_result_free(result) };
    }
