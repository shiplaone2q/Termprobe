use crate::session::SessionInput;
use crate::types::*;
use termprobe::api::*;
use termprobe::locator_query::LocatorExpression;
use termprobe::profile::{Profile, Rgb};
use termprobe::shell::Shell;
use termprobe::ExecutionContext;

type Result<T> = std::result::Result<T, TermprobeError>;

include!("input_generated.rs");

impl TuiOptionalU64 {
    pub(crate) fn option(self) -> Option<u64> {
        self.present.then_some(self.value)
    }
}
impl TuiOptionalF64 {
    pub(crate) fn option(self) -> Option<f64> {
        self.present.then_some(self.value)
    }
}
impl TuiOptionalBool {
    pub(crate) fn option(self) -> Option<bool> {
        self.present.then_some(self.value)
    }
}

// Every exported entrypoint is unsafe: the caller must provide valid readable
// buffers for the duration of the call. Null/length and UTF-8 checks catch
// representable usage mistakes, but cannot prove foreign pointer validity.
pub(crate) unsafe fn slice<'a, T>(data: *const T, len: usize) -> Result<&'a [T]> {
    if len == 0 {
        return Ok(&[]);
    }
    if data.is_null() || len > isize::MAX as usize / std::mem::size_of::<T>() {
        return Err(TermprobeError::usage(
            "invalid native array pointer or length",
        ));
    }
    Ok(unsafe { std::slice::from_raw_parts(data, len) })
}
impl TuiString {
    pub(crate) unsafe fn optional(self) -> Result<Option<String>> {
        if self.data.is_null() {
            if self.len != 0 {
