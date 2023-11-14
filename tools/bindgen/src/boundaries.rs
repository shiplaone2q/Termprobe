//! Fields and enums come from owned Rust models. Overrides describe transport contracts only.
use crate::{emit, format_source, NOTICE};
use anyhow::{bail, Context, Result};
use quote::ToTokens;
use std::{collections::BTreeMap, fmt::Write, fs, path::Path};
use syn::{Fields, Item};

#[path = "javascript.rs"]
mod javascript;

#[derive(Clone)]
struct Field {
    name: String,
    ty: String,
}

struct Models {
    fields: BTreeMap<String, Vec<Field>>,
    enums: BTreeMap<String, Vec<String>>,
}

impl Models {
    fn load(root: &Path) -> Result<Self> {
        let mut models = Self {
            fields: BTreeMap::new(),
            enums: BTreeMap::new(),
        };
        for module in [
            "api",
            "locator_query",
            "diagnostics",
            "profile",
            "shell/mod",
            "terminal/backend",
            "terminal/cell",
        ] {
            models.parse(&fs::read_to_string(
                root.join(format!("crates/termprobe/src/{module}.rs")),
            )?)?;
        }
        Ok(models)
    }

    fn parse(&mut self, source: &str) -> Result<()> {
        fn fields(values: &Fields) -> Vec<Field> {
            values
                .iter()
                .enumerate()
                .map(|(i, f)| Field {
                    name: f
                        .ident
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| i.to_string()),
                    ty: f.ty.to_token_stream().to_string().replace(' ', ""),
                })
                .collect()
        }
        for item in syn::parse_file(source)?.items {
            match item {
                Item::Struct(item) if matches!(item.fields, Fields::Named(_)) => {
                    self.fields
                        .insert(item.ident.to_string(), fields(&item.fields));
                }
                Item::Enum(item) => {
                    let name = item.ident.to_string();
                    self.enums.insert(
                        name.clone(),
                        item.variants.iter().map(|v| v.ident.to_string()).collect(),
                    );
                    for variant in item.variants {
                        self.fields.insert(
                            format!("{name}::{}", variant.ident),
                            fields(&variant.fields),
                        );
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn fields(&self, name: &str) -> Result<Vec<Field>> {
        self.fields
            .get(name)
            .cloned()
            .with_context(|| format!("missing model {name}"))
    }
}

const OUTPUTS: &[&str] = &[
    "OpenResult",
    "Cursor",
    "Size",
    "Viewport",
    "EffectiveTimeouts",
    "BellEvent",
    "State",
    "TerminalColors",
    "Cell",
    "TextPosition",
    "TextSpan",
    "TextMatch",
    "PackedScreen",
];

fn c_name(name: &str) -> String {
    format!(
        "Tui{}",
        match name {
            "EffectiveTimeouts" => "Timeouts",
            "TextPosition" => "Position",
            "TextSpan" => "Span",
            "TextMatch" => "Match",
            other => other,
        }
    )
}
fn ident(name: &str) -> &str {
    if name == "char" {
        "r#char"
    } else {
        name
    }
}
fn snake(name: &str) -> String {
    if name == "OpenResult" {
        return "open_value".into();
    }
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i != 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}
fn js_key(name: &str) -> String {
    let mut words = name.split('_');
    let mut out = words.next().unwrap_or_default().to_owned();
    for word in words {
        let mut chars = word.chars();
        if let Some(c) = chars.next() {
            out.extend(c.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}
fn option(ty: &str) -> Option<&str> {
    ty.strip_prefix("Option<").and_then(|s| s.strip_suffix('>'))
}
fn c_type(ty: &str) -> Result<String> {
    Ok(match ty {
        "String" | "Option<String>" | "PathBuf" | "Option<PathBuf>" => "TuiString".into(),
        "Option<u64>" | "Option<u32>" | "Option<u8>" => "TuiOptionalU64".into(),
        "Option<i32>" => "TuiOptionalI32".into(),
        "Option<bool>" => "TuiOptionalBool".into(),
        "Option<f64>" => "TuiOptionalF64".into(),
        "CellColor" => "TuiColor".into(),
        "bool" | "u16" | "u32" | "u64" | "u8" | "f64" | "i32" | "usize" => ty.into(),
        t if OUTPUTS.contains(&t) || t == "Timeouts" => c_name(t),
        _ => bail!("no C mapping for {ty}"),
    })
}
fn c_output(ty: &str, value: &str) -> Result<String> {
    Ok(match ty {
        "String" => format!("s.string({value})"),
        "Option<String>" => format!("s.optional({value})"),
        "Option<i32>" => format!("integer({value})"),
        "Option<u32>" => format!("TuiOptionalU64 {{ present: {value}.is_some(), value: u64::from({value}.unwrap_or_default()) }}"),
        "CellColor" => format!("color({value})"),
        t if OUTPUTS.contains(&t) => format!("{}(s, {value})", snake(t)),
