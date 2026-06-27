//! Misc AutoTune commands (stop, recommendations, heatmap, send/burn, lock/unlock, autosend).

use crate::AppState;
use serde::Serialize;
use tauri::Manager;

/// Stops AutoTune data collection.
///
/// Clears the AutoTune config and stops processing realtime data.
/// Recommendations remain available until explicitly cleared.
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn stop_autotune(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.autotune_state.lock().await;
    guard.stop();

    let mut secondary_guard = state.autotune_secondary_state.lock().await;
    secondary_guard.stop();

    // Clear the config
    *state.autotune_config.lock().await = None;
    Ok(())
}

#[derive(Serialize)]
pub struct AutoTuneHeatEntry {
    pub cell_x: usize,
    pub cell_y: usize,
    pub hit_weighting: f64,
    pub change_magnitude: f64,
    pub beginning_value: f64,
    pub recommended_value: f64,
    pub hit_count: u32,
}

/// Retrieves current AutoTune recommendations.
///
/// Returns all accumulated VE correction recommendations with their
/// confidence weights (hit counts).
///
/// Returns: Vector of recommendations per cell
#[tauri::command]
pub async fn get_autotune_recommendations(
    state: tauri::State<'_, AppState>,
    table_name: Option<String>,
) -> Result<Vec<libretune_core::autotune::AutoTuneRecommendation>, String> {
    let secondary_name = state
        .autotune_config
        .lock()
        .await
        .as_ref()
        .and_then(|config| config.secondary_table_name.clone());

    let use_secondary = matches!(
        (table_name.as_deref(), secondary_name.as_deref()),
        (Some(table), Some(secondary)) if table == secondary
    );

    if use_secondary {
        let guard = state.autotune_secondary_state.lock().await;
        Ok(guard.get_recommendations())
    } else {
        let guard = state.autotune_state.lock().await;
        Ok(guard.get_recommendations())
    }
}

/// Retrieves AutoTune heatmap data for visualization.
///
/// Returns per-cell data for rendering coverage and change heatmaps.
///
/// Returns: Vector of heatmap entries with weighting and change magnitude
#[tauri::command]
pub async fn get_autotune_heatmap(
    state: tauri::State<'_, AppState>,
    table_name: Option<String>,
) -> Result<Vec<AutoTuneHeatEntry>, String> {
    let secondary_name = state
        .autotune_config
        .lock()
        .await
        .as_ref()
        .and_then(|config| config.secondary_table_name.clone());

    let recs = if matches!(
        (table_name.as_deref(), secondary_name.as_deref()),
        (Some(table), Some(secondary)) if table == secondary
    ) {
        let guard = state.autotune_secondary_state.lock().await;
        guard.get_recommendations()
    } else {
        let guard = state.autotune_state.lock().await;
        guard.get_recommendations()
    };

    let mut entries: Vec<AutoTuneHeatEntry> = Vec::new();
    for r in recs.iter() {
        let change = (r.recommended_value - r.beginning_value).abs();
        entries.push(AutoTuneHeatEntry {
            cell_x: r.cell_x,
            cell_y: r.cell_y,
            hit_weighting: r.hit_weighting,
            change_magnitude: change,
            beginning_value: r.beginning_value,
            recommended_value: r.recommended_value,
            hit_count: r.hit_count,
        });
    }

    Ok(entries)
}

