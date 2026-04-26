//! Reset to defaults & CSV import/export Tauri commands.

use libretune_core::ini::DataType;
use libretune_core::tune::TuneValue;

use crate::commands::constant_values::read_constant_from_cache_or_tune;
use crate::state::AppState;

/// Reset all tune values to their INI-defined defaults
#[tauri::command]
pub async fn reset_tune_to_defaults(state: tauri::State<'_, AppState>) -> Result<u32, String> {
    let def_guard = state.definition.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;
    let mut tune_guard = state.current_tune.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let cache = cache_guard.as_mut().ok_or("Tune cache not loaded")?;
    let tune = tune_guard.as_mut().ok_or("No tune loaded")?;

    let mut reset_count = 0u32;

    // Reset each constant to its default value
    for (name, constant) in &def.constants {
        // Skip arrays - they don't have simple defaults
        if !matches!(constant.shape, libretune_core::ini::Shape::Scalar) {
            continue;
        }

        // Get default value from INI [Defaults] section
        let default_value = if let Some(&default_val) = def.default_values.get(name) {
            default_val
        } else {
            // No default defined - use min value as fallback
            constant.min
        };

        // Update PC variable locally
        if constant.is_pc_variable {
            cache.local_values.insert(name.clone(), default_value);
            tune.constants
                .insert(name.clone(), TuneValue::Scalar(default_value));
            reset_count += 1;
            continue;
        }

        // Update ECU constant in cache and tune file
        // Convert display value to raw value for storage
        let raw_value = constant.display_to_raw(default_value);

        // Update tune file
        tune.constants
            .insert(name.clone(), TuneValue::Scalar(default_value));

        // Encode value to bytes and write to cache
        let bytes = encode_constant_value(raw_value, &constant.data_type);
        cache.write_bytes(constant.page, constant.offset, &bytes);
        reset_count += 1;
    }

    Ok(reset_count)
}

/// Export tune data to CSV file
#[tauri::command]
pub async fn export_tune_as_csv(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<u32, String> {
    let def_guard = state.definition.lock().await;
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let mut csv_lines = Vec::new();
    csv_lines.push(
        "Name,Page,Offset,Shape,Value,Units,Min,Max,Scale,Translate,DataType,IsPcVariable"
            .to_string(),
    );

    let mut export_count = 0u32;

    // Export all constants
    for (name, constant) in &def.constants {
        // Get the current value(s)
        let value_str = if constant.data_type == DataType::String {
            // String constant — read raw bytes from cache/tune
            let str_len = constant.size_bytes();
            let raw = if let Some(cache) = cache_guard.as_ref() {
                cache
                    .read_bytes(constant.page, constant.offset, str_len as u16)
                    .map(|b| b.to_vec())
            } else {
                None
            };
            let raw = raw.or_else(|| {
                tune_guard.as_ref().and_then(|tune| {
                    tune.pages.get(&constant.page).and_then(|page_data| {
                        let start = constant.offset as usize;
                        let end = start + str_len;
                        if end <= page_data.len() {
                            Some(page_data[start..end].to_vec())
                        } else {
                            None
                        }
                    })
                })
            });
            if let Some(bytes) = raw {
                // Trim null padding
                let s = String::from_utf8_lossy(&bytes);
                let trimmed = s.trim_end_matches('\0');
                format!("\"{}\"", trimmed.replace('"', "\"\""))
            } else {
                "\"\"".to_string()
            }
        } else if matches!(constant.shape, libretune_core::ini::Shape::Scalar) {
            // Scalar constant
            let value = read_constant_from_cache_or_tune(
                name,
                constant,
                def.endianness,
                tune_guard.as_ref(),
                cache_guard.as_ref(),
            );
            format!("{}", value)
        } else {
            // Array constant — read all elements
            let elem_size = constant.data_type.size_bytes();
            let elem_count = constant.shape.element_count();
            let mut values = Vec::with_capacity(elem_count);

            for idx in 0..elem_count {
                let offset = constant.offset + (idx * elem_size) as u16;
                let raw_bytes = if let Some(cache) = cache_guard.as_ref() {
                    cache
                        .read_bytes(constant.page, offset, elem_size as u16)
                        .map(|b| b.to_vec())
                } else {
                    None
                };
                let raw_bytes = raw_bytes.or_else(|| {
                    tune_guard.as_ref().and_then(|tune| {
                        tune.pages.get(&constant.page).and_then(|page_data| {
                            let start = offset as usize;
                            let end = start + elem_size;
                            if end <= page_data.len() {
                                Some(page_data[start..end].to_vec())
                            } else {
                                None
                            }
                        })
                    })
                });
                let raw_val = if let Some(bytes) = raw_bytes {
                    constant
                        .data_type
                        .read_from_bytes(&bytes, 0, def.endianness)
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                let display_val = constant.raw_to_display(raw_val);
                values.push(format!("{}", display_val));
            }
            format!("\"[{}]\"", values.join(","))
        };

        let shape_str = match &constant.shape {
            libretune_core::ini::Shape::Scalar => "scalar".to_string(),
            libretune_core::ini::Shape::Array1D(n) => format!("[{}]", n),
            libretune_core::ini::Shape::Array2D { rows, cols } => format!("[{}x{}]", rows, cols),
        };

        // Escape name and units for CSV (in case they contain commas)
        let escaped_name = if name.contains(',') || name.contains('"') {
            format!("\"{}\"", name.replace('"', "\"\""))
        } else {
            name.clone()
        };
        let escaped_units = if constant.units.contains(',') || constant.units.contains('"') {
            format!("\"{}\"", constant.units.replace('"', "\"\""))
        } else {
            constant.units.clone()
        };

        let data_type_str = format!("{:?}", constant.data_type);

        csv_lines.push(format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}",
            escaped_name,
            constant.page,
            constant.offset,
            shape_str,
            value_str,
            escaped_units,
            constant.min,
            constant.max,
            constant.scale,
            constant.translate,
            data_type_str,
            constant.is_pc_variable
        ));
        export_count += 1;
    }

    // Write to file
    let csv_content = csv_lines.join("\n");
    std::fs::write(&path, csv_content).map_err(|e| format!("Failed to write CSV file: {}", e))?;

    Ok(export_count)
}

