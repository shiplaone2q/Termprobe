use std::path::{Path, PathBuf};

use dialoguer::{theme::ColorfulTheme, Select};

const SKILL_NAME: &str = "termprobe";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InstallScope {
    Repository,
    Global,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AgentDirectory {
    Agents,
    Claude,
}

impl AgentDirectory {
    fn name(self) -> &'static str {
        match self {
            Self::Agents => ".agents",
            Self::Claude => ".claude",
        }
    }
}

pub fn add(manifest: &str, references: &[(&str, &str)]) -> i32 {
    match add_interactive(manifest, references) {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("termprobe skill: {message}");
            1
        }
    }
}

pub fn render(manifest: &str, references: &[(&str, &str)]) -> String {
    let manifest = localize_links(manifest, references);
    let mut output = manifest.trim_end().to_string();
    for (_, contents) in references {
        let contents = localize_links(contents, references);
        output.push_str("\n\n---\n\n");
        output.push_str(contents.trim());
    }
    output.push('\n');
    output
}

fn localize_links(document: &str, references: &[(&str, &str)]) -> String {
    let mut output = document.replace("](../SKILL.md)", "](#termprobe)");
    for (path, contents) in references {
        output = output.replace(&format!("]({path}#"), "](#");
        if let Some(anchor) = first_heading_anchor(contents) {
            output = output.replace(&format!("]({path})"), &format!("](#{anchor})"));
        }
    }
    output
}

fn first_heading_anchor(document: &str) -> Option<String> {
    let heading = document.lines().find_map(|line| line.strip_prefix("# "))?;
    let anchor = heading
        .chars()
        .filter_map(|character| {
            if character.is_ascii_alphanumeric() {
                Some(character.to_ascii_lowercase())
            } else if matches!(character, ' ' | '-') {
                Some('-')
            } else {
                None
            }
        })
        .collect::<String>();
    (!anchor.is_empty()).then_some(anchor)
}

fn add_interactive(manifest: &str, references: &[(&str, &str)]) -> Result<(), String> {
    let theme = ColorfulTheme::default();
    let scope_items = [
        "Repository local (current project)",
        "Global (all projects)",
    ];
    let scope = match Select::with_theme(&theme)
        .with_prompt("Install scope")
        .items(&scope_items)
        .default(0)
        .interact_opt()
        .map_err(|error| format!("could not read install scope: {error}"))?
    {
        Some(0) => InstallScope::Repository,
        Some(1) => InstallScope::Global,
        Some(_) => unreachable!("scope picker returned an unknown item"),
        None => return cancelled(),
    };

    let directory_items = [".agents (GitHub Copilot / Codex)", ".claude (Claude Code)"];
    let directory = match Select::with_theme(&theme)
        .with_prompt("Skills directory")
        .items(&directory_items)
        .default(0)
        .interact_opt()
        .map_err(|error| format!("could not read skills directory: {error}"))?
    {
        Some(0) => AgentDirectory::Agents,
        Some(1) => AgentDirectory::Claude,
        Some(_) => unreachable!("directory picker returned an unknown item"),
