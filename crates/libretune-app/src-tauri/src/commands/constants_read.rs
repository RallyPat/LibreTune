//! Constant read commands (metadata + values).

use crate::commands::constant_values::{read_constant_from_cache, read_constant_from_page_bytes};
use crate::state::AppState;
use crate::ConstantInfo;
use libretune_core::ini::{Constant, DataType, Endianness};
use libretune_core::protocol::Connection;
use libretune_core::tune::{TuneCache, TuneFile, TuneValue};

/// Read a constant from synced tune/cache data when available.
/// Returns `None` when the value is not present in local tune state.
fn try_read_from_tune_or_cache(
    name: &str,
    constant: &Constant,
    endianness: Endianness,
    tune: Option<&TuneFile>,
    cache: Option<&TuneCache>,
) -> Option<f64> {
    if let Some(tune) = tune {
        if let Some(page_data) = tune.pages.get(&constant.page) {
            if let Some(val) = read_constant_from_page_bytes(constant, page_data, endianness) {
                return Some(val);
            }
        }

        if let Some(tune_value) = tune.constants.get(name) {
            return Some(tune_value_to_f64(tune_value, constant));
        }
    }

    if let Some(cache) = cache {
        if constant.size_bytes() > 0 || constant.data_type == DataType::Bits {
            if cache
                .read_bytes(
                    constant.page,
                    constant.offset,
                    constant.size_bytes().max(1) as u16,
                )
                .is_some()
            {
                return Some(read_constant_from_cache(constant, endianness, cache));
            }
        }
    }

    None
}

fn tune_value_to_f64(tune_value: &TuneValue, constant: &Constant) -> f64 {
    match tune_value {
        TuneValue::Scalar(v) => *v,
        TuneValue::String(s) if constant.data_type == DataType::Bits => constant
            .bit_options
            .iter()
            .position(|opt| opt == s)
            .or_else(|| {
                constant
                    .bit_options
                    .iter()
                    .position(|opt| opt.eq_ignore_ascii_case(s))
            })
            .map(|i| i as f64)
            .unwrap_or(0.0),
        TuneValue::Bool(b) if constant.data_type == DataType::Bits => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        TuneValue::Array(arr) => arr.first().copied().unwrap_or(0.0),
        _ => 0.0,
    }
}

#[allow(dead_code)]
fn read_bits_from_ecu(constant: &Constant, conn: &mut Connection) -> Result<f64, String> {
    let bit_pos = constant.bit_position.unwrap_or(0);
    let bit_size = constant.bit_size.unwrap_or(1);

    let byte_offset = (bit_pos / 8) as u16;
    let bit_in_byte = bit_pos % 8;

    let bits_remaining_after_first_byte = bit_size.saturating_sub(8 - bit_in_byte);
    let bytes_needed = if bits_remaining_after_first_byte > 0 {
        1 + bits_remaining_after_first_byte.div_ceil(8)
    } else {
        1
    };

    let read_offset = constant.offset + byte_offset;
    let params = libretune_core::protocol::commands::ReadMemoryParams {
        can_id: 0,
        page: constant.page,
        offset: read_offset,
        length: bytes_needed as u16,
    };
    let bytes = conn.read_memory(params).map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Ok(0.0);
    }

    let first_byte = bytes[0];
    let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
    let mask_first = if bits_in_first_byte >= 8 {
        0xFF
    } else {
        (1u8 << bits_in_first_byte) - 1
    };
    let mut bit_val = ((first_byte >> bit_in_byte) & mask_first) as u32;

    if bits_remaining_after_first_byte > 0 && bytes.len() > 1 {
        let mut bits_collected = bits_in_first_byte;
        for byte in bytes.iter().skip(1) {
            let remaining_bits = bit_size - bits_collected;
            if remaining_bits == 0 {
                break;
            }
            let bits_from_this_byte = remaining_bits.min(8);
            let mask = if bits_from_this_byte >= 8 {
                0xFF
            } else {
                (1u8 << bits_from_this_byte) - 1
            };
            let val_from_byte = (*byte & mask) as u32;
            bit_val |= val_from_byte << bits_collected;
            bits_collected += bits_from_this_byte;
        }
    }

    Ok(bit_val as f64)
}

#[allow(dead_code)]
fn read_scalar_from_ecu(
    constant: &Constant,
    endianness: Endianness,
    conn: &mut Connection,
) -> Result<f64, String> {
    let length = constant.size_bytes() as u16;
    if length == 0 {
        return Ok(0.0);
    }

    let params = libretune_core::protocol::commands::ReadMemoryParams {
        can_id: 0,
        page: constant.page,
        offset: constant.offset,
        length,
    };

    let raw_data = conn.read_memory(params).map_err(|e| e.to_string())?;
    if let Some(raw_val) = constant
        .data_type
        .read_from_bytes(&raw_data, 0, endianness)
    {
        return Ok(constant.raw_to_display(raw_val));
    }
    Ok(0.0)
}

