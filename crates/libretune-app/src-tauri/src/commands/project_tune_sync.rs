//! Project<->ECU tune sync commands.

use crate::state::AppState;
use libretune_core::tune::TuneFile;
use tokio::time::{sleep, Duration};

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

/// Pause realtime streaming so bulk page writes don't race the OCH poller.
/// The stream treats write responses as bad realtime frames and disconnects after 3 errors.
async fn pause_realtime_stream(state: &tauri::State<'_, AppState>) {
    let mut task_guard = state.streaming_task.lock().await;
    if let Some(handle) = task_guard.take() {
        handle.abort();
    }
    drop(task_guard);
    // Let the aborted task release the connection lock.
    sleep(Duration::from_millis(80)).await;
}

/// Write the project tune to ECU
/// Loads the tune from the project's CurrentTune.msq and writes all pages to ECU
#[tauri::command]
pub async fn write_project_tune_to_ecu(
    _app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let project_guard = state.current_project.lock().await;
    let def_guard = state.definition.lock().await;

    let project = project_guard.as_ref().ok_or("No project open")?;
    let _def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // Load project tune
    let tune_path = project.current_tune_path();
    let tune =
        TuneFile::load(&tune_path).map_err(|e| format!("Failed to load project tune: {}", e))?;

    drop(project_guard);
    drop(def_guard);

    pause_realtime_stream(&state).await;

    let mut pages: Vec<(u8, Vec<u8>)> = tune.pages.iter().map(|(k, v)| (*k, v.clone())).collect();
    pages.sort_by_key(|(p, _)| *p);

    {
        let mut conn_guard = state.connection.lock().await;
        let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

        // Bulk write must not auto-burn between pages (2s sleep per page change freezes the link).
        // Caller burns once after all pages are in RAM.
        conn.set_auto_burn_on_page_change(false);
        conn.clear_rx_buffer();

        let write_result = (|| {
            for (page_num, page_data) in &pages {
                conn.write_page(*page_num, page_data)
                    .map_err(|e| format!("Failed to write page {}: {}", page_num, e))?;
            }
            Ok::<(), String>(())
        })();

        conn.clear_rx_buffer();
        conn.set_auto_burn_on_page_change(true);
        write_result?;
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
    let project_guard = state.current_project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    let tune_path = project.current_tune_path();
    drop(project_guard);

    let ini_signature = {
        let def_guard = state.definition.lock().await;
        def_guard.as_ref().map(|d| d.signature.clone())
    };

    // Sync cache pages into the in-memory tune before writing disk.
    let mut tune = {
        let tune_guard = state.current_tune.lock().await;
        tune_guard.as_ref().ok_or("No tune loaded")?.clone()
    };
    {
        let cache_guard = state.tune_cache.lock().await;
        if let Some(cache) = cache_guard.as_ref() {
            for page_num in 0..cache.page_count() {
                if let Some(page_data) = cache.get_page(page_num) {
                    tune.pages.insert(page_num, page_data.to_vec());
                }
            }
        }
    }
    if let Some(sig) = ini_signature {
        tune.signature = sig;
    }

    tune.save(&tune_path)
        .map_err(|e| format!("Failed to save tune to project: {}", e))?;

    *state.current_tune.lock().await = Some(tune);
    *state.current_tune_path.lock().await = Some(tune_path);
    *state.tune_modified.lock().await = false;

    Ok(())
}