/// Applies AutoTune recommendations to the VE table.
///
/// Writes the recommended VE corrections to the target table,
/// updating both tune cache and ECU memory.
///
/// # Arguments
/// * `table_name` - Target VE table name
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn send_autotune_recommendations(
    state: tauri::State<'_, AppState>,
    table_name: String,
) -> Result<(), String> {
    // Collect recommendations
    let secondary_name = state
        .autotune_config
        .lock()
        .await
        .as_ref()
        .and_then(|config| config.secondary_table_name.clone());

    let recs = if matches!(
        (Some(table_name.as_str()), secondary_name.as_deref()),
        (Some(table), Some(secondary)) if table == secondary
    ) {
        let guard = state.autotune_secondary_state.lock().await;
        guard.get_recommendations()
    } else {
        let guard = state.autotune_state.lock().await;
        guard.get_recommendations()
    };
    if recs.is_empty() {
        return Err("No recommendations to send".to_string());
    }

    // Ensure connection and definition exist
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    // Find target table
    let table = def
        .get_table_by_name_or_map(&table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    // Read current table map values
    let constant = def
        .constants
        .get(&table.map)
        .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?;

    let element_count = constant.shape.element_count();
    let element_size = constant.data_type.size_bytes();
    let length = constant.size_bytes() as u16;

    if length == 0 {
        return Err("Table has zero length".to_string());
    }

    let params = libretune_core::protocol::commands::ReadMemoryParams {
        can_id: 0,
        page: constant.page,
        offset: constant.offset,
        length,
    };

    let raw_data = conn.read_memory(params).map_err(|e| e.to_string())?;

    // Convert to display values
    let mut values: Vec<f64> = Vec::with_capacity(element_count);
    for i in 0..element_count {
        let offset = i * element_size;
        if let Some(raw_val) = constant
            .data_type
            .read_from_bytes(&raw_data, offset, def.endianness)
        {
            values.push(constant.raw_to_display(raw_val));
        } else {
            values.push(0.0);
        }
    }

    // Determine table dimensions
    let x_size = table.x_size;
    let y_size = table.y_size;

    // Apply recommendations
    for r in recs.iter() {
        if r.cell_x >= x_size || r.cell_y >= y_size {
            eprintln!(
                "[WARN] send_autotune_recommendations: recommendation out of bounds: {}x{}",
                r.cell_x, r.cell_y
            );
            continue;
        }
        let idx = r.cell_y * x_size + r.cell_x;
        values[idx] = r.recommended_value;
    }

    // Convert back to raw bytes
    let mut raw_out = vec![0u8; constant.size_bytes()];
    for (i, val) in values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant
            .data_type
            .write_to_bytes(&mut raw_out, offset, raw_val, def.endianness);
    }

    // Write back to ECU
    let write_params = libretune_core::protocol::commands::WriteMemoryParams {
        can_id: 0,
        page: constant.page,
        offset: constant.offset,
        data: raw_out,
    };

    conn.write_memory(write_params).map_err(|e| e.to_string())?;

    Ok(())
}

