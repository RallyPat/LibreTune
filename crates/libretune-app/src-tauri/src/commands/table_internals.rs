//! TableData struct and internal table helpers (extracted from lib.rs).

use crate::{clean_axis_label, AppState};
use libretune_core::dynamic_table::{self, TableSizeInfo};
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
    /// Present when the INI declares TunerStudio dynamically sized arrays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_info: Option<TableSizeInfoDto>,
}

#[derive(Serialize, Clone)]
pub(crate) struct TableSizeInfoDto {
    pub resizable: bool,
    pub cols_const: String,
    pub rows_const: String,
    pub min_cols: usize,
    pub max_cols: usize,
    pub min_rows: usize,
    pub max_rows: usize,
    pub max_elements: usize,
    pub active_cols: usize,
    pub active_rows: usize,
}

impl From<&TableSizeInfo> for TableSizeInfoDto {
    fn from(info: &TableSizeInfo) -> Self {
        Self {
            resizable: true,
            cols_const: info.cols_const.clone(),
            rows_const: info.rows_const.clone(),
            min_cols: info.min_cols,
            max_cols: info.max_cols,
            min_rows: info.min_rows,
            max_rows: info.max_rows,
            max_elements: info.max_elements,
            active_cols: info.active_cols,
            active_rows: info.active_rows,
        }
    }
}

// Tune health/anomaly/predicted_fills/dyno_overlay extracted to commands/tune_health.rs
/// Helper function to get table data internally (avoids code duplication)
pub(crate) async fn get_table_data_internal(
    state: &tauri::State<'_, AppState>,
    table_name: &str,
) -> Result<TableData, String> {
    let def_guard = state.definition.lock().await;
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

    // Snapshot enough to resolve active size without re-locking definition later.
    let size_snapshot = dynamic_table::table_size_info(def, table, &|_| None).map(|info| {
        let cols_c = def.constants.get(&info.cols_const).cloned();
        let rows_c = def.constants.get(&info.rows_const).cloned();
        let defaults = def.default_values.clone();
        let max_elements = info.max_elements;
        (info, cols_c, rows_c, defaults, max_elements)
    });

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

    let x_bins_full = read_const_values(&x_const, tune_guard.as_ref(), endianness);
    let y_bins_full = if let Some(ref y) = y_const {
        read_const_values(y, tune_guard.as_ref(), endianness)
    } else {
        vec![0.0]
    };
    let z_flat = read_const_values(&z_const, tune_guard.as_ref(), endianness);

    let size_info = size_snapshot.map(|(mut info, cols_c, rows_c, defaults, max_elements)| {
        if let Some(ref c) = cols_c {
            if let Some(v) = read_scalar_from_tune(c, tune_guard.as_ref(), endianness) {
                info.active_cols = (v.round() as usize).clamp(info.min_cols, info.max_cols);
            } else if let Some(v) = defaults.get(&info.cols_const) {
                info.active_cols = (v.round() as usize).clamp(info.min_cols, info.max_cols);
            }
        }
        if let Some(ref c) = rows_c {
            if let Some(v) = read_scalar_from_tune(c, tune_guard.as_ref(), endianness) {
                info.active_rows = (v.round() as usize).clamp(info.min_rows, info.max_rows);
            } else if let Some(v) = defaults.get(&info.rows_const) {
                info.active_rows = (v.round() as usize).clamp(info.min_rows, info.max_rows);
            }
        }
        info.max_elements = max_elements;
        while info.active_cols * info.active_rows > info.max_elements
            && info.active_cols > info.min_cols
        {
            info.active_cols -= 1;
        }
        while info.active_cols * info.active_rows > info.max_elements
            && info.active_rows > info.min_rows
        {
            info.active_rows -= 1;
        }
        info
    });

    drop(tune_guard);

    let (x_bins, y_bins, z_values, size_dto) = if let Some(ref info) = size_info {
        let x_bins = dynamic_table::slice_bins(&x_bins_full, info.active_cols);
        let y_bins = if is_3d {
            dynamic_table::slice_bins(&y_bins_full, info.active_rows)
        } else {
            y_bins_full
        };
        let z_values = dynamic_table::unpack_z(
            &z_flat,
            info.active_cols,
            if is_3d { info.active_rows } else { 1 },
        );
        (x_bins, y_bins, z_values, Some(TableSizeInfoDto::from(info)))
    } else {
        let x_size = x_bins_full.len();
        let y_size = if is_3d { y_bins_full.len() } else { 1 };
        let mut z_values = Vec::with_capacity(y_size);
        for y in 0..y_size {
            let mut row = Vec::with_capacity(x_size);
            for x in 0..x_size {
                let idx = y * x_size + x;
                row.push(*z_flat.get(idx).unwrap_or(&0.0));
            }
            z_values.push(row);
        }
        (x_bins_full, y_bins_full, z_values, None)
    };

    let size_info = if let Some(mut dto) = size_dto {
        // Hide Set Size when a connected ECU fully mismatches the INI.
        if crate::commands::signature_helpers::connected_signature_is_mismatch(state).await {
            dto.resizable = false;
        }
        Some(dto)
    } else {
        None
    };

    Ok(TableData {
        name: table_name_out,
        title: table_title,
        x_bins,
        y_bins,
        z_values,
        x_axis_name: clean_axis_label(&x_label),
        y_axis_name: clean_axis_label(&y_label),
        x_output_channel,
        y_output_channel,
        size_info,
    })
}

