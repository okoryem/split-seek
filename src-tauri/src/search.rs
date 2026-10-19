//! Search and ranking: asks Jev about every prepared frame, groups back-to-back
//! matching frames of a clip into one stretch, then sorts the stretches by their
//! best score and splits them at the cutoff.

use futures::stream::{self, StreamExt};
use std::path::PathBuf;
use std::time::Instant;

use crate::frames::{Frame, SAMPLE_EVERY_SEC};
use crate::jev::{Jev, JevError};

/// Moments scoring at or above this are shown by default. Tuned on real footage.
pub const CUTOFF: f32 = 0.60;
/// How many Jev questions run at once.
const PARALLEL_REQUESTS: usize = 64;

/// One result row: a stretch of a clip where consecutive frames all sit on the same
/// side of the cutoff. A single matching frame is a stretch of length one.
#[derive(Debug, Clone)]
pub struct Moment {
    pub clip_path: PathBuf,
    pub clip_name: String,
    pub start_sec: u32,
    pub end_sec: u32,
    /// The best-scoring frame in the stretch; its score is the stretch's score.
    pub best_sec: u32,
    pub score: f32,
    /// LOG preview of the best frame.
    pub log_jpg: PathBuf,
}

pub struct SearchResult {
    /// Stretches at or above the cutoff, best first.
    pub above: Vec<Moment>,
    /// Stretches below the cutoff, best first.
    pub below: Vec<Moment>,
    pub frames_scored: usize,
    /// Frames Jev couldn't answer for, even after retries. Left out of the results.
    pub frames_failed: usize,
    pub elapsed_ms: u128,
}

/// Which copy of each frame Jev looks at.
#[derive(Clone, Copy, PartialEq)]
pub enum Source {
    Boosted,
    RawLog,
}

pub async fn search(
    jev: &Jev,
    frames: &[Frame],
    phrase: &str,
    source: Source,
) -> Result<SearchResult, JevError> {
    let start = Instant::now();
    // One job per frame, each owning its path, so the search can run on any thread.
    let jobs: Vec<_> = frames
        .iter()
        .enumerate()
        .map(|(i, frame)| {
            let path = match source {
                Source::Boosted => frame.boost_jpg.clone(),
                Source::RawLog => frame.log_jpg.clone(),
            };
            async move {
                let score = match tokio::fs::read(&path).await {
                    Ok(jpeg) => jev.score_jpeg_with_retry(&jpeg, phrase).await.map(|s| s.noul),
                    Err(e) => Err(JevError::Fatal(e.to_string())),
                };
                (i, score)
            }
        })
        .collect();
    let results: Vec<(usize, Result<f32, JevError>)> =
        stream::iter(jobs).buffer_unordered(PARALLEL_REQUESTS).collect().await;

    let mut scored = Vec::new();
    let mut failures = Vec::new();
    for (i, r) in results {
        match r {
            Ok(score) => scored.push((i, score)),
            Err(e) => failures.push(e),
        }
    }
    // If nothing came back at all (bad key, Jev down), report why instead of "nothing found".
    if scored.is_empty() {
        if let Some(e) = failures.into_iter().next() {
            return Err(e);
        }
        return Ok(SearchResult {
            above: vec![],
            below: vec![],
            frames_scored: 0,
            frames_failed: 0,
            elapsed_ms: start.elapsed().as_millis(),
        });
    }

    let frames_scored = scored.len();
    let frames_failed = failures.len();

    // Frames arrive in clip-then-time order from preparation; restore that order.
    scored.sort_by_key(|(i, _)| *i);
    let mut clip_key = 0;
    let points: Vec<Point> = scored
        .iter()
        .enumerate()
        .map(|(n, &(i, score))| {
            if n > 0 && frames[scored[n - 1].0].clip_path != frames[i].clip_path {
                clip_key += 1;
            }
            Point { clip: clip_key, t_sec: frames[i].t_sec, score }
        })
        .collect();

    let (above, below) = rank(group(&points, CUTOFF), CUTOFF);
    let to_moment = |span: Span| {
        let best = &frames[scored[span.best].0];
        Moment {
            clip_path: best.clip_path.clone(),
            clip_name: best.clip_name.clone(),
            start_sec: points[span.first].t_sec,
            end_sec: points[span.last].t_sec,
            best_sec: best.t_sec,
            score: span.score,
            log_jpg: best.log_jpg.clone(),
        }
    };
    let result = SearchResult {
        above: above.into_iter().map(to_moment).collect(),
        below: below.into_iter().map(to_moment).collect(),
        frames_scored,
        frames_failed,
        elapsed_ms: start.elapsed().as_millis(),
    };
    eprintln!(
        "[search] \"{phrase}\": {} frames in {} ms ({} failed), {} stretches above cutoff",
        result.frames_scored,
        result.elapsed_ms,
        result.frames_failed,
        result.above.len()
    );
    Ok(result)
}

