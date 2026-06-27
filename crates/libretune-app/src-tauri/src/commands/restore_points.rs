//! Restore point management commands.

use crate::state::AppState;
use libretune_core::tune::TuneCache;
use tauri::Emitter;

/// Info about a restore point
#[derive(Debug, Clone, serde::Serialize)]
pub struct RestorePointResponse {
    pub filename: String,
    pub path: String,
    pub created: String,
    pub size_bytes: u64,
}

/// Create a restore point from the current tune
#[tauri::command]
pub async fn create_restore_point(
    state: tauri::State<'_, AppState>,
) -> Result<RestorePointResponse, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let restore_path = project
        .create_restore_point()
        .map_err(|e| format!("Failed to create restore point: {}", e))?;

    // Auto-prune if max_restore_points is set
    let max_points = project.config.settings.max_restore_points;
    if max_points > 0 {
        let _ = project.prune_restore_points(max_points as usize);
    }

    let metadata = std::fs::metadata(&restore_path)
        .map_err(|e| format!("Failed to read restore point metadata: {}", e))?;

    Ok(RestorePointResponse {
        filename: restore_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        path: restore_path.to_string_lossy().to_string(),
        created: chrono::Utc::now().to_rfc3339(),
        size_bytes: metadata.len(),
    })
}

/// List restore points for the current project
#[tauri::command]
pub async fn list_restore_points(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RestorePointResponse>, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let points = project
        .list_restore_points()
        .map_err(|e| format!("Failed to list restore points: {}", e))?;

    Ok(points
        .into_iter()
        .map(|p| RestorePointResponse {
            filename: p.filename,
            path: p.path.to_string_lossy().to_string(),
            created: p.created,
            size_bytes: p.size_bytes,
        })
        .collect())
}

/// Load a restore point as the current tune
#[tauri::command]
pub async fn load_restore_point(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    filename: String,
) -> Result<(), String> {
    let mut proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_mut()
        .ok_or_else(|| "No project open".to_string())?;

    project
        .load_restore_point(&filename)
        .map_err(|e| format!("Failed to load restore point: {}", e))?;

    // Reload the tune into cache
    if let Some(ref tune) = project.current_tune {
        let def_guard = state.definition.read().await;
        if let Some(ref def) = *def_guard {
            let cache = TuneCache::from_definition(def);
            let mut cache_guard = state.tune_cache.lock().await;
            *cache_guard = Some(cache);

            if let Some(cache) = cache_guard.as_mut() {
                // Load page data
                for (page_num, page_data) in &tune.pages {
                    cache.load_page(*page_num, page_data.clone());
                }

                // Apply constants
                use libretune_core::tune::TuneValue;
                for (name, tune_value) in &tune.constants {
                    if let Some(constant) = def.constants.get(name) {
                        if constant.is_pc_variable {
                            if let TuneValue::Scalar(v) = tune_value {
                                cache.local_values.insert(name.clone(), *v);
                            }
                            continue;
                        }

                        let length = constant.size_bytes() as u16;
                        if length == 0 {
                            continue;
                        }

                        let element_size = constant.data_type.size_bytes();
                        let element_count = constant.shape.element_count();
                        let mut raw_data = vec![0u8; length as usize];

                        match tune_value {
                            TuneValue::Scalar(v) => {
                                let raw_val = constant.display_to_raw(*v);
                                constant.data_type.write_to_bytes(
                                    &mut raw_data,
                                    0,
                                    raw_val,
                                    def.endianness,
                                );
                                let _ =
                                    cache.write_bytes(constant.page, constant.offset, &raw_data);
                            }
                            TuneValue::Array(arr) => {
                                for (i, val) in arr.iter().take(element_count).enumerate() {
                                    let raw_val = constant.display_to_raw(*val);
                                    let offset = i * element_size;
                                    constant.data_type.write_to_bytes(
                                        &mut raw_data,
                                        offset,
                                        raw_val,
                                        def.endianness,
                                    );
                                }
                                let _ =
                                    cache.write_bytes(constant.page, constant.offset, &raw_data);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // Notify UI
    let _ = app.emit("tune:loaded", "restore_point");

    Ok(())
}

/// Delete a restore point by filename.
#[tauri::command]
pub async fn delete_restore_point(
    state: tauri::State<'_, AppState>,
    filename: String,
) -> Result<(), String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    project
        .delete_restore_point(&filename)
        .map_err(|e| format!("Failed to delete restore point: {}", e))
}
