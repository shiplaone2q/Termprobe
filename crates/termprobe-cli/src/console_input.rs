use std::io::{self, Cursor, Read};

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Console::{GetStdHandle, ReadConsoleW, STD_INPUT_HANDLE};

pub(crate) struct ConsoleInput {
    handle: HANDLE,
    bytes: Cursor<Vec<u8>>,
    high_surrogate: Option<u16>,
}

impl ConsoleInput {
    pub(crate) fn new() -> Self {
        Self {
            handle: unsafe { GetStdHandle(STD_INPUT_HANDLE) },
            bytes: Cursor::new(Vec::new()),
            high_surrogate: None,
        }
    }
}
