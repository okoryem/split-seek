pub mod clip_protocol;
pub mod frames;
pub mod jev;
pub mod scan;
pub mod search;

use base64::Engine as _;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use frames::{Prepared, Progress};
use jev::Jev;
use search::{Moment, Source};

/// Most near-miss rows sent to the window for "Show more".
const MAX_BELOW_ROWS: usize = 30;

/// What the engine keeps while the app is open: the folder currently prepared, and
/// the Jev client (created on first search, so a key added to .env later still works).
#[derive(Default)]
struct AppState {
    prepared: Mutex<Option<Prepared>>,
    jev: Mutex<Option<Arc<Jev>>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    done: usize,
    total: usize,
    clip: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrepareSummary {
    folder: String,
    folder_name: String,
    clip_count: usize,
    remembered_count: usize,
    frame_count: usize,
    skipped_count: usize,
}

/// Prepares every clip in `path` (from the cache where possible), sending a
/// `prepare-progress` event after each clip. Replaces any folder prepared before.
#[tauri::command]
async fn prepare_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<PrepareSummary, String> {
    let folder = PathBuf::from(&path);
    if !folder.is_dir() {
        return Err("That's a file, not a folder. Choose or drop a folder of clips.".into());
    }
    let mut slot = state.prepared.lock().await;
    *slot = None;

    let started = std::time::Instant::now();
    let prepared = frames::prepare(&folder, &frames::default_cache_root(), |p: Progress| {
        let _ = app.emit(
            "prepare-progress",
            ProgressEvent { done: p.done, total: p.total, clip: p.clip.to_string() },
        );
    })
    .await
    .map_err(|e| e.to_string())?;

    eprintln!(
        "[prepare] {}: {} clips ({} remembered, {} skipped) → {} frames in {:.1} s",
        folder.display(),
        prepared.clip_count,
        prepared.cached_count,
        prepared.skipped.len(),
        prepared.frames.len(),
        started.elapsed().as_secs_f32()
    );
    for s in &prepared.skipped {
        eprintln!("[prepare]   skipped (ffmpeg couldn't read it): {}", s.display());
    }

    let summary = PrepareSummary {
        folder: path,
        folder_name: folder
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| folder.display().to_string()),
        clip_count: prepared.clip_count,
        remembered_count: prepared.cached_count,
        frame_count: prepared.frames.len(),
        skipped_count: prepared.skipped.len(),
    };
    *slot = Some(prepared);
    Ok(summary)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    clip_path: String,
    clip_name: String,
    start_sec: u32,
    end_sec: u32,
    best_sec: u32,
    score: f32,
    /// The best frame's original LOG preview, as a data URL.
    preview: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResponse {
    above: Vec<Row>,
    /// The best near-misses, at most MAX_BELOW_ROWS.
    below: Vec<Row>,
    below_total: usize,
    cutoff: f32,
    frames_scored: usize,
    frames_failed: usize,
    elapsed_ms: u64,
}

fn to_row(m: Moment) -> Row {
    let preview = std::fs::read(&m.log_jpg)
        .map(|bytes| {
            format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
        })
        .unwrap_or_default();
    Row {
        clip_path: m.clip_path.display().to_string(),
        clip_name: m.clip_name,
        start_sec: m.start_sec,
        end_sec: m.end_sec,
        best_sec: m.best_sec,
        score: m.score,
        preview,
    }
}

/// Asks Jev about every prepared frame and returns ranked stretches with LOG previews.
#[tauri::command]
async fn search(state: State<'_, AppState>, phrase: String) -> Result<SearchResponse, String> {
    let phrase = phrase.trim().to_string();
    if phrase.is_empty() {
        return Err("Type what you're looking for.".into());
    }
    let jev = {
        let mut slot = state.jev.lock().await;
        if slot.is_none() {
            *slot = Some(Arc::new(Jev::from_env().map_err(|e| e.to_string())?));
        }
        slot.clone().unwrap()
    };
    let prepared = state.prepared.lock().await;
    let prepared = prepared.as_ref().ok_or("Choose a folder first.")?;

    let result = match search::search(&jev, &prepared.frames, &phrase, Source::Boosted).await {
        Ok(r) => r,
        Err(e) => {
            // A rejected key: drop the client so the next search re-reads .env.
            if matches!(e, jev::JevError::BadKey) {
                *state.jev.lock().await = None;
            }
            return Err(e.to_string());
        }
    };

    let below_total = result.below.len();
    let started = std::time::Instant::now();
    let response = SearchResponse {
        above: result.above.into_iter().map(to_row).collect(),
        below: result.below.into_iter().take(MAX_BELOW_ROWS).map(to_row).collect(),
        below_total,
        cutoff: search::CUTOFF,
        frames_scored: result.frames_scored,
        frames_failed: result.frames_failed,
        elapsed_ms: result.elapsed_ms as u64,
    };
    eprintln!("[search]   previews attached in {} ms", started.elapsed().as_millis());
    Ok(response)
}

/// Opens Finder with the clip selected. Only clips in the prepared folder are allowed.
#[tauri::command]
async fn reveal_in_finder(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let clip = PathBuf::from(&path);
    let known = state
        .prepared
        .lock()
        .await
        .as_ref()
        .is_some_and(|p| p.frames.iter().any(|f| f.clip_path == clip));
    if !known {
        return Err("That clip isn't in the prepared folder.".into());
    }
    let status = std::process::Command::new("open")
        .arg("-R")
        .arg(&clip)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("Finder couldn't show {}", clip.display()));
    }
    Ok(())
}

/// Deletes every remembered frame. The current folder must be prepared again afterwards.
#[tauri::command]
async fn forget_prepared_frames(state: State<'_, AppState>) -> Result<(), String> {
    *state.prepared.lock().await = None;
    frames::forget_all(&frames::default_cache_root()).map_err(|e| e.to_string())?;
    eprintln!("[cache] forgot all prepared frames");
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        // Video preview: serves clips of the prepared folder only, in pieces.
        .register_asynchronous_uri_scheme_protocol("clip", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let path = clip_protocol::path_from_uri_path(request.uri().path());
            let range = request
                .headers()
                .get(tauri::http::header::RANGE)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            tauri::async_runtime::spawn(async move {
                let allowed = app
                    .state::<AppState>()
                    .prepared
                    .lock()
                    .await
                    .as_ref()
                    .is_some_and(|p| p.frames.iter().any(|f| f.clip_path == path));
                let response = tauri::async_runtime::spawn_blocking(move || {
                    clip_protocol::respond(&path, allowed, range.as_deref())
                })
                .await
                .unwrap();
                responder.respond(response);
            });
        })
        .invoke_handler(tauri::generate_handler![
            prepare_folder,
            search,
            reveal_in_finder,
            forget_prepared_frames
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
