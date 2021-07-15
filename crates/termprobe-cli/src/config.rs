use std::path::PathBuf;

use sha2::{Digest, Sha256};

pub const SHUTDOWN_DRAIN_MS: u64 = 2_000;
pub const MONITOR_FRAME_MS: u64 = 50;
pub const IDLE_TIMEOUT_MS: u64 = 4 * 60 * 60 * 1_000;
pub const IDLE_CHECK_INTERVAL_MS: u64 = 5 * 60 * 1_000;

pub fn home_dir() -> PathBuf {
    termprobe::config::home_dir()
}

pub fn ensure_home() -> std::io::Result<PathBuf> {
    let dir = home_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn pid_file(session: &str) -> PathBuf {
    home_dir().join(format!("{session}.pid"))
}

pub fn daemon_lock_file(session: &str) -> PathBuf {
    home_dir().join(format!("{session}.pid.lock"))
}

pub fn log_file(session: &str) -> PathBuf {
    home_dir().join(format!("{session}.log"))
}

pub fn recording_pointer_file(session: &str) -> PathBuf {
    home_dir().join(format!("{session}.recording"))
}

pub fn recording_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("TERMPROBE__HOME") {
        return PathBuf::from(dir).join("recordings");
    }
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("termprobe")
}

pub fn recording_file(session: &str) -> PathBuf {
    recording_dir().join(format!("{session}.cast"))
}
