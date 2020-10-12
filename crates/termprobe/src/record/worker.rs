use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{mpsc, Arc};
use std::time::Instant;

use sha2::{Digest, Sha256};

use super::{
    cast, AutomaticRecordingSnapshot, CaptureError, Message, StartRecording, StoppedRecording,
};

struct ActiveRecording {
    writer: cast::CastWriter,
    decoder: cast::IncrementalDecoder,
    request: StartRecording,
    started: Instant,
    last_at: Instant,
    error: Option<String>,
}

pub(super) fn worker_loop(
    receiver: mpsc::Receiver<Message>,
    mut primary: Option<cast::CastWriter>,
    logger: Arc<crate::logger::Logger>,
    cols: u16,
    rows: u16,
) {
    let protected = primary.as_ref().map(cast::CastWriter::protect).transpose();
    let mut seed = super::seed::State::new(cols, rows);
    let mut primary_decoder = cast::IncrementalDecoder::default();
    let mut primary_last_at = Instant::now();
    let mut active: Option<ActiveRecording> = None;

    while let Ok(message) = receiver.recv() {
        match message {
            Message::Data { at, bytes } => {
                primary_last_at = at;
                let text = primary_decoder.push(&bytes);
                seed.process(text.as_bytes());
                if !text.is_empty() {
                    if let Some(writer) = primary.as_mut() {
                        if let Err(error) = writer.write_output(at, &text) {
                            logger.event(&format!("automatic recording failed: {error}"));
                            primary = None;
                        }
                    }
                }
                if let Some(recording) = active.as_mut().filter(|recording| at >= recording.started)
                {
                    recording.last_at = at;
                    let text = recording.decoder.push(&bytes);
                    if text.is_empty() {
                        continue;
                    }
                    let result = recording.writer.write_output(at, &text);
                    remember_error(recording, result);
                }
            }
            Message::Resize { at, cols, rows } => {
                seed.resize(cols, rows);
