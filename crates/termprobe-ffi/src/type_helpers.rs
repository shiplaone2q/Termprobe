use std::ffi::c_void;

/// A borrowed UTF-8 byte string. NULL denotes absence; a non-NULL pointer
/// with zero length denotes an explicitly empty string. Inputs live through
/// the call only. All output pointers live until tui_result_free.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TuiString {
    pub data: *const u8,
    pub len: usize,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TuiOptionalU64 {
    pub present: bool,
    pub value: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TuiOptionalI32 {
    pub present: bool,
    pub value: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TuiOptionalBool {
    pub present: bool,
    pub value: bool,
