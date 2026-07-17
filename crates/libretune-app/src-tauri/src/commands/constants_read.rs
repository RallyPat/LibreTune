//! Constant read commands (metadata + values).

use crate::state::AppState;
use crate::ConstantInfo;
use libretune_core::ini::DataType;

/// Retrieves constant metadata from the INI definition.
///
/// Gets information about a constant including its type, units, min/max,
/// bit options (for dropdown fields), and visibility conditions.
///
/// # Arguments
/// * `name` - Constant name from INI definition
///
/// Returns: ConstantInfo with metadata for UI rendering
#[tauri::command]
pub async fn get_constant(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<ConstantInfo, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let constant = def
        .constants
        .get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;

    // Determine value_type from DataType
    let value_type = match constant.data_type {
        DataType::String => "string".to_string(),
        DataType::Bits => "bits".to_string(),
        _ => {
            // Check if it's an array
            match &constant.shape {
                libretune_core::ini::Shape::Scalar => "scalar".to_string(),
                _ => "array".to_string(),
            }
        }
    };

    eprintln!(
        "[DEBUG] get_constant '{}': bit_options.len()={}, value_type={}",
        name,
        constant.bit_options.len(),
        value_type
    );
    if !constant.bit_options.is_empty() && constant.bit_options.len() <= 10 {
        eprintln!(
            "[DEBUG] get_constant '{}': bit_options={:?}",
            name, constant.bit_options
        );
    }

    Ok(ConstantInfo {
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
        is_pc_variable: constant.is_pc_variable,
    })
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
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let tune_guard = state.current_tune.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let conn = conn_guard.as_mut();

    let constant = def
        .constants
        .get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;

    // For string type, read the raw bytes and convert to UTF-8 string
    if constant.data_type != DataType::String {
        return Err(format!("Constant {} is not a string type", name));
    }

    // When offline, try reading directly from TuneFile first (simpler and more reliable)
    if conn.is_none() {
        if let Some(tune) = tune_guard.as_ref() {
            if let Some(tune_value) = tune.constants.get(&name) {
                use libretune_core::tune::TuneValue;
                if let TuneValue::String(s) = tune_value {
                    return Ok(s.clone());
                }
            }
        }
    }

    // Get string length from shape (e.g., Array1D(32) means 32 chars)
    let length = constant.shape.element_count() as u16;
    if length == 0 {
        return Ok(String::new());
    }

    // If connected to ECU, always read from ECU (live data)
    if let Some(conn) = conn {
        let params = libretune_core::protocol::commands::ReadMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            length,
        };

        let raw_data = conn.read_memory(params).map_err(|e| e.to_string())?;
        // Convert to string, stopping at first null byte
        let s = String::from_utf8_lossy(&raw_data);
        let s = s.trim_end_matches('\0').to_string();
        return Ok(s);
    }

    // If offline and not in TuneFile, return empty string (should always be in TuneFile)
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
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let conn = conn_guard.as_mut();

    let constant = def
        .constants
        .get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;

    // PC variables are stored locally, not on ECU
    if constant.is_pc_variable {
        // Check local cache first
        if let Some(cache) = cache_guard.as_ref() {
            if let Some(&val) = cache.local_values.get(&name) {
                return Ok(val);
            }
        }
        // Fall back to default value from INI
        if let Some(&default_val) = def.default_values.get(&name) {
            return Ok(default_val);
        }
        // Last resort: use min value or 0
        return Ok(constant.min);
    }

    // When offline, ALWAYS read from TuneFile (MSQ file) - no cache fallback
    if conn.is_none() {
        if let Some(tune) = tune_guard.as_ref() {
            if let Some(tune_value) = tune.constants.get(&name) {
                use libretune_core::tune::TuneValue;
                match tune_value {
                    TuneValue::Scalar(v) => {
                        // For bits constants, the value might be a string - need to look it up
                        if constant.data_type == libretune_core::ini::DataType::Bits {
                            // If it's already a number, return it (even if it maps to "INVALID" - that's what's in the MSQ)
                            let index = *v as usize;
                            if index < constant.bit_options.len() {
                                let option_str = &constant.bit_options[index];
                                eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as Scalar({}), returning as bits index (maps to '{}')", 
                                    name, v, option_str);
                            } else {
                                eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as Scalar({}), but out of range (bit_options len={}), returning anyway", 
                                    name, v, constant.bit_options.len());
                            }
                            return Ok(*v);
                        } else {
                            eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as Scalar({}), returning directly", name, v);
                            return Ok(*v);
                        }
                    }
                    TuneValue::String(s)
                        if constant.data_type == libretune_core::ini::DataType::Bits =>
                    {
                        // Look up string in bit_options
                        if let Some(index) = constant.bit_options.iter().position(|opt| opt == s) {
                            eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as String('{}'), matched at index {}", name, s, index);
                            return Ok(index as f64);
                        }
                        // Try case-insensitive
                        if let Some(index) = constant
                            .bit_options
                            .iter()
                            .position(|opt| opt.eq_ignore_ascii_case(s))
                        {
                            eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as String('{}'), case-insensitive match at index {}", name, s, index);
                            return Ok(index as f64);
                        }
                        eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as String('{}'), but not found in bit_options, returning 0", 
                            name, s);
                        return Ok(0.0);
                    }
                    TuneValue::String(_s) => {
                        // Non-bits string constants - should use get_constant_string_value
                        eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as String, but constant is not Bits type, returning 0", name);
                        return Ok(0.0);
                    }
                    TuneValue::Array(arr) => {
                        // For arrays, return first element or 0
                        if !arr.is_empty() {
                            return Ok(arr[0]);
                        }
                        return Ok(0.0);
                    }
                    TuneValue::Bool(b) => {
                        return Ok(if *b { 1.0 } else { 0.0 });
                    }
                }
            } else {
                // Constant not in TuneFile - return 0 (or default)
                eprintln!(
                    "[DEBUG] get_constant_value: Constant '{}' not found in TuneFile, returning 0",
                    name
                );
                return Ok(0.0);
            }
        } else {
            // No tune file loaded - return 0
            eprintln!("[DEBUG] get_constant_value: No TuneFile loaded, returning 0");
            return Ok(0.0);
        }
    }

    // When online, read from ECU
    // Handle bits constants specially (they're packed, size_bytes() == 0)
    if constant.data_type == libretune_core::ini::DataType::Bits {
        let bit_pos = constant.bit_position.unwrap_or(0);
        let bit_size = constant.bit_size.unwrap_or(1);

        // Calculate which byte contains the bits and the bit position within that byte
        let byte_offset = (bit_pos / 8) as u16;
        let bit_in_byte = bit_pos % 8;

        // Calculate how many bytes we need to read (may span multiple bytes)
        let bits_remaining_after_first_byte = bit_size.saturating_sub(8 - bit_in_byte);
        let bytes_needed = if bits_remaining_after_first_byte > 0 {
            // Need multiple bytes: first byte + additional bytes
            1 + bits_remaining_after_first_byte.div_ceil(8)
        } else {
            // All bits fit in one byte
            1
        };

        // Read the byte(s) containing the bits from ECU
        let read_offset = constant.offset + byte_offset;
        if let Some(conn) = conn {
            let params = libretune_core::protocol::commands::ReadMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: read_offset,
                length: bytes_needed as u16,
            };
            if let Ok(bytes) = conn.read_memory(params) {
                if bytes.is_empty() {
                    return Ok(0.0);
                }

                // Extract bits from the first byte
                let first_byte = bytes[0];
                let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
                let mask_first = if bits_in_first_byte >= 8 {
                    0xFF
                } else {
                    (1u8 << bits_in_first_byte) - 1
                };
                let mut bit_val = ((first_byte >> bit_in_byte) & mask_first) as u32;

                // If bits span multiple bytes, extract from additional bytes
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

                // Return the raw bit value (index into bit_options array)
                eprintln!("[DEBUG] get_constant_value: Read bits constant '{}' from ECU: bit_val={}, bit_options len={}", 
                    name, bit_val, constant.bit_options.len());
                return Ok(bit_val as f64);
            }
        }

        eprintln!(
            "[DEBUG] get_constant_value: Could not read bits constant '{}' from ECU, returning 0",
            name
        );
        return Ok(0.0);
    }

    let length = constant.size_bytes() as u16;
    if length == 0 {
        return Ok(0.0);
    } // Zero-size constants (shouldn't happen for non-bits)

    // If connected to ECU, always read from ECU (live data)
    if let Some(conn) = conn {
        let params = libretune_core::protocol::commands::ReadMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            length,
        };

        let raw_data = conn.read_memory(params).map_err(|e| e.to_string())?;
        if let Some(raw_val) = constant
            .data_type
            .read_from_bytes(&raw_data, 0, def.endianness)
        {
            return Ok(constant.raw_to_display(raw_val));
        }
        return Ok(0.0);
    }

    // If offline, read from cache (MSQ data)
    if let Some(cache) = cache_guard.as_ref() {
        if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset, length) {
            if let Some(raw_val) = constant
                .data_type
                .read_from_bytes(raw_data, 0, def.endianness)
            {
                return Ok(constant.raw_to_display(raw_val));
            }
        }
    }

    // No cache and not connected - return 0
    Ok(0.0)
}
