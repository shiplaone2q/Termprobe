use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::api::RecordingFormat;
use crate::record::frames::Frame;

use super::raster::FrameRenderer;

pub(crate) fn encode(
    path: &Path,
    format: RecordingFormat,
    frames: &[Frame],
    renderer: &mut dyn FrameRenderer,
    fps: u8,
    ffmpeg_path: Option<&Path>,
) -> anyhow::Result<()> {
    if frames.is_empty() {
        anyhow::bail!("recording timeline contains no frames");
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    match format {
        RecordingFormat::Apng => encode_apng(path, frames, renderer),
        RecordingFormat::Gif => encode_gif(path, frames, renderer),
        RecordingFormat::Mp4 => {
            let ffmpeg_path = ffmpeg_path
                .ok_or_else(|| anyhow::anyhow!("MP4 recording is missing its ffmpeg executable"))?;
            encode_mp4(path, frames, renderer, fps, ffmpeg_path)
        }
        RecordingFormat::Cast => anyhow::bail!("cast recordings do not require animation encoding"),
    }
}

pub(crate) fn encode_png(
    path: &Path,
    frame: &Frame,
    renderer: &mut dyn FrameRenderer,
) -> anyhow::Result<()> {
    let (staged_path, file) = stage_png(path)?;
    let result = (|| -> anyhow::Result<()> {
        let (width, height) = renderer.pixel_size();
        let mut output = BufWriter::new(file);
        let mut encoder = png::Encoder::new(&mut output, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_adaptive_filter(png::AdaptiveFilterType::Adaptive);
        let mut writer = encoder.write_header()?;
        let image = renderer.render(frame)?;
        writer.write_image_data(image.as_raw())?;
        writer.finish()?;
        output.flush()?;
        drop(output);
        std::fs::rename(&staged_path, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(staged_path);
    }
    result
}

fn stage_png(path: &Path) -> std::io::Result<(PathBuf, File)> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    loop {
        let staged_path = path.with_file_name(format!(
            ".termprobe-png-{}-{}.stage",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged_path)
        {
            Ok(file) => return Ok((staged_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

fn encode_mp4(
    path: &Path,
    frames: &[Frame],
    renderer: &mut dyn FrameRenderer,
    fps: u8,
    ffmpeg_path: &Path,
) -> anyhow::Result<()> {
    let timeline = mp4_timeline(frames, fps)?;
    let (width, height) = renderer.pixel_size();
    let mut child = mp4_command(ffmpeg_path, path, width, height, fps)
        .spawn()
        .map_err(|error| {
            anyhow::anyhow!(
                "failed to start ffmpeg at {}: {error}",
                ffmpeg_path.display()
            )
        })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to open ffmpeg input"))?;
    let write_result = stream_mp4_frames(&mut stdin, &timeline, frames, renderer);
    drop(stdin);
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        if stderr.is_empty() {
            anyhow::bail!("ffmpeg failed with {}", output.status);
        }
        anyhow::bail!("ffmpeg failed with {}: {stderr}", output.status);
    }
    write_result
}

fn mp4_command(
    ffmpeg_path: &Path,
    output_path: &Path,
    width: u32,
    height: u32,
    fps: u8,
) -> Command {
    let mut command = Command::new(ffmpeg_path);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("rawvideo")
        .arg("-pixel_format")
        .arg("rgba")
        .arg("-video_size")
        .arg(format!("{width}x{height}"))
        .arg("-framerate")
        .arg(fps.to_string())
        .arg("-i")
        .arg("pipe:0")
        .arg("-an")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-crf")
        .arg("18")
        .arg("-vf")
        .arg("pad=ceil(iw/2)*2:ceil(ih/2)*2")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-movflags")
        .arg("+faststart")
        .arg("-f")
        .arg("mp4")
        .arg(output_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    command
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mp4Step {
    frame: usize,
    repeats: usize,
}

fn mp4_timeline(frames: &[Frame], fps: u8) -> anyhow::Result<Vec<Mp4Step>> {
    if fps == 0 {
        anyhow::bail!("recording fps must be greater than zero");
    }
    let mut output: Vec<Mp4Step> = Vec::new();
    let mut elapsed = 0u128;
    let mut emitted = 0u128;
    for (index, frame) in frames.iter().enumerate() {
        elapsed = elapsed
            .checked_add(frame.duration.as_nanos())
            .ok_or_else(|| anyhow::anyhow!("MP4 recording timeline is too long"))?;
        let scaled = elapsed
            .checked_mul(u128::from(fps))
            .ok_or_else(|| anyhow::anyhow!("MP4 recording timeline is too long"))?;
        let rounded = scaled.saturating_add(500_000_000) / 1_000_000_000;
        if rounded <= emitted {
            continue;
        }
        output.push(Mp4Step {
            frame: index,
            repeats: (rounded - emitted)
                .try_into()
                .map_err(|_| anyhow::anyhow!("MP4 recording timeline is too long"))?,
        });
        emitted = rounded;
    }
    if output.is_empty() {
        output.push(Mp4Step {
            frame: frames.len().saturating_sub(1),
            repeats: 1,
        });
    } else if output
        .last()
        .is_some_and(|step| step.frame + 1 < frames.len())
    {
        let last_frame = frames.len() - 1;
        if let Some(previous) = output.iter_mut().rev().find(|step| step.repeats > 1) {
            previous.repeats -= 1;
            output.push(Mp4Step {
                frame: last_frame,
                repeats: 1,
            });
        } else if let Some(previous) = output.last_mut() {
            previous.frame = last_frame;
        }
    }
    Ok(output)
}

fn stream_mp4_frames(
    output: &mut dyn Write,
    timeline: &[Mp4Step],
    frames: &[Frame],
    renderer: &mut dyn FrameRenderer,
) -> anyhow::Result<()> {