fn read_scalar_from_tune(
    constant: &Constant,
    tune: Option<&TuneFile>,
    endianness: libretune_core::ini::Endianness,
) -> Option<f64> {
    let tune_file = tune?;
    if let Some(tune_value) = tune_file.constants.get(&constant.name) {
        match tune_value {
            TuneValue::Scalar(v) => return Some(*v),
            TuneValue::Array(arr) if !arr.is_empty() => return Some(arr[0]),
            _ => {}
        }
    }
    let page_data = tune_file.pages.get(&constant.page)?;
    let offset = constant.offset as usize;
    let raw = constant
        .data_type
        .read_from_bytes(page_data, offset, endianness)?;
    Some(constant.raw_to_display(raw))
}

/// Helper function to update table z_values internally
pub(crate) async fn update_table_z_values_internal(
    state: &tauri::State<'_, AppState>,
    table_name: &str,
    z_values: Vec<Vec<f64>>,
) -> Result<(), String> {
    // Snapshot only what we need from the definition, then drop the lock
    // before doing any ECU I/O below — holding it across a blocking
    // conn.write_memory() call starves every other command that needs the
    // definition (e.g. load_tune, table/constant reads).
    let (constant, endianness, default_page_bytes) = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let table = def
            .get_table_by_name_or_map(table_name)
            .ok_or_else(|| format!("Table {} not found", table_name))?;
        let constant = def
            .constants
            .get(&table.map)
            .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?
            .clone();
        let default_page_bytes = def
            .page_sizes
            .get(constant.page as usize)
            .copied()
            .unwrap_or(256) as usize;
        (constant, def.endianness, default_page_bytes)
    };

    let mut conn_guard = state.connection.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;

    let allocated = dynamic_table::allocated_elements(&constant);
    let active_rows = z_values.len();
    let active_cols = z_values.first().map(|r| r.len()).unwrap_or(0);
    let active_len = active_rows.saturating_mul(active_cols);

    // Fixed tables: require full footprint. Dynamic tables: pack active region.
    let flat_values = if constant.dynamic_size.is_some() {
        if active_len == 0 || active_len > allocated {
            return Err(format!(
                "Invalid dynamic table size: {}x{} (budget {})",
                active_rows, active_cols, allocated
            ));
        }
        let mut allocated_flat = vec![0.0; allocated];
        if let Some(cache) = cache_guard.as_ref() {
            if let Some(page) = cache.get_page(constant.page) {
                let element_size = constant.data_type.size_bytes();
                let start = constant.offset as usize;
                for (i, slot) in allocated_flat.iter_mut().enumerate() {
                    let off = start + i * element_size;
                    if let Some(raw) = constant.data_type.read_from_bytes(page, off, endianness) {
                        *slot = constant.raw_to_display(raw);
                    }
                }
            }
        }
        dynamic_table::pack_z_into(&mut allocated_flat, &z_values);
        allocated_flat
    } else {
        let flat: Vec<f64> = z_values.into_iter().flatten().collect();
        if flat.len() != allocated {
            return Err(format!(
                "Invalid data size: expected {}, got {}",
                allocated,
                flat.len()
            ));
        }
        flat
    };

    // Convert display values to raw bytes
    let element_size = constant.data_type.size_bytes();
    let mut raw_data = vec![0u8; constant.size_bytes()];

    for (i, val) in flat_values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant
            .data_type
            .write_to_bytes(&mut raw_data, offset, raw_val, endianness);
    }

    // Write to TuneCache if available
    if let Some(cache) = cache_guard.as_mut() {
        if cache.write_bytes(constant.page, constant.offset, &raw_data) {
            // Also update TuneFile in memory
            let mut tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_mut() {
                let page_data = tune
                    .pages
                    .entry(constant.page)
                    .or_insert_with(|| vec![0u8; default_page_bytes]);
                let start = constant.offset as usize;
                let end = start + raw_data.len();
                if end <= page_data.len() {
                    page_data[start..end].copy_from_slice(&raw_data);
                }
            }
            *state.tune_modified.lock().await = true;
        }
    }

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

    Ok(())
}

