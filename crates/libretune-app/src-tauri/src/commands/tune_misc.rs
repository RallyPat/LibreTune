//! Miscellaneous tune commands: string constant updates, tune source switching.

use crate::commands::project_tune_sync::save_tune_to_project;
use crate::state::AppState;
use libretune_core::ini::DataType;
use libretune_core::tune::TuneFile;
use tauri::Emitter;

/// Update a string-type constant
#[tauri::command]
pub async fn update_constant_string(
    state: tauri::State<'_, AppState>,
    _app: tauri::AppHandle,
    name: String,
    value: String,
) -> Result<(), String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let constant = def
        .constants
        .get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;

    // Validate it's a string type
    if constant.data_type != DataType::String {
        return Err(format!("Constant {} is not a string type", name));
    }

    let max_len = constant.size_bytes();
    if max_len == 0 {
        return Err(format!("String constant {} has zero length", name));
    }

    // Encode string to bytes: fixed-length, null-padded
    let mut raw_data = vec![0u8; max_len];
    let copy_len = value.len().min(max_len);
    raw_data[..copy_len].copy_from_slice(&value.as_bytes()[..copy_len]);
    // Remaining bytes are already 0 (null padding)

    // Write to TuneCache if available
    let mut cache_guard = state.tune_cache.lock().await;
    if let Some(cache) = cache_guard.as_mut() {
        cache.write_bytes(constant.page, constant.offset, &raw_data);
    }

    // Update TuneFile in memory
    let mut tune_guard = state.current_tune.lock().await;
    if let Some(tune) = tune_guard.as_mut() {
        let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
            let def_guard_inner = &def;
            vec![
                0u8;
                def_guard_inner
                    .page_sizes
                    .get(constant.page as usize)
                    .copied()
                    .unwrap_or(256) as usize
            ]
        });
        let start = constant.offset as usize;
        let end = start + raw_data.len();
        if end <= page_data.len() {
            page_data[start..end].copy_from_slice(&raw_data);
        }
        tune.constants.insert(
            name.clone(),
            libretune_core::tune::TuneValue::String(value.clone()),
        );
    }

    // Mark tune as modified
    *state.tune_modified.lock().await = true;

    // Write to ECU if connected
    let mut conn_guard = state.connection.lock().await;
    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data,
        };
        if let Err(e) = conn.write_memory(params) {
            eprintln!("[WARN] Failed to write string constant to ECU: {}", e);
        }
    }

    eprintln!("Updated string constant '{}' to: '{}'", name, value);

    Ok(())
}

/// Use the project's saved tune file, discarding any ECU data.
///
/// Loads the tune from the project's CurrentTune.msq file and populates
/// the tune cache. Used when there's a conflict between project and ECU data.
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn use_project_tune(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let project_guard = state.current_project.lock().await;
    let project = project_guard.as_ref().ok_or("No project loaded")?;

    // Load project tune from disk
    let tune_path = project.current_tune_path();
    if tune_path.exists() {
        let tune = TuneFile::load(&tune_path)
            .map_err(|e| format!("Failed to load project tune: {}", e))?;

        // Populate TuneCache from project tune
        {
            let mut cache_guard = state.tune_cache.lock().await;
            if let Some(cache) = cache_guard.as_mut() {
                for (page_num, page_data) in &tune.pages {
                    cache.load_page(*page_num, page_data.clone());
                }
            }
        }

        // Set as current tune
        *state.current_tune.lock().await = Some(tune);
        *state.current_tune_path.lock().await = Some(tune_path);
        *state.tune_modified.lock().await = false;

        // Emit event to trigger re-sync if connected
        let _ = app.emit("tune:loaded", "project");
    } else {
        return Err("Project tune file not found".to_string());
    }

    Ok(())
}

/// Use the ECU's tune data, discarding project file changes.
///
/// Keeps the currently synced ECU data and writes it to the project tune file.
#[tauri::command]
pub async fn use_ecu_tune(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // ECU tune is already loaded from sync — persist to project and clear modified flag
    save_tune_to_project(state).await?;
    Ok(())
}
