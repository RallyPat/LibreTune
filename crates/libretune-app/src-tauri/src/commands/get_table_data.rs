//! get_table_data command (extracted from lib.rs).

use crate::{infer_z_output_channel, set_conn_lock_holder, AppState, TableData};
use crate::commands::constant_values::read_constant_from_cache_or_tune;
use crate::commands::util_helpers::clean_axis_label;
use libretune_core::ini::{Constant, EcuDefinition, Endianness};
use libretune_core::protocol::Connection;
use libretune_core::tune::{TuneCache, TuneFile};

/// Pre-cloned data for resolving a `{bitStringValue(list, indexVar)}` axis label.
/// Avoids re-acquiring the definition read lock while cache/tune locks are held.
type AxisPreload = Option<(Vec<String>, Constant, Endianness)>;

/// Extract the bit_options and index constant needed to resolve a bitStringValue label.
/// Returns None for plain string labels (which resolve directly without any lock).
fn preload_axis_label(label: &str, def: &EcuDefinition) -> AxisPreload {
    let trimmed = label.trim();
    let inner = trimmed.strip_prefix('{')?.strip_suffix('}')?;
    let rest = inner.trim().strip_prefix("bitStringValue(")?;
    let rest = rest.strip_suffix(')')?;
    let comma = rest.find(',')?;
    let list_name = rest[..comma].trim();
    let index_var = rest[comma + 1..].trim();
    let list_const = def.constants.get(list_name)?;
    let index_const = def.constants.get(index_var)?;
    Some((list_const.bit_options.clone(), index_const.clone(), def.endianness))
}

