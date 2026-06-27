//! load_ini command (extracted from lib.rs).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::paths::get_definitions_dir;
use crate::{load_settings, save_settings, AppState};
use libretune_core::ini::EcuDefinition;
use libretune_core::realtime::Evaluator;
use libretune_core::tune::TuneCache;
use tauri::Emitter;

#[tauri::command]
pub async fn load_ini(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    // Resolve path: absolute paths stay as-is, relative paths are resolved from definitions dir
    let full_path = if Path::new(&path).is_absolute() {
        PathBuf::from(&path)
    } else {
        get_definitions_dir(&app).join(&path)
    };

    println!("Loading INI from: {:?}", full_path);
    match EcuDefinition::from_file(full_path.to_string_lossy().as_ref()) {
        Ok(def) => {
            println!(
                "Successfully loaded INI: {} ({} tables, {} pages)",
                def.signature,
                def.tables.len(),
                def.n_pages
            );

            // Get current tune before updating definition (if any)
            let current_tune = {
                let tune_guard = state.current_tune.lock().await;
                tune_guard.as_ref().cloned()
            };

            // Update definition
            let def_clone = def.clone();
            let mut guard = state.definition.write().await;
            *guard = Some(def);
            drop(guard);

            // Cache output channels to avoid repeated cloning in realtime streaming loop
            let mut channels_cache_guard = state.cached_output_channels.lock().await;
            *channels_cache_guard = Some(Arc::new(def_clone.output_channels.clone()));
            drop(channels_cache_guard);

            // Initialize Math Channel Evaluator
            let evaluator = Evaluator::new(&def_clone);
            let mut evaluator_guard = state.evaluator.lock().await;
            *evaluator_guard = Some(evaluator);
            drop(evaluator_guard);

            // Initialize TuneCache from new definition
            let cache = TuneCache::from_definition(&def_clone);
            let mut cache_guard = state.tune_cache.lock().await;
            *cache_guard = Some(cache);

            // Re-apply current tune to new cache if we have one
            if let Some(tune) = current_tune {
                eprintln!("[DEBUG] load_ini: Re-applying tune data to new INI definition");
                use libretune_core::tune::TuneValue;

                let mut applied_count = 0;
                let mut skipped_count = 0;

                for (name, tune_value) in &tune.constants {
                    if let Some(constant) = def_clone.constants.get(name) {
                        // PC variables
                        if constant.is_pc_variable {
                            match tune_value {
                                TuneValue::Scalar(v) => {
                                    cache_guard
                                        .as_mut()
                                        .unwrap()
                                        .local_values
                                        .insert(name.clone(), *v);
                                    applied_count += 1;
                                }
                                TuneValue::Array(arr) if !arr.is_empty() => {
                                    cache_guard
                                        .as_mut()
                                        .unwrap()
                                        .local_values
                                        .insert(name.clone(), arr[0]);
                                    applied_count += 1;
                                }
                                _ => {
                                    skipped_count += 1;
                                }
                            }
                            continue;
                        }

                        // Handle bits constants specially (they're packed, size_bytes() == 0)
                        if constant.data_type == libretune_core::ini::DataType::Bits {
                            let cache = cache_guard.as_mut().unwrap();
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
                                            continue;
                                        }
                                    }
                                }
                                _ => {
                                    skipped_count += 1;
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
                            } else {
                                skipped_count += 1;
                            }
                            continue;
                        }

                        // Skip zero-size constants (shouldn't happen for non-bits)
                        let length = constant.size_bytes() as u16;
                        if length == 0 {
                            skipped_count += 1;
                            continue;
                        }

                        // Convert and write to cache
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
                                    def_clone.endianness,
                                );
                                if cache_guard.as_mut().unwrap().write_bytes(
                                    constant.page,
                                    constant.offset,
                                    &raw_data,
                                ) {
                                    applied_count += 1;
                                } else {
                                    skipped_count += 1;
                                }
                            }
                            TuneValue::Array(arr) => {
                                let last_value = arr.last().copied().unwrap_or(0.0);

                                for i in 0..element_count {
                                    let val = if i < arr.len() { arr[i] } else { last_value };
                                    let raw_val = constant.display_to_raw(val);
                                    let offset = i * element_size;
                                    constant.data_type.write_to_bytes(
                                        &mut raw_data,
                                        offset,
                                        raw_val,
                                        def_clone.endianness,
                                    );
                                }

                                if cache_guard.as_mut().unwrap().write_bytes(
                                    constant.page,
                                    constant.offset,
                                    &raw_data,
                                ) {
                                    applied_count += 1;
                                } else {
                                    skipped_count += 1;
                                }
                            }
                            TuneValue::String(_) | TuneValue::Bool(_) => {
                                skipped_count += 1;
                            }
                        }
                    } else {
                        skipped_count += 1;
                    }
                }

                eprintln!("[DEBUG] load_ini: Re-applied tune constants - applied: {}, skipped: {}, total: {}", 
                    applied_count, skipped_count, tune.constants.len());

                // Emit event to notify UI that tune was re-applied
                let _ = app.emit("tune:loaded", "ini_changed");
            }
            drop(cache_guard);

            // Emit event to notify UI that definition is fully loaded with stats
            let _ = app.emit(
                "definition:loaded",
                serde_json::json!({
                    "signature": def_clone.signature,
                    "tables": def_clone.tables.len(),
                    "curves": def_clone.curves.len(),
                    "dialogs": def_clone.dialogs.len(),
                    "constants": def_clone.constants.len(),
                }),
            );
            eprintln!("[INFO] load_ini: Emitted definition:loaded event (tables={}, curves={}, dialogs={})",
                def_clone.tables.len(), def_clone.curves.len(), def_clone.dialogs.len());

            // Save as last INI
            let mut settings = load_settings(&app);
            settings.last_ini_path = Some(full_path.to_string_lossy().to_string());
            save_settings(&app, &settings);

            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to parse INI {:?}: {}", full_path, e);
            eprintln!("{}", err_msg);
            Err(err_msg)
        }
    }
}
