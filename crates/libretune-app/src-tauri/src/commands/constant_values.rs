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
    let (scalar_constants, endianness) = {
        let def_guard = state.definition.read().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let scalars: Vec<(String, libretune_core::ini::Constant)> = def
            .constants
            .iter()
            .filter(|(_, c)| matches!(c.shape, libretune_core::ini::Shape::Scalar))
            .map(|(name, c)| (name.clone(), c.clone()))
            .collect();
        (scalars, def.endianness)
    };

    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;

    let mut values = HashMap::with_capacity(scalar_constants.len());
    for (name, constant) in scalar_constants {
        let value = read_constant_from_cache_or_tune(
            &name,
            &constant,
            endianness,
            tune_guard.as_ref(),
            cache_guard.as_ref(),
        );
        values.insert(name, value);
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
    // Try tune file — page bytes are authoritative when present (ECU sync / MSQ page data)
    if let Some(tune) = tune {
        if let Some(page_data) = tune.pages.get(&constant.page) {
            if let Some(val) = read_constant_from_page_bytes(constant, page_data, endianness) {
                return val;
            }
        }

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

/// Refresh scalar/bits entries in `tune.constants` from synced page bytes.
pub(crate) fn refresh_tune_constants_from_pages(
    tune: &mut libretune_core::tune::TuneFile,
    def: &libretune_core::ini::EcuDefinition,
) {
    use libretune_core::ini::Shape;
    use libretune_core::tune::TuneValue;

    for (name, constant) in &def.constants {
        if constant.is_pc_variable {
            continue;
        }
        if !matches!(constant.shape, Shape::Scalar) {
            continue;
        }
        if let Some(page_data) = tune.pages.get(&constant.page) {
            if let Some(val) = read_constant_from_page_bytes(constant, page_data, def.endianness) {
                tune.constants.insert(name.clone(), TuneValue::Scalar(val));
            }
        }
    }
}

pub(crate) fn read_constant_from_page_bytes(
    constant: &libretune_core::ini::Constant,
    page_data: &[u8],
    endianness: libretune_core::ini::Endianness,
) -> Option<f64> {
    if constant.data_type == DataType::Bits {
        let bit_pos = constant.bit_position.unwrap_or(0) as usize;
        let bit_size = constant.bit_size.unwrap_or(1) as usize;
        let byte_offset = bit_pos / 8;
        let bit_in_byte = bit_pos % 8;
        let bits_remaining_after_first_byte = bit_size.saturating_sub(8 - bit_in_byte);
        let bytes_needed = if bits_remaining_after_first_byte > 0 {
            1 + bits_remaining_after_first_byte.div_ceil(8)
        } else {
            1
        };

        if byte_offset + bytes_needed > page_data.len() {
            return None;
        }

        let first_byte = page_data[byte_offset];
        let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
        let mask_first = if bits_in_first_byte >= 8 {
            0xFF
        } else {
            (1u8 << bits_in_first_byte) - 1
        };
        let mut bit_val = ((first_byte >> bit_in_byte) & mask_first) as u32;

        if bits_remaining_after_first_byte > 0 {
            for (i, &byte) in page_data[byte_offset + 1..byte_offset + bytes_needed]
                .iter()
                .enumerate()
            {
                let bits_collected = bits_in_first_byte + i * 8;
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
                bit_val |= ((byte & mask) as u32) << bits_collected;
            }
        }

        return Some(bit_val as f64);
    }

    let length = constant.size_bytes();
    let offset = constant.offset as usize;
    if length == 0 || offset + length > page_data.len() {
        return None;
    }

    constant
        .data_type
        .read_from_bytes(page_data, offset, endianness)
        .map(|raw| constant.raw_to_display(raw))
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
