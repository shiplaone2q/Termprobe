//! Machine-readable description of the full cli surface, generated from the
//! clap command model so it can never drift from the real implementation.
//! Printed by `termprobe agent-context`.

use clap::{Arg, ArgAction, Command, CommandFactory};
use serde_json::{json, Map, Value};

use crate::cli::Cli;

const SCHEMA_VERSION: &str = "1";

/// Render the agent-context document as pretty JSON.
pub fn render() -> String {
    let cmd = Cli::command();

    let mut global_flags = Vec::new();
    for arg in cmd.get_arguments() {
        if arg.is_global_set() {
            global_flags.push(arg_json(arg));
        }
    }

    let doc = json!({
        "schema_version": SCHEMA_VERSION,
        "name": cmd.get_name(),
        "version": env!("CARGO_PKG_VERSION"),
        "about": cmd.get_about().map(|s| s.to_string()),
        "exit_codes": exit_codes(),
        "global_flags": global_flags,
        "commands": commands_json(&cmd),
    });

    serde_json::to_string_pretty(&doc).unwrap_or_default()
}

fn exit_codes() -> Value {
    json!({
        "0": "success",
        "1": "assertion or wait condition not met",
        "2": "usage / invalid argument",
        "3": "no active session (run `open` or `run` first)",
        "4": "daemon or IPC error",
        "5": "internal error",
    })
}

/// Build a `{ name: {...} }` map of a command's visible subcommands.
fn commands_json(cmd: &Command) -> Value {
    let mut map = Map::new();
    for sub in cmd.get_subcommands() {
        if sub.is_hide_set() {
            continue;
        }
        map.insert(sub.get_name().to_string(), command_json(sub));
    }
    Value::Object(map)
}

fn command_json(cmd: &Command) -> Value {
    let args: Vec<Value> = cmd
        .get_arguments()
        .filter(|a| !a.is_global_set() && !a.is_hide_set())
        .map(arg_json)
        .collect();

    let mut obj = Map::new();
    if let Some(about) = cmd.get_about() {
        obj.insert("about".into(), json!(about.to_string()));
    }
    if !args.is_empty() {
        obj.insert("args".into(), Value::Array(args));
    }
    let subs = commands_json(cmd);
    if subs.as_object().map(|m| !m.is_empty()).unwrap_or(false) {
        obj.insert("subcommands".into(), subs);
    }
    Value::Object(obj)
}

fn arg_json(arg: &Arg) -> Value {
    let positional = arg.is_positional();
    let is_flag = matches!(arg.get_action(), ArgAction::SetTrue | ArgAction::SetFalse);

