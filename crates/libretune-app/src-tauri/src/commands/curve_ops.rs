//! Curve get/update commands.

use crate::AppState;
use libretune_core::ini::Constant;
use libretune_core::protocol::Connection;
use libretune_core::tune::TuneFile;
use serde::Serialize;

#[derive(Serialize)]
pub struct CurveData {
    pub name: String,
    pub title: String,
    pub x_bins: Vec<f64>,
    pub y_bins: Vec<f64>,
    pub x_label: String,
    pub y_label: String,
    /// X-axis range: (min, max, step)
    pub x_axis: Option<(f32, f32, f32)>,
    /// Y-axis range: (min, max, step)
    pub y_axis: Option<(f32, f32, f32)>,
    /// Output channel name for live cursor (e.g., "coolant")
    pub x_output_channel: Option<String>,
    /// Gauge name for live display
    pub gauge: Option<String>,
}
/// # Arguments
/// * `curve_name` - Curve name from INI definition
///
/// Returns: CurveData with x/y values and metadata
#[tauri::command]
pub async fn get_curve_data(
    state: tauri::State<'_, AppState>,
    curve_name: String,
) -> Result<CurveData, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or_else(|| {
        eprintln!(
            "[WARN] get_curve_data: Definition not loaded when looking for '{}'",
            curve_name
        );
        "Definition not loaded".to_string()
    })?;
    let endianness = def.endianness;

    // Diagnostic logging
    eprintln!(
        "[DEBUG] get_curve_data: Looking for '{}' in {} curves ({} map entries)",
        curve_name,
        def.curves.len(),
        def.curve_map_to_name.len()
    );

    let curve = def.get_curve_by_name_or_map(&curve_name).ok_or_else(|| {
        // Log available curves for debugging
        let available: Vec<_> = def.curves.keys().take(10).cloned().collect();
        eprintln!(
            "[WARN] get_curve_data: Curve '{}' not found. Available curves (first 10): {:?}",
            curve_name, available
        );
        format!(
            "Curve '{}' not found (checked {} curves, {} map entries)",
            curve_name,
            def.curves.len(),
            def.curve_map_to_name.len()
        )
    })?;

    eprintln!(
        "[DEBUG] get_curve_data: Found curve '{}' (title: {})",
        curve.name, curve.title
    );

    // Clone the constant info we need
    let x_const = def
        .constants
        .get(&curve.x_bins)
        .ok_or_else(|| format!("Constant {} not found", curve.x_bins))?
        .clone();
    let y_const = def
        .constants
        .get(&curve.y_bins)
        .ok_or_else(|| format!("Constant {} not found", curve.y_bins))?
        .clone();

    // Clone curve metadata
    let curve_name_out = curve.name.clone();
    let curve_title = curve.title.clone();
    let x_label = curve.column_labels.0.clone();
    let y_label = curve.column_labels.1.clone();
    let x_axis = curve.x_axis;
    let y_axis = curve.y_axis;
    let x_output_channel = curve.x_output_channel.clone();
    let gauge = curve.gauge.clone();

    drop(def_guard);

    // Helper to read constant data from TuneFile (offline) or ECU (online)
    fn read_const_from_source(
        constant: &Constant,
        tune: Option<&TuneFile>,
        conn: &mut Option<&mut Connection>,
        endianness: libretune_core::ini::Endianness,
    ) -> Result<Vec<f64>, String> {
        let element_count = constant.shape.element_count();
        let element_size = constant.data_type.size_bytes();
        let length = constant.size_bytes() as u16;

        eprintln!(
            "[DEBUG] read_const_from_source: '{}' - shape={:?}, element_count={}, element_size={}, total_length={}",
            constant.name, constant.shape, element_count, element_size, length
        );

        // If offline, read from TuneFile (MSQ file)
        if conn.is_none() {
            if let Some(tune_file) = tune {
                // First try named constants (parsed from MSQ <constant> tags)
                if let Some(tune_value) = tune_file.constants.get(&constant.name) {
                    use libretune_core::tune::TuneValue;
                    eprintln!(
                        "[DEBUG] read_const_from_source: '{}' found in TuneFile.constants",
                        constant.name
                    );
                    match tune_value {
                        TuneValue::Array(arr) => {
                            eprintln!("[DEBUG] read_const_from_source: '{}' returning {} array values from constants", constant.name, arr.len());
                            return Ok(arr.clone());
                        }
                        TuneValue::Scalar(v) => {
                            return Ok(vec![*v]);
                        }
                        _ => {}
                    }
                }

                // Fallback: try to read from raw page data using INI offset
                // This handles cases where the constant wasn't explicitly in the MSQ file
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
                        eprintln!("[DEBUG] read_const_from_source: '{}' returning {} values from page data", constant.name, values.len());
                        return Ok(values);
                    } else {
                        eprintln!("[WARN] read_const_from_source: '{}' offset {} + size {} exceeds page {} length {}", 
                            constant.name, offset, total_bytes, constant.page, page_data.len());
                    }
                } else {
                    eprintln!("[WARN] read_const_from_source: '{}' page {} not found in TuneFile.pages (available: {:?})", 
                        constant.name, constant.page, tune_file.pages.keys().collect::<Vec<_>>());
                }
            }
            // If not found anywhere, return zeros
            eprintln!(
                "[DEBUG] read_const_from_source: '{}' returning {} zeros (not in TuneFile)",
                constant.name, element_count
            );
            return Ok(vec![0.0; element_count]);
        }

        // For ECU reads, we need valid length
        if length == 0 {
            eprintln!(
                "[WARN] read_const_from_source: '{}' has length=0, cannot read from ECU",
                constant.name
            );
            return Ok(vec![0.0; element_count]);
        }

        // If connected to ECU, read from ECU (live data)
        if let Some(ref mut conn_ptr) = conn {
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

        Ok(vec![0.0; element_count])
    }

    // Get tune and connection
    let tune_guard = state.current_tune.lock().await;
    let mut conn_guard = state.connection.lock().await;
    let mut conn = conn_guard.as_mut();

    let x_bins = read_const_from_source(&x_const, tune_guard.as_ref(), &mut conn, endianness)?;
    let y_bins = read_const_from_source(&y_const, tune_guard.as_ref(), &mut conn, endianness)?;

    Ok(CurveData {
        name: curve_name_out,
        title: curve_title,
        x_bins,
        y_bins,
        x_label,
        y_label,
        x_axis,
        y_axis,
        x_output_channel,
        gauge,
    })
}

