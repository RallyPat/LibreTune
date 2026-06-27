//! Cross-file tune comparison and merge Tauri commands.

use libretune_core::tune::{TuneDiff, TuneFile, TuneValue};

use crate::state::AppState;

/// Compare two tune files (from disk) and return detailed diff
#[tauri::command]
pub async fn compare_tune_files(path_a: String, path_b: String) -> Result<TuneDiff, String> {
    let tune_a = TuneFile::load(&path_a).map_err(|e| format!("Failed to load tune A: {}", e))?;
    let tune_b = TuneFile::load(&path_b).map_err(|e| format!("Failed to load tune B: {}", e))?;

    Ok(TuneDiff::compare(&tune_a, &tune_b))
}

/// Merge selected constants from another tune file into the current tune
#[tauri::command]
pub async fn merge_from_tune(
    state: tauri::State<'_, AppState>,
    source_path: String,
    constant_names: Vec<String>,
) -> Result<usize, String> {
    let source =
        TuneFile::load(&source_path).map_err(|e| format!("Failed to load source tune: {}", e))?;

    let mut tune_guard = state.current_tune.lock().await;
    let tune = tune_guard.as_mut().ok_or("No tune loaded")?;

    let merged = TuneDiff::merge_selected(tune, &source, &constant_names);

    if merged > 0 {
        let mut cache_guard = state.tune_cache.lock().await;
        let def_guard = state.definition.read().await;
        if let (Some(cache), Some(def)) = (cache_guard.as_mut(), def_guard.as_ref()) {
            for name in &constant_names {
                if let Some(value) = source.constants.get(name) {
                    if let Some(constant) = def.constants.get(name) {
                        if let TuneValue::Scalar(v) = value {
                            let raw_val = ((*v - constant.translate) / constant.scale) as i64;
                            let page = constant.page;
                            let offset = constant.offset;
                            cache.write_bytes(page, offset, &[(raw_val & 0xFF) as u8]);
                        }
                    }
                }
            }
        }
    }

    Ok(merged)
}
