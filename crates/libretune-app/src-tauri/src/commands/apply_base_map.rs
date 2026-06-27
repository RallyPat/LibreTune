//! Apply base map command (extracted from lib.rs).

use crate::AppState;
use libretune_core::tune::{TuneCache, TuneFile};

#[tauri::command]
pub async fn apply_base_map(
    state: tauri::State<'_, AppState>,
    base_map: serde_json::Value,
) -> Result<serde_json::Value, String> {
    use libretune_core::basemap::generator::{
        generate_afr_table, generate_ignition_table, generate_load_bins, generate_rpm_bins,
        generate_ve_table,
    };
    use libretune_core::basemap::EngineSpec;

    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("No ECU definition loaded")?;
    let endianness = def.endianness;

    // Deserialize engine_spec from the base map so we can re-generate at correct dimensions
    let engine_spec: EngineSpec = serde_json::from_value(
        base_map
            .get("engine_spec")
            .cloned()
            .ok_or("Missing engine_spec in base map")?,
    )
    .map_err(|e| format!("Invalid engine_spec: {}", e))?;

    let req_fuel: Option<f64> = base_map.get("req_fuel").and_then(|v| v.as_f64());
    let scalars: Option<serde_json::Map<String, serde_json::Value>> =
        base_map.get("scalars").and_then(|v| v.as_object()).cloned();

    // Find tables by searching common naming patterns across ECU platforms
    // Speeduino names: veTable1Tbl, sparkTbl, afrTable1Tbl
    // rusEFI/FOME names: veTableTbl, ignitionTableTbl, lambdaTableTbl/afrTableTbl
    let ve_table_names = ["veTable1Tbl", "veTableTbl", "fuelTable1Tbl", "fuelTableTbl"];
    let ign_table_names = [
        "sparkTbl",
        "ignitionTableTbl",
        "advTable1Tbl",
        "ignitionTbl",
        "spark1Tbl",
    ];
    let afr_table_names = [
        "afrTable1Tbl",
        "lambdaTableTbl",
        "afrTableTbl",
        "lambdaTable1Tbl",
    ];

    let mut applied = Vec::<String>::new();
    let mut errors = Vec::<String>::new();

    // Helper: write a 2D table's Z values into cache
    fn write_table_z(
        def: &libretune_core::ini::EcuDefinition,
        cache: &mut TuneCache,
        tune: &mut TuneFile,
        table_name: &str,
        values_2d: &[Vec<f64>],
        endianness: libretune_core::ini::Endianness,
    ) -> Result<String, String> {
        let table = def
            .get_table_by_name_or_map(table_name)
            .ok_or_else(|| format!("Table '{}' not found", table_name))?;
        let constant = def.constants.get(&table.map).ok_or_else(|| {
            format!(
                "Constant '{}' not found for table '{}'",
                table.map, table_name
            )
        })?;

        let flat: Vec<f64> = values_2d.iter().flatten().cloned().collect();
        let expected = constant.shape.element_count();

        if flat.len() != expected {
            return Err(format!(
                "Table '{}' dimension mismatch: generated {} cells but INI expects {}",
                table_name,
                flat.len(),
                expected
            ));
        }

        let element_size = constant.data_type.size_bytes();
        let mut raw_data = vec![0u8; constant.size_bytes()];
        for (i, val) in flat.iter().enumerate() {
            let raw_val = constant.display_to_raw(*val);
            let offset = i * element_size;
            constant
                .data_type
                .write_to_bytes(&mut raw_data, offset, raw_val, endianness);
        }

        cache.write_bytes(constant.page, constant.offset, &raw_data);

        // Also update tune file page data
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

        Ok(table.title.clone())
    }

    // Helper: write axis bin values to a constant
    fn write_axis_bins(
        def: &libretune_core::ini::EcuDefinition,
        cache: &mut TuneCache,
        tune: &mut TuneFile,
        const_name: &str,
        values: &[f64],
        endianness: libretune_core::ini::Endianness,
    ) -> Result<(), String> {
        let constant = match def.constants.get(const_name) {
            Some(c) => c,
            None => return Ok(()), // Axis constant not found, skip silently
        };
        let expected = constant.shape.element_count();
        let mut final_values = values.to_vec();
        final_values.resize(expected, *values.last().unwrap_or(&0.0));
        final_values.truncate(expected);

        let element_size = constant.data_type.size_bytes();
        let mut raw_data = vec![0u8; constant.size_bytes()];
        for (i, val) in final_values.iter().enumerate() {
            let raw_val = constant.display_to_raw(*val);
            let offset = i * element_size;
            constant
                .data_type
                .write_to_bytes(&mut raw_data, offset, raw_val, endianness);
        }

        cache.write_bytes(constant.page, constant.offset, &raw_data);

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
        Ok(())
    }

    /// Look up a table definition by trying a list of candidate names.
    /// Returns the matched table definition and the actual cols and rows.
    /// Dimensions are resolved from the map constant's Shape (authoritative source),
    /// falling back to TableDefinition x_size/y_size, then x_bins/y_bins constants.
    fn find_table_with_dims<'a>(
        def: &'a libretune_core::ini::EcuDefinition,
        candidates: &[&str],
    ) -> Option<(&'a libretune_core::ini::TableDefinition, usize, usize)> {
        for name in candidates {
            if let Some(table) = def.get_table_by_name_or_map(name) {
                // Primary: get dimensions from the map constant's Shape
                if let Some(map_const) = def.constants.get(&table.map) {
                    match &map_const.shape {
                        libretune_core::ini::Shape::Array2D { rows, cols } => {
                            if *cols > 0 && *rows > 0 {
                                eprintln!("[DEBUG] find_table_with_dims: '{}' map '{}' shape Array2D {}x{}", name, table.map, cols, rows);
                                return Some((table, *cols, *rows));
                            }
                        }
                        libretune_core::ini::Shape::Array1D(size) => {
                            eprintln!(
                                "[DEBUG] find_table_with_dims: '{}' map '{}' shape Array1D({})",
                                name, table.map, size
                            );
                            return Some((table, *size, 1));
                        }
                        _ => {}
                    }
                }
                // Fallback: use x_bins/y_bins constant shapes
                let cols = if let Some(xc) = def.constants.get(&table.x_bins) {
                    xc.shape.x_size()
                } else {
                    table.x_size
                };
                let rows = if let Some(ref yb) = table.y_bins {
                    if let Some(yc) = def.constants.get(yb) {
                        yc.shape.x_size()
                    } else {
                        table.y_size
                    }
                } else {
                    table.y_size
                };
                // Last resort: TableDefinition x_size/y_size
                let cols = if cols > 0 { cols } else { table.x_size };
                let rows = if rows > 0 { rows } else { table.y_size.max(1) };
                eprintln!(
                    "[DEBUG] find_table_with_dims: '{}' fallback dims {}x{}",
                    name, cols, rows
                );
                if cols > 0 && rows > 0 {
                    return Some((table, cols, rows));
                }
            }
        }
        None
    }

    // Acquire cache and tune locks
    let mut cache_guard = state.tune_cache.lock().await;
    let cache = cache_guard.as_mut().ok_or("Tune cache not initialized")?;
    let mut tune_guard = state.current_tune.lock().await;
    // Create an empty TuneFile if none exists (e.g. new project with no imported tune)
    if tune_guard.is_none() {
        let sig = def.signature.clone();
        *tune_guard = Some(TuneFile::new(&sig));
    }
    let tune = tune_guard.as_mut().unwrap();

    // Apply VE table — generate at the INI's actual table dimensions
    if let Some((table_def, cols, rows)) = find_table_with_dims(def, &ve_table_names) {
        let table_name = table_def.name.clone();
        let title = table_def.title.clone();
        let x_bins_name = table_def.x_bins.clone();
        let y_bins_name = table_def.y_bins.clone();
        eprintln!(
            "[INFO] apply_base_map: VE table '{}' has {}x{} (cols x rows)",
            table_name, cols, rows
        );

        let rpm_bins = generate_rpm_bins(engine_spec.idle_rpm, engine_spec.redline_rpm, cols);
        let load_bins = generate_load_bins(engine_spec.max_load_kpa(), rows);
        let ve_data = generate_ve_table(&engine_spec, &rpm_bins, &load_bins);

        match write_table_z(def, cache, tune, &table_name, &ve_data, endianness) {
            Ok(_) => {
                let _ = write_axis_bins(def, cache, tune, &x_bins_name, &rpm_bins, endianness);
                if let Some(ref y_name) = y_bins_name {
                    let _ = write_axis_bins(def, cache, tune, y_name, &load_bins, endianness);
                }
                applied.push(format!("{} (VE {}x{})", title, cols, rows));
            }
            Err(e) => errors.push(format!("VE: {}", e)),
        }
    }

    // Apply ignition table — generate at the INI's actual table dimensions
    if let Some((table_def, cols, rows)) = find_table_with_dims(def, &ign_table_names) {
        let table_name = table_def.name.clone();
        let title = table_def.title.clone();
        let x_bins_name = table_def.x_bins.clone();
        let y_bins_name = table_def.y_bins.clone();
        eprintln!(
            "[INFO] apply_base_map: Ignition table '{}' has {}x{} (cols x rows)",
            table_name, cols, rows
        );

        let rpm_bins = generate_rpm_bins(engine_spec.idle_rpm, engine_spec.redline_rpm, cols);
        let load_bins = generate_load_bins(engine_spec.max_load_kpa(), rows);
        let ign_data = generate_ignition_table(&engine_spec, &rpm_bins, &load_bins);

        match write_table_z(def, cache, tune, &table_name, &ign_data, endianness) {
            Ok(_) => {
                let _ = write_axis_bins(def, cache, tune, &x_bins_name, &rpm_bins, endianness);
                if let Some(ref y_name) = y_bins_name {
                    let _ = write_axis_bins(def, cache, tune, y_name, &load_bins, endianness);
                }
                applied.push(format!("{} (Ignition {}x{})", title, cols, rows));
            }
            Err(e) => errors.push(format!("Ignition: {}", e)),
        }
    }

    // Apply AFR table — generate at the INI's actual table dimensions
    if let Some((table_def, cols, rows)) = find_table_with_dims(def, &afr_table_names) {
        let table_name = table_def.name.clone();
        let title = table_def.title.clone();
        let x_bins_name = table_def.x_bins.clone();
        let y_bins_name = table_def.y_bins.clone();
        eprintln!(
            "[INFO] apply_base_map: AFR table '{}' has {}x{} (cols x rows)",
            table_name, cols, rows
        );

        let rpm_bins = generate_rpm_bins(engine_spec.idle_rpm, engine_spec.redline_rpm, cols);
        let load_bins = generate_load_bins(engine_spec.max_load_kpa(), rows);
        let afr_data = generate_afr_table(&engine_spec, &rpm_bins, &load_bins);

        match write_table_z(def, cache, tune, &table_name, &afr_data, endianness) {
            Ok(_) => {
                let _ = write_axis_bins(def, cache, tune, &x_bins_name, &rpm_bins, endianness);
                if let Some(ref y_name) = y_bins_name {
                    let _ = write_axis_bins(def, cache, tune, y_name, &load_bins, endianness);
                }
                applied.push(format!("{} (AFR {}x{})", title, cols, rows));
            }
            Err(e) => errors.push(format!("AFR: {}", e)),
        }
    }

    // Apply scalar constants (reqFuel, etc.)
    if let Some(rf) = req_fuel {
        // Try common reqFuel constant names
        for name in &["reqFuel", "req_fuel", "required_fuel"] {
            if let Some(constant) = def.constants.get(*name) {
                let raw_val = constant.display_to_raw(rf);
                let element_size = constant.data_type.size_bytes();
                let mut raw_data = vec![0u8; element_size];
                constant
                    .data_type
                    .write_to_bytes(&mut raw_data, 0, raw_val, endianness);
                cache.write_bytes(constant.page, constant.offset, &raw_data);
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
                applied.push(format!("reqFuel = {:.1} ms", rf));
                break;
            }
        }
    }

    // Apply other scalars from the map
    if let Some(map) = scalars {
        for (name, val) in &map {
            if let Some(v) = val.as_f64() {
                if let Some(constant) = def.constants.get(name.as_str()) {
                    let raw_val = constant.display_to_raw(v);
                    let element_size = constant.data_type.size_bytes();
                    let mut raw_data = vec![0u8; element_size];
                    constant
                        .data_type
                        .write_to_bytes(&mut raw_data, 0, raw_val, endianness);
                    cache.write_bytes(constant.page, constant.offset, &raw_data);
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
            }
        }
    }

    // Mark tune as modified
    *state.tune_modified.lock().await = true;

    // Auto-save the tune to the project directory so it exists on disk.
    // This is critical for new projects that had no tune file — without this,
    // "Use Project Tune" would fail with "Project tune file not found".
    {
        let project_guard = state.current_project.lock().await;
        if let Some(project) = project_guard.as_ref() {
            let tune_path = project.current_tune_path();
            // Sync cache data into tune pages before saving
            for page_num in 0..def.n_pages {
                if let Some(page_data) = cache.get_page(page_num) {
                    tune.pages.insert(page_num, page_data.to_vec());
                }
            }
            if let Err(e) = tune.save(&tune_path) {
                eprintln!("[WARN] apply_base_map: failed to auto-save tune: {}", e);
                errors.push(format!("Failed to save tune to disk: {}", e));
            } else {
                eprintln!("[INFO] apply_base_map: auto-saved tune to {:?}", tune_path);
                // Update the current tune path so future operations find it
                drop(project_guard);
                *state.current_tune_path.lock().await = Some(tune_path);
                *state.tune_modified.lock().await = false;
            }
        }
    }

    if applied.is_empty() {
        errors.push("No matching tables found in the loaded INI definition".to_string());
    }

    let mut result = serde_json::Map::new();
    result.insert("applied".to_string(), serde_json::json!(applied));
    result.insert("errors".to_string(), serde_json::json!(errors));
    eprintln!(
        "[INFO] apply_base_map: applied={:?}, errors={:?}",
        applied, errors
    );
    Ok(serde_json::Value::Object(result))
}
