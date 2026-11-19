//! Saved folders: after a folder is prepared, a small record of which cached frames
//! belong to which clip is saved, so the folder can be searched again later, even
//! with its drive unplugged. One JSON file per folder, in
//! `~/Library/Caches/com.splitseek.app/folders/`. The frames themselves stay in the
//! frame cache (frames.rs); a record only points at them.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::frames::{self, Prepared};

#[derive(Debug, Serialize, Deserialize)]
pub struct SavedFolder {
    pub folder: PathBuf,
    /// Seconds since 1970 when the folder was last prepared.
    pub prepared_at: u64,
    pub clip_count: usize,
    pub frame_count: usize,
    pub skipped: Vec<PathBuf>,
    pub clips: Vec<SavedClip>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SavedClip {
    pub clip_path: PathBuf,
    /// The clip's folder name inside the frame cache.
    pub cache_dir: String,
}

/// Where saved-folder records live: beside the frame cache.
pub fn default_root() -> PathBuf {
    let frames = frames::default_cache_root();
    frames.parent().map(|p| p.join("folders")).unwrap_or_else(|| frames.join("folders"))
}

fn record_path(root: &Path, folder: &Path) -> PathBuf {
    root.join(format!("{:016x}.json", frames::fnv1a(&folder.display().to_string())))
}

/// Saves (or replaces) the record for a freshly prepared folder.
pub fn save(root: &Path, prepared: &Prepared) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    let record = SavedFolder {
        folder: prepared.folder.clone(),
        prepared_at: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        clip_count: prepared.clip_count,
        frame_count: prepared.frames.len(),
        skipped: prepared.skipped.clone(),
        clips: prepared
            .clip_dirs
            .iter()
            .map(|(clip_path, cache_dir)| SavedClip { clip_path: clip_path.clone(), cache_dir: cache_dir.clone() })
            .collect(),
    };
    let json = serde_json::to_vec(&record).map_err(std::io::Error::other)?;
    // Write then rename, so a crash never leaves half a record.
    let path = record_path(root, &prepared.folder);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, &path)
}

/// Every saved folder, most recently prepared first. Unreadable records are skipped.
pub fn list(root: &Path) -> Vec<SavedFolder> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut folders: Vec<SavedFolder> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok())
        .collect();
    folders.sort_by(|a, b| b.prepared_at.cmp(&a.prepared_at));
    folders
}

/// Rebuilds a saved folder from its record and the frame cache, without touching the
/// folder itself. Clips whose frames are no longer cached are left out. Returns
/// `None` if there's no record or none of its frames are left.
pub fn load(root: &Path, cache_root: &Path, folder: &Path) -> Option<Prepared> {
    let record: SavedFolder = serde_json::from_slice(&std::fs::read(record_path(root, folder)).ok()?).ok()?;
    let mut frames_out = Vec::new();
    let mut clip_dirs = Vec::new();
    for clip in record.clips {
        let mut clip_frames = frames::list_frames(&clip.clip_path, &cache_root.join(&clip.cache_dir));
        if !clip_frames.is_empty() {
            frames_out.append(&mut clip_frames);
            clip_dirs.push((clip.clip_path, clip.cache_dir));
        }
    }
    if frames_out.is_empty() {
        return None;
    }
    Some(Prepared {
        folder: record.folder,
        frames: frames_out,
        clip_count: record.clip_count,
        cached_count: clip_dirs.len(),
        skipped: record.skipped,
        clip_dirs,
    })
}

/// Removes one folder from the saved list. Its cached frames are kept.
pub fn remove(root: &Path, folder: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(record_path(root, folder)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Removes every saved-folder record.
pub fn forget_all(root: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(root) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Makes a fake cached clip with `n` frame pairs.
    fn fake_cached_clip(cache: &Path, dir: &str, n: u32) {
        let d = cache.join(dir);
        std::fs::create_dir_all(&d).unwrap();
        for i in 1..=n {
            std::fs::write(d.join(format!("log_{i:05}.jpg")), b"x").unwrap();
            std::fs::write(d.join(format!("boost_{i:05}.jpg")), b"x").unwrap();
        }
    }

    fn prepared(folder: &str, clips: &[(&str, &str)], cache: &Path) -> Prepared {
        let clip_dirs: Vec<(PathBuf, String)> =
            clips.iter().map(|(c, d)| (PathBuf::from(c), d.to_string())).collect();
        let frames = clip_dirs.iter().flat_map(|(c, d)| frames::list_frames(c, &cache.join(d))).collect();
        Prepared {
            folder: PathBuf::from(folder),
            frames,
            clip_count: clips.len(),
            cached_count: 0,
            skipped: Vec::new(),
            clip_dirs,
        }
    }

    #[test]
    fn saved_folder_loads_without_the_drive() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        fake_cached_clip(cache.path(), "A-1", 3);
        fake_cached_clip(cache.path(), "B-2", 2);
        // The folder doesn't exist on disk, like an unplugged drive.
        let folder = "/Volumes/Gone/Peru";
        let p = prepared(folder, &[("/Volumes/Gone/Peru/A.MP4", "A-1"), ("/Volumes/Gone/Peru/B.MP4", "B-2")], cache.path());
        save(root.path(), &p).unwrap();

        let loaded = load(root.path(), cache.path(), Path::new(folder)).unwrap();
        assert_eq!(loaded.folder, PathBuf::from(folder));
        assert_eq!(loaded.frames.len(), 5);
        assert_eq!((loaded.clip_count, loaded.cached_count), (2, 2));
        assert_eq!(loaded.frames[3].clip_name, "B.MP4");
        assert_eq!(loaded.frames[4].t_sec, 2);

        let listed = list(root.path());
        assert_eq!(listed.len(), 1);
        assert_eq!((listed[0].clip_count, listed[0].frame_count), (2, 5));
    }

    #[test]
    fn clips_whose_frames_were_deleted_are_left_out() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        fake_cached_clip(cache.path(), "A-1", 2);
        fake_cached_clip(cache.path(), "B-2", 2);
        let p = prepared("/x", &[("/x/A.MP4", "A-1"), ("/x/B.MP4", "B-2")], cache.path());
        save(root.path(), &p).unwrap();

        std::fs::remove_dir_all(cache.path().join("B-2")).unwrap();
        let loaded = load(root.path(), cache.path(), Path::new("/x")).unwrap();
        assert_eq!((loaded.frames.len(), loaded.cached_count), (2, 1));

        std::fs::remove_dir_all(cache.path().join("A-1")).unwrap();
        assert!(load(root.path(), cache.path(), Path::new("/x")).is_none());
    }

    #[test]
    fn saving_again_replaces_and_remove_forgets() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        fake_cached_clip(cache.path(), "A-1", 1);
        fake_cached_clip(cache.path(), "B-2", 1);
        save(root.path(), &prepared("/x", &[("/x/A.MP4", "A-1")], cache.path())).unwrap();
        save(root.path(), &prepared("/x", &[("/x/A.MP4", "A-1"), ("/x/B.MP4", "B-2")], cache.path())).unwrap();
        save(root.path(), &prepared("/y", &[("/y/A.MP4", "A-1")], cache.path())).unwrap();
        assert_eq!(list(root.path()).len(), 2);

        remove(root.path(), Path::new("/x")).unwrap();
        let left = list(root.path());
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].folder, PathBuf::from("/y"));
        assert!(load(root.path(), cache.path(), Path::new("/x")).is_none());

        forget_all(root.path()).unwrap();
        assert!(list(root.path()).is_empty());
    }
}