/// Helper function to update a constant array (used for table axis bins)
pub(crate) async fn update_constant_array_internal(
    state: &tauri::State<'_, AppState>,
    constant_name: &str,
    values: Vec<f64>,
) -> Result<(), String> {
    // Snapshot only what we need from the definition, then drop the lock
    // before doing any ECU I/O below — holding it across a blocking
    // conn.write_memory() call starves every other command that needs the
    // definition (e.g. load_tune, table/constant reads).
    let (constant, endianness, default_page_bytes) = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let constant = def
            .constants
            .get(constant_name)
            .ok_or_else(|| format!("Constant {} not found", constant_name))?
            .clone();
        let default_page_bytes = def
            .page_sizes
            .get(constant.page as usize)
            .copied()
            .unwrap_or(256) as usize;
        (constant, def.endianness, default_page_bytes)
    };

    let mut conn_guard = state.connection.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;

    let allocated = dynamic_table::allocated_elements(&constant);
    let values = if constant.dynamic_size.is_some() {
        if values.is_empty() || values.len() > allocated {
            return Err(format!(
                "Invalid dynamic axis size for {}: got {}, allocated {}",
                constant_name,
                values.len(),
                allocated
            ));
        }
        let mut full = vec![0.0; allocated];
        if let Some(cache) = cache_guard.as_ref() {
            if let Some(page) = cache.get_page(constant.page) {
                let element_size = constant.data_type.size_bytes();
                let start = constant.offset as usize;
                for (i, slot) in full.iter_mut().enumerate() {
                    let off = start + i * element_size;
                    if let Some(raw) = constant.data_type.read_from_bytes(page, off, endianness) {
                        *slot = constant.raw_to_display(raw);
                    }
                }
            }
        }
        full[..values.len()].copy_from_slice(&values);
        // Extend unused bins with the last active value (stable for firmware clamps).
        if let Some(&last) = values.last() {
            for slot in full.iter_mut().skip(values.len()) {
                *slot = last;
            }
        }
        full
    } else if values.len() != allocated {
        return Err(format!(
            "Invalid data size for {}: expected {}, got {}",
            constant_name,
            allocated,
            values.len()
        ));
    } else {
        values
    };

    let element_size = constant.data_type.size_bytes();
    let mut raw_data = vec![0u8; constant.size_bytes()];

    for (i, val) in values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant
            .data_type
            .write_to_bytes(&mut raw_data, offset, raw_val, endianness);
    }

    if let Some(cache) = cache_guard.as_mut() {
        if cache.write_bytes(constant.page, constant.offset, &raw_data) {
            let mut tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_mut() {
                let page_data = tune
                    .pages
                    .entry(constant.page)
                    .or_insert_with(|| vec![0u8; default_page_bytes]);

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
