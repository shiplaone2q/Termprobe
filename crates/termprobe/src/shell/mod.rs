use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::config::home_dir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shell {
    Bash,
    Powershell,
    Pwsh,
    Cmd,
    Fish,
    Zsh,
    Xonsh,
    Elvish,
    Nushell,
}

impl Shell {
    pub fn as_str(&self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Powershell => "powershell",
            Shell::Pwsh => "pwsh",
            Shell::Cmd => "cmd",
            Shell::Fish => "fish",
            Shell::Zsh => "zsh",
            Shell::Xonsh => "xonsh",
            Shell::Elvish => "elvish",
            Shell::Nushell => "nushell",
        }
    }

    /// The character used to submit a typed command line.
    pub fn return_char(&self) -> &'static str {
        match self {
            Shell::Xonsh => "\n",
            _ => "\r",
        }
    }
}

pub fn default_shell() -> Shell {
    if cfg!(windows) {
        Shell::Powershell
    } else if cfg!(target_os = "macos") {
        Shell::Zsh
    } else {
        Shell::Bash
    }
}

pub fn scripts_dir() -> PathBuf {
    home_dir().join("shell")
}

/// Materialize the bundled shell-integration scripts into the home directory.
pub fn write_integration_scripts() -> std::io::Result<()> {
    write_scripts_to(&scripts_dir())
}

fn write_scripts_to(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let files: &[(&str, &str)] = &[
        (
            "shellIntegration.bash",
            include_str!("../../shell/shellIntegration.bash"),
        ),
        (
            "shellIntegration.fish",
            include_str!("../../shell/shellIntegration.fish"),
        ),
        (
            "shellIntegration.ps1",
            include_str!("../../shell/shellIntegration.ps1"),
        ),
        (
            "shellIntegration.xsh",
