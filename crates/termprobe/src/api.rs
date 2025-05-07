use std::fmt;
use std::path::PathBuf;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diagnostics::{FailureArtifactRef, FailureDetails, FailureObservation, FailureReport};
use crate::shell::Shell;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AutomaticRecordingMode {
    #[default]
    Disabled,
    OnFailure,
    Always,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AutomaticRecording {
    #[serde(skip)]
    pub mode: AutomaticRecordingMode,
    pub directory: Option<PathBuf>,
}

impl AutomaticRecording {
    pub fn validate(&self) -> Result<(), TermprobeError> {
        if self
            .directory
            .as_ref()
            .is_some_and(|directory| directory.as_os_str().is_empty())
        {
            return Err(TermprobeError::usage(
                "automatic recording directory must not be empty",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Timeouts {
    pub text: Option<u64>,
    pub idle: Option<u64>,
    pub command: Option<u64>,
    pub exit: Option<u64>,
    pub ready: Option<u64>,
}

impl Timeouts {
    pub fn get(&self, class: crate::config::TimeoutClass) -> Option<u64> {
        use crate::config::TimeoutClass::*;
        match class {
            Text => self.text,
            Idle => self.idle,
            Command => self.command,
            Exit => self.exit,
            Ready => self.ready,
        }
    }

    /// Apply higher-precedence timeout values over these defaults.
    pub fn with_overrides(self, overrides: Self) -> Self {
        Self {
            text: overrides.text.or(self.text),
            idle: overrides.idle.or(self.idle),
            command: overrides.command.or(self.command),
            exit: overrides.exit.or(self.exit),
            ready: overrides.ready.or(self.ready),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenOptions {
    pub backend: crate::terminal::backend::Backend,
    pub shell: Option<Shell>,
    /// Terminal settings, already resolved from the config file by the
    /// client. The daemon never reads that file: it is long-lived and shared,
    /// so it has no single working directory to resolve a project-local config
    /// against.
    pub profile: crate::profile::Profile,
    pub cols: u16,
    pub rows: u16,
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    pub wait_ready: Option<bool>,
    pub restart: bool,
    pub timeouts: Timeouts,
    pub recording: AutomaticRecording,
    /// How this session's screenshots and recordings are drawn, resolved
    /// from the config file by the client alongside `profile`.
    pub style: crate::render::style::Style,
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self {
            backend: crate::terminal::backend::Backend::default(),
            shell: None,
            profile: crate::profile::Profile::default(),
            cols: crate::config::DEFAULT_COLS,
            rows: crate::config::DEFAULT_ROWS,
            cwd: None,
            env: Vec::new(),
            wait_ready: None,
            restart: false,
            timeouts: Timeouts::default(),
            recording: AutomaticRecording::default(),
            style: crate::render::style::Style::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub backend: crate::terminal::backend::Backend,
    pub program: String,
    pub args: Vec<String>,
    /// Terminal settings, already resolved from the config file by the
    /// client. The daemon never reads that file: it is long-lived and shared,
    /// so it has no single working directory to resolve a project-local config
    /// against.
    pub profile: crate::profile::Profile,
    pub cols: u16,
    pub rows: u16,
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    pub wait_ready: Option<bool>,
    pub restart: bool,
    pub timeouts: Timeouts,
    pub recording: AutomaticRecording,
    /// How this session's screenshots and recordings are drawn, resolved
    /// from the config file by the client alongside `profile`.
    pub style: crate::render::style::Style,
}

/// Clipboard text or regex.
#[derive(Debug, Clone)]
pub enum ClipboardPattern {
    Text(String),
    Regex(regex::Regex),
}

impl ClipboardPattern {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub fn regex(pattern: &str) -> Result<Self, regex::Error> {
        regex::Regex::new(pattern).map(Self::Regex)
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Text(text) => text,
            Self::Regex(regex) => regex.as_str(),
        }
    }

    pub(crate) fn matches(&self, value: &str) -> bool {
        match self {
            Self::Text(text) => value.contains(text),
            Self::Regex(regex) => regex.is_match(value),
        }
    }
}

impl From<String> for ClipboardPattern {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<&str> for ClipboardPattern {
    fn from(text: &str) -> Self {
        Self::Text(text.to_string())
    }
}

impl From<regex::Regex> for ClipboardPattern {
    fn from(regex: regex::Regex) -> Self {
        Self::Regex(regex)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyAction {
    #[default]
    Press,
    Down,
    Repeat,
    Up,
}

/// A terminal mouse button.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    #[default]
    Left,
    Middle,
    Right,
}

/// Button and modifier state for a mouse action.
///
/// The default is an unmodified left button.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MouseOptions {
    pub button: MouseButton,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
}

impl MouseOptions {
    pub const fn new(button: MouseButton) -> Self {
        Self {
            button,
            alt: false,
            ctrl: false,
            shift: false,
        }
    }

    pub const fn with_alt(mut self) -> Self {
        self.alt = true;
        self
    }

    pub const fn with_ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    pub const fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }

    /// Return the SGR mouse button code used by terminal protocols.
    pub const fn sgr_code(self) -> u8 {
        let button = match self.button {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
        };
        button + 4 * self.shift as u8 + 8 * self.alt as u8 + 16 * self.ctrl as u8
    }

    /// Decode an SGR mouse button code used by protocol adapters.
    pub const fn from_sgr_code(code: u8) -> Option<Self> {
        if code & !0b1_1111 != 0 {
            return None;
        }
        let button = match code & 0b11 {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            _ => return None,
        };
        Some(Self {
            button,
            shift: code & 4 != 0,
            alt: code & 8 != 0,
            ctrl: code & 16 != 0,
        })
    }
}

impl From<MouseButton> for MouseOptions {
    fn from(button: MouseButton) -> Self {
        Self::new(button)
    }
}

mod mouse_options_code {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::MouseOptions;

    pub fn serialize<S>(options: &MouseOptions, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(options.sgr_code())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<MouseOptions, D::Error>
    where
        D: Deserializer<'de>,
    {
        let code = u8::deserialize(deserializer)?;
        MouseOptions::from_sgr_code(code).ok_or_else(|| {
            serde::de::Error::custom(format!("invalid SGR mouse button code {code}"))
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WhitespaceMode {
    #[default]
    Exact,
    Normalize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchOccurrence {
    Any,
    #[default]
    Unique,
    First,
    Last,
    Nth(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextAnchor {
    pub text: String,
    #[serde(default)]
    pub regex: bool,
    #[serde(default)]
    pub occurrence: MatchOccurrence,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextScope {
    pub after: Option<TextAnchor>,
    pub before: Option<TextAnchor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextSelector {
    pub text: String,
    pub regex: bool,
    pub full: bool,
    pub whitespace: WhitespaceMode,
    pub scope: TextScope,
}

impl Default for TextSelector {
    fn default() -> Self {
        Self {
            text: String::new(),
            regex: false,
            full: false,
            whitespace: WhitespaceMode::Exact,
            scope: TextScope::default(),
        }
    }
}

impl TextSelector {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }
}

impl From<&str> for TextSelector {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

impl From<String> for TextSelector {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TextStyle {
    pub foreground: Option<String>,
    pub background: Option<String>,
    pub bold: Option<bool>,
    pub dim: Option<bool>,
    pub italic: Option<bool>,
    pub underline_style: Option<String>,
    pub underline_color: Option<String>,
    pub inverse: Option<bool>,
    pub hidden: Option<bool>,
    pub strikethrough: Option<bool>,
    pub blink: Option<bool>,
}

impl TextStyle {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
/// Select contiguous per-row runs whose cells match every requested style.
pub struct StyleSelector {
    pub style: TextStyle,
    pub full: bool,
}

impl From<TextStyle> for StyleSelector {
    fn from(style: TextStyle) -> Self {
        Self {
            style,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "selector",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LocatorSelector {
    Text(TextSelector),
    Style(StyleSelector),
    Link(LinkSelector),
    And {
        left: Box<LocatorQuery>,
        right: Box<LocatorQuery>,
    },
    Or {
        left: Box<LocatorQuery>,
        right: Box<LocatorQuery>,
    },
    Filter {
        input: Box<LocatorQuery>,
        has: Option<Box<LocatorQuery>>,
        has_not: Option<Box<LocatorQuery>>,
    },
}

/// Select cells by their exact OSC 8 URI. An empty URI requires no link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkSelector {
    pub uri: String,
    #[serde(default)]
    pub full: bool,
}

impl From<&str> for LinkSelector {
    fn from(uri: &str) -> Self {
        Self::from(uri.to_owned())
    }
}

impl From<String> for LinkSelector {
    fn from(uri: String) -> Self {
        Self { uri, full: false }
    }
}

impl LocatorSelector {
    pub fn full(&self) -> bool {
        match self {
            Self::Text(selector) => selector.full,
            Self::Style(selector) => selector.full,
            Self::Link(selector) => selector.full,
            _ => self.children().iter().any(|query| query.uses_full_grid()),
        }
    }

    pub fn description(&self) -> String {
        match self {
            Self::Text(selector) => selector.text.clone(),
            Self::Style(_) => "style".to_string(),
            Self::Link(selector) => format!("link {:?}", selector.uri),
            Self::And { left, right } => format!(
                "({}) and ({})",
                left.selector.description(),
                right.selector.description()
            ),
            Self::Or { left, right } => format!(
                "({}) or ({})",
                left.selector.description(),
                right.selector.description()
            ),
            Self::Filter {
                input,
                has,
                has_not,
