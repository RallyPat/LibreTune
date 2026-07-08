//! load_tune command (extracted from lib.rs).

use crate::commands::tune_info::TuneInfo;
use crate::{
    find_matching_inis_internal, load_settings, AppState, SignatureMatchType, SignatureMismatchInfo,
};
use libretune_core::tune::{PageState, TuneCache, TuneFile};
use std::path::PathBuf;
use tauri::Emitter;

#[tauri::command]
pub async fn load_tune(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    path: String,
) -> Result<TuneInfo, String> {
    eprintln!("\n[INFO] ========================================");
    eprintln!("[INFO] LOADING TUNE FILE: {}", path);
    eprintln!("[INFO] ========================================");

    let tune = TuneFile::load(&path).map_err(|e| format!("Failed to load tune: {}", e))?;

    eprintln!("[INFO] ✓ Tune file loaded successfully");
    eprintln!("[INFO]   Signature: '{}'", tune.signature);
    eprintln!("[INFO]   Constants: {}", tune.constants.len());
    eprintln!("[INFO]   Pages: {}", tune.pages.len());

    // Debug: List first 20 constant names to see what we parsed
    let constant_names: Vec<String> = tune.constants.keys().take(20).cloned().collect();
    eprintln!(
        "[DEBUG] load_tune: Sample constants from MSQ: {:?}",
        constant_names
    );

    // Debug: Check VE table constants specifically
    let ve_table_in_tune = tune.constants.contains_key("veTable");
    let ve_rpm_bins_in_tune = tune.constants.contains_key("veRpmBins");
    let ve_load_bins_in_tune = tune.constants.contains_key("veLoadBins");
    eprintln!(
        "[DEBUG] load_tune: VE constants in tune - veTable: {}, veRpmBins: {}, veLoadBins: {}",
        ve_table_in_tune, ve_rpm_bins_in_tune, ve_load_bins_in_tune
    );

    // Check if MSQ signature matches current INI definition (informational only)
    // We'll still apply constants by name match regardless of signature match
    let def_guard = state.definition.lock().await;
    let current_ini_signature = def_guard.as_ref().map(|d| d.signature.clone());
    let current_ini_prefix = def_guard.as_ref().and_then(|d| d.signature_prefix.clone());
    drop(def_guard);

    if let Some(ref ini_sig) = current_ini_signature {
        let match_type = crate::commands::signature_helpers::compare_signatures_with_prefix(
            &tune.signature,
            ini_sig,
            current_ini_prefix.as_deref(),
        );
        if match_type != SignatureMatchType::Exact {
            eprintln!("[INFO] load_tune: MSQ signature '{}' {} current INI signature '{}' - will apply constants by name match", 
                tune.signature,
                if match_type == SignatureMatchType::Partial { "partially matches" } else { "does not match" },
                ini_sig);
            eprintln!("[INFO] load_tune: This is normal - many constants (like VE table, ignition tables) will still work across different INI versions");

            // Only show dialog for complete mismatches, and only if we find better matching INIs
            if match_type == SignatureMatchType::Mismatch {
                let matching_inis = find_matching_inis_internal(&state, &tune.signature).await;
                let matching_count = matching_inis.len();

                // Only show dialog if we found better matching INIs
                if matching_count > 0 {
                    let current_ini_path = {
                        let settings = load_settings(&app);
                        settings.last_ini_path.clone()
                    };

                    let mismatch_info = SignatureMismatchInfo {
                        ecu_signature: tune.signature.clone(),
                        ini_signature: ini_sig.clone(),
                        match_type,
                        current_ini_path,
                        matching_inis,
                    };

                    let _ = app.emit("signature:mismatch", &mismatch_info);
                    eprintln!("[INFO] load_tune: Found {} better matching INI file(s). You can switch in the dialog, or continue with current INI.", matching_count);
                }
            }
        } else {
            eprintln!("[INFO] load_tune: MSQ signature matches current INI definition");
        }
    } else {
        eprintln!("[WARN] load_tune: No INI definition loaded - will apply constants by name match if definition is loaded later");
    }

    // Check for INI version migration if tune has a saved manifest (LibreTune 1.1+ tunes)
    // This helps users understand what changed between INI versions
    {
        use libretune_core::tune::migration::compare_manifests;

        let def_guard = state.definition.lock().await;
        if let (Some(saved_manifest), Some(def)) = (&tune.constant_manifest, def_guard.as_ref()) {
            let migration_report = compare_manifests(saved_manifest, def);

            // Only report if there are actual changes
            if migration_report.severity != "none" {
                eprintln!(
                    "[INFO] load_tune: INI version migration detected (severity: {})",
                    migration_report.severity
                );
                eprintln!(
                    "[INFO]   Missing in tune (new in INI): {}",
                    migration_report.missing_in_tune.len()
                );
                eprintln!(
                    "[INFO]   Missing in INI (removed): {}",
                    migration_report.missing_in_ini.len()
                );
                eprintln!(
                    "[INFO]   Type changed: {}",
                    migration_report.type_changed.len()
                );
                eprintln!(
                    "[INFO]   Scale/offset changed: {}",
                    migration_report.scale_changed.len()
                );

                // Store in state for frontend access
                *state.migration_report.lock().await = Some(migration_report.clone());

                // Emit event to notify frontend
                let _ = app.emit("tune:migration_needed", &migration_report);
            } else {
                // Clear any previous migration report
                *state.migration_report.lock().await = None;
            }
        } else if tune.constant_manifest.is_some() {
            eprintln!(
                "[DEBUG] load_tune: Tune has manifest but no INI loaded - migration check deferred"
            );
        } else {
            eprintln!("[DEBUG] load_tune: Tune has no manifest (pre-1.1 format) - migration check skipped");
            // Clear any previous migration report
            *state.migration_report.lock().await = None;
        }
        drop(def_guard);
    }

    let info = TuneInfo {
        path: Some(path.clone()),
        signature: tune.signature.clone(),
        modified: false,
        has_tune: true,
    };

    // Populate TuneCache from loaded tune data
    // This allows table operations to use cached data instead of reading from ECU
    {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref();
        let mut cache_guard = state.tune_cache.lock().await;

        // Initialize cache if it doesn't exist, or reinitialize if it was reset
        if cache_guard.is_none() {
            if let Some(def) = def {
                eprintln!("[DEBUG] load_tune: Initializing cache from definition");
                *cache_guard = Some(TuneCache::from_definition(def));
            } else {
                eprintln!("[WARN] load_tune: No definition loaded, cannot initialize cache");
                return Err("No ECU definition loaded. Please open a project first.".to_string());
            }
        }

        // Ensure cache is initialized even if it exists but is empty
        if let Some(cache) = cache_guard.as_mut() {
            if cache.page_count() == 0 {
                if let Some(def) = def {
                    eprintln!("[DEBUG] load_tune: Cache exists but is empty, reinitializing from definition");
                    *cache_guard = Some(TuneCache::from_definition(def));
                }
            }
        }

        if let Some(cache) = cache_guard.as_mut() {
            // First, load any raw page data
            for (page_num, page_data) in &tune.pages {
                cache.load_page(*page_num, page_data.clone());
                eprintln!(
                    "[DEBUG] load_tune: populated cache page {} with {} bytes",
                    page_num,
                    page_data.len()
                );
            }

            // Then, apply constants from tune file to cache
            if let Some(def) = def {
                eprintln!(
                    "[DEBUG] load_tune: Definition loaded - {} constants in definition",
                    def.constants.len()
                );

                // Debug: Check if VE table constants are in the definition
                let ve_table_in_def = def.constants.contains_key("veTable");
                let ve_rpm_bins_in_def = def.constants.contains_key("veRpmBins");
                let ve_load_bins_in_def = def.constants.contains_key("veLoadBins");
                eprintln!("[DEBUG] load_tune: VE constants in definition - veTable: {}, veRpmBins: {}, veLoadBins: {}", 
                    ve_table_in_def, ve_rpm_bins_in_def, ve_load_bins_in_def);

                // Debug: Show what veTable constant looks like if it exists
                if let Some(ve_const) = def.constants.get("veTable") {
                    eprintln!("[DEBUG] load_tune: veTable constant - page={}, offset={}, size={}, shape={:?}", 
                        ve_const.page, ve_const.offset, ve_const.size_bytes(), ve_const.shape);
                }

                use libretune_core::tune::TuneValue;

                let mut applied_count = 0;
                let mut skipped_count = 0;
                let mut failed_count = 0;
                let mut pcvar_count = 0;
                let mut zero_size_count = 0;
                let mut string_bool_count = 0;

                for (name, tune_value) in &tune.constants {
                    // Debug VE table constants
                    if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                        eprintln!(
                            "[DEBUG] load_tune: Found VE constant '{}' in MSQ file",
                            name
                        );
                    }

                    // Look up constant in definition
                    if let Some(constant) = def.constants.get(name) {
                        // PC variables are stored locally, not in page data
                        if constant.is_pc_variable {
                            match tune_value {
                                TuneValue::Scalar(v) => {
                                    cache.local_values.insert(name.clone(), *v);
                                    pcvar_count += 1;
                                    eprintln!(
                                        "[DEBUG] load_tune: set PC variable '{}' = {}",
                                        name, v
                                    );
                                }
                                TuneValue::Array(arr) if !arr.is_empty() => {
                                    // For arrays, store first value (or handle differently if needed)
                                    cache.local_values.insert(name.clone(), arr[0]);
                                    pcvar_count += 1;
                                    eprintln!(
                                        "[DEBUG] load_tune: set PC variable '{}' = {} (from array)",
                                        name, arr[0]
                                    );
                                }
                                _ => {
                                    skipped_count += 1;
                                    eprintln!("[DEBUG] load_tune: skipping PC variable '{}' (unsupported value type)", name);
                                }
                            }
                            continue;
                        }

                        // Handle bits constants specially (they're packed, size_bytes() == 0)
                        if constant.data_type == libretune_core::ini::DataType::Bits {
                            // Bits constants: read current byte(s), modify the bits, write back
                            let bit_pos = constant.bit_position.unwrap_or(0);
                            let bit_size = constant.bit_size.unwrap_or(1);

                            // Calculate which byte(s) contain the bits
                            let byte_offset = (bit_pos / 8) as u16;
                            let bit_in_byte = bit_pos % 8;

                            // Calculate how many bytes we need
                            let bits_remaining_after_first_byte =
                                bit_size.saturating_sub(8 - bit_in_byte);
                            let bytes_needed = if bits_remaining_after_first_byte > 0 {
                                1 + bits_remaining_after_first_byte.div_ceil(8)
                            } else {
                                1
                            };
                            let bytes_needed_usize = bytes_needed as usize;

                            // Read current byte(s) value (or 0 if not present)
                            let read_offset = constant.offset + byte_offset;
                            let mut current_bytes: Vec<u8> = cache
                                .read_bytes(constant.page, read_offset, bytes_needed as u16)
                                .map(|s| s.to_vec())
                                .unwrap_or_else(|| vec![0u8; bytes_needed_usize]);

                            // Ensure we have enough bytes
                            while current_bytes.len() < bytes_needed_usize {
                                current_bytes.push(0u8);
                            }

                            // Get the bit value from MSQ (index into bit_options)
                            // MSQ can store bits constants as numeric indices, option strings, or booleans
                            let bit_value = match tune_value {
                                TuneValue::Scalar(v) => *v as u32,
                                TuneValue::Array(arr) if !arr.is_empty() => arr[0] as u32,
                                TuneValue::Bool(b) => {
                                    // Boolean values: true = 1, false = 0
                                    // For bits constants with 2 options (like ["false", "true"]),
                                    // boolean true maps to index 1, false to index 0
                                    if *b {
                                        1
                                    } else {
                                        0
                                    }
                                }
                                TuneValue::String(s) => {
                                    // Look up the string in bit_options to find its index
                                    if let Some(index) =
                                        constant.bit_options.iter().position(|opt| opt == s)
                                    {
                                        index as u32
                                    } else {
                                        // Try case-insensitive match
                                        if let Some(index) = constant
                                            .bit_options
                                            .iter()
                                            .position(|opt| opt.eq_ignore_ascii_case(s))
                                        {
                                            index as u32
                                        } else {
                                            skipped_count += 1;
                                            eprintln!("[DEBUG] load_tune: skipping bits constant '{}' (string '{}' not found in bit_options: {:?})", name, s, constant.bit_options);
                                            continue;
                                        }
                                    }
                                }
                                _ => {
                                    skipped_count += 1;
                                    eprintln!("[DEBUG] load_tune: skipping bits constant '{}' (unsupported value type)", name);
                                    continue;
                                }
                            };

                            // Modify the first byte
                            let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
                            let mask_first = if bits_in_first_byte >= 8 {
                                0xFF
                            } else {
                                (1u8 << bits_in_first_byte) - 1
                            };
                            let value_first = (bit_value & mask_first as u32) as u8;
                            current_bytes[0] = (current_bytes[0] & !(mask_first << bit_in_byte))
                                | (value_first << bit_in_byte);

                            // If bits span multiple bytes, modify additional bytes
                            if bits_remaining_after_first_byte > 0 {
                                let mut bits_collected = bits_in_first_byte;
                                for i in 1..bytes_needed_usize.min(current_bytes.len()) {
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
                                    let value_from_bit =
                                        ((bit_value >> bits_collected) & mask as u32) as u8;
                                    current_bytes[i] = (current_bytes[i] & !mask) | value_from_bit;
                                    bits_collected += bits_from_this_byte;
                                }
                            }

                            // Write the modified byte(s) back
                            if cache.write_bytes(constant.page, read_offset, &current_bytes) {
                                applied_count += 1;
                                eprintln!("[DEBUG] load_tune: ✓ Applied bits constant '{}' = {} (bit_pos={}, bit_size={}, bytes={})", 
                                    name, bit_value, bit_pos, bit_size, bytes_needed);
                            } else {
                                failed_count += 1;
                                eprintln!(
                                    "[DEBUG] load_tune: ✗ Failed to write bits constant '{}'",
                                    name
                                );
                            }
                            continue;
                        }

                        // Skip if constant has no size (shouldn't happen for non-bits)
                        let length = constant.size_bytes() as u16;
                        if length == 0 {
                            zero_size_count += 1;
                            skipped_count += 1;
                            eprintln!(
                                "[DEBUG] load_tune: skipping constant '{}' (zero size)",
                                name
                            );
                            continue;
                        }

                        // Convert tune value to raw bytes
                        let element_size = constant.data_type.size_bytes();
                        let element_count = constant.shape.element_count();
                        let mut raw_data = vec![0u8; length as usize];

                        match tune_value {
                            TuneValue::Scalar(v) => {
                                let raw_val = constant.display_to_raw(*v);
                                constant.data_type.write_to_bytes(
                                    &mut raw_data,
                                    0,
                                    raw_val,
                                    def.endianness,
                                );
                                // Check if page exists before writing
                                let page_exists = cache.page_size(constant.page).is_some();
                                let page_state_before = cache.page_state(constant.page);

                                if name == "veTable" || name == "veRpmBins" || name == "veLoadBins"
                                {
                                    eprintln!("[DEBUG] load_tune: About to write '{}' - page={}, page_exists={}, page_state={:?}, offset={}, len={}", 
                                        name, constant.page, page_exists, page_state_before, constant.offset, length);
                                }

                                if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                    applied_count += 1;
                                    let page_state_after = cache.page_state(constant.page);

                                    // Verify the data was actually written by reading it back
                                    if name == "veTable"
                                        || name == "veRpmBins"
                                        || name == "veLoadBins"
                                    {
                                        let verify_read = cache.read_bytes(
                                            constant.page,
                                            constant.offset,
                                            length,
                                        );
                                        eprintln!("[DEBUG] load_tune: ✓ Applied constant '{}' = {} (scalar, page={}, offset={}, state={:?}, verify_read={})", 
                                            name, v, constant.page, constant.offset, page_state_after, verify_read.is_some());
                                    }
                                } else {
                                    failed_count += 1;
                                    if name == "veTable"
                                        || name == "veRpmBins"
                                        || name == "veLoadBins"
                                    {
                                        eprintln!("[DEBUG] load_tune: ✗ Failed to write constant '{}' (scalar, page={}, offset={}, len={}, page_size={:?}, page_exists={})", 
                                            name, constant.page, constant.offset, length, cache.page_size(constant.page), page_exists);
                                    }
                                }
                            }
                            TuneValue::Array(arr) => {
                                // Handle size mismatches: write what we have, pad or truncate as needed
                                let write_count = arr.len().min(element_count);
                                let last_value = arr.last().copied().unwrap_or(0.0);

                                for i in 0..element_count {
                                    let val = if i < arr.len() {
                                        arr[i]
                                    } else {
                                        // Pad with last value if array is smaller
                                        last_value
                                    };
                                    let raw_val = constant.display_to_raw(val);
                                    let offset = i * element_size;
                                    constant.data_type.write_to_bytes(
                                        &mut raw_data,
                                        offset,
                                        raw_val,
                                        def.endianness,
                                    );
                                }

                                // Check if page exists before writing
                                let page_exists = cache.page_size(constant.page).is_some();
                                let page_state_before = cache.page_state(constant.page);

                                if name == "veTable" || name == "veRpmBins" || name == "veLoadBins"
                                {
                                    if arr.len() != element_count {
                                        eprintln!("[DEBUG] load_tune: array size mismatch for '{}': expected {}, got {} (will pad/truncate)", 
                                            name, element_count, arr.len());
                                    }
                                    eprintln!("[DEBUG] load_tune: About to write '{}' - page={}, page_exists={}, page_state={:?}, offset={}, len={}", 
                                        name, constant.page, page_exists, page_state_before, constant.offset, length);
                                }

                                if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                    applied_count += 1;
                                    let page_state_after = cache.page_state(constant.page);

                                    // Verify the data was actually written by reading it back
                                    if name == "veTable"
                                        || name == "veRpmBins"
                                        || name == "veLoadBins"
                                    {
                                        let verify_read = cache.read_bytes(
                                            constant.page,
                                            constant.offset,
                                            length,
                                        );
                                        eprintln!("[DEBUG] load_tune: ✓ Applied constant '{}' (array, {} elements written, {} expected, page={}, offset={}, state={:?}, verify_read={})", 
                                            name, write_count, element_count, constant.page, constant.offset, page_state_after, verify_read.is_some());
                                    }
                                } else {
                                    failed_count += 1;
                                    if name == "veTable"
                                        || name == "veRpmBins"
                                        || name == "veLoadBins"
                                    {
                                        eprintln!("[DEBUG] load_tune: ✗ Failed to write constant '{}' (array, page={}, offset={}, len={}, page_size={:?}, page_exists={})", 
                                            name, constant.page, constant.offset, length, cache.page_size(constant.page), page_exists);
                                    }
                                }
                            }
                            TuneValue::String(_) | TuneValue::Bool(_) => {
                                string_bool_count += 1;
                                skipped_count += 1;
                                eprintln!("[DEBUG] load_tune: skipping constant '{}' (string/bool not supported for page data)", name);
                            }
                        }
                    } else {
                        skipped_count += 1;
                        if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                            eprintln!(
                                "[DEBUG] load_tune: constant '{}' not found in definition",
                                name
                            );
                        }
                    }
                }

                // Print prominent summary
                let total_accounted = applied_count + pcvar_count + skipped_count + failed_count;
                eprintln!("\n[INFO] ========================================");
                eprintln!("[INFO] Tune Load Summary:");
                eprintln!("[INFO]   Total constants in MSQ: {}", tune.constants.len());
                eprintln!(
                    "[INFO]   Successfully applied (page data): {}",
                    applied_count
                );
                eprintln!("[INFO]   PC variables applied: {}", pcvar_count);
                eprintln!("[INFO]   Failed to apply: {}", failed_count);
                eprintln!("[INFO]   Skipped:");
                eprintln!(
                    "[INFO]     - Not in definition: {}",
                    skipped_count - zero_size_count - string_bool_count
                );
                eprintln!("[INFO]     - Zero size (packed bits): {}", zero_size_count);
                eprintln!(
                    "[INFO]     - String/Bool (unsupported): {}",
                    string_bool_count
                );
                eprintln!("[INFO]   Total skipped: {}", skipped_count);
                if total_accounted != tune.constants.len() {
                    eprintln!(
                        "[WARN]   ⚠ Accounting mismatch: {} constants unaccounted for!",
                        tune.constants.len() - total_accounted
                    );
                }
                eprintln!("[INFO] ========================================\n");

                // Debug: Check page states after loading and show actual data sizes
                eprintln!("[DEBUG] load_tune: Page states after loading:");
                for page in 0..cache.page_count() {
                    let state = cache.page_state(page);
                    let def_size = cache.page_size(page);
                    let actual_size = cache.get_page(page).map(|p| p.len()).unwrap_or(0);
                    if state != PageState::NotLoaded || def_size.is_some() || actual_size > 0 {
                        eprintln!("[DEBUG] load_tune:   Page {}: state={:?}, def_size={:?}, actual_data_size={} bytes", 
                            page, state, def_size, actual_size);
                    }
                }

                if applied_count > 0 {
                    let total_applied = applied_count + pcvar_count;
                    eprintln!("[INFO] ✓ Successfully loaded {} constants into cache ({} page data + {} PC variables).", 
                        total_applied, applied_count, pcvar_count);
                    eprintln!("[INFO]   Important tables like VE, ignition, and fuel should work even if some constants don't match.");
                    eprintln!("[INFO]   All open tables will refresh automatically.");

                    // Informational note if many constants were skipped (not a warning - this is normal)
                    if skipped_count > applied_count && skipped_count > 100 {
                        let applied_percent =
                            (total_applied as f64 / tune.constants.len() as f64 * 100.0) as u32;
                        eprintln!("[INFO] ℹ Note: {} constants ({}%) were skipped - they're not in the current INI definition.", skipped_count, 100 - applied_percent);
                        eprintln!("[INFO]   This is normal when INI versions differ. Core tuning tables should still work.");
                        eprintln!("[INFO]   If you need those constants, switch to a matching INI file in Settings.");
                    }
                } else {
                    eprintln!("[WARN] ⚠ No constants were applied! This usually means the MSQ file doesn't match the current INI definition.");
                    eprintln!("[WARN]   MSQ signature: '{}'", tune.signature);
                    eprintln!("[WARN]   Check the Signature Mismatch dialog (if shown) or switch to a matching INI file in Settings.");
                }
            } else {
                eprintln!("[DEBUG] load_tune: no definition loaded, skipping constant application");
            }
        }
    }

    *state.current_tune.lock().await = Some(tune.clone());
    *state.current_tune_path.lock().await = Some(PathBuf::from(path));
    *state.tune_modified.lock().await = false;

    // If a project is open, save the tune to the project's CurrentTune.msq
    // This ensures it will be auto-loaded next time the project is opened
    let proj_guard = state.current_project.lock().await;
    if let Some(ref project) = *proj_guard {
        let project_tune_path = project.path.join("CurrentTune.msq");
        if let Err(e) = tune.save(&project_tune_path) {
            eprintln!("[WARN] Failed to save tune to project folder: {}", e);
        } else {
            eprintln!("[INFO] ✓ Saved tune to project: {:?}", project_tune_path);
            // Update the stored tune path to point to the project's tune file
            *state.current_tune_path.lock().await = Some(project_tune_path);
        }
    }
    drop(proj_guard);

    // Emit event to notify UI that tune was loaded
    let _ = app.emit("tune:loaded", "file");

    Ok(info)
}
