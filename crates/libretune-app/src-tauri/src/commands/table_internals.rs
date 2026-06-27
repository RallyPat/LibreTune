//! TableData struct and internal table helpers (extracted from lib.rs).

use crate::{infer_z_output_channel, resolve_table_axis_label, AppState};
use libretune_core::ini::Constant;
use libretune_core::tune::{TuneFile, TuneValue};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct TableData {
    pub name: String,
    pub title: String,
    pub x_bins: Vec<f64>,
    pub y_bins: Vec<f64>,
    pub z_values: Vec<Vec<f64>>,
    pub x_axis_name: String,
    pub y_axis_name: String,
    /// Output channel name for X-axis (used for live cell highlighting)
    pub x_output_channel: Option<String>,
    /// Output channel name for Y-axis (used for live cell highlighting)
    pub y_output_channel: Option<String>,
    /// Output channel name for Z/table result (live output value readout)
    pub z_output_channel: Option<String>,
}

// Tune health/anomaly/predicted_fills/dyno_overlay extracted to commands/tune_health.rs
/// Helper function to get table data internally (avoids code duplication)
pub(crate) async fn get_table_data_internal(
    state: &tauri::State<'_, AppState>,
    table_name: &str,
) -> Result<TableData, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let endianness = def.endianness;

    let table = def
        .get_table_by_name_or_map(table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    let x_bins_name = table.x_bins.clone();
    let y_bins_name = table.y_bins.clone();
    let map_name = table.map.clone();
    let is_3d = table.is_3d();
    let table_name_out = table.name.clone();
    let table_title = table.title.clone();
    let x_label = table
        .x_label
        .clone()
        .unwrap_or_else(|| table.x_bins.clone());
    let y_label = table
        .y_label
        .clone()
        .unwrap_or_else(|| table.y_bins.clone().unwrap_or_default());
    let x_output_channel = table.x_output_channel.clone();
    let y_output_channel = table.y_output_channel.clone();

    let x_const = def
        .constants
        .get(&x_bins_name)
        .ok_or_else(|| format!("Constant {} not found", x_bins_name))?
        .clone();
    let y_const = y_bins_name
        .as_ref()
        .and_then(|name| def.constants.get(name).cloned());
    let z_const = def
        .constants
        .get(&map_name)
        .ok_or_else(|| format!("Constant {} not found", map_name))?
        .clone();

    drop(def_guard);

    // Read from tune file (offline mode)
    let tune_guard = state.current_tune.lock().await;

    fn read_const_values(
        constant: &Constant,
        tune: Option<&TuneFile>,
        endianness: libretune_core::ini::Endianness,
    ) -> Vec<f64> {
        let element_count = constant.shape.element_count();
        let element_size = constant.data_type.size_bytes();
        if let Some(tune_file) = tune {
            if let Some(tune_value) = tune_file.constants.get(&constant.name) {
                match tune_value {
                    TuneValue::Array(arr) => return arr.clone(),
                    TuneValue::Scalar(v) => return vec![*v],
                    _ => {}
                }
            }

            if let Some(page_data) = tune_file.pages.get(&constant.page) {
                let offset = constant.offset as usize;
                let total_bytes = element_count * element_size;
                if offset + total_bytes <= page_data.len() {
                    let mut values = Vec::with_capacity(element_count);
                    for i in 0..element_count {
                        let elem_offset = offset + i * element_size;
                        if let Some(raw_val) =
                            constant
                                .data_type
                                .read_from_bytes(page_data, elem_offset, endianness)
                        {
                            values.push(constant.raw_to_display(raw_val));
                        } else {
                            values.push(0.0);
                        }
                    }
                    return values;
                }
            }
        }
        vec![0.0; element_count]
    }

    let x_bins = read_const_values(&x_const, tune_guard.as_ref(), endianness);
    let y_bins = if let Some(ref y) = y_const {
        read_const_values(y, tune_guard.as_ref(), endianness)
    } else {
        vec![0.0]
    };
    let z_flat = read_const_values(&z_const, tune_guard.as_ref(), endianness);

    let (x_axis_name, y_axis_name) = {
        let def_guard = state.definition.read().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        (
            resolve_table_axis_label(&x_label, def, tune_guard.as_ref(), None),
            resolve_table_axis_label(&y_label, def, tune_guard.as_ref(), None),
        )
    };

    drop(tune_guard);

    // Reshape Z values into 2D array [y][x]
    let x_size = x_bins.len();
    let y_size = if is_3d { y_bins.len() } else { 1 };

    let mut z_values = Vec::with_capacity(y_size);
    for y in 0..y_size {
        let mut row = Vec::with_capacity(x_size);
        for x in 0..x_size {
            let idx = y * x_size + x;
            row.push(*z_flat.get(idx).unwrap_or(&0.0));
        }
        z_values.push(row);
    }

    let z_output_channel = infer_z_output_channel(&x_output_channel);

    Ok(TableData {
        name: table_name_out,
        title: table_title,
        x_bins,
        y_bins,
        z_values,
        x_axis_name,
        y_axis_name,
        x_output_channel,
        y_output_channel,
        z_output_channel,
    })
}

