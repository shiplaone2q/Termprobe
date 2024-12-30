use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize};

use super::*;
use crate::terminal::integration::CommandTracker;

#[test]
fn fish_source_uses_a_single_literal_argument() {
    assert_eq!(
        fish_literal("C:\\a $HOME `echo` '雪' \"quoted\""),
        "'C:\\\\a $HOME `echo` \\'雪\\' \"quoted\"'"
    );
}

#[test]
fn nushell_source_uses_a_non_interpolating_literal() {
    assert_eq!(
        nu_literal("C:\\a $HOME `echo` '雪' \"quoted\"\n\r\t"),
        "\"C:\\\\a $HOME `echo` '雪' \\\"quoted\\\"\\n\\r\\t\""
    );
}

#[test]
fn powershell_passes_an_encoded_literal_script_path() {
    let dir = Path::new("space $HOME `echo` '雪' \"quoted\" \\ newline\n");
    for shell in [Shell::Powershell, Shell::Pwsh] {
        let launch = launch_with_scripts(shell, dir).unwrap();
        let script = path_str(&dir.join("shellIntegration.ps1"));
        assert_eq!(
            launch.args,
            [
                "-NoLogo",
                "-NoProfile",
                "-NoExit",
                "-EncodedCommand",
                &powershell_encoded_command(&format!("& {}", powershell_literal(&script))),
            ]
        );
    }
}

#[test]
fn zsh_disables_global_startup_files() {
    let directory = TestDirectory::new(Shell::Zsh);
    let scripts = directory.0.join("shell");
    write_scripts_to(&scripts).unwrap();
    let launch = launch_with_scripts(Shell::Zsh, &scripts).unwrap();
    assert_eq!(launch.args, ["-d"]);
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(shell: Shell) -> Self {
        let mut name = if shell == Shell::Zsh {
            "zsh-safe".to_string()
        } else {
            "space $HOME `echo` '雪' %2F %25 #".to_string()
        };
        if cfg!(unix) && shell != Shell::Zsh {
            name.push_str(" \"quoted\"");
            if !matches!(shell, Shell::Powershell | Shell::Pwsh) {
                name.push_str(" \\");
            }
            name.push_str(" newline");
            name.push('\n');
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("target")
            .join("shell-tests")
            .join(format!("{}-{}", shell.as_str(), std::process::id()))
            .join(name);
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        for _ in 0..20 {
            match std::fs::remove_dir_all(self.0.parent().unwrap()) {
                Ok(()) => return,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    }
}

struct TestShell {
    child: Box<dyn Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    output: Receiver<Vec<u8>>,
    tracker: CommandTracker,
    bytes: Vec<u8>,
    shell: Shell,
