use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::Instant;

use crate::api::RecordingFormat;

pub(crate) mod cast;
#[cfg(feature = "recording-raster")]
pub mod frames;
mod path;
mod seed;
mod worker;

use worker::worker_loop;

#[derive(Clone)]
pub(crate) struct Capture {
    sender: mpsc::Sender<Message>,
}

pub(crate) struct Recorder {
    sender: mpsc::Sender<Message>,
    worker: Option<JoinHandle<()>>,
    automatic: bool,
}

pub(crate) struct StartRecording {
    pub target_path: PathBuf,
    pub capture_path: PathBuf,
    pub format: RecordingFormat,
    pub cols: u16,
    pub rows: u16,
    pub env: Vec<(String, String)>,
    pub initial_output: String,
    #[cfg(feature = "recording-raster")]
    pub zoom: f64,
    /// Frame style; not stored in the asciicast data.
    #[cfg(feature = "recording-raster")]
    pub style: crate::render::style::Style,
    #[cfg(feature = "recording-raster")]
    pub background: Option<crate::api::CaptureBackground>,
    #[cfg(feature = "recording-raster")]
    pub timeline: frames::TimelineOptions,
    #[cfg(feature = "recording-raster")]
    pub ffmpeg_path: Option<PathBuf>,
}

pub(crate) struct StoppedRecording {
    pub target_path: PathBuf,
    #[cfg(feature = "recording-raster")]
    pub capture_path: PathBuf,
    pub format: RecordingFormat,
    #[cfg(feature = "recording-raster")]
    pub zoom: f64,
    #[cfg(feature = "recording-raster")]
    pub style: crate::render::style::Style,
    #[cfg(feature = "recording-raster")]
    pub background: Option<crate::api::CaptureBackground>,
    #[cfg(feature = "recording-raster")]
    pub timeline: frames::TimelineOptions,
    #[cfg(feature = "recording-raster")]
    pub ffmpeg_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AutomaticRecordingSnapshot {
    pub bytes: u64,
    pub sha256: String,
    pub last_committed_ms: Option<u64>,
}

#[derive(Debug)]
pub(crate) enum CaptureError {
    AlreadyActive,
    NotActive,
    WorkerStopped,
    Io(String),
}

impl Recorder {
    #[cfg(test)]
    pub fn create(
        path: Option<PathBuf>,
        cols: u16,
        rows: u16,
        env: &[(String, String)],
        required: bool,
        logger: Arc<crate::logger::Logger>,
    ) -> std::io::Result<Self> {
        Self::create_at(path, cols, rows, env, required, logger, Instant::now())
    }

    pub fn create_at(
        path: Option<PathBuf>,
        cols: u16,
        rows: u16,
        env: &[(String, String)],
        required: bool,
        logger: Arc<crate::logger::Logger>,
        started: Instant,
    ) -> std::io::Result<Self> {
        let writer = match path {
            Some(path) => match cast::CastWriter::create(&path, cols, rows, env, started) {
                Ok(writer) => Some(writer),
                Err(error) if required => return Err(error),
                Err(error) => {
                    logger.event(&format!(
                        "automatic recording disabled; failed to create {}: {error}",
                        path.display()
                    ));
                    None
                }
            },
            None => None,
        };
        let automatic = writer.is_some();
        let (sender, receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || worker_loop(receiver, writer, logger, cols, rows));
        Ok(Self {
            sender,
            worker: Some(worker),
            automatic,
        })
    }

    pub fn automatic_enabled(&self) -> bool {
        self.automatic
    }

    pub fn capture(&self) -> Capture {
        Capture {
            sender: self.sender.clone(),
        }
    }

    pub fn start(&self, request: StartRecording) -> Result<(), CaptureError> {
        let (reply, response) = mpsc::sync_channel(0);
        self.sender
            .send(Message::Start {
                at: Instant::now(),
                request: Box::new(request),
