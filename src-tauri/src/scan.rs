//! Folder intake: finds the video files in a folder, including subfolders
//! (SD cards nest clips under DCIM/...). Only reads directory listings.

use std::path::{Path, PathBuf};

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v"];

/// All videos under `dir`, sorted by path. Skips hidden files and folders
/// (including macOS `._` sidecar files) and anything that isn't a video,
/// such as DJI `.LRF` proxies.
pub fn find_videos(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    walk(dir, &mut found)?;
    found.sort();
    Ok(found)
}

fn walk(dir: &Path, found: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let kind = entry.file_type()?;
        // Follow linked files (e.g. a clip linked from another drive), but not linked
        // folders, which could loop.
        let is_linked_file = kind.is_symlink() && path.is_file();
        if kind.is_dir() {
            // An unreadable subfolder shouldn't stop the whole scan.
            let _ = walk(&path, found);
        } else if (kind.is_file() || is_linked_file) && is_video(&path) {
            found.push(path);
        }
    }
    Ok(())
}

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_nested_videos_and_skips_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("DCIM/100MEDIA")).unwrap();
        fs::create_dir_all(root.join(".Trashes")).unwrap();
        for f in [
            "a.MP4",
            "b.mov",
            "DCIM/100MEDIA/c.m4v",
            "DCIM/100MEDIA/c.LRF",
            "._a.MP4",
            ".Trashes/d.mp4",
            "notes.txt",
            "photo.CR3",
        ] {
            fs::write(root.join(f), b"").unwrap();
        }

        let names: Vec<String> = find_videos(root)
            .unwrap()
            .iter()
            .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["DCIM/100MEDIA/c.m4v", "a.MP4", "b.mov"]);
    }

    #[test]
    fn empty_folder_has_no_videos() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_videos(dir.path()).unwrap().is_empty());
    }
}
