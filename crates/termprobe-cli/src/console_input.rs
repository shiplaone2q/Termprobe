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

impl Read for ConsoleInput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        loop {
            let read = self.bytes.read(buffer)?;
            if read != 0 {
                return Ok(read);
            }

            let mut utf16 = [0u16; 1024];
            let offset = if let Some(high) = self.high_surrogate.take() {
                utf16[0] = high;
                1
            } else {
                0
            };
            let mut count = 0;
