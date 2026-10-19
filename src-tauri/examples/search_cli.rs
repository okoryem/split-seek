//! Build step 2: search a folder of clips from the terminal.
//! Usage: cargo run --example search_cli -- <folder> ["<phrase>" ...] [--raw | --compare]
//! With no phrases, it asks for them one at a time after preparing.
//!   --raw      Jev looks at the flat LOG frames instead of the boosted ones
//!   --compare  run every phrase both ways, side by side
//!   --forget   clear remembered frames first, so every clip is prepared fresh

use splitseek_lib::frames::{self, Progress};
use splitseek_lib::jev::Jev;
use splitseek_lib::search::{self, format_range, format_timestamp, Moment, SearchResult, Source, CUTOFF};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};

const SHOW_ABOVE: usize = 15;
const SHOW_BELOW: usize = 5;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let raw = args.iter().any(|a| a == "--raw");
    let compare = args.iter().any(|a| a == "--compare");
    let forget = args.iter().any(|a| a == "--forget");
    let mut positional = args.iter().filter(|a| !a.starts_with("--"));
    let Some(folder) = positional.next().map(PathBuf::from) else {
        eprintln!("usage: search_cli <folder> [\"<phrase>\" ...] [--raw | --compare]");
        std::process::exit(2);
    };
    let phrases: Vec<String> = positional.cloned().collect();

    let jev = Jev::from_env().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });

    println!("Pulling frames from your clips in {} …", folder.display());
    let start = Instant::now();
    let cache_root = frames::default_cache_root();
    if forget {
        frames::forget_all(&cache_root).expect("couldn't clear the frame cache");
    }
    let prepared = frames::prepare(&folder, &cache_root, |p: Progress| {
        print!("\r  {} / {}  {:<50}", p.done, p.total, p.clip);
        let _ = std::io::stdout().flush();
    })
    .await
    .unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    println!(
        "\rPrepared {} clips ({} remembered) → {} frames in {:.1} s ({} skipped). Cache: {}",
        prepared.clip_count,
        prepared.cached_count,
        prepared.frames.len(),
        start.elapsed().as_secs_f32(),
        prepared.skipped.len(),
        cache_root.display()
    );
    for s in &prepared.skipped {
        println!("  skipped (ffmpeg couldn't read it): {}", s.display());
    }

    let run = |phrase: String| {
        let jev = &jev;
        let frames = &prepared.frames;
        async move {
            if compare {
                for (label, source) in [("BOOSTED", Source::Boosted), ("RAW LOG", Source::RawLog)] {
                    println!("\n── {label} ──");
                    report(&phrase, search::search(jev, frames, &phrase, source).await);
                }
            } else {
                let source = if raw { Source::RawLog } else { Source::Boosted };
                report(&phrase, search::search(jev, frames, &phrase, source).await);
            }
        }
    };

    if phrases.is_empty() {
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        loop {
            print!("\nSearch (empty to quit): ");
            let _ = std::io::stdout().flush();
            match lines.next_line().await {
                Ok(Some(line)) if !line.trim().is_empty() => run(line.trim().to_string()).await,
                _ => break,
            }
        }
    } else {
        for phrase in phrases {
            run(phrase).await;
        }
    }
}

fn report(phrase: &str, result: Result<SearchResult, splitseek_lib::jev::JevError>) {
    let r = match result {
        Ok(r) => r,
        Err(e) => return println!("\"{phrase}\": {e}"),
    };
    println!(
        "\n\"{phrase}\" — {} frames in {:.1} s{}",
        r.frames_scored,
        r.elapsed_ms as f32 / 1000.0,
        if r.frames_failed > 0 { format!(", {} failed", r.frames_failed) } else { String::new() }
    );
    if r.above.is_empty() && r.below.is_empty() {
        return println!("  Nothing found.");
    }
    if r.above.is_empty() {
        println!("  Nothing above {:.0}%, but these came close:", CUTOFF * 100.0);
    }
    for m in r.above.iter().take(SHOW_ABOVE) {
        row(m);
    }
    if r.above.len() > SHOW_ABOVE {
        println!("  … {} more stretches above the cutoff", r.above.len() - SHOW_ABOVE);
    }
    if !r.below.is_empty() {
        println!("  -- below {:.0}% ({} stretches), best:", CUTOFF * 100.0, r.below.len());
        for m in r.below.iter().take(SHOW_BELOW) {
            row(m);
        }
    }
}

fn row(m: &Moment) {
    println!(
        "  {:>5.1}%  {:<13} best at {:<6} {}",
        m.score * 100.0,
        format_range(m.start_sec, m.end_sec),
        format_timestamp(m.best_sec),
        m.clip_name
    );
}