/// Burns the AutoTune recommendations to ECU flash memory.
///
/// Permanently saves the current table values (including any AutoTune
/// changes) to non-volatile ECU memory.
///
/// # Arguments
/// * `table_name` - Target table to burn
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn burn_autotune_recommendations(
    state: tauri::State<'_, AppState>,
    table_name: String,
) -> Result<(), String> {
    // Ensure connection and definition exist
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    // Find target table constant page
    let table = def
        .get_table_by_name_or_map(&table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    let constant = def
        .constants
        .get(&table.map)
        .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?;

    let params = libretune_core::protocol::commands::BurnParams {
        can_id: 0,
        page: constant.page,
    };

    conn.burn(params).map_err(|e| e.to_string())?;

    Ok(())
}

/// Locks specific cells from AutoTune updates.
///
/// Prevents AutoTune from modifying the specified cells during
/// data collection and recommendation generation.
///
/// # Arguments
/// * `cells` - Vector of (x, y) cell coordinates to lock
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn lock_autotune_cells(
    state: tauri::State<'_, AppState>,
    cells: Vec<(usize, usize)>,
    table_name: Option<String>,
) -> Result<(), String> {
    let secondary_name = state
        .autotune_config
        .lock()
        .await
        .as_ref()
        .and_then(|config| config.secondary_table_name.clone());

    let use_secondary = matches!(
        (table_name.as_deref(), secondary_name.as_deref()),
        (Some(table), Some(secondary)) if table == secondary
    );

    if use_secondary {
        let mut guard = state.autotune_secondary_state.lock().await;
        guard.lock_cells(cells);
    } else {
        let mut guard = state.autotune_state.lock().await;
        guard.lock_cells(cells);
    }
    Ok(())
}

/// Starts automatic periodic sending of AutoTune recommendations.
///
/// Spawns a background task that applies AutoTune recommendations
/// at the specified interval.
///
/// # Arguments
/// * `table_name` - Target VE table name
/// * `interval_ms` - Send interval in milliseconds (default: 15000)
///
/// Returns: Nothing on success
#[allow(dead_code)]
#[tauri::command]
pub async fn start_autotune_autosend(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    table_name: String,
    interval_ms: Option<u64>,
) -> Result<(), String> {
    let interval = interval_ms.unwrap_or(15000);

    // Ensure connection and definition exist
    {
        let conn_guard = state.connection.lock().await;
        let def_guard = state.definition.read().await;
        if conn_guard.is_none() || def_guard.is_none() {
            return Err("Connection or definition missing".to_string());
        }
    }

    let mut task_guard = state.autotune_send_task.lock().await;
    if task_guard.is_some() {
        // Already running
        return Ok(());
    }

    let app_handle = app.clone();
    let table = table_name.clone();

    let handle = tokio::spawn(async move {
        let app_state = app_handle.state::<AppState>();
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(interval));
        loop {
            ticker.tick().await;

            // Run send_autotune_recommendations logic
            let secondary_name = app_state
                .autotune_config
                .lock()
                .await
                .as_ref()
                .and_then(|config| config.secondary_table_name.clone());

            let recs = if matches!(
                (Some(table.as_str()), secondary_name.as_deref()),
                (Some(table_name), Some(secondary)) if table_name == secondary
            ) {
                let guard = app_state.autotune_secondary_state.lock().await;
                guard.get_recommendations()
            } else {
                let guard = app_state.autotune_state.lock().await;
                guard.get_recommendations()
            };

            if recs.is_empty() {
                continue;
            }

            // Acquire definition snapshot first, then connection. Do not hold both locks
            // simultaneously to avoid deadlocks with other code paths.
            let def = {
                let def_guard = app_state.definition.read().await;
                match def_guard.as_ref() {
                    Some(d) => d.clone(),
                    None => continue,
                }
            };

            let mut conn_guard = app_state.connection.lock().await;
            let conn = match conn_guard.as_mut() {
                Some(c) => c,
                None => continue,
            };

            // Find table constant
            let table_def = match def.get_table_by_name_or_map(&table) {
                Some(t) => t.clone(),
                None => continue,
            };

            let constant = match def.constants.get(&table_def.map) {
                Some(cnst) => cnst.clone(),
                None => continue,
            };

            // Read current data
            let params = libretune_core::protocol::commands::ReadMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: constant.offset,
                length: constant.size_bytes() as u16,
            };
            let raw_data = match conn.read_memory(params) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let element_count = constant.shape.element_count();
            let element_size = constant.data_type.size_bytes();
            let mut values: Vec<f64> = Vec::with_capacity(element_count);
            for i in 0..element_count {
                let off = i * element_size;
                if let Some(rv) = constant
                    .data_type
                    .read_from_bytes(&raw_data, off, def.endianness)
                {
                    values.push(constant.raw_to_display(rv));
                } else {
                    values.push(0.0);
                }
            }

            let x_size = table_def.x_size;
            let y_size = table_def.y_size;

            // Apply recommendations
            for r in recs.iter() {
                if r.cell_x >= x_size || r.cell_y >= y_size {
                    continue;
                }
                let idx = r.cell_y * x_size + r.cell_x;
                values[idx] = r.recommended_value;
            }

            // Convert back to bytes
            let mut raw_out = vec![0u8; constant.size_bytes()];
            for (i, v) in values.iter().enumerate() {
                let rv = constant.display_to_raw(*v);
                let offset = i * element_size;
                constant
                    .data_type
                    .write_to_bytes(&mut raw_out, offset, rv, def.endianness);
            }

            let write_params = libretune_core::protocol::commands::WriteMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: constant.offset,
                data: raw_out,
            };
            let _ = conn.write_memory(write_params);
        }
    });

    *task_guard = Some(handle);

    Ok(())
}

/// Stops the AutoTune autosend background task.
///
/// Aborts the periodic recommendation sending task.
///
/// Returns: Nothing on success
#[allow(dead_code)]
#[tauri::command]
pub async fn stop_autotune_autosend(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut task_guard = state.autotune_send_task.lock().await;
    if let Some(h) = task_guard.take() {
        h.abort();
    }
    Ok(())
}

/// Unlocks previously locked AutoTune cells.
///
/// # Arguments
/// * `cells` - Vector of (x, y) cell coordinates to unlock
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn unlock_autotune_cells(
    state: tauri::State<'_, AppState>,
    cells: Vec<(usize, usize)>,
    table_name: Option<String>,
) -> Result<(), String> {
    let secondary_name = state
        .autotune_config
        .lock()
        .await
        .as_ref()
        .and_then(|config| config.secondary_table_name.clone());

    let use_secondary = matches!(
        (table_name.as_deref(), secondary_name.as_deref()),
        (Some(table), Some(secondary)) if table == secondary
    );

    if use_secondary {
        let mut guard = state.autotune_secondary_state.lock().await;
        guard.unlock_cells(cells);
    } else {
        let mut guard = state.autotune_state.lock().await;
        guard.unlock_cells(cells);
    }
    Ok(())
}
