//! Miscellaneous tune commands: string constant updates, tune source switching.

use crate::commands::tune_apply::materialize_project_pages;
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
    let def_guard = state.definition.lock().await;
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

/// Use LibreTune / project settings: merge MSQ constants onto the ECU base, save, write, burn.
///
/// Never bulk-writes zero-padded "project pages" from Load Tune — that corrupts the ECU.
#[tauri::command]
pub async fn use_project_tune(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let (tune_path, ini_signature) = {
        let project_guard = state.current_project.lock().await;
        let project = project_guard.as_ref().ok_or("No project loaded")?;
        let tune_path = project.current_tune_path();
        let ini_signature = {
            let def_guard = state.definition.lock().await;
            def_guard
                .as_ref()
                .map(|d| d.signature.clone())
                .unwrap_or_else(|| project.config.signature.clone())
        };
        (tune_path, ini_signature)
    };

    let mut project_msq = if tune_path.exists() {
        TuneFile::load(&tune_path).map_err(|e| format!("Failed to load project tune: {}", e))?
    } else {
        return Err("Project tune file not found".to_string());
    };
    project_msq.signature = ini_signature.clone();

    // Safe pages = ECU base (from mismatch snapshot) + MSQ constants / full pageData.
    // Fall back to in-memory ECU tune pages if snapshot is gone.
    let ecu_base = {
        let snapshot_guard = state.tune_mismatch_snapshot.lock().await;
        if let Some(snapshot) = snapshot_guard.as_ref() {
            snapshot.ecu_pages.clone()
        } else {
            let tune_guard = state.current_tune.lock().await;
            tune_guard
                .as_ref()
                .map(|t| t.pages.clone())
                .unwrap_or_default()
        }
    };

    if ecu_base.is_empty() {
        return Err(
            "No ECU page data available to merge. Connect and sync first, then choose LibreTune settings."
                .to_string(),
        );
    }

    let merged_pages = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        materialize_project_pages(def, &project_msq, &ecu_base)
    };

    project_msq.pages = merged_pages;

    {
        let mut cache_guard = state.tune_cache.lock().await;
        if let Some(cache) = cache_guard.as_mut() {
            for (page_num, page_data) in &project_msq.pages {
                cache.load_page(*page_num, page_data.clone());
            }
        }
    }

    project_msq
        .save(&tune_path)
        .map_err(|e| format!("Failed to save project tune: {}", e))?;

    *state.current_tune.lock().await = Some(project_msq);
    *state.current_tune_path.lock().await = Some(tune_path);
    *state.tune_modified.lock().await = false;
    *state.tune_mismatch_snapshot.lock().await = None;

    let _ = app.emit("tune:loaded", "project");

    if state.connection.lock().await.is_some() {
        let write_result = crate::commands::project_tune_sync::write_project_tune_to_ecu(
            app.clone(),
            state.clone(),
        )
        .await;
        if let Err(e) = write_result {
            let _ = crate::commands::realtime_stream::start_realtime_stream(
                app.clone(),
                state.clone(),
                Some(50),
            )
            .await;
            return Err(format!(
                "Saved CurrentTune.msq, but failed to write to ECU: {}",
                e
            ));
        }

        let burn_result = crate::commands::tune_io::burn_to_ecu(app.clone(), state.clone()).await;
        {
            let mut conn_guard = state.connection.lock().await;
            if let Some(conn) = conn_guard.as_mut() {
                conn.clear_rx_buffer();
            }
        }
        let _ = crate::commands::realtime_stream::start_realtime_stream(
            app.clone(),
            state.clone(),
            Some(50),
        )
        .await;

        burn_result.map_err(|e| {
            format!(
                "Saved CurrentTune.msq and wrote RAM, but burn failed: {}",
                e
            )
        })
    } else {
        Ok(())
    }
}

/// Use ECU settings: overwrite CurrentTune.msq on disk with the ECU tune.
#[tauri::command]
pub async fn use_ecu_tune(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    {
        let snapshot_guard = state.tune_mismatch_snapshot.lock().await;
        if let Some(snapshot) = snapshot_guard.as_ref() {
            let ini_signature = {
                let def_guard = state.definition.lock().await;
                def_guard
                    .as_ref()
                    .map(|d| d.signature.clone())
                    .unwrap_or_default()
            };
            let mut cache_guard = state.tune_cache.lock().await;
            if let Some(cache) = cache_guard.as_mut() {
                for (page_num, page_data) in &snapshot.ecu_pages {
                    cache.load_page(*page_num, page_data.clone());
                }
            }
            drop(cache_guard);

            let mut tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_mut() {
                tune.pages = snapshot.ecu_pages.clone();
                if !ini_signature.is_empty() {
                    tune.signature = ini_signature;
                }
            } else {
                let mut tune = TuneFile::new(ini_signature);
                tune.pages = snapshot.ecu_pages.clone();
                *tune_guard = Some(tune);
            }
        }
    }

    *state.tune_modified.lock().await = false;
    *state.tune_mismatch_snapshot.lock().await = None;

    let has_project = state.current_project.lock().await.is_some();
    if has_project {
        crate::commands::project_tune_sync::save_tune_to_project(state.clone()).await?;
        let _ = app.emit("tune:loaded", "ecu");
    }

    Ok(())
}