fn constant_to_info(constant: &Constant) -> ConstantInfo {
    let value_type = match constant.data_type {
        DataType::String => "string".to_string(),
        DataType::Bits => "bits".to_string(),
        _ => match &constant.shape {
            libretune_core::ini::Shape::Scalar => "scalar".to_string(),
            _ => "array".to_string(),
        },
    };

    ConstantInfo {
        name: constant.name.clone(),
        label: constant.label.clone(),
        units: constant.units.clone(),
        digits: constant.digits,
        min: constant.min,
        max: constant.max,
        value_type,
        bit_options: constant.bit_options.clone(),
        help: constant.help.clone(),
        visibility_condition: constant.visibility_condition.clone(),
    }
}

/// Retrieves constant metadata from the INI definition.
#[tauri::command]
pub async fn get_constant(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<ConstantInfo, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let constant = def
        .constants
        .get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;

    Ok(constant_to_info(constant))
}

/// Batch-fetch constant metadata for dialog fields (one IPC call instead of N).
#[tauri::command]
pub async fn get_constants_batch(
    state: tauri::State<'_, AppState>,
    names: Vec<String>,
) -> Result<std::collections::HashMap<String, ConstantInfo>, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let mut result = std::collections::HashMap::with_capacity(names.len());
    for name in names {
        if let Some(constant) = def.constants.get(&name) {
            result.insert(name, constant_to_info(constant));
        }
    }
    Ok(result)
}

/// Retrieves a string constant's current value.
///
/// # Arguments
/// * `name` - String constant name from INI definition
///
/// Returns: The string value
#[tauri::command]
pub async fn get_constant_string_value(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<String, String> {
    let constant = {
        let def_guard = state.definition.read().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        def.constants
            .get(&name)
            .ok_or_else(|| format!("Constant {} not found", name))?
            .clone()
    };

    if constant.data_type != DataType::String {
        return Err(format!("Constant {} is not a string type", name));
    }

    let length = constant.shape.element_count() as u16;
    if length == 0 {
        return Ok(String::new());
    }

    // Offline: read from tune constants map
    {
        let is_offline = match state.connection.try_lock() {
            Ok(guard) => guard.is_none(),
            Err(_) => true,
        };
        if is_offline {
            let tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_ref() {
                if let Some(TuneValue::String(s)) = tune.constants.get(&name) {
                    return Ok(s.clone());
                }
            }
        }
    }

    // Online ECU read — no definition lock held during I/O
    let mut conn_guard = state.connection.try_lock();
    let conn = match &mut conn_guard {
        Ok(guard) => guard.as_mut(),
        Err(_) => None,
    };

    if let Some(conn) = conn {
        let params = libretune_core::protocol::commands::ReadMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            length,
        };

        let raw_data = conn.read_memory(params).map_err(|e| e.to_string())?;
        let s = String::from_utf8_lossy(&raw_data);
        return Ok(s.trim_end_matches('\0').to_string());
    }

    Ok(String::new())
}

/// Retrieves a numeric constant's current value.
///
/// Reads from tune file (offline) or ECU memory (online). For PC variables,
/// reads from local cache. Handles bit-field extraction automatically.
///
/// # Arguments
/// * `name` - Constant name from INI definition
///
/// Returns: Current value in display units (scaled/translated)
#[tauri::command]
pub async fn get_constant_value(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<f64, String> {
    let (constant, endianness, default_value) = {
        let def_guard = state.definition.read().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let constant = def
            .constants
            .get(&name)
            .ok_or_else(|| format!("Constant {} not found", name))?
            .clone();
        let default_value = def.default_values.get(&name).copied();
        (constant, def.endianness, default_value)
    };

    // PC variables are stored locally, not on ECU
    if constant.is_pc_variable {
        let cache_guard = state.tune_cache.lock().await;
        if let Some(cache) = cache_guard.as_ref() {
            if let Some(&val) = cache.local_values.get(&name) {
                return Ok(val);
            }
        }
        return Ok(default_value.unwrap_or(constant.min));
    }

    // Prefer synced tune/cache (offline and connected) — short-lived locks only
    {
        let cache_guard = state.tune_cache.lock().await;
        let tune_guard = state.current_tune.lock().await;
        if let Some(val) = try_read_from_tune_or_cache(
            &name,
            &constant,
            endianness,
            tune_guard.as_ref(),
            cache_guard.as_ref(),
        ) {
            return Ok(val);
        }
    }

    let mut conn_guard = state.connection.try_lock();
    let conn = match &mut conn_guard {
        Ok(guard) => guard.as_mut(),
        Err(_) => None,
    };

    if conn.is_none() {
        return Ok(default_value.unwrap_or(constant.min));
    }

    // Dialog/UI reads must not block on per-field ECU serial I/O — that queues hundreds
    // of slow reads when a settings page opens and freezes the app. Use tune/cache only;
    // users refresh values via ECU sync.
    let _ = conn;
    Ok(default_value.unwrap_or(constant.min))
}
