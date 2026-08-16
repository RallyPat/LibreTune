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

    Ok(collect_scalar_constant_values(
        def,
        tune_guard.as_ref(),
        cache_guard.as_ref(),
    ))
}

/// Current value of every scalar constant, read from tune/cache (no ECU
/// round-trip). Shared by visibility-condition evaluation and INI gauge-range
/// expression resolution (`{rpmhigh}` etc.).
pub(crate) fn collect_scalar_constant_values(
    def: &libretune_core::ini::EcuDefinition,
    tune: Option<&libretune_core::tune::TuneFile>,
    cache: Option<&libretune_core::tune::TuneCache>,
) -> HashMap<String, f64> {
    let mut values = HashMap::new();
    for (name, constant) in &def.constants {
        // Skip array constants (only need scalars for visibility conditions)
        if !matches!(constant.shape, libretune_core::ini::Shape::Scalar) {
            continue;
        }

        let value = read_constant_from_cache_or_tune(name, constant, def.endianness, tune, cache);

        values.insert(name.clone(), value);
    }
    values
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

#[cfg(test)]
mod tests {
    use super::*;
    use libretune_core::ini::{Constant, DataType, EcuDefinition, Endianness, Shape};
    use libretune_core::tune::{TuneCache, TuneFile};

    /// Reproduce the issue behind the offline constant read bug: an MSQ that
    /// stores its data as `<pageData>` blobs has an EMPTY `tune.constants` map.
    /// The value lives only in the decoded cache bytes. Reading such a
    /// constant must fall back to the cache, not return 0.
    #[test]
    fn cache_fallback_when_named_constant_absent() {
        // reqFuel = scalar, U08, offset 24, "ms", scale 0.1 (Speeduino-like).
        // A raw byte of 126 -> display 12.6 ms.
        let req_fuel = Constant {
            name: "reqFuel".to_string(),
            label: None,
            page: 0,
            offset: 24,
            data_type: DataType::U08,
            endianness_override: None,
            shape: Shape::Scalar,
            bit_position: None,
            bit_size: None,
            display_offset: 0,
            units: "ms".to_string(),
            scale: 0.1,
            translate: 0.0,
            min: 0.0,
            max: 25.5,
            digits: 1,
            help: None,
            visibility_condition: None,
            bit_options: Vec::new(),
            is_pc_variable: false,
            dynamic_size: None,
        };

        // Build a cache with a single page, load raw bytes, write reqFuel=126.
        let mut def = EcuDefinition::default();
        def.page_sizes = vec![64];
        def.n_pages = 1;
        let mut cache = TuneCache::from_definition(&def);
        cache.load_page(0, vec![0u8; 64]);
        assert!(cache.write_bytes(0, 24, &[126]));

        // TuneFile with NO named constants (the <pageData>-only MSQ case).
        let tune = TuneFile::new("test");

        let val = read_constant_from_cache_or_tune(
            "reqFuel",
            &req_fuel,
            Endianness::Little,
            Some(&tune),
            Some(&cache),
        );
        assert!(
            (val - 12.6).abs() < 1e-9,
            "expected reqFuel=12.6 ms from cache fallback, got {val}"
        );
    }

    /// Same scenario but for a bits constant: packed value lives in cache
    /// bytes, not in `tune.constants`. Must extract the bit field, not 0.
    #[test]
    fn cache_fallback_for_bits_constant() {
        // nCylinders = bits, U08, offset 36, [4:7] (Speeduino-like). A byte
        // value of 0b0101_0000 -> bits [4:7] = 0b0101 = 5 (index).
        let n_cyl = Constant {
            name: "nCylinders".to_string(),
            label: None,
            page: 0,
            offset: 36,
            data_type: DataType::Bits,
            endianness_override: None,
            shape: Shape::Scalar,
            bit_position: Some(4),
            bit_size: Some(4),
            display_offset: 0,
            units: String::new(),
            scale: 1.0,
            translate: 0.0,
            min: 0.0,
            max: 15.0,
            digits: 0,
            help: None,
            visibility_condition: None,
            bit_options: vec![
                "INVALID", "1", "2", "3", "4", "5", "6", "INVALID", "8", "INVALID", "INVALID",
                "INVALID", "INVALID", "INVALID", "INVALID", "INVALID",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            is_pc_variable: false,
            dynamic_size: None,
        };

        let mut def = EcuDefinition::default();
        def.page_sizes = vec![64];
        def.n_pages = 1;
        let mut cache = TuneCache::from_definition(&def);
        cache.load_page(0, vec![0u8; 64]);
        // 0b0101_0000: bits [4:7] = 0101 = 5
        assert!(cache.write_bytes(0, 36, &[0b0101_0000]));

        let tune = TuneFile::new("test");

        let val = read_constant_from_cache_or_tune(
            "nCylinders",
            &n_cyl,
            Endianness::Little,
            Some(&tune),
            Some(&cache),
        );
        assert_eq!(val, 5.0, "expected bits[4:7]=5 from cache fallback");
    }

    /// Named constant in `tune.constants` takes priority over cache (so an
    /// MSQ that DOES store `<constant>` tags is honored).
    #[test]
    fn named_constant_takes_priority_over_cache() {
        let req_fuel = Constant::new("reqFuel", 0, 24, DataType::U08);
        let mut def = EcuDefinition::default();
        def.page_sizes = vec![64];
        def.n_pages = 1;
        let mut cache = TuneCache::from_definition(&def);
        cache.load_page(0, vec![0u8; 64]);

        let mut tune = TuneFile::new("test");
        tune.constants.insert(
            "reqFuel".to_string(),
            libretune_core::tune::TuneValue::Scalar(9.5),
        );

        let val = read_constant_from_cache_or_tune(
            "reqFuel",
            &req_fuel,
            Endianness::Little,
            Some(&tune),
            Some(&cache),
        );
        assert!(
            (val - 9.5).abs() < 1e-9,
            "named constant should win, expected 9.5 got {val}"
        );
    }
}
