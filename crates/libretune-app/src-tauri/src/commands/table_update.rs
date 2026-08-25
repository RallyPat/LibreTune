//! Table z-values update command.

use crate::AppState;

#[tauri::command]
pub async fn update_table_data(
    state: tauri::State<'_, AppState>,
    table_name: String,
    z_values: Vec<Vec<f64>>,
) -> Result<(), String> {
    // Snapshot only what we need from the definition, then drop the lock
    // before doing any ECU I/O below — holding it across a blocking
    // conn.write_memory() call starves every other command that needs the
    // definition. This is the primary table-cell-edit command (the main
    // table editor calls it on every cell edit), so it's likely the single
    // most frequently invoked path that had this bug.
    let (constant, endianness, default_page_bytes) = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;

        let table = def
            .get_table_by_name_or_map(&table_name)
            .ok_or_else(|| format!("Table {} not found", table_name))?;

        let constant = def
            .constants
            .get(&table.map)
            .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?
            .clone();

        let default_page_bytes = def
            .page_sizes
            .get(constant.page as usize)
            .copied()
            .unwrap_or(256) as usize;

        (constant, def.endianness, default_page_bytes)
    };

    // Flatten z_values
    let flat_values: Vec<f64> = z_values.into_iter().flatten().collect();

    if flat_values.len() != constant.shape.element_count() {
        return Err(format!(
            "Invalid data size: expected {}, got {}",
            constant.shape.element_count(),
            flat_values.len()
        ));
    }

    // Convert display values to raw bytes
    let element_size = constant.data_type.size_bytes();
    let mut raw_data = vec![0u8; constant.size_bytes()];

    for (i, val) in flat_values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant
            .data_type
            .write_to_bytes(&mut raw_data, offset, raw_val, endianness);
    }

    let mut conn_guard = state.connection.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;

    // Always write to TuneCache if available (enables offline editing)
    if let Some(cache) = cache_guard.as_mut() {
        crate::commands::table_internals::mirror_write_into_tune(
            &state,
            cache,
            &constant,
            &raw_data,
            default_page_bytes,
            &flat_values,
        )
        .await;
    }

    // Write to ECU if connected (optional - offline mode works without this)
    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data.clone(),
        };

        // Read the table straight back. A legacy-protocol write is
        // fire-and-forget, so "the write returned Ok" only ever meant "the
        // bytes left the host" — which is how a corrupted ignition table
        // reached a running engine on 18 Aug 2026 with both writes reported
        // successful. Chunking stops that particular overrun; the read-back is
        // what turns the next silent divergence into a visible error.
        match conn.write_memory_verified(params) {
            Ok(()) => {}
            // The ECU is holding something other than what was sent. This is
            // not a connectivity problem and must not be swallowed: the tune
            // on screen no longer matches the tune the engine is running.
            Err(e @ libretune_core::protocol::ProtocolError::WriteVerificationFailed { .. }) => {
                return Err(format!(
                    "ECU did not store table {table_name} as written — do not \
                     drive on this tune until it is re-sent and verified: {e}"
                ));
            }
            // The write went out but could not be read back. On a legacy ECU
            // this is the buffer-overrun signature itself (the read command
            // was eaten as table data), so it must fail as loudly as a
            // mismatch — this exact case was once a WARN line while a
            // corrupted ignition table sat in a running engine.
            Err(
                e @ libretune_core::protocol::ProtocolError::WriteVerificationUnavailable { .. },
            ) => {
                return Err(format!(
                    "Table {table_name} was sent but the ECU's copy could not \
                     be confirmed — re-send it before trusting or burning \
                     this tune: {e}"
                ));
            }
            // A write that never went out (not connected, port gone) keeps
            // the existing offline-tolerant behaviour: the edit is already in
            // the local tune cache above.
            Err(e) => {
                eprintln!("[WARN] Failed to write to ECU (offline mode?): {}", e);
            }
        }
    }

    Ok(())
}
