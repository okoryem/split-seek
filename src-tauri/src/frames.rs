//! Frame preparer: runs ffmpeg on each clip to pull one frame every 2 seconds,
//! saving a LOG preview and a boosted copy for Jev into a temporary folder.
//! Original clips are only ever read.

use futures::stream::{self, StreamExt};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tokio::process::Command;

use crate::scan;

/// Seconds between sampled frames.
pub const SAMPLE_EVERY_SEC: u32 = 2;
/// How many clips ffmpeg works on at once.
const PARALLEL_CLIPS: usize = 4;

/// Smaller side becomes 288 px, so 16:9 footage gives 512×288 and vertical gives 288×512.
/// Jev rejects images under 280 px on either side. JPEG needs full-range color, which
/// some cameras' video isn't tagged as.
const SCALE: &str = "scale=288:288:force_original_aspect_ratio=increase:force_divisible_by=2:out_range=full,format=yuvj420p";
/// Generic LOG boost: more contrast and color. Tuned in the raw-vs-boosted check.
const BOOST: &str = "eq=contrast=1.35:saturation=1.6:gamma=1.1";

#[derive(Debug, Clone)]
pub struct Frame {
    pub clip_path: PathBuf,
    pub clip_name: String,
    pub t_sec: u32,
    pub log_jpg: PathBuf,
    pub boost_jpg: PathBuf,
}

/// A folder that's been prepared. Dropping it deletes the temporary frames.
pub struct Prepared {
    pub folder: PathBuf,
    pub temp_dir: TempDir,
    pub frames: Vec<Frame>,
    pub clip_count: usize,
    /// Clips ffmpeg couldn't read.
    pub skipped: Vec<PathBuf>,
}

#[derive(Debug)]
pub enum PrepareError {
    NoVideos,
    Io(String),
}

impl std::fmt::Display for PrepareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrepareError::NoVideos => write!(f, "There are no videos in this folder."),
            PrepareError::Io(m) => write!(f, "{m}"),
        }
    }
}

/// Progress after each clip finishes.
pub struct Progress<'a> {
    pub done: usize,
    pub total: usize,
    pub clip: &'a str,
}

pub async fn prepare(
    folder: &Path,
    on_progress: impl Fn(Progress),
) -> Result<Prepared, PrepareError> {
    let clips = scan::find_videos(folder).map_err(|e| PrepareError::Io(e.to_string()))?;
    if clips.is_empty() {
        return Err(PrepareError::NoVideos);
    }
    let temp_dir = tempfile::Builder::new()
        .prefix("splitseek-")
        .tempdir()
        .map_err(|e| PrepareError::Io(e.to_string()))?;

    let total = clips.len();
    let mut frames = Vec::new();
    let mut skipped = Vec::new();
    let mut done = 0;

    let mut jobs = stream::iter(clips.iter().enumerate())
        .map(|(i, clip)| {
            let out = temp_dir.path().join(format!("{i:04}"));
            async move { (clip, extract(clip, &out).await) }
        })
        .buffer_unordered(PARALLEL_CLIPS);

    while let Some((clip, result)) = jobs.next().await {
        done += 1;
        let name = clip_name(clip);
        match result {
            Ok(mut clip_frames) if !clip_frames.is_empty() => frames.append(&mut clip_frames),
            _ => skipped.push(clip.clone()),
        }
        on_progress(Progress { done, total, clip: &name });
    }
    drop(jobs);

    frames.sort_by(|a, b| a.clip_path.cmp(&b.clip_path).then(a.t_sec.cmp(&b.t_sec)));
    Ok(Prepared {
        folder: folder.to_path_buf(),
        temp_dir,
        frames,
        clip_count: total,
        skipped,
    })
}

/// Pulls frames from one clip. Tries the fast keyframe-only pass first; if the clip's
/// keyframes are too far apart to give a frame every 2 seconds, decodes it fully instead.
async fn extract(clip: &Path, out: &Path) -> Result<Vec<Frame>, String> {
    let fast = run_ffmpeg(clip, out, true).await;
    let expected = expected_frames(clip).await;
    match fast {
        Ok(frames) if frames.len() * 2 >= expected && !frames.is_empty() => Ok(frames),
        _ => {
            let _ = std::fs::remove_dir_all(out);
            run_ffmpeg(clip, out, false).await
        }
    }
}

/// One ffmpeg run with two outputs: LOG previews and boosted frames.
async fn run_ffmpeg(clip: &Path, out: &Path, keyframes_only: bool) -> Result<Vec<Frame>, String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let graph = format!(
        "[0:v]fps=1/{SAMPLE_EVERY_SEC},{SCALE},split=2[log][b];[b]{BOOST}[boost]"
    );
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-nostdin", "-loglevel", "fatal"]);
    if keyframes_only {
        // Decode keyframes only: ~30× faster on 4K HEVC.
        cmd.args(["-skip_frame", "nokey"]);
    }
    let status = cmd
        // Decode on the Mac's video hardware.
        .args(["-hwaccel", "videotoolbox"])
        .arg("-i")
        .arg(clip)
        .args(["-filter_complex", &graph])
        .args(["-map", "[log]", "-q:v", "4"])
        .arg(out.join("log_%05d.jpg"))
        .args(["-map", "[boost]", "-q:v", "4"])
        .arg(out.join("boost_%05d.jpg"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map_err(|e| format!("couldn't run ffmpeg: {e}"))?;
    if !status.success() {
        return Err(format!("ffmpeg failed on {}", clip.display()));
    }

    let name = clip_name(clip);
    let mut frames = Vec::new();
    for n in 1.. {
        let log_jpg = out.join(format!("log_{n:05}.jpg"));
        let boost_jpg = out.join(format!("boost_{n:05}.jpg"));
        if !log_jpg.exists() || !boost_jpg.exists() {
            break;
        }
        frames.push(Frame {
            clip_path: clip.to_path_buf(),
            clip_name: name.clone(),
            t_sec: timestamp_of(n),
            log_jpg,
            boost_jpg,
        });
    }
    Ok(frames)
}

/// How many frames a clip should give at one every 2 seconds, from its duration.
async fn expected_frames(clip: &Path) -> usize {
    let output = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"])
        .arg(clip)
        .output()
        .await;
    let secs: f64 = output
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0.0);
    (secs / SAMPLE_EVERY_SEC as f64).ceil() as usize
}

/// ffmpeg numbers frames from 1; frame n was taken at (n-1) × 2 seconds.
fn timestamp_of(n: u32) -> u32 {
    (n - 1) * SAMPLE_EVERY_SEC
}

fn clip_name(clip: &Path) -> String {
    clip.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_numbers_map_to_timestamps() {
        assert_eq!(timestamp_of(1), 0);
        assert_eq!(timestamp_of(2), 2);
        assert_eq!(timestamp_of(31), 60);
    }
}
