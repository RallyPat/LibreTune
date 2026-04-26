//! Constant value reading commands and helpers.

use crate::AppState;
use libretune_core::ini::DataType;
use std::collections::HashMap;

fn bit_mask_u8(bits: u8) -> u8 {
    if bits >= 8 {
        0xFF
    } else {
        (1u8 << bits) - 1
    }
}

#[tauri::command]
pub async fn get_all_constant_values(
    state: tauri::State<'_, AppState>,
) -> Result<HashMap<String, f64>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // NO connection lock! Read from cache/tune only.
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;

    let mut values = HashMap::new();
    for (name, constant) in &def.constants {
        // Skip array constants (only need scalars for visibility conditions)
        if !matches!(constant.shape, libretune_core::ini::Shape::Scalar) {
            continue;
        }

        let value = read_constant_from_cache_or_tune(
            name,
            constant,
            def.endianness,
            tune_guard.as_ref(),
            cache_guard.as_ref(),
        );

        values.insert(name.clone(), value);
    }

    Ok(values)
}

/// Read a single constant value from tune file or cache (no ECU connection needed).
/// Priority: TuneFile → TuneCache → default 0.0
pub(crate) fn read_constant_from_cache_or_tune(
    name: &str,
    constant: &libretune_core::ini::Constant,
    endianness: libretune_core::ini::Endianness,
    tune: Option<&libretune_core::tune::TuneFile>,
    cache: Option<&libretune_core::tune::TuneCache>,
) -> f64 {
    // Try tune file first
    if let Some(tune) = tune {
        if let Some(tune_value) = tune.constants.get(name) {
            use libretune_core::tune::TuneValue;
            match tune_value {
                TuneValue::Scalar(v) => return *v,
                TuneValue::Bool(b) if constant.data_type == DataType::Bits => {
                    return if *b { 1.0 } else { 0.0 };
                }
                TuneValue::String(s) if constant.data_type == DataType::Bits => {
                    if let Some(index) = constant.bit_options.iter().position(|opt| opt == s) {
                        return index as f64;
                    } else if let Some(index) = constant
                        .bit_options
                        .iter()
                        .position(|opt| opt.eq_ignore_ascii_case(s))
                    {
                        return index as f64;
                    }
                    return 0.0;
                }
                _ => {} // fall through to cache
            }
        }
    }

    // Try cache
    if let Some(cache) = cache {
        return read_constant_from_cache(constant, endianness, cache);
    }

    0.0
}

/// Read a constant value from the tune cache bytes.
pub(crate) fn read_constant_from_cache(
    constant: &libretune_core::ini::Constant,
    endianness: libretune_core::ini::Endianness,
    cache: &libretune_core::tune::TuneCache,
) -> f64 {
    let length = constant.size_bytes() as u16;
    if length > 0 {
        if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset, length) {
            if let Some(raw_val) = constant.data_type.read_from_bytes(raw_data, 0, endianness) {
                return constant.raw_to_display(raw_val);
            }
        }
    } else if constant.data_type == DataType::Bits {
        let byte_offset = (constant.bit_position.unwrap_or(0) / 8) as u16;
        let bit_in_byte = constant.bit_position.unwrap_or(0) % 8;
        let bytes_needed = (bit_in_byte + constant.bit_size.unwrap_or(0)).div_ceil(8) as u16;
        if let Some(raw_data) = cache.read_bytes(
            constant.page,
            constant.offset + byte_offset,
            bytes_needed.max(1),
        ) {
            let mut bit_value = 0u64;
            for (i, &byte) in raw_data.iter().enumerate() {
                let bit_start = if i == 0 { bit_in_byte } else { 0 };
                let bit_end = if i == bytes_needed.saturating_sub(1) as usize {
                    bit_in_byte + constant.bit_size.unwrap_or(0)
                } else {
                    8
                };
                let bits =
                    ((byte >> bit_start) & bit_mask_u8(bit_end.saturating_sub(bit_start))) as u64;
                bit_value |= bits << (i * 8);
            }
            return bit_value as f64;
        }
    }
    0.0
}