/// Helper function to update table z_values internally
pub(crate) async fn update_table_z_values_internal(
    state: &tauri::State<'_, AppState>,
    table_name: &str,
    z_values: Vec<Vec<f64>>,
) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.read().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    crate::commands::tune_persist::ensure_tune_cache(state, def).await;

    let mut cache_guard = state.tune_cache.lock().await;

    let table = def
        .get_table_by_name_or_map(table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    let constant = def
        .constants
        .get(&table.map)
        .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?;

    // Flatten z_values
    let flat_values: Vec<f64> = z_values.into_iter().flatten().collect();

    if flat_values.len() != constant.shape.element_count() {
        return Err(format!(
            "Invalid data size: expected {}, got {}",
            constant.shape.element_count(),
            flat_values.len()
        ));
    }

    // Convert display values to raw bytes
    let element_size = constant.data_type.size_bytes();
    let mut raw_data = vec![0u8; constant.size_bytes()];

    for (i, val) in flat_values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant
            .data_type
            .write_to_bytes(&mut raw_data, offset, raw_val, def.endianness);
    }

    if let Some(cache) = cache_guard.as_mut() {
        cache.write_bytes(constant.page, constant.offset, &raw_data);
    }

    {
        let mut tune_guard = state.current_tune.lock().await;
        if let Some(tune) = tune_guard.as_mut() {
            crate::commands::tune_persist::write_bytes_to_tune_pages(
                tune,
                def,
                constant.page,
                constant.offset,
                &raw_data,
            );
        }
    }
    *state.tune_modified.lock().await = true;

    // Write to ECU if connected (optional)
    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data,
        };
        if let Err(e) = conn.write_memory(params) {
            eprintln!("[WARN] Failed to write to ECU: {}", e);
        }
    }

    drop(conn_guard);
    drop(cache_guard);
    drop(def_guard);

    crate::commands::tune_persist::maybe_auto_save_project_tune(state).await;

    Ok(())
}

/// Helper function to update a constant array (used for table axis bins)
pub(crate) async fn update_constant_array_internal(
    state: &tauri::State<'_, AppState>,
    constant_name: &str,
    values: Vec<f64>,
) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.read().await;
    let mut cache_guard = state.tune_cache.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let constant = def
        .constants
        .get(constant_name)
        .ok_or_else(|| format!("Constant {} not found", constant_name))?;

    if values.len() != constant.shape.element_count() {
        return Err(format!(
            "Invalid data size for {}: expected {}, got {}",
            constant_name,
            constant.shape.element_count(),
            values.len()
        ));
    }

    let element_size = constant.data_type.size_bytes();
    let mut raw_data = vec![0u8; constant.size_bytes()];

    for (i, val) in values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant
            .data_type
            .write_to_bytes(&mut raw_data, offset, raw_val, def.endianness);
    }

    if let Some(cache) = cache_guard.as_mut() {
        if cache.write_bytes(constant.page, constant.offset, &raw_data) {
            let mut tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_mut() {
                let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
                    vec![
                        0u8;
                        def.page_sizes
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
            }

            *state.tune_modified.lock().await = true;
        }
    }

    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data.clone(),
        };
        if let Err(e) = conn.write_memory(params) {
            eprintln!(
                "[WARN] Failed to write axis bins '{}' to ECU: {}",
                constant_name, e
            );
        }
    }

    Ok(())
}
