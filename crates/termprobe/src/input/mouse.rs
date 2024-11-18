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
pub fn motion(x: u16, y: u16) -> String {
    format!("{CSI}<35;{};{}M", u32::from(x) + 1, u32::from(y) + 1)
}

/// SGR drag motion at (x, y), preserving the button and modifier bits.
pub fn drag_motion(x: u16, y: u16, options: MouseOptions) -> String {
    format!(
        "{CSI}<{};{};{}M",
        options.sgr_code() | 32,
        u32::from(x) + 1,
        u32::from(y) + 1
    )
}

/// Scroll wheel: SGR codes 64 (up) and 65 (down).
pub fn scroll(x: u16, y: u16, up: bool) -> String {
    let code = if up { 64 } else { 65 };
