//! PTY spawning and control via `portable-pty`.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use portable_pty::{native_pty_system, Child, CommandBuilder, ExitStatus, MasterPty, PtySize};

use crate::api::TermprobeError;
use crate::shell::Launch;

pub struct Pty {
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    // Process control must never wait for a blocked input write.
    child: Mutex<Box<dyn Child + Send + Sync>>,
    closed: Arc<AtomicBool>,
}

#[cfg(unix)]
struct CancellableReader {
    reader: Box<dyn Read + Send>,
    readiness: std::os::fd::OwnedFd,
    closed: Arc<AtomicBool>,
    shutdown_started: Option<std::time::Instant>,
}

#[cfg(unix)]
impl Read for CancellableReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        use std::os::fd::AsRawFd;

        loop {
            if self.closed.load(Ordering::Acquire) {
                let started = self
                    .shutdown_started
                    .get_or_insert_with(std::time::Instant::now);
                // Drain final output, but a descendant holding or continuously
                // writing the slave must not keep teardown alive indefinitely.
                if started.elapsed() >= Duration::from_millis(250) {
                    return Ok(0);
                }
            }
            match self.reader.read(buffer) {
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    let mut readiness = libc::pollfd {
                        fd: self.readiness.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    // Wake immediately for final output/EOF rather than letting
                    // the process watcher overtake a sleeping reader.
                    let result = unsafe {
                        libc::poll(&mut readiness, 1, crate::config::POLL_DELAY_MS as i32)
                    };
                    if result == -1 {
                        let error = std::io::Error::last_os_error();
                        if error.kind() != std::io::ErrorKind::Interrupted {
                            return Err(error);
                        }
                    }
                }
                result => return result,
            }
        }
    }
}

#[cfg(unix)]
fn nonblocking_master(master: &dyn MasterPty) -> std::io::Result<std::os::fd::OwnedFd> {
    use std::os::fd::BorrowedFd;

    let fd = master.as_raw_fd().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "PTY has no file descriptor",
        )
    })?;
    // The master owns this descriptor. Duplicated readers/writers share its
    // nonblocking flag, allowing both directions to observe shutdown.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    // Poll a separately owned duplicate: kill can close the master and the OS
    // can reuse its descriptor while the reader is still draining.
    unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned()
}

fn child_status(child: &mut dyn Child) -> std::io::Result<Option<ProcessExit>> {
    #[cfg(windows)]
    if let Some(handle) = child.as_raw_handle() {
        use windows_sys::Win32::Foundation::{WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT};
        use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};

        // GetExitCodeProcess alone cannot distinguish STILL_ACTIVE from a real
        // exit code of 259. The child lock keeps its process handle alive here.
        return match unsafe { WaitForSingleObject(handle, 0) } {
            WAIT_OBJECT_0 => {
                let mut code = 0;
                if unsafe { GetExitCodeProcess(handle, &mut code) } == 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(Some(ProcessExit {
                        code: code as i32,
                        signal: None,
                    }))
                }
            }
            WAIT_TIMEOUT => Ok(None),
            WAIT_FAILED => Err(std::io::Error::last_os_error()),
            status => Err(std::io::Error::other(format!(
                "unexpected process wait status: {status}"
            ))),
        };
    }
    child.try_wait().map(|status| status.map(Into::into))
}

pub(crate) fn validate_size(cols: u16, rows: u16) -> Result<(), TermprobeError> {
    if cols == 0 || rows == 0 {
        return Err(TermprobeError::usage(
            "terminal columns and rows must be greater than zero",
        ));
    }
    #[cfg(windows)]
    if cols > i16::MAX as u16 || rows > i16::MAX as u16 {
        return Err(TermprobeError::usage(
            "terminal columns and rows must not exceed 32767 on Windows",
        ));
    }
    Ok(())
}

pub(crate) fn validate_cwd(cwd: &Path) -> Result<(), TermprobeError> {
    let metadata = cwd.metadata().map_err(|error| {
        TermprobeError::usage(format!("invalid session cwd {}: {error}", cwd.display()))
    })?;
    if !metadata.is_dir() {
        return Err(TermprobeError::usage(format!(
            "session cwd is not a directory: {}",
            cwd.display()
        )));
    }
    Ok(())
}

pub struct SpawnOptions {
    pub cols: u16,
    pub rows: u16,
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessExit {
    pub(crate) code: i32,
    pub(crate) signal: Option<String>,
}

impl From<ExitStatus> for ProcessExit {
    fn from(status: ExitStatus) -> Self {
        Self {
            code: status.exit_code() as i32,
            signal: status.signal().map(str::to_string),
        }
    }
}

impl Pty {
    /// Spawn a program in a fresh PTY, returning the controller and a reader
    /// for its output.
    pub fn spawn(
        target: &str,
        args: &[String],
        opts: &SpawnOptions,
    ) -> anyhow::Result<(Pty, Box<dyn Read + Send>)> {
        Self::spawn_with_cwd(target, args, opts, opts.cwd.as_deref().map(Path::new))
    }

    pub(crate) fn spawn_with_cwd(
        target: &str,
        args: &[String],
        opts: &SpawnOptions,
        cwd: Option<&Path>,
    ) -> anyhow::Result<(Pty, Box<dyn Read + Send>)> {
        validate_size(opts.cols, opts.rows)?;
        if let Some(cwd) = cwd {
            validate_cwd(cwd)?;
        }
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