/// Import tune data from CSV file
#[tauri::command]
pub async fn import_tune_from_csv(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<u32, String> {
    let def_guard = state.definition.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;
    let mut tune_guard = state.current_tune.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let cache = cache_guard.as_mut().ok_or("Tune cache not loaded")?;
    let tune = tune_guard.as_mut().ok_or("No tune loaded")?;

    // Read CSV file
    let csv_content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read CSV file: {}", e))?;

    let mut import_count = 0u32;
    let mut errors = Vec::new();

    for (line_num, line) in csv_content.lines().enumerate() {
        // Skip header
        if line_num == 0 && (line.starts_with("Name,") || line.starts_with("\"Name\"")) {
            continue;
        }

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        // Parse CSV line (simple parser - handles basic quoting)
        let fields: Vec<&str> = parse_csv_line(line);

        // Support both old format (11 cols: Name,Page,Offset,Value,...)
        // and new format (12 cols: Name,Page,Offset,Shape,Value,...)
        let (name, value_field) = if fields.len() >= 12 {
            // New format with Shape column
            (fields[0].trim(), fields[4].trim())
        } else if fields.len() >= 4 {
            // Legacy format without Shape column
            (fields[0].trim(), fields[3].trim())
        } else {
            errors.push(format!("Line {}: too few fields", line_num + 1));
            continue;
        };

        // Find constant in definition
        let constant = match def.constants.get(name) {
            Some(c) => c,
            None => {
                // Constant not found - skip silently (might be from different INI)
                continue;
            }
        };

        // Handle string constants
        if constant.data_type == DataType::String {
            let str_val = value_field
                .trim_start_matches('"')
                .trim_end_matches('"')
                .replace("\"\"", "\"");
            let max_len = constant.size_bytes();
            let mut raw_data = vec![0u8; max_len];
            let copy_len = str_val.len().min(max_len);
            raw_data[..copy_len].copy_from_slice(&str_val.as_bytes()[..copy_len]);
            cache.write_bytes(constant.page, constant.offset, &raw_data);
            tune.constants
                .insert(name.to_string(), TuneValue::String(str_val));
            import_count += 1;
            continue;
        }

        // Handle array constants (value looks like "[1.0,2.0,3.0]")
        if !matches!(constant.shape, libretune_core::ini::Shape::Scalar) {
            let array_str = value_field
                .trim_start_matches('"')
                .trim_end_matches('"')
                .trim_start_matches('[')
                .trim_end_matches(']');

            let elem_size = constant.data_type.size_bytes();
            let elem_count = constant.shape.element_count();
            let values: Vec<f64> = array_str
                .split(',')
                .filter_map(|s| s.trim().parse::<f64>().ok())
                .collect();

            let parse_count = values.len().min(elem_count);
            for (idx, &display_val) in values.iter().take(parse_count).enumerate() {
                let clamped = display_val.clamp(constant.min, constant.max);
                let raw_val = constant.display_to_raw(clamped);
                let offset = constant.offset + (idx * elem_size) as u16;
                let mut bytes = vec![0u8; elem_size];
                constant
                    .data_type
                    .write_to_bytes(&mut bytes, 0, raw_val, def.endianness);
                cache.write_bytes(constant.page, offset, &bytes);
            }

            tune.constants
                .insert(name.to_string(), TuneValue::Array(values));
            import_count += 1;
            continue;
        }

        // Scalar constant
        let value: f64 = match value_field.parse() {
            Ok(v) => v,
            Err(_) => {
                errors.push(format!(
                    "Line {}: invalid value '{}'",
                    line_num + 1,
                    value_field
                ));
                continue;
            }
        };

        // Find constant in definition
        let constant = match def.constants.get(name) {
            Some(c) => c,
            None => {
                // Constant not found - skip silently (might be from different INI)
                continue;
            }
        };

        // Validate value is within bounds
        let clamped_value = value.clamp(constant.min, constant.max);
        if (clamped_value - value).abs() > 0.0001 {
            errors.push(format!(
                "Line {}: value {} clamped to {} (range {}-{})",
                line_num + 1,
                value,
                clamped_value,
                constant.min,
                constant.max
            ));
        }

        // Update PC variable locally
        if constant.is_pc_variable {
            cache.local_values.insert(name.to_string(), clamped_value);
            tune.constants
                .insert(name.to_string(), TuneValue::Scalar(clamped_value));
            import_count += 1;
            continue;
        }

        // Update ECU constant
        let raw_value = constant.display_to_raw(clamped_value);
        tune.constants
            .insert(name.to_string(), TuneValue::Scalar(clamped_value));

        // Encode value to bytes and write to cache
        let bytes = encode_constant_value(raw_value, &constant.data_type);
        cache.write_bytes(constant.page, constant.offset, &bytes);
        import_count += 1;
    }

    // Log errors if any
    if !errors.is_empty() {
        eprintln!("[CSV Import] {} warnings/errors:", errors.len());
        for err in errors.iter().take(10) {
            eprintln!("  {}", err);
        }
        if errors.len() > 10 {
            eprintln!("  ... and {} more", errors.len() - 10);
        }
    }

    Ok(import_count)
}
/// Simple CSV line parser that handles quoted fields
pub(crate) fn parse_csv_line(line: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    let chars: Vec<char> = line.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == ',' && !in_quotes {
            let field = &line[start..i];
            // Strip surrounding quotes if present
            let trimmed = field.trim();
            if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
                fields.push(&trimmed[1..trimmed.len() - 1]);
            } else {
                fields.push(trimmed);
            }
            start = i + 1;
        }
    }

    // Add last field
    let field = &line[start..];
    let trimmed = field.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        fields.push(&trimmed[1..trimmed.len() - 1]);
    } else {
        fields.push(trimmed);
    }

    fields
}

/// Encode a constant value to bytes based on data type (big-endian)
pub(crate) fn encode_constant_value(raw_value: f64, data_type: &DataType) -> Vec<u8> {
    match data_type {
        DataType::U08 => vec![raw_value.clamp(0.0, 255.0) as u8],
        DataType::S08 => vec![raw_value.clamp(-128.0, 127.0) as i8 as u8],
        DataType::U16 => {
            let val = raw_value.clamp(0.0, 65535.0) as u16;
            val.to_be_bytes().to_vec()
        }
        DataType::S16 => {
            let val = raw_value.clamp(-32768.0, 32767.0) as i16;
            val.to_be_bytes().to_vec()
        }
        DataType::U32 => {
            let val = raw_value.clamp(0.0, 4294967295.0) as u32;
            val.to_be_bytes().to_vec()
        }
        DataType::S32 => {
            let val = raw_value.clamp(-2147483648.0, 2147483647.0) as i32;
            val.to_be_bytes().to_vec()
        }
        DataType::F32 => (raw_value as f32).to_be_bytes().to_vec(),
        DataType::F64 => raw_value.to_be_bytes().to_vec(),
        DataType::Bits | DataType::String => {
            vec![raw_value.clamp(0.0, 255.0) as u8]
        }
    }
}
