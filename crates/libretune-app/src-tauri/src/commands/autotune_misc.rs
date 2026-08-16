//! Misc AutoTune commands (stop, recommendations, heatmap, send/burn, lock/unlock, autosend).

use crate::AppState;
use serde::Serialize;
use tauri::Manager;

/// Refuses to apply/burn recommendations computed against a different ECU
/// definition than the one currently loaded (e.g. reconnected to a
/// different ECU/INI without stopping AutoTune first). Without this check a
/// same-named table in the new definition would silently receive
/// corrections computed against the old one's layout/units.
///
/// `session_signature` is `None` if no AutoTune session was ever started
/// (already handled elsewhere by "no recommendations to send") or, in
/// practice, always `Some` once a session exists — `None` is treated as
/// "nothing to check against" rather than a failure.
fn check_definition_matches(
    session_signature: &Option<String>,
    current_signature: &str,
    action: &str,
) -> Result<(), String> {
    if let Some(started_against) = session_signature {
        if started_against != current_signature {
            return Err(format!(
                "AutoTune session was started against a different ECU definition ('{}' vs. current '{}') — stop and restart AutoTune before {}",
                started_against, current_signature, action
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_signature_passes() {
        assert!(check_definition_matches(
            &Some("speeduino 202501".to_string()),
            "speeduino 202501",
            "applying recommendations"
        )
        .is_ok());
    }

    #[test]
    fn mismatched_signature_fails_with_both_signatures_named() {
        let err = check_definition_matches(
            &Some("speeduino 202501".to_string()),
            "speeduino 202402",
            "applying recommendations",
        )
        .unwrap_err();
        assert!(err.contains("speeduino 202501"));
        assert!(err.contains("speeduino 202402"));
    }

    #[test]
    fn no_recorded_session_signature_passes() {
        // No session signature recorded (e.g. legacy/edge-case config) —
        // nothing to compare against, so don't block the action.
        assert!(check_definition_matches(&None, "speeduino 202501", "burning").is_ok());
    }
}

/// Stops AutoTune data collection.
///
/// Clears the AutoTune config and stops processing realtime data.
/// Recommendations remain available until explicitly cleared.
///
/// Returns: Nothing on success
/// Whether an AutoTune session is live, and against which tables.
///
/// The session runs in the backend and survives the AutoTune view unmounting
/// (e.g. switching to the dashboard and back). The view calls this on mount to
/// re-attach instead of assuming no session exists.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutotuneStatus {
    pub running: bool,
    pub table_name: Option<String>,
    pub secondary_table_name: Option<String>,
}

#[tauri::command]
pub async fn get_autotune_status(
    state: tauri::State<'_, AppState>,
) -> Result<AutotuneStatus, String> {
    let config_guard = state.autotune_config.lock().await;
    Ok(match config_guard.as_ref() {
        Some(config) => AutotuneStatus {
            running: true,
            table_name: Some(config.table_name.clone()),
            secondary_table_name: config.secondary_table_name.clone(),
        },
        None => AutotuneStatus {
            running: false,
            table_name: None,
            secondary_table_name: None,
        },
    })
}

#[tauri::command]
pub async fn stop_autotune(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.autotune_state.lock().await;
    tracing::info!(
        accepted_samples = guard.total_samples(),
        cells_with_recommendations = guard.recommendations.len(),
        "stop_autotune: stopping (recommendations retained until cleared)"
    );
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
    let (secondary_name, session_signature) = {
        let config_guard = state.autotune_config.lock().await;
        let config = config_guard.as_ref();
        (
            config.and_then(|c| c.secondary_table_name.clone()),
            config.map(|c| c.definition_signature.clone()),
        )
    };

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
        tracing::info!(table = %table_name, "send_autotune_recommendations: nothing to send (no recommendations accumulated)");
        return Err("No recommendations to send".to_string());
    }
    tracing::info!(
        table = %table_name,
        recommendations = recs.len(),
        "send_autotune_recommendations: applying to ECU RAM"
    );

    // Snapshot what we need from the definition, then drop the lock before
    // doing any ECU I/O below — holding it across the read/write round-trip
    // starves every other command that needs the definition (e.g. load_tune,
    // table views) for the duration of both serial transactions.
    let mut conn_guard = state.connection.lock().await;
    let (constant, endianness, x_size, y_size, current_signature) = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let table = def
            .get_table_by_name_or_map(&table_name)
            .ok_or_else(|| format!("Table {} not found", table_name))?;
        let constant = def
            .constants
            .get(&table.map)
            .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?
            .clone();
        (
            constant,
            def.endianness,
            table.x_size,
            table.y_size,
            def.signature.clone(),
        )
    };
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    check_definition_matches(
        &session_signature,
        &current_signature,
        "applying recommendations",
    )?;

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

    // Decode the table the ECU actually returned. A cell that will not decode
    // used to become 0.0, and since the whole array is written back below, that
    // put a zero-fuel cell into a live VE table - the engine leans out wherever
    // the tune passes through it. A short read is the realistic way to get
    // here, and it silently affects every cell after the truncation point.
    //
    // There is no safe value to substitute, so refuse the apply instead.
    let mut values: Vec<f64> = Vec::with_capacity(element_count);
    for i in 0..element_count {
        let offset = i * element_size;
        let raw_val = constant
            .data_type
            .read_from_bytes(&raw_data, offset, endianness)
            .ok_or_else(|| {
                format!(
                    "Refusing to apply: cell {i} of {element_count} could not be read back from \
                     the ECU ({} bytes returned for a {length}-byte table). Applying would write \
                     a zero into a live fuel table. Re-sync the ECU and try again.",
                    raw_data.len()
                )
            })?;
        values.push(constant.raw_to_display(raw_val));
    }

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
            .write_to_bytes(&mut raw_out, offset, raw_val, endianness);
    }

    // Write back to ECU
    let write_params = libretune_core::protocol::commands::WriteMemoryParams {
        can_id: 0,
        page: constant.page,
        offset: constant.offset,
        data: raw_out,
    };

    conn.write_memory(write_params).map_err(|e| e.to_string())?;

    tracing::info!(
        table = %table_name,
        cells_written = recs.len(),
        "send_autotune_recommendations: written to ECU RAM (burn separately to persist)"
    );
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
    let session_signature = state
        .autotune_config
        .lock()
        .await
        .as_ref()
        .map(|c| c.definition_signature.clone());

    // Snapshot the target page (and current signature) and drop the
    // definition lock before the blocking burn call below — a flash burn
    // (erase + program cycle) is typically slower than a plain read/write,
    // so holding the lock across it starves other commands for longer than
    // usual.
    let mut conn_guard = state.connection.lock().await;
    let (page, current_signature) = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let table = def
            .get_table_by_name_or_map(&table_name)
            .ok_or_else(|| format!("Table {} not found", table_name))?;
        let constant = def
            .constants
            .get(&table.map)
            .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?;
        (constant.page, def.signature.clone())
    };
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    // Refuse to burn against a different ECU definition than the AutoTune
    // session started against — see send_autotune_recommendations for why.
    check_definition_matches(&session_signature, &current_signature, "burning")?;

    let params = libretune_core::protocol::commands::BurnParams { can_id: 0, page };

    tracing::info!(table = %table_name, page, "burn_autotune_recommendations: burning page to ECU flash");
    conn.burn(params).map_err(|e| e.to_string())?;

    tracing::info!(table = %table_name, page, "burn_autotune_recommendations: burn complete");
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
        let def_guard = state.definition.lock().await;
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
            let (secondary_name, session_signature) = {
                let config_guard = app_state.autotune_config.lock().await;
                let config = config_guard.as_ref();
                (
                    config.and_then(|c| c.secondary_table_name.clone()),
                    config.map(|c| c.definition_signature.clone()),
                )
            };

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
                let def_guard = app_state.definition.lock().await;
                match def_guard.as_ref() {
                    Some(d) => d.clone(),
                    None => continue,
                }
            };

            // Skip this tick if the definition changed out from under the
            // session (e.g. reconnected to a different ECU/INI) — see
            // send_autotune_recommendations for why this matters.
            if check_definition_matches(&session_signature, &def.signature, "autosending").is_err()
            {
                continue;
            }

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
