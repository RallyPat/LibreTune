//! Project<->ECU tune sync commands.

use crate::commands::tune_persist::{maybe_auto_save_project_tune, persist_tune_to_path};
use crate::state::AppState;
use libretune_core::tune::TuneFile;

#[tauri::command]
pub async fn mark_tune_modified(state: tauri::State<'_, AppState>) -> Result<(), String> {
    *state.tune_modified.lock().await = true;
    Ok(())
}

/// Compare the current project tune with the tune synced from ECU
/// Returns true if they differ, false if identical
#[tauri::command]
pub async fn compare_project_and_ecu_tunes(
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let tune_guard = state.current_tune.lock().await;
    let project_guard = state.current_project.lock().await;

    // Get ECU tune (synced from ECU, currently in current_tune)
    let ecu_tune = match tune_guard.as_ref() {
        Some(t) => t,
        None => return Ok(false), // No ECU tune, can't compare
    };

    // Get project tune path and load it
    let project_tune = if let Some(ref project) = *project_guard {
        let tune_path = project.current_tune_path();
        if tune_path.exists() {
            match TuneFile::load(&tune_path) {
                Ok(tune) => Some(tune),
                Err(e) => {
                    eprintln!("[WARN] Failed to load project tune for comparison: {}", e);
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // If no project tune, they're different (ECU has data, project doesn't)
    let project_tune = match project_tune {
        Some(t) => t,
        None => return Ok(true), // Different - project has no tune
    };

    // Compare page data
    // Get all unique page numbers
    let mut all_pages: Vec<u8> = project_tune
        .pages
        .keys()
        .chain(ecu_tune.pages.keys())
        .copied()
        .collect();
    all_pages.sort();
    all_pages.dedup();

    // Compare each page
    for page_num in all_pages {
        let project_page = project_tune.pages.get(&page_num);
        let ecu_page = ecu_tune.pages.get(&page_num);

        match (project_page, ecu_page) {
            (None, None) => continue,                             // Both missing, skip
            (Some(_), None) | (None, Some(_)) => return Ok(true), // One missing, different
            (Some(p), Some(e)) => {
                if p != e {
                    return Ok(true); // Pages differ
                }
            }
        }
    }

    // All pages match
    Ok(false)
}

/// Write the project tune to ECU
/// Loads the tune from the project's CurrentTune.msq and writes all pages to ECU
#[tauri::command]
pub async fn write_project_tune_to_ecu(
    _app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let project_guard = state.current_project.lock().await;
    let def_guard = state.definition.read().await;

    let project = project_guard.as_ref().ok_or("No project open")?;
    let _def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // Load project tune
    let tune_path = project.current_tune_path();
    let tune =
        TuneFile::load(&tune_path).map_err(|e| format!("Failed to load project tune: {}", e))?;

    drop(project_guard);
    drop(def_guard);

    // Write all pages to ECU
    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    // Sort pages for consistent writing
    let mut pages: Vec<(u8, &Vec<u8>)> = tune.pages.iter().map(|(k, v)| (*k, v)).collect();
    pages.sort_by_key(|(p, _)| *p);

    for (page_num, page_data) in pages {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: page_num,
            offset: 0,
            data: page_data.clone(),
        };
        conn.write_memory(params)
            .map_err(|e| format!("Failed to write page {}: {}", page_num, e))?;
    }

    // Update cache and current_tune with project tune
    {
        let mut cache_guard = state.tune_cache.lock().await;
        if let Some(cache) = cache_guard.as_mut() {
            for (page_num, page_data) in &tune.pages {
                cache.load_page(*page_num, page_data.clone());
            }
        }
    }

    let mut tune_guard = state.current_tune.lock().await;
    *tune_guard = Some(tune);

    // Update path to project tune file
    *state.current_tune_path.lock().await = Some(tune_path);

    // Mark as not modified (freshly loaded from project)
    *state.tune_modified.lock().await = false;

    Ok(())
}

/// Save the current tune to the project's tune file
#[tauri::command]
pub async fn save_tune_to_project(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let tune_path = {
        let project_guard = state.current_project.lock().await;
        let project = project_guard.as_ref().ok_or("No project open")?;
        project.current_tune_path()
    };

    persist_tune_to_path(&state, tune_path).await
}

/// Auto-save tune to project if modified (no-op when no project or not dirty).
#[tauri::command]
pub async fn auto_save_project_tune(state: tauri::State<'_, AppState>) -> Result<(), String> {
    maybe_auto_save_project_tune(&state).await;
    Ok(())
}
