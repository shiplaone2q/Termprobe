//! JavaScript names and defaults are transport metadata, not duplicate Rust models.
use super::*;
use std::collections::BTreeSet;

// JavaScript method, owned operation, result variant.
const METHODS: &[(&str, &str, &str)] = &[
    ("restart", "Restart", "Open"),
    ("state", "State", "State"),
    ("text", "Text", "Text"),
    ("packedScreen", "PackedScreen", "PackedScreen"),
    ("cells", "Cells", "Cells"),
    ("findLocator", "FindLocator", "Matches"),
    ("waitLocator", "WaitLocator", "Unit"),
    ("expectLocator", "WaitLocator", "Unit"),
    ("clickLocator", "ClickLocator", "Unit"),
    ("highlightLocator", "HighlightLocator", "Matches"),
    ("getCommand", "GetCommand", "Command"),
    ("getOutput", "GetOutput", "Output"),
    ("getExitCode", "GetExitCode", "ExitCode"),
    ("getCwd", "GetCwd", "Cwd"),
    ("getTitle", "GetTitle", "Title"),
    ("getClipboard", "GetClipboard", "Clipboard"),
    ("getCursor", "GetCursor", "Cursor"),
    ("getSize", "GetSize", "Size"),
    ("getViewport", "GetViewport", "Viewport"),
    ("scroll", "Scroll.Lines", "Unit"),
    ("scrollToTop", "Scroll.Top", "Unit"),
    ("scrollToBottom", "Scroll.Bottom", "Unit"),
    ("getBellCount", "GetBellCount", "BellCount"),
    ("getBellEvents", "GetBellEvents", "BellEvents"),
    ("write", "Write", "Unit"),
    ("type", "Write", "Unit"),
    ("submit", "Submit", "Unit"),
    ("press", "Key.Press", "Unit"),
    ("keyDown", "Key.Down", "Unit"),
    ("repeat", "Key.Repeat", "Unit"),
    ("keyUp", "Key.Up", "Unit"),
    ("mouseClick", "Mouse.Click", "Unit"),
    ("mouseMove", "Mouse.Move", "Unit"),
    ("mouseDown", "Mouse.Down", "Unit"),
    ("mouseUp", "Mouse.Up", "Unit"),
    ("mouseDrag", "Mouse.Drag", "Unit"),
    ("mouseScroll", "Mouse.Scroll", "Unit"),
    ("resize", "Resize", "Unit"),
    ("signal", "Signal", "Unit"),
    ("waitTitle", "WaitTitle", "Unit"),
    ("expectTitle", "ExpectTitle", "Unit"),
    ("waitClipboard", "WaitClipboard", "Unit"),
    ("waitIdle", "WaitIdle", "Unit"),
    ("waitCommand", "WaitCommand", "Unit"),
    ("waitExit", "WaitExit", "Unit"),
    ("waitReady", "WaitReady", "Unit"),
    ("waitBell", "WaitBell", "Unit"),
    ("expectExitCode", "ExpectExitCode", "Unit"),
    ("expectOutput", "ExpectOutput", "Unit"),
    ("expectBellCount", "ExpectBellCount", "Unit"),
    ("snapshot", "Snapshot", "Snapshot"),
    ("screenshot", "Screenshot", "Screenshot"),
    ("startRecording", "StartRecording", "Unit"),
    ("stopRecording", "StopRecording", "Recording"),
];

const INPUTS: &[(&str, &str)] = &[
    ("Timeouts", "Timeouts"),
    ("AutomaticRecording", "AutomaticRecordingOptions"),
    ("TraceOptions", "TraceOptions"),
    ("FailureArtifactOptions", "FailureArtifactOptions"),
    ("OpenOptions", "OpenOptions"),
    ("RunOptions", "RunOptions"),
    ("TextStyle", "LocatorStyle"),
    ("LocatorNode", "LocatorNode"),
    ("LocatorExpression", "LocatorExpression"),
    ("Operation::WaitTitle", "TitleOptions"),
    ("Operation::WaitTitle", "ClipboardWaitOptions"),
    ("Operation::Snapshot", "SnapshotOptions"),
    ("Operation::Screenshot", "ScreenshotOptions"),
    ("Operation::StartRecording", "RecordingOptions"),
];

// These getters read current values without an intentional wait. Other operations
// retain Rust tasks so waits, input, and lifecycle work cannot exhaust Koffi's pool.
const SHORT_READS: &[&str] = &[
    "GetCommand",
    "GetOutput",
    "GetExitCode",
    "GetCwd",
    "GetTitle",
    "GetClipboard",
    "GetCursor",
    "GetSize",
    "GetViewport",
    "GetBellCount",
    "GetBellEvents",
];
const ENUMS: &[(&str, &str)] = &[
    ("Backend", "Backend"),
    ("Shell", "Shell"),
    ("RecordingFormat", "RecordingFormat"),
    ("SnapshotResult", "SnapshotResult"),
    ("UnderlineStyle", "UnderlineStyle"),
    ("LocatorNodeKind", "LocatorNodeKind"),
    ("LocatorDirection", "LocatorStageDirection"),
];

fn ts_type(ty: &str) -> Result<String> {
    if let Some(inner) = option(ty) {
        return Ok(format!("{} | null", ts_type(inner)?));
    }
    if let Some(inner) = ty.strip_prefix("Box<").and_then(|s| s.strip_suffix('>')) {
        return ts_type(inner);
    }
    if let Some(inner) = ty.strip_prefix("Vec<").and_then(|s| s.strip_suffix('>')) {
        return Ok(format!("Array<{}>", ts_type(inner)?));
    }
    Ok(match ty {
        "u8" | "u16" | "u32" | "u64" | "usize" | "i32" | "f64" | "MouseOptions" => "number".into(),
        "String"
        | "PathBuf"
        | "WhitespaceMode"
        | "TraceMode"
        | "FailureArtifactMode"
        | "ScreenshotResult" => "string".into(),
        "bool" => "boolean".into(),
        "()" => "void".into(),
        "(String,String)" => "[string, string]".into(),
        "BTreeMap<String,bool>" => "Record<string, boolean>".into(),
        "std::collections::BTreeMap<u8,String>" => "Record<string, string>".into(),
        "CellColor" => "Color".into(),
        "TextStyle" => "LocatorStyle".into(),
        "LocatorDirection" => "LocatorStageDirection".into(),
        "LocatorQuery" => "LocatorExpression".into(),
        t if OUTPUTS.contains(&t)
            || INPUTS.iter().any(|(_, target)| *target == t)
            || ENUMS.iter().any(|(_, target)| *target == t)
            || t == "MouseClickOptions" =>
        {
            t.into()
        }
        _ => bail!("no JavaScript mapping for {ty}"),
    })
}

fn parameter(mut f: Field) -> Field {
    f.name = match f.name.as_str() {
        "query" => "expression",
        "options" if f.ty == "MouseOptions" => "button",
        other => other,
    }
    .into();
    if f.ty == "bool" || matches!(f.name.as_str(), "w" | "h" | "clicks" | "amount" | "button") {
        f.ty = format!("Option<{}>", f.ty);
    }
    f
}

fn parameters(models: &Models, method: &str, variant: &str) -> Result<Vec<Field>> {
    let field = |name: &str, ty: &str| Field {
        name: name.into(),
        ty: ty.into(),
    };
    Ok(match method {
        "scroll" => {
            let mut fields = models.fields("ScrollTarget::Lines")?;
            fields[0].name = "lines".into();
