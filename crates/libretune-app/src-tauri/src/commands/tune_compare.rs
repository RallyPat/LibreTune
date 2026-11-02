//! Cross-file tune comparison and merge Tauri commands.

use libretune_core::tune::{TuneDiff, TuneFile};

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

    // Lock order: definition, then tune_cache, then current_tune — matching the
    // convention used everywhere else these are held together (avoids an AB-BA
    // deadlock against e.g. get_table_data/get_all_constant_values). Acquired
    // upfront even though cache/def are only used conditionally below.
    let def_guard = state.definition.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;
    let mut tune_guard = state.current_tune.lock().await;
    let tune = tune_guard.as_mut().ok_or("No tune loaded")?;

    let merged = TuneDiff::merge_selected(tune, &source, &constant_names);

    if merged > 0 {
        if let (Some(cache), Some(def)) = (cache_guard.as_mut(), def_guard.as_ref()) {
            // Encode via the shared helper (width, endianness, bits, arrays, strings).
            let mut selected = TuneFile::new("");
            selected.constants = constant_names
                .iter()
                .filter_map(|n| Some((n.clone(), source.constants.get(n)?.clone())))
                .collect();
            super::tune_apply::apply_tune_constants_to_cache(
                cache,
                def,
                &selected,
                &Default::default(),
            );
        }
    }

    Ok(merged)
}
