//! Terminal snapshot serialization + on-disk `.snap` comparison.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use super::super::terminal::cell::{display_width, truncate_to_columns, Attrs, Color, EmuCell};

pub enum SnapshotStatus {
    Passed,
    Written,
    Updated,
    Failed { expected: String, actual: String },
}

fn snapshot_dir(base: &Path) -> PathBuf {
    base.join("__snapshots__")
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if " /\\<>:\"'|?*".contains(c) { '-' } else { c })
        .collect()
}

fn snapshot_path(base: &Path, name: &str) -> PathBuf {
    snapshot_dir(base).join(format!("{}.snap", sanitize(name)))
}

fn color_value(c: Option<Color>) -> Value {
    match c {
        None => Value::String(crate::assert::color::DEFAULT.to_string()),
        Some(Color::Rgb(r, g, b)) => Value::String(format!("#{r:02x}{g:02x}{b:02x}")),
        Some(c) => json!(c.to_index()),
    }
}

fn shift(prev: &EmuCell, cur: &EmuCell) -> Map<String, Value> {
    let mut m = Map::new();
    if prev.fg != cur.fg {
        m.insert("fg".into(), color_value(cur.fg));
    }
    if prev.bg != cur.bg {
        m.insert("bg".into(), color_value(cur.bg));
    }
    if prev.underline_color != cur.underline_color {
        m.insert("underline_color".into(), color_value(cur.underline_color));
    }
    for (attr, key) in [
        (Attrs::BOLD, "bold"),
        (Attrs::DIM, "dim"),
        (Attrs::ITALIC, "italic"),
        (Attrs::INVERSE, "inverse"),
        (Attrs::INVISIBLE, "invisible"),
        (Attrs::STRIKE, "strike"),
        (Attrs::BLINK, "blink"),
    ] {
        if prev.has(attr) != cur.has(attr) {
            m.insert(key.into(), json!(cur.has(attr)));
        }
    }
    // The style, not a boolean: a curly underline and a single one are
    // different renderings, and a snapshot that only recorded "underlined"
    // would pass when one silently became the other.
    if prev.underline != cur.underline {
        m.insert("underline".into(), json!(cur.underline.name()));
    }
    // The URI, not a boolean: a snapshot that only recorded "this is a link"
    // would pass when the link silently started pointing somewhere else,
    // which is the whole thing worth asserting about a link. The `id=` is
    // deliberately not recorded, because it is an anchor for joining a
    // wrapped link rather than anything a reader can check, and the ghostty
    // backend cannot report it at all.
    if prev.uri() != cur.uri() {
        m.insert("link".into(), json!(cur.uri().unwrap_or_default()));
    }
    m
}

fn baseline() -> EmuCell {
    EmuCell::blank()
}

/// Serialize a grid into a boxed text view, optionally followed by attributes.
///
/// The attributes are a JSON object carrying whatever the box itself cannot
