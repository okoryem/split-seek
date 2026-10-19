//! Frame preparer: runs ffmpeg on each clip to pull one frame every 2 seconds,
//! saving a LOG preview and a boosted copy for Jev into SplitSeek's frame cache.
//! Each clip gets its own cache folder, so a clip is only ever prepared once
//! unless it changes. Original clips are only ever read.

use futures::stream::{self, StreamExt};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
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
/// Generic LOG boost: more contrast and color.
const BOOST: &str = "eq=contrast=1.35:saturation=1.6:gamma=1.1";

#[derive(Debug, Clone)]
pub struct Frame {
    pub clip_path: PathBuf,
    pub clip_name: String,
    pub t_sec: u32,
    pub log_jpg: PathBuf,
    pub boost_jpg: PathBuf,
}

/// A folder that's been prepared.
pub struct Prepared {
    pub folder: PathBuf,
    pub frames: Vec<Frame>,
    pub clip_count: usize,
    /// Clips loaded from the cache without running ffmpeg.
    pub cached_count: usize,
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

/// Where remembered frames live: `~/Library/Caches/com.splitseek.app/frames`.
pub fn default_cache_root() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    home.join("Library/Caches/com.splitseek.app/frames")
}

/// Deletes every remembered frame.
pub fn forget_all(cache_root: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(cache_root) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

pub async fn prepare(
    folder: &Path,
    cache_root: &Path,
    on_progress: impl Fn(Progress) + Send + Sync,
) -> Result<Prepared, PrepareError> {
    let clips = scan::find_videos(folder).map_err(|e| PrepareError::Io(e.to_string()))?;
    if clips.is_empty() {
        return Err(PrepareError::NoVideos);
    }
    std::fs::create_dir_all(cache_root).map_err(|e| PrepareError::Io(e.to_string()))?;

    let total = clips.len();
    let mut frames = Vec::new();
    let mut skipped = Vec::new();
    let mut cached_count = 0;
    let mut done = 0;

    // Each job owns its paths so preparation can run on any thread.
    let mut jobs = stream::iter(clips)
        .map(|clip| {
            let root = cache_root.to_path_buf();
            async move {
                let result = prepare_clip(&clip, &root).await;
                (clip, result)
            }
        })
        .buffer_unordered(PARALLEL_CLIPS);

    while let Some((clip, result)) = jobs.next().await {
        done += 1;
        match result {
            Ok((mut clip_frames, from_cache)) if !clip_frames.is_empty() => {
                frames.append(&mut clip_frames);
                cached_count += from_cache as usize;
            }
            _ => skipped.push(clip.clone()),
        }
        on_progress(Progress { done, total, clip: &clip_name(&clip) });
    }
    drop(jobs);

    frames.sort_by(|a, b| a.clip_path.cmp(&b.clip_path).then(a.t_sec.cmp(&b.t_sec)));
    Ok(Prepared {
        folder: folder.to_path_buf(),
        frames,
        clip_count: total,
        cached_count,
        skipped,
    })
}

/// Loads one clip's frames from the cache, or extracts them. Returns (frames, from_cache).
async fn prepare_clip(clip: &Path, cache_root: &Path) -> Result<(Vec<Frame>, bool), String> {
    let meta = std::fs::metadata(clip).map_err(|e| e.to_string())?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dir = cache_root.join(clip_key(clip, meta.len(), mtime));
    if dir.is_dir() {
        return Ok((list_frames(clip, &dir), true));
    }

    // Write into a .partial folder and rename when complete, so an interrupted
    // preparation never looks like a finished clip.
    let partial = dir.with_extension("partial");
    let _ = std::fs::remove_dir_all(&partial);
    if let Err(e) = extract(clip, &partial).await {
        let _ = std::fs::remove_dir_all(&partial);
        return Err(e);
    }
    std::fs::rename(&partial, &dir).map_err(|e| e.to_string())?;
    Ok((list_frames(clip, &dir), false))
}

/// A cache folder name that changes whenever the clip's path, size, or modified time does.
/// Readable prefix from the file name, then a 64-bit FNV-1a hash.
fn clip_key(clip: &Path, size: u64, mtime_secs: u64) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    let input = format!("{}\0{size}\0{mtime_secs}", clip.display());
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let stem: String = clip
        .file_stem()
        .map(|s| s.to_string_lossy().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(40).collect())
        .unwrap_or_default();
    format!("{stem}-{hash:016x}")
}