/// One scored frame, in clip-then-time order. `clip` is a number per clip.
struct Point {
    clip: usize,
    t_sec: u32,
    score: f32,
}

/// A run of consecutive points, by position in the points list.
#[derive(Debug, PartialEq)]
struct Span {
    first: usize,
    last: usize,
    best: usize,
    score: f32,
}

/// Joins back-to-back frames of the same clip that sit on the same side of the cutoff.
fn group(points: &[Point], cutoff: f32) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    for (n, p) in points.iter().enumerate() {
        let continues = n > 0 && {
            let prev = &points[n - 1];
            prev.clip == p.clip
                && p.t_sec == prev.t_sec + SAMPLE_EVERY_SEC
                && (prev.score >= cutoff) == (p.score >= cutoff)
        };
        match spans.last_mut() {
            Some(span) if continues => {
                span.last = n;
                if p.score > span.score {
                    span.best = n;
                    span.score = p.score;
                }
            }
            _ => spans.push(Span { first: n, last: n, best: n, score: p.score }),
        }
    }
    spans
}

/// Sorts stretches best first and splits them at the cutoff.
fn rank(mut spans: Vec<Span>, cutoff: f32) -> (Vec<Span>, Vec<Span>) {
    spans.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.first.cmp(&b.first)));
    let split = spans.partition_point(|s| s.score >= cutoff);
    let below = spans.split_off(split);
    (spans, below)
}

/// 75 → "1:15"
pub fn format_timestamp(t_sec: u32) -> String {
    format!("{}:{:02}", t_sec / 60, t_sec % 60)
}

/// "0:04–0:26", or just "0:04" for a single frame.
pub fn format_range(start_sec: u32, end_sec: u32) -> String {
    if start_sec == end_sec {
        format_timestamp(start_sec)
    } else {
        format!("{}–{}", format_timestamp(start_sec), format_timestamp(end_sec))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(list: &[(usize, u32, f32)]) -> Vec<Point> {
        list.iter().map(|&(clip, t_sec, score)| Point { clip, t_sec, score }).collect()
    }

    #[test]
    fn back_to_back_matches_become_one_stretch() {
        // Clip 0: 0:00 no, 0:02–0:06 yes (best at 0:04), 0:08 no.
        let p = pts(&[(0, 0, 0.1), (0, 2, 0.8), (0, 4, 0.95), (0, 6, 0.7), (0, 8, 0.2)]);
        let spans = group(&p, 0.30);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1], Span { first: 1, last: 3, best: 2, score: 0.95 });
    }

    #[test]
    fn stretches_stop_at_clip_boundaries_and_gaps() {
        // Same times in two clips, and a missing frame (failed) at 0:04 in clip 0.
        let p = pts(&[(0, 0, 0.9), (0, 2, 0.9), (0, 6, 0.9), (1, 8, 0.9), (1, 10, 0.9)]);
        let spans = group(&p, 0.30);
        let ranges: Vec<(usize, usize)> = spans.iter().map(|s| (s.first, s.last)).collect();
        assert_eq!(ranges, vec![(0, 1), (2, 2), (3, 4)]);
    }

    #[test]
    fn ranks_stretches_best_first_and_splits_at_cutoff() {
        let p = pts(&[(0, 0, 0.5), (1, 0, 0.9), (2, 0, 0.29), (3, 0, 0.3)]);
        let (above, below) = rank(group(&p, 0.30), 0.30);
        let a: Vec<f32> = above.iter().map(|s| s.score).collect();
        let b: Vec<f32> = below.iter().map(|s| s.score).collect();
        assert_eq!(a, vec![0.9, 0.5, 0.3]);
        assert_eq!(b, vec![0.29]);
    }

    #[test]
    fn formats_timestamps_and_ranges() {
        assert_eq!(format_timestamp(0), "0:00");
        assert_eq!(format_timestamp(75), "1:15");
        assert_eq!(format_timestamp(600), "10:00");
        assert_eq!(format_range(4, 26), "0:04–0:26");
        assert_eq!(format_range(8, 8), "0:08");
    }
}
