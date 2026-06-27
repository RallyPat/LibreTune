//! Shared tune persistence helpers: cache sync, auto-save, cache initialization.

use std::path::PathBuf;
use std::time::Duration;

use crate::state::AppState;
use tauri::Manager;
use libretune_core::ini::{DataType, EcuDefinition};
use libretune_core::tune::{TuneCache, TuneFile, TuneValue};

/// Ensure a TuneCache exists, seeding from the current in-memory tune pages when possible.
pub(crate) async fn ensure_tune_cache(state: &AppState, def: &EcuDefinition) {
    let mut cache_guard = state.tune_cache.lock().await;
    if cache_guard.is_some() {
        return;
    }

    let mut cache = TuneCache::from_definition(def);
    if let Some(tune) = state.current_tune.lock().await.as_ref() {
        for (page_num, page_data) in &tune.pages {
            cache.load_page(*page_num, page_data.clone());
        }
    }
    *cache_guard = Some(cache);
}

/// Copy TuneCache page/constant data into a TuneFile before writing to disk.
pub(crate) fn sync_cache_to_tune(tune: &mut TuneFile, cache: &TuneCache, def: &EcuDefinition) -> usize {
    for page_num in 0..def.n_pages {
        if let Some(page_data) = cache.get_page(page_num) {
            tune.pages.insert(page_num, page_data.to_vec());
        }
    }

    let mut constants_saved = 0;

    for (name, constant) in &def.constants {
        if constant.is_pc_variable {
            if let Some(value) = cache.local_values.get(name) {
                tune.set_constant_with_page(name.clone(), TuneValue::Scalar(*value), constant.page);
                constants_saved += 1;
            }
            continue;
        }

        if constant.data_type == DataType::Bits {
            let byte_offset = (constant.bit_position.unwrap_or(0) / 8) as u16;
            let bit_in_byte = constant.bit_position.unwrap_or(0) % 8;
            let bit_size = constant.bit_size.unwrap_or(0);
            let bytes_needed = (bit_in_byte + bit_size).div_ceil(8).max(1) as u16;

            if let Some(bytes) =
                cache.read_bytes(constant.page, constant.offset + byte_offset, bytes_needed)
            {
                let mut bit_val: u32 = 0;
                let mut bits_remaining = bit_size;
                let mut current_bit = bit_in_byte;

                for byte in bytes.iter().take(bytes_needed as usize) {
                    let bits_in_this_byte = bits_remaining.min(8 - current_bit);
                    let mask = if bits_in_this_byte == 0 {
                        0
                    } else if bits_in_this_byte == 8 && current_bit == 0 {
                        0xFFu8
                    } else {
                        let base_mask = (1u8 << bits_in_this_byte.min(7)) - 1;
                        base_mask << current_bit
                    };
                    let extracted = ((*byte & mask) >> current_bit) as u32;
                    bit_val |= extracted << (bit_size - bits_remaining);

                    bits_remaining = bits_remaining.saturating_sub(bits_in_this_byte);
                    if bits_remaining == 0 {
                        break;
                    }
                    current_bit = 0;
                }

                let bit_index = bit_val as usize;
                if bit_index < constant.bit_options.len() {
                    tune.set_constant_with_page(
                        name.clone(),
                        TuneValue::String(constant.bit_options[bit_index].clone()),
                        constant.page,
                    );
                } else {
                    tune.set_constant_with_page(
                        name.clone(),
                        TuneValue::Scalar(bit_val as f64),
                        constant.page,
                    );
                }
                constants_saved += 1;
            }
            continue;
        }

        let length = constant.size_bytes() as u16;
        if length == 0 {
            continue;
        }

        if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset, length) {
            let element_count = constant.shape.element_count();
            let element_size = constant.data_type.size_bytes();
            let mut values = Vec::new();

            for i in 0..element_count {
                let offset = i * element_size;
                if let Some(raw_val) =
                    constant
                        .data_type
                        .read_from_bytes(raw_data, offset, def.endianness)
                {
                    values.push(constant.raw_to_display(raw_val));
                } else {
                    values.push(0.0);
                }
            }

            let tune_value = if element_count == 1 {
                TuneValue::Scalar(values[0])
            } else {
                TuneValue::Array(values)
            };

            tune.set_constant_with_page(name.clone(), tune_value, constant.page);
            constants_saved += 1;
        }
    }

    constants_saved
}

/// Write the current in-memory tune (with cache merged) to the given path.
pub(crate) async fn persist_tune_to_path(
    state: &AppState,
    save_path: PathBuf,
) -> Result<(), String> {
    let mut tune_guard = state.current_tune.lock().await;
    let cache_guard = state.tune_cache.lock().await;
    let def_guard = state.definition.read().await;

    let tune = tune_guard.as_mut().ok_or("No tune loaded")?;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    if let Some(cache) = cache_guard.as_ref() {
        let saved = sync_cache_to_tune(tune, cache, def);
        eprintln!("[DEBUG] persist_tune: synced {} constants from cache", saved);
    }

    tune.touch();

    let ini_name = state
        .current_project
        .lock()
        .await
        .as_ref()
        .map(|p| p.config.ecu_definition.clone())
        .unwrap_or_else(|| "unknown.ini".to_string());
    tune.ini_metadata = Some(def.generate_ini_metadata(&ini_name));
    tune.constant_manifest = Some(def.generate_constant_manifest());

    if let Some(parent) = save_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    tune.save(&save_path)
        .map_err(|e| format!("Failed to save tune: {}", e))?;

    drop(tune_guard);
    drop(cache_guard);
    drop(def_guard);

    *state.current_tune_path.lock().await = Some(save_path);
    *state.tune_modified.lock().await = false;

    Ok(())
}

/// Auto-save the current tune to the open project's CurrentTune.msq when modified.
pub(crate) async fn maybe_auto_save_project_tune(state: &AppState) {
    let project_path = {
        let project_guard = state.current_project.lock().await;
        match project_guard.as_ref() {
            Some(project) => project.current_tune_path(),
            None => return,
        }
    };

    if !*state.tune_modified.lock().await {
        return;
    }

    if let Err(e) = persist_tune_to_path(state, project_path).await {
        eprintln!("[WARN] Auto-save to project failed: {}", e);
    }
}

/// Debounced auto-save — coalesces rapid field edits into one disk write.
pub(crate) fn schedule_debounced_auto_save(app: tauri::AppHandle) {
    tokio::spawn(async move {
        let generation = {
            let state = app.state::<AppState>();
            let mut gen = state.autosave_generation.lock().await;
            *gen += 1;
            *gen
        };

        tokio::time::sleep(Duration::from_secs(2)).await;

        let state = app.state::<AppState>();
        let current = *state.autosave_generation.lock().await;
        if current != generation {
            return;
        }

        maybe_auto_save_project_tune(&state).await;
    });
}

/// Write raw table bytes into tune pages (used when cache is unavailable).
pub(crate) fn write_bytes_to_tune_pages(
    tune: &mut TuneFile,
    def: &EcuDefinition,
    page: u8,
    offset: u16,
    raw_data: &[u8],
) {
    let page_data = tune.pages.entry(page).or_insert_with(|| {
        vec![
            0u8;
            def.page_sizes
                .get(page as usize)
                .copied()
                .unwrap_or(256) as usize
        ]
    });

    let start = offset as usize;
    let end = start + raw_data.len();
    if end <= page_data.len() {
        page_data[start..end].copy_from_slice(raw_data);
    }
}
