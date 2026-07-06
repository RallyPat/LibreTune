//! Single-shot realtime data fetch command.

use crate::commands::realtime_stream::apply_channel_aliases;
use crate::state::AppState;
use std::collections::HashMap;
use std::sync::Arc;

/// Polls the ECU for current sensor values and computed channels.
/// Used for gauges, status bar, and table highlighting.
///
/// Returns: HashMap of channel names to current values
#[tauri::command]
pub async fn get_realtime_data(
    state: tauri::State<'_, AppState>,
) -> Result<HashMap<String, f64>, String> {
    // Use cached output channels to avoid expensive cloning.
    // IMPORTANT: acquire each lock independently to avoid deadlocks.
    let (channels_arc, endianness) = {
        let cached: Option<Arc<HashMap<String, libretune_core::ini::OutputChannel>>>;
        {
            let channels_cache_guard = state.cached_output_channels.lock().await;
            cached = channels_cache_guard.as_ref().map(Arc::clone);
        } // cached_output_channels lock released

        let def_guard = state.definition.lock().await;
        if let Some(channels) = cached {
            let endianness = def_guard
                .as_ref()
                .map(|d| d.endianness)
                .unwrap_or(libretune_core::ini::Endianness::Little);
            (channels, endianness)
        } else if let Some(def) = &*def_guard {
            (Arc::new(def.output_channels.clone()), def.endianness)
        } else {
            return Err("Connection or definition missing".to_string());
        }
    };

    // Now lock connection only for I/O
    let raw_data = {
        let mut conn_guard = state.connection.lock().await;
        let conn = match conn_guard.as_mut() {
            Some(c) => c,
            None => return Err("Connection or definition missing".to_string()),
        };
        conn.get_realtime_data().map_err(|e| e.to_string())?
    };

    // Use Evaluator if available, otherwise fallback (should exist if INI loaded)
    let evaluator_guard = state.evaluator.lock().await;

    let data = if let Some(evaluator) = &*evaluator_guard {
        let def_guard = state.definition.lock().await;
        if let Some(def) = &*def_guard {
            evaluator.process(&raw_data, def)
        } else {
            // Fallback if definition locking fails
            return Err("Definition missing during evaluation".to_string());
        }
    } else {
        // Fallback: Manual parsing (basic channels only) if Evaluator not available
        let mut results = HashMap::new();

        // First pass: Parse all raw channels
        for (name, channel) in channels_arc.iter() {
            if !channel.is_computed() {
                if let Some(val) = channel.parse(&raw_data, endianness) {
                    results.insert(name.clone(), val);
                }
            }
        }

        results
    };

    let mut data = data;
    apply_channel_aliases(&mut data);

    Ok(data)
}