/// Pulls frames from one clip. Tries the fast keyframe-only pass first; if the clip's
/// keyframes are too far apart to give a frame every 2 seconds, decodes it fully instead.
async fn extract(clip: &Path, out: &Path) -> Result<(), String> {
    let fast_ok = run_ffmpeg(clip, out, true).await.is_ok();
    let got = list_frames(clip, out).len();
    if fast_ok && got > 0 && got * 2 >= expected_frames(clip).await {
        return Ok(());
    }
    let _ = std::fs::remove_dir_all(out);
    run_ffmpeg(clip, out, false).await?;
    if list_frames(clip, out).is_empty() {
        return Err(format!("no frames from {}", clip.display()));
    }
    Ok(())
}

/// One ffmpeg run with two outputs: LOG previews and boosted frames.
async fn run_ffmpeg(clip: &Path, out: &Path, keyframes_only: bool) -> Result<(), String> {
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
    Ok(())
}

/// The frames already written to `dir`, in order, until the first missing pair.
fn list_frames(clip: &Path, dir: &Path) -> Vec<Frame> {
    let name = clip_name(clip);
    let mut frames = Vec::new();
    for n in 1.. {
        let log_jpg = dir.join(format!("log_{n:05}.jpg"));
        let boost_jpg = dir.join(format!("boost_{n:05}.jpg"));
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
    frames
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

    #[test]
    fn cache_key_changes_when_the_clip_does() {
        let p = Path::new("/Volumes/Passport/Peru/DJI_0001.MP4");
        let key = clip_key(p, 1000, 50);
        assert_eq!(key, clip_key(p, 1000, 50));
        assert!(key.starts_with("DJI_0001-"));
        assert_ne!(key, clip_key(p, 1001, 50), "size changed");
        assert_ne!(key, clip_key(p, 1000, 51), "modified time changed");
        assert_ne!(key, clip_key(Path::new("/Volumes/Passport/Chile/DJI_0001.MP4"), 1000, 50));
    }

    /// Needs ffmpeg on the PATH: makes a tiny clip, prepares it twice, and checks the
    /// second run comes from the cache and a leftover `.partial` folder is ignored.
    #[tokio::test]
    async fn second_prepare_comes_from_cache() {
        let clips = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let clip = clips.path().join("test.mp4");
        let made = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-f", "lavfi", "-i", "testsrc=duration=5:size=640x360:rate=30"])
            .args(["-c:v", "libx264", "-g", "30"])
            .arg(&clip)
            .status()
            .unwrap();
        assert!(made.success());

        // A leftover from an interrupted run must not count as cached.
        let meta = std::fs::metadata(&clip).unwrap();
        let mtime = meta.modified().unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let key = clip_key(&clip, meta.len(), mtime);
        std::fs::create_dir_all(cache.path().join(format!("{key}.partial"))).unwrap();

        let first = prepare(clips.path(), cache.path(), |_| {}).await.unwrap();
        assert_eq!((first.clip_count, first.cached_count), (1, 0));
        assert!(!first.frames.is_empty());
        assert!(!cache.path().join(format!("{key}.partial")).exists());

        let second = prepare(clips.path(), cache.path(), |_| {}).await.unwrap();
        assert_eq!((second.clip_count, second.cached_count), (1, 1));
        assert_eq!(second.frames.len(), first.frames.len());

        // Nothing was written beside the original clip.
        assert_eq!(std::fs::read_dir(clips.path()).unwrap().count(), 1);

        forget_all(cache.path()).unwrap();
        assert!(!cache.path().exists());
    }
}