/// Updates table Z values in the tune cache and optionally writes to ECU.
///
/// Converts display values to raw bytes and writes to the tune cache.
/// If connected to ECU, also writes to ECU memory. Works in offline mode.
///
/// # Arguments
/// * `curve_name` - Curve name from INI definition
/// * `y_values` - Vector of new Y values in display units
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn update_curve_data(
    state: tauri::State<'_, AppState>,
    curve_name: String,
    y_values: Vec<f64>,
) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let curve = def
        .get_curve_by_name_or_map(&curve_name)
        .ok_or_else(|| format!("Curve {} not found", curve_name))?;

    // Get the Y-bins constant (the values we're updating)
    let constant = def.constants.get(&curve.y_bins).ok_or_else(|| {
        format!(
            "Constant {} not found for curve {}",
            curve.y_bins, curve_name
        )
    })?;

    if y_values.len() != constant.shape.element_count() {
        return Err(format!(
            "Invalid data size: expected {}, got {}",
            constant.shape.element_count(),
            y_values.len()
        ));
    }

    // Convert display values to raw bytes
    let element_size = constant.data_type.size_bytes();
    let mut raw_data = vec![0u8; constant.size_bytes()];

    for (i, val) in y_values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant
            .data_type
            .write_to_bytes(&mut raw_data, offset, raw_val, def.endianness);
    }

    // Write to TuneCache if available (enables offline editing)
    if let Some(cache) = cache_guard.as_mut() {
        if cache.write_bytes(constant.page, constant.offset, &raw_data) {
            // Also update TuneFile in memory
            let mut tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_mut() {
                // Update the parsed constants map (used by get_curve_data)
                tune.constants.insert(
                    constant.name.clone(),
                    libretune_core::tune::TuneValue::Array(y_values.clone()),
                );

                // Also update raw page data
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

            // Mark tune as modified
            *state.tune_modified.lock().await = true;
        }
    }

    // Write to ECU if connected
    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data.clone(),
        };

        if let Err(e) = conn.write_memory(params) {
            eprintln!("[WARN] Failed to write curve to ECU (offline mode?): {}", e);
        }
    }

    Ok(())
}
