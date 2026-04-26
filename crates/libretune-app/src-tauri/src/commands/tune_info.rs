//! Tune info & new tune commands.

use crate::state::AppState;
use libretune_core::tune::TuneFile;
use serde::Serialize;

#[derive(Serialize)]
pub struct TuneInfo {
    pub path: Option<String>,
    pub signature: String,
    pub modified: bool,
    pub has_tune: bool,
}

/// Gets information about the currently loaded tune.
///
/// Returns: TuneInfo with path, signature, and modification status
#[tauri::command]
pub async fn get_tune_info(state: tauri::State<'_, AppState>) -> Result<TuneInfo, String> {
    let tune_guard = state.current_tune.lock().await;
    let path_guard = state.current_tune_path.lock().await;
    let modified = *state.tune_modified.lock().await;

    match &*tune_guard {
        Some(tune) => Ok(TuneInfo {
            path: path_guard.as_ref().map(|p| p.to_string_lossy().to_string()),
            signature: tune.signature.clone(),
            modified,
            has_tune: true,
        }),
        None => Ok(TuneInfo {
            path: None,
            signature: String::new(),
            modified: false,
            has_tune: false,
        }),
    }
}

#[tauri::command]
pub async fn new_tune(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let def_guard = state.definition.lock().await;
    let signature = def_guard
        .as_ref()
        .map(|d| d.signature.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let tune = TuneFile::new(&signature);

    *state.current_tune.lock().await = Some(tune);
    *state.current_tune_path.lock().await = None;
    *state.tune_modified.lock().await = false;

    Ok(())
}
