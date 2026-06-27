//! Output channel info and status-bar defaults.

use crate::load_settings;
use crate::state::{AppState, StreamStats};
use serde::Serialize;

/// Output channel info returned to frontend
#[derive(Serialize, Clone)]
pub struct ChannelInfo {
    /// Channel name/identifier
    name: String,
    /// Human-readable label (if available)
    label: Option<String>,
    /// Unit of measurement
    units: String,
    /// Scale factor for display
    scale: f64,
    /// Translate offset for display  
    translate: f64,
}

/// Get all available output channels from the INI definition
#[tauri::command]
pub async fn get_available_channels(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ChannelInfo>, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let mut channels: Vec<ChannelInfo> = def
        .output_channels
        .values()
        .map(|ch| ChannelInfo {
            name: ch.name.clone(),
            label: ch.label.clone(),
            units: ch.units.clone(),
            scale: ch.scale,
            translate: ch.translate,
        })
        .collect();

    // Append user math channels
    let math_channels_guard = state.math_channels.lock().await;
    for ch in math_channels_guard.iter() {
        channels.push(ChannelInfo {
            name: ch.name.clone(),
            label: Some(ch.name.clone()),
            units: ch.units.clone(),
            scale: 1.0,
            translate: 0.0,
        });
    }

    // Sort by name for consistent ordering
    channels.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(channels)
}

/// Full output channel communication status for the diagnostics view.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OutputChannelStatusInfo {
    /// Total output channels defined in the INI
    total_channels: usize,
    /// Channels that read bytes from the OCH block (non-expression, valid offset)
    channels_consumed: usize,
    /// Channels that are computed via expressions
    channels_computed: usize,
    /// User-defined math channels
    channels_math: usize,
    /// ochBlockSize from INI protocol settings (bytes)
    och_block_size: u32,
    /// Max unused runtime range from INI (0 = disabled)
    max_unused_runtime_range: u32,
    /// Number of OCH blocks needed per read (always 1 for burst, may differ for OCH)
    och_blocks_needed: u32,
    /// Current transfer mode (Burst / OCH / Demo)
    transfer_mode: String,
    /// Human-readable reason the transfer mode was chosen
    transfer_reason: String,
    /// Stream stats
    stream: StreamStats,
    /// Estimated records per second (ticks_success / elapsed_seconds)
    records_per_second: f64,
}

/// Get comprehensive output channel communication status.
///
/// Returns structural data (INI-derived) plus live stream statistics.
#[tauri::command]
pub async fn get_output_channel_status(
    state: tauri::State<'_, AppState>,
) -> Result<OutputChannelStatusInfo, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let total_channels = def.output_channels.len();
    let och_block_size = def.protocol.och_block_size;
    let max_unused_runtime_range = def.protocol.max_unused_runtime_range;

    // Count channels that consume bytes from the OCH block vs computed channels
    let mut channels_consumed: usize = 0;
    let mut channels_computed: usize = 0;
    for ch in def.output_channels.values() {
        if ch.is_computed() {
            channels_computed += 1;
        } else {
            // Non-expression channel; check if offset fits within och_block_size
            let end = ch.offset as u32 + ch.size_bytes() as u32;
            if och_block_size == 0 || end <= och_block_size {
                channels_consumed += 1;
            }
        }
    }

    drop(def_guard);

    // Math channel count
    let math_guard = state.math_channels.lock().await;
    let channels_math = math_guard.len();
    drop(math_guard);

    // Stream stats
    let stats = state.stream_stats.lock().await;
    let stream = stats.clone();
    drop(stats);

    // Calculate records/second
    let records_per_second = if stream.started_at_ms > 0 {
        let elapsed_ms = chrono::Utc::now().timestamp_millis() - stream.started_at_ms;
        if elapsed_ms > 0 {
            (stream.ticks_success as f64) / (elapsed_ms as f64 / 1000.0)
        } else {
            0.0
        }
    } else {
        0.0
    };

    // OCH blocks needed (always 1 for current implementation)
    let och_blocks_needed = if och_block_size > 0 { 1 } else { 0 };

    Ok(OutputChannelStatusInfo {
        total_channels,
        channels_consumed,
        channels_computed,
        channels_math,
        och_block_size,
        max_unused_runtime_range,
        och_blocks_needed,
        transfer_mode: stream.transfer_mode.clone(),
        transfer_reason: stream.transfer_reason.clone(),
        stream,
        records_per_second,
    })
}

/// Get suggested status bar channels based on user settings, FrontPage, or common defaults
#[tauri::command]
pub async fn get_status_bar_defaults(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<String>, String> {
    // First check if user has saved custom status bar channels
    let settings = load_settings(&app);
    if !settings.status_bar_channels.is_empty() {
        return Ok(settings.status_bar_channels);
    }

    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // Try to get channels from FrontPage gauges first
    if let Some(fp) = &def.frontpage {
        if !fp.gauges.is_empty() {
            // Get the channel names for the first few gauges
            let mut channels = Vec::new();
            for gauge_name in fp.gauges.iter().take(4) {
                if let Some(gauge) = def.gauges.get(gauge_name) {
                    channels.push(gauge.channel.clone());
                }
            }
            if !channels.is_empty() {
                return Ok(channels);
            }
        }
    }

    // Fall back to common channel names if they exist
    let common_channels = [
        "RPM", "rpm", "AFR", "afr", "lambda", "MAP", "map", "TPS", "tps", "coolant", "CLT", "IAT",
    ];
    let mut defaults = Vec::new();
    for name in common_channels.iter() {
        if def.output_channels.contains_key(*name) && !defaults.contains(&name.to_string()) {
            defaults.push(name.to_string());
            if defaults.len() >= 4 {
                break;
            }
        }
    }

    // If still empty, just take first 4 channels
    if defaults.is_empty() {
        defaults = def.output_channels.keys().take(4).cloned().collect();
    }

    Ok(defaults)
}
