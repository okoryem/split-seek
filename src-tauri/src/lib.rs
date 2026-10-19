pub mod frames;
pub mod jev;
pub mod scan;
pub mod search;

use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use frames::{Prepared, Progress};

/// What the engine keeps while the app is open: the folder currently prepared.
#[derive(Default)]
struct AppState {
    prepared: Mutex<Option<Prepared>>,
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
        .invoke_handler(tauri::generate_handler![prepare_folder, forget_prepared_frames])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
