//! Terminal emulator selection and construction.

use serde::{Deserialize, Serialize};

use crate::event::BellTracker;
use crate::profile::Profile;
use crate::terminal::alacritty::AlacrittyEmu;
use crate::terminal::emu::Emulator;

#[cfg(feature = "rio")]
use crate::terminal::rio::RioEmu;

#[cfg(feature = "ghostty")]
use crate::terminal::ghostty::GhosttyEmu;
#[cfg(feature = "xtermjs")]
use crate::terminal::xtermjs::XtermJsEmu;

/// The terminal emulator a session uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Alacritty,
    #[cfg(feature = "ghostty")]
    Ghostty,
    #[cfg(feature = "rio")]
    Rio,
    #[cfg(feature = "xtermjs")]
    Xtermjs,
}

impl Backend {
    pub const ALL: &'static [Self] = &[
        Self::Alacritty,
        #[cfg(feature = "ghostty")]
        Self::Ghostty,
        #[cfg(feature = "rio")]
        Self::Rio,
        #[cfg(feature = "xtermjs")]
        Self::Xtermjs,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Alacritty => "alacritty",
            #[cfg(feature = "ghostty")]
            Self::Ghostty => "ghostty",
            #[cfg(feature = "rio")]
