//! SGR mouse-event encoders.

use crate::api::MouseOptions;

const CSI: &str = "\u{1b}[";

/// SGR press at (x, y), 0-based.
pub fn down(x: u16, y: u16, options: MouseOptions) -> String {
    format!(
        "{CSI}<{};{};{}M",
        options.sgr_code(),
        u32::from(x) + 1,
        u32::from(y) + 1
    )
}

/// SGR release at (x, y), 0-based.
pub fn up(x: u16, y: u16, options: MouseOptions) -> String {
    format!(
        "{CSI}<{};{};{}m",
        options.sgr_code(),
        u32::from(x) + 1,
        u32::from(y) + 1
    )
}

/// SGR motion at (x, y), 0-based (button-motion bit set).
