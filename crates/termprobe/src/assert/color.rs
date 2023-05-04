//! Color parsing and comparison for `expect --fg/--bg`.

use super::super::terminal::cell::Color;
use crate::terminal::emu::Emulator;

/// The spelling of [`Expected::Default`], on the command line and in messages.
pub const DEFAULT: &str = "default";

#[derive(Debug, Clone)]
pub enum Expected {
    /// The terminal's default color, i.e. the cell set no color of its own.
    Default,
    Ansi256(u8),
    Hex(u8, u8, u8),
    Rgb(u8, u8, u8),
}

impl Expected {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let s = s.trim();
        if s.eq_ignore_ascii_case(DEFAULT) {
            return Ok(Expected::Default);
        }
        if let Some(hex) = s.strip_prefix('#') {
            let (r, g, b) = parse_hex(hex).map_err(|_| invalid(s))?;
            return Ok(Expected::Hex(r, g, b));
        }
        if s.contains(',') {
            let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
            let parsed: Result<Vec<u8>, _> = parts.iter().map(|p| p.parse::<u8>()).collect();
            match parsed.ok().as_deref() {
                Some([r, g, b]) => return Ok(Expected::Rgb(*r, *g, *b)),
                _ => return Err(invalid(s)),
            }
        }
        let n: u8 = s.parse().map_err(|_| invalid(s))?;
        Ok(Expected::Ansi256(n))
    }

    pub fn describe(&self) -> String {
        match self {
            Expected::Default => DEFAULT.to_string(),
            Expected::Ansi256(n) => n.to_string(),
            Expected::Hex(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            Expected::Rgb(r, g, b) => format!("{r},{g},{b}"),
        }
    }
}

/// A consistent, enumerated error for any unparseable color value.
fn invalid(got: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "color must be \"{DEFAULT}\", ansi256 (0-255), hex (#rrggbb), or rgb (r,g,b) (got: \"{got}\")"
    )
}

fn parse_hex(hex: &str) -> anyhow::Result<(u8, u8, u8)> {
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        anyhow::bail!("hex color must be 6 digits");
    }
    let r = u8::from_str_radix(&hex[0..2], 16)?;
    let g = u8::from_str_radix(&hex[2..4], 16)?;
    let b = u8::from_str_radix(&hex[4..6], 16)?;
    Ok((r, g, b))
}

/// Does a cell's resolved color match the expected color?
///
/// A cell that set no color of its own matches `default`, and also matches the
/// concrete RGB value the terminal currently uses for that foreground or
/// background. It never matches an ANSI index because it did not select one.
///
/// A concrete `#rrggbb` is resolved through the session profile, the same table
/// the screenshot renderer draws with. These used to be two separate hardcoded
/// tables that disagreed on every ANSI slot, so `expect --fg "#800000"` passed
/// on a cell a screenshot painted `#e88388`.
pub fn matches(
    cell: Option<Color>,
    expected: &Expected,
    colors: &dyn Emulator,
    is_fg: bool,
) -> bool {
    match expected {
        Expected::Default => cell.is_none(),
        Expected::Ansi256(n) => cell.is_some_and(|cell| cell.to_index() == *n),
        Expected::Hex(er, eg, eb) | Expected::Rgb(er, eg, eb) => {
            let got = colors.resolve(cell, is_fg);
            (got.r, got.g, got.b) == (*er, *eg, *eb)
        }
    }
}

/// Render a cell's color in the same space as the expected value, for messages.
pub fn describe_cell(
    cell: Option<Color>,
    expected: &Expected,
    colors: &dyn Emulator,
    is_fg: bool,
) -> String {
    match expected {
        Expected::Default => cell
            .map(|cell| cell.to_index().to_string())
            .unwrap_or_else(|| DEFAULT.to_string()),
        Expected::Ansi256(_) => cell
            .map(|cell| cell.to_index().to_string())
            .unwrap_or_else(|| DEFAULT.to_string()),
        _ => colors.resolve(cell, is_fg).to_hex(),
    }
}

pub fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    if r == g && g == b {
        if r < 8 {
            return 16;
        }
        if r > 248 {
