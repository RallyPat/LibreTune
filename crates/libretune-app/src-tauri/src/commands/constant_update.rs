//! Update constant value command.

use crate::AppState;

#[tauri::command]
pub async fn update_constant(
    state: tauri::State<'_, AppState>,
    name: String,
    value: f64,
) -> Result<(), String> {
    // Snapshot only what we need from the definition, then drop the lock before
    // doing any ECU I/O below. Holding `state.definition` across a blocking
    // conn.read_memory()/write_memory() call starves every other command that
    // needs the definition (e.g. load_tune) if the ECU I/O stalls.
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let constant = def
        .constants
        .get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?
        .clone();
    let endianness = def.endianness;
    let default_page_bytes = def
        .page_sizes
        .get(constant.page as usize)
        .copied()
        .unwrap_or(256) as usize;
    drop(def_guard);

    // Block assigning a pin that another output already uses (rusEFI Settings Error).
    if constant.data_type == libretune_core::ini::DataType::Bits {
        crate::commands::pin_conflicts::deny_if_pin_conflict(&state, &name, value).await?;
    }

    let mut conn_guard = state.connection.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;

    // PC variables are stored locally, not on ECU
    if constant.is_pc_variable {
        if let Some(cache) = cache_guard.as_mut() {
            cache.local_values.insert(name.clone(), value);
        }
        // Also update tune.constants for consistency
        let mut tune_guard = state.current_tune.lock().await;
        if let Some(tune) = tune_guard.as_mut() {
            tune.constants
                .insert(name, libretune_core::tune::TuneValue::Scalar(value));
        }
        return Ok(());
    }

    // Handle bits constants specially (they're packed, size_bytes() == 0)
    if constant.data_type == libretune_core::ini::DataType::Bits {
        let bit_pos = constant.bit_position.unwrap_or(0);
        let bit_size = constant.bit_size.unwrap_or(1);

        // Calculate which byte contains the bits and the bit position within that byte
        let byte_offset = (bit_pos / 8) as u16;
        let bit_in_byte = bit_pos % 8;

        // Calculate how many bytes we need to read/write (may span multiple bytes)
        let bits_remaining_after_first_byte = bit_size.saturating_sub(8 - bit_in_byte);
        let bytes_needed: usize = if bits_remaining_after_first_byte > 0 {
            (1 + bits_remaining_after_first_byte.div_ceil(8)) as usize
        } else {
            1
        };

        let read_offset = constant.offset + byte_offset;
        let new_bit_val = value as u32;

        // Read existing bytes from cache or ECU
        let mut existing_bytes = vec![0u8; bytes_needed];
        if let Some(cache) = cache_guard.as_ref() {
            if let Some(bytes) = cache.read_bytes(constant.page, read_offset, bytes_needed as u16) {
                existing_bytes.copy_from_slice(bytes);
            }
        } else if let Some(conn) = conn_guard.as_mut() {
            let params = libretune_core::protocol::commands::ReadMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: read_offset,
                length: bytes_needed as u16,
            };
            if let Ok(bytes) = conn.read_memory(params) {
                let copy_len = bytes.len().min(existing_bytes.len());
                existing_bytes[..copy_len].copy_from_slice(&bytes[..copy_len]);
            }
        }

        // Apply the new bit value using masks
        // For single-byte case (most common for flags like [1:1])
        if bytes_needed == 1 {
            let mask = if bit_size >= 8 {
                0xFF
            } else {
                ((1u8 << bit_size) - 1) << bit_in_byte
            };
            existing_bytes[0] =
                (existing_bytes[0] & !mask) | (((new_bit_val as u8) << bit_in_byte) & mask);
        } else {
            // Multi-byte case: apply bits across multiple bytes
            let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
            let mask_first = if bits_in_first_byte >= 8 {
                0xFF
            } else {
                ((1u8 << bits_in_first_byte) - 1) << bit_in_byte
            };
            let val_first = ((new_bit_val as u8) << bit_in_byte) & mask_first;
            existing_bytes[0] = (existing_bytes[0] & !mask_first) | val_first;

            let mut bits_written = bits_in_first_byte;
            for byte in existing_bytes.iter_mut().skip(1) {
                let remaining_bits = bit_size - bits_written;
                if remaining_bits == 0 {
                    break;
                }
                let bits_for_this_byte = remaining_bits.min(8);
                let mask = if bits_for_this_byte >= 8 {
                    0xFF
                } else {
                    (1u8 << bits_for_this_byte) - 1
                };
                let val_for_byte = ((new_bit_val >> bits_written) as u8) & mask;
                *byte = (*byte & !mask) | val_for_byte;
                bits_written += bits_for_this_byte;
            }
        }

        // Write modified bytes to cache
        if let Some(cache) = cache_guard.as_mut() {
            cache.write_bytes(constant.page, read_offset, &existing_bytes);
        }

        // Update TuneFile in memory (both pages and constants)
        let mut tune_guard = state.current_tune.lock().await;
        if let Some(tune) = tune_guard.as_mut() {
            // Update page data
            let page_data = tune
                .pages
                .entry(constant.page)
                .or_insert_with(|| vec![0u8; default_page_bytes]);
            let start = read_offset as usize;
            let end = start + existing_bytes.len();
            if end <= page_data.len() {
                page_data[start..end].copy_from_slice(&existing_bytes);
            }

            // Update constants HashMap for offline reads
            tune.constants
                .insert(name.clone(), libretune_core::tune::TuneValue::Scalar(value));
        }

        // Mark tune as modified
        *state.tune_modified.lock().await = true;

        // Write to ECU if connected
        if let Some(conn) = conn_guard.as_mut() {
            let params = libretune_core::protocol::commands::WriteMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: read_offset,
                data: existing_bytes,
            };
            if let Err(e) = conn.write_memory(params) {
                eprintln!("[WARN] Failed to write bits constant to ECU: {}", e);
            }
        }

        eprintln!(
            "[DEBUG] update_constant: Updated bits constant '{}' to value {}",
            name, value
        );
        return Ok(());
    }

    // Convert display value to raw bytes (for non-bits constants)
    let raw_val = constant.display_to_raw(value);
    let mut raw_data = vec![0u8; constant.size_bytes()];
    constant
        .data_type
        .write_to_bytes(&mut raw_data, 0, raw_val, endianness);

    // Always write to TuneCache if available (enables offline editing)
    if let Some(cache) = cache_guard.as_mut() {
        if cache.write_bytes(constant.page, constant.offset, &raw_data) {
            // Also update TuneFile in memory
            let mut tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_mut() {
                // Get or create page data
                let page_data = tune
                    .pages
                    .entry(constant.page)
                    .or_insert_with(|| vec![0u8; default_page_bytes]);

                // Update the page data
                let start = constant.offset as usize;
                let end = start + raw_data.len();
                if end <= page_data.len() {
                    page_data[start..end].copy_from_slice(&raw_data);
                }

                // Update constants HashMap for offline reads
                tune.constants
                    .insert(name.clone(), libretune_core::tune::TuneValue::Scalar(value));
            }

            // Mark tune as modified
            *state.tune_modified.lock().await = true;
        }
    }

    // Write to ECU if connected (optional - offline mode works without this)
    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data.clone(),
        };

        // Don't fail if ECU write fails - offline mode should still work
        if let Err(e) = conn.write_memory(params) {
            eprintln!("[WARN] Failed to write to ECU (offline mode?): {}", e);
        }
    }

    Ok(())
}
