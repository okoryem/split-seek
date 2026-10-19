//! `clip://` — serves the prepared folder's video files to the window's player,
//! in pieces (HTTP range requests), with the right video type. Only clips in the
//! prepared folder are served; everything else gets 403.
//!
//! Why not Tauri's built-in asset protocol: it decides the file type from a
//! lowercase extension or the file's first bytes, so Canon/DJI `.MP4` files were
//! labelled `text/html` and the player showed a black picture.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tauri::http::{header, Response, StatusCode};

/// Largest piece sent per request. Players ask for more as they need it.
const MAX_CHUNK: u64 = 4 * 1024 * 1024;

/// `clip://localhost/%2FUsers%2F…%2Fclip.MP4` → `/Users/…/clip.MP4`
pub fn path_from_uri_path(uri_path: &str) -> PathBuf {
    let encoded = uri_path.trim_start_matches('/');
    PathBuf::from(percent_encoding::percent_decode_str(encoded).decode_utf8_lossy().into_owned())
}

pub fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("mov") => "video/quicktime",
        _ => "video/mp4",
    }
}

/// The inclusive byte range to send for a `Range` header (or none), capped at MAX_CHUNK.
/// Returns None if the range can't be satisfied.
pub fn byte_range(range_header: Option<&str>, file_len: u64) -> Option<(u64, u64)> {
    if file_len == 0 {
        return None;
    }
    let last = file_len - 1;
    let (start, end) = match range_header.and_then(|h| h.trim().strip_prefix("bytes=")) {
        None => (0, last),
        Some(spec) => {
            let (a, b) = spec.split(',').next()?.split_once('-')?;
            match (a.trim(), b.trim()) {
                // "bytes=-500": the last 500 bytes
                ("", n) => {
                    let n: u64 = n.parse().ok()?;
                    (file_len.saturating_sub(n), last)
                }
                (a, "") => (a.parse().ok()?, last),
                (a, b) => (a.parse().ok()?, b.parse::<u64>().ok()?.min(last)),
            }
        }
    };
    if start > end || start > last {
        return None;
    }
    Some((start, end.min(start + MAX_CHUNK - 1)))
}

/// Builds the response for one request, given whether the clip is allowed.
pub fn respond(path: &Path, allowed: bool, range_header: Option<&str>) -> Response<Vec<u8>> {
    let status_only = |status: StatusCode| {
        Response::builder().status(status).body(Vec::new()).unwrap()
    };
    if !allowed {
        return status_only(StatusCode::FORBIDDEN);
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return status_only(StatusCode::NOT_FOUND);
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let Some((start, end)) = byte_range(range_header, len) else {
        return Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{len}"))
            .body(Vec::new())
            .unwrap();
    };

    let mut body = vec![0; (end - start + 1) as usize];
    if file.seek(SeekFrom::Start(start)).and_then(|_| file.read_exact(&mut body)).is_err() {
        return status_only(StatusCode::INTERNAL_SERVER_ERROR);
    }
    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, content_type(path))
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"))
        .header(header::CONTENT_LENGTH, body.len().to_string())
        .body(body)
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_clip_path() {
        assert_eq!(
            path_from_uri_path("/%2FVolumes%2FNew%20Volume%2FPeru%2FA_0001.MP4"),
            PathBuf::from("/Volumes/New Volume/Peru/A_0001.MP4")
        );
    }

    #[test]
    fn video_type_ignores_extension_case() {
        assert_eq!(content_type(Path::new("a.MP4")), "video/mp4");
        assert_eq!(content_type(Path::new("a.m4v")), "video/mp4");
        assert_eq!(content_type(Path::new("a.MOV")), "video/quicktime");
    }

    #[test]
    fn ranges() {
        let mb = 1024 * 1024;
        assert_eq!(byte_range(Some("bytes=0-1023"), 10 * mb), Some((0, 1023)));
        assert_eq!(byte_range(Some("bytes=100-"), 1000), Some((100, 999)));
        assert_eq!(byte_range(Some("bytes=-100"), 1000), Some((900, 999)));
        assert_eq!(byte_range(Some("bytes=0-99999"), 1000), Some((0, 999)));
        assert_eq!(byte_range(None, 1000), Some((0, 999)));
        // Open-ended requests on a big file are capped at one chunk.
        assert_eq!(byte_range(Some("bytes=0-"), 100 * mb), Some((0, 4 * mb - 1)));
        assert_eq!(byte_range(Some("bytes=2000-"), 1000), None);
        assert_eq!(byte_range(Some("bytes=abc"), 1000), None);
    }

    #[test]
    fn serves_a_piece_of_an_allowed_file_and_refuses_others() {
        let dir = tempfile::tempdir().unwrap();
        let clip = dir.path().join("CLIP.MP4");
        std::fs::write(&clip, (0..=255u8).collect::<Vec<_>>()).unwrap();

        let ok = respond(&clip, true, Some("bytes=10-19"));
        assert_eq!(ok.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(ok.body(), &(10..=19u8).collect::<Vec<_>>());
        assert_eq!(ok.headers()[header::CONTENT_TYPE], "video/mp4");
        assert_eq!(ok.headers()[header::CONTENT_RANGE], "bytes 10-19/256");

        assert_eq!(respond(&clip, false, None).status(), StatusCode::FORBIDDEN);
    }
}
