//! get_table_data command (extracted from lib.rs).

use crate::{infer_z_output_channel, resolve_table_axis_label, set_conn_lock_holder, AppState, TableData};
use libretune_core::ini::Constant;
use libretune_core::protocol::Connection;
use libretune_core::tune::{TuneCache, TuneFile};

#[tauri::command]
pub async fn get_table_data(
    state: tauri::State<'_, AppState>,
    table_name: String,
) -> Result<TableData, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let endianness = def.endianness;

    let table = def
        .get_table_by_name_or_map(&table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    // Clone the table info we need
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

    // Collect constant info we need
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

    // Helper to read constant data from TuneFile (offline) or ECU (online)
    fn read_const_from_source(
        constant: &Constant,
        tune: Option<&TuneFile>,
        _cache: Option<&TuneCache>,
        conn: &mut Option<&mut Connection>,
        endianness: libretune_core::ini::Endianness,
    ) -> Result<Vec<f64>, String> {
        let element_count = constant.shape.element_count();
        let element_size = constant.data_type.size_bytes();
        let length = constant.size_bytes() as u16;

        if length == 0 {
            return Ok(vec![0.0; element_count]);
        }

        // If offline, read from TuneFile constants first, then fall back to raw page data
        if conn.is_none() {
            if let Some(tune_file) = tune {
                if let Some(tune_value) = tune_file.constants.get(&constant.name) {
                    use libretune_core::tune::TuneValue;
                    match tune_value {
                        TuneValue::Array(arr) => {
                            eprintln!("[DEBUG] read_const_from_source: CACHE HIT for '{}' (page={}, offset={}, len={}, offline mode)", 
                                constant.name, constant.page, constant.offset, length);
                            return Ok(arr.clone());
                        }
                        TuneValue::Scalar(v) => {
                            eprintln!("[DEBUG] read_const_from_source: Found '{}' in TuneFile as Scalar({}), returning as single-element array", 
                                constant.name, v);
                            return Ok(vec![*v]);
                        }
                        _ => {
                            eprintln!("[DEBUG] read_const_from_source: Found '{}' in TuneFile but wrong type, falling through", constant.name);
                        }
                    }
                }

                if let Some(page_data) = tune_file.pages.get(&constant.page) {
                    let offset = constant.offset as usize;
                    let total_bytes = element_count * element_size;
                    if offset + total_bytes <= page_data.len() {
                        eprintln!("[DEBUG] read_const_from_source: '{}' reading from TuneFile.pages[{}] at offset {}", 
                            constant.name, constant.page, offset);
                        let mut values = Vec::with_capacity(element_count);
                        for i in 0..element_count {
                            let elem_offset = offset + i * element_size;
                            if let Some(raw_val) = constant.data_type.read_from_bytes(
                                page_data,
                                elem_offset,
                                endianness,
                            ) {
                                values.push(constant.raw_to_display(raw_val));
                            } else {
                                values.push(0.0);
                            }
                        }
                        return Ok(values);
                    }
                    eprintln!("[WARN] read_const_from_source: '{}' offset {} + size {} exceeds page {} length {}", 
                        constant.name, offset, total_bytes, constant.page, page_data.len());
                } else {
                    eprintln!("[DEBUG] read_const_from_source: Page {} not found in TuneFile.pages for '{}'", constant.page, constant.name);
                }
                eprintln!("[DEBUG] read_const_from_source: Constant '{}' not found in TuneFile, returning zeros", constant.name);
                return Ok(vec![0.0; element_count]);
            }
            eprintln!("[DEBUG] read_const_from_source: No TuneFile loaded, returning zeros");
            return Ok(vec![0.0; element_count]);
        }

        // When connected, prefer TuneFile.pages (populated by sync_ecu_data) over a live
        // ECU read. Static table data does not change unless the user edits it, so the synced
        // cache is authoritative. Only fall back to a live ECU read if the page was never
        // synced (e.g. user opened a table before syncing).
        if let Some(tune_file) = tune {
            if let Some(page_data) = tune_file.pages.get(&constant.page) {
                let byte_offset = constant.offset as usize;
                let total_bytes = element_count * element_size;
                if byte_offset + total_bytes <= page_data.len() {
                    eprintln!(
                        "[DEBUG] read_const_from_source: '{}' from TuneFile cache (connected hit)",
                        constant.name
                    );
                    let mut values = Vec::with_capacity(element_count);
                    for i in 0..element_count {
                        let elem_offset = byte_offset + i * element_size;
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
                    return Ok(values);
                }
            }
        }

        // Cache miss – fall back to a live ECU read (e.g. not yet synced)
        if let Some(ref mut conn_ptr) = conn {
            eprintln!(
                "[DEBUG] read_const_from_source: reading '{}' from ECU (cache miss, online)",
                constant.name
            );
            let params = libretune_core::protocol::commands::ReadMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: constant.offset,
                length,
            };

            let raw_data = conn_ptr.read_memory(params).map_err(|e| e.to_string())?;

            let mut values = Vec::new();
            for i in 0..element_count {
                let offset = i * element_size;
                if let Some(raw_val) = constant
                    .data_type
                    .read_from_bytes(&raw_data, offset, endianness)
                {
                    values.push(constant.raw_to_display(raw_val));
                } else {
                    values.push(0.0);
                }
            }
            return Ok(values);
        }

        // If offline and not in TuneFile, return zeros (should always be in TuneFile)
        eprintln!(
            "[DEBUG] read_const_from_source: Constant '{}' not found in TuneFile, returning zeros",
            constant.name
        );
        Ok(vec![0.0; element_count])
    }

    // Get tune and cache; use try_lock on connection so realtime stream isn't blocked
    let tune_guard = state.current_tune.lock().await;
    let cache_guard = state.tune_cache.lock().await;
    set_conn_lock_holder("get_table_data");
    let mut conn_guard_result = state.connection.try_lock();
    let mut conn_slot: Option<&mut Connection> = match &mut conn_guard_result {
        Ok(guard) => guard.as_mut(),
        Err(_) => None,
    };

    let x_bins = read_const_from_source(
        &x_const,
        tune_guard.as_ref(),
        cache_guard.as_ref(),
        &mut conn_slot,
        endianness,
    )?;
    let y_bins = if let Some(ref y) = y_const {
        read_const_from_source(
            y,
            tune_guard.as_ref(),
            cache_guard.as_ref(),
            &mut conn_slot,
            endianness,
        )?
    } else {
        vec![0.0]
    };
    let z_flat = read_const_from_source(
        &z_const,
        tune_guard.as_ref(),
        cache_guard.as_ref(),
        &mut conn_slot,
        endianness,
    )?;

    set_conn_lock_holder("(none)");
    drop(conn_guard_result);

    let (x_axis_name, y_axis_name) = {
        let def_guard = state.definition.read().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        (
            resolve_table_axis_label(&x_label, def, tune_guard.as_ref(), cache_guard.as_ref()),
            resolve_table_axis_label(&y_label, def, tune_guard.as_ref(), cache_guard.as_ref()),
        )
    };

    drop(cache_guard);

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