/// Resolve axis label using pre-cloned data (no def lock needed).
fn resolve_axis_label_preloaded(
    label: &str,
    preload: &AxisPreload,
    tune: Option<&TuneFile>,
    cache: Option<&TuneCache>,
) -> String {
    if let Some((list_options, index_const, endianness)) = preload {
        let index = read_constant_from_cache_or_tune(
            &index_const.name,
            index_const,
            *endianness,
            tune,
            cache,
        ) as usize;
        if let Some(opt) = list_options.get(index) {
            let opt = opt.trim();
            if !opt.is_empty() && !opt.eq_ignore_ascii_case("INVALID") {
                return opt.to_string();
            }
        }
    }
    clean_axis_label(label)
}

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

    // Pre-extract axis label resolution data while we hold the definition lock.
    // This lets us resolve axis names later (after acquiring cache/tune) without
    // re-acquiring the definition lock — preventing a potential ABBA deadlock
    // (get_table_data holds tune_cache+current_tune → tries definition.read;
    //  close_project holds definition.write → tries current_tune.lock).
    let x_axis_preload = preload_axis_label(&x_label, def);
    let y_axis_preload = preload_axis_label(&y_label, def);

    drop(def_guard);

    // Helper to read constant data from TuneCache (primary), TuneFile (fallback), or ECU (online).
    fn read_const_from_source(
        constant: &Constant,
        tune: Option<&TuneFile>,
        cache: Option<&TuneCache>,
        conn: &mut Option<&mut Connection>,
        endianness: libretune_core::ini::Endianness,
    ) -> Result<Vec<f64>, String> {
        let element_count = constant.shape.element_count();
        let element_size = constant.data_type.size_bytes();
        let length = constant.size_bytes() as u16;

        if length == 0 {
            return Ok(vec![0.0; element_count]);
        }

        // Decode raw bytes at a base offset into display values.
        let decode = |data: &[u8], base: usize| -> Vec<f64> {
            let mut values = Vec::with_capacity(element_count);
            for i in 0..element_count {
                let elem_off = base + i * element_size;
                if let Some(raw_val) = constant.data_type.read_from_bytes(data, elem_off, endianness) {
                    values.push(constant.raw_to_display(raw_val));
                } else {
                    values.push(0.0);
                }
            }
            values
        };

        if conn.is_none() {
            // 1. TuneCache — most authoritative; populated by open_project and includes edits.
            if let Some(cache) = cache {
                if let Some(raw) = cache.read_bytes(constant.page, constant.offset, length) {
                    eprintln!("[DEBUG] read_const_from_source: '{}' from TuneCache (page={}, offset={})",
                        constant.name, constant.page, constant.offset);
                    return Ok(decode(raw, 0));
                }
            }

            // 2. TuneFile.constants — parsed TuneValue entries from MSQ <constant> tags.
            if let Some(tune_file) = tune {
                if let Some(tune_value) = tune_file.constants.get(&constant.name) {
                    use libretune_core::tune::TuneValue;
                    match tune_value {
                        TuneValue::Array(arr) => {
                            eprintln!("[DEBUG] read_const_from_source: '{}' from TuneFile.constants (array)", constant.name);
                            return Ok(arr.clone());
                        }
                        TuneValue::Scalar(v) => {
                            eprintln!("[DEBUG] read_const_from_source: '{}' from TuneFile.constants (scalar={})", constant.name, v);
                            return Ok(vec![*v]);
                        }
                        _ => {
                            eprintln!("[DEBUG] read_const_from_source: '{}' in TuneFile.constants but unsupported type, trying pages", constant.name);
                        }
                    }
                }

                // 3. TuneFile.pages — raw binary page data from MSQ <pageData> tags.
                if let Some(page_data) = tune_file.pages.get(&constant.page) {
                    let offset = constant.offset as usize;
                    let total_bytes = element_count * element_size;
                    if offset + total_bytes <= page_data.len() {
                        eprintln!("[DEBUG] read_const_from_source: '{}' from TuneFile.pages[{}] at offset {}",
                            constant.name, constant.page, offset);
                        return Ok(decode(page_data, offset));
                    }
                    eprintln!("[WARN] read_const_from_source: '{}' offset {} + {} exceeds page {} length {}",
                        constant.name, offset, total_bytes, constant.page, page_data.len());
                } else {
                    eprintln!("[DEBUG] read_const_from_source: page {} not found in TuneFile for '{}'", constant.page, constant.name);
                }
            }

            eprintln!("[DEBUG] read_const_from_source: '{}' not found anywhere, returning zeros", constant.name);
            return Ok(vec![0.0; element_count]);
        }

        // --- Connected path ---
        // Prefer TuneCache (synced from ECU), then TuneFile pages, then live read.

        if let Some(cache) = cache {
            if let Some(raw) = cache.read_bytes(constant.page, constant.offset, length) {
                eprintln!("[DEBUG] read_const_from_source: '{}' from TuneCache (connected hit)", constant.name);
                return Ok(decode(raw, 0));
            }
        }

        if let Some(tune_file) = tune {
            if let Some(page_data) = tune_file.pages.get(&constant.page) {
                let byte_offset = constant.offset as usize;
                let total_bytes = element_count * element_size;
                if byte_offset + total_bytes <= page_data.len() {
                    eprintln!("[DEBUG] read_const_from_source: '{}' from TuneFile.pages (connected fallback)", constant.name);
                    return Ok(decode(page_data, byte_offset));
                }
            }
        }

        // Cache miss — fall back to a live ECU read.
        if let Some(ref mut conn_ptr) = conn {
            eprintln!("[DEBUG] read_const_from_source: reading '{}' from ECU (cache miss)", constant.name);
            let params = libretune_core::protocol::commands::ReadMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: constant.offset,
                length,
            };
            let raw_data = conn_ptr.read_memory(params).map_err(|e| e.to_string())?;
            return Ok(decode(&raw_data, 0));
        }

        eprintln!("[DEBUG] read_const_from_source: '{}' not found, returning zeros", constant.name);
        Ok(vec![0.0; element_count])
    }

    // Acquire in tune_cache → current_tune order for consistent lock ordering.
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;
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

    // Resolve axis names using pre-cloned data — no definition lock needed here,
    // which prevents the ABBA deadlock with close_project.
    let x_axis_name = resolve_axis_label_preloaded(&x_label, &x_axis_preload, tune_guard.as_ref(), cache_guard.as_ref());
    let y_axis_name = resolve_axis_label_preloaded(&y_label, &y_axis_preload, tune_guard.as_ref(), cache_guard.as_ref());

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
