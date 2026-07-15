//! Data logging Tauri commands.

use libretune_core::datalog::DataLogger;
use serde::Serialize;
use std::collections::HashMap;

use crate::state::AppState;

#[derive(Serialize)]
pub struct LoggingStatus {
    is_recording: bool,
    entry_count: usize,
    duration_ms: u64,
    channels: Vec<String>,
}

#[derive(Serialize)]
pub struct LogEntryData {
    timestamp_ms: u64,
    values: HashMap<String, f64>,
}

#[tauri::command]
pub async fn start_logging(
    state: tauri::State<'_, AppState>,
    sample_rate: Option<f64>,
) -> Result<(), String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let mut channels: Vec<String> = def.output_channels.keys().cloned().collect();

    // Also log the canonical alias names (RPM, MAP, TPS, …) that the realtime
    // stream adds via apply_channel_aliases, so recorded logs and saved CSVs
    // use the same channel names as the dashboards and graph pages.
    let mut probe: HashMap<String, f64> = channels.iter().map(|c| (c.clone(), 0.0)).collect();
    super::realtime_stream::apply_channel_aliases(&mut probe);
    for name in probe.keys() {
        if !channels.iter().any(|c| c == name) {
            channels.push(name.clone());
        }
    }

    let mut logger = state.data_logger.lock().await;

    // Recording appends to the current session (one continuous log until the
    // user clears it). Only build a fresh logger when there is no session yet
    // or the channel set changed (e.g. a different INI was loaded).
    let mut existing: Vec<&String> = logger.channels().iter().collect();
    let mut incoming: Vec<&String> = channels.iter().collect();
    existing.sort();
    incoming.sort();
    if existing != incoming {
        *logger = DataLogger::new(channels);
    }

    if let Some(rate) = sample_rate {
        logger.set_sample_rate(rate);
    }
    logger.start();

    Ok(())
}

#[tauri::command]
pub async fn stop_logging(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut logger = state.data_logger.lock().await;
    logger.stop();
    Ok(())
}

#[tauri::command]
pub async fn get_logging_status(
    state: tauri::State<'_, AppState>,
) -> Result<LoggingStatus, String> {
    let logger = state.data_logger.lock().await;
    Ok(LoggingStatus {
        is_recording: logger.is_recording(),
        entry_count: logger.entry_count(),
        duration_ms: logger.duration().as_millis() as u64,
        channels: logger.channels().to_vec(),
    })
}

#[tauri::command]
pub async fn get_log_entries(
    state: tauri::State<'_, AppState>,
    start_index: Option<usize>,
    count: Option<usize>,
) -> Result<Vec<LogEntryData>, String> {
    let logger = state.data_logger.lock().await;
    let channels = logger.channels();

    let start = start_index.unwrap_or(0);
    let max_count = count.unwrap_or(1000);

    let entries: Vec<LogEntryData> = logger
        .entries()
        .skip(start)
        .take(max_count)
        .map(|entry| {
            let mut values = HashMap::new();
            for (i, channel) in channels.iter().enumerate() {
                if let Some(&val) = entry.values.get(i) {
                    values.insert(channel.clone(), val);
                }
            }
            LogEntryData {
                timestamp_ms: entry.timestamp.as_millis() as u64,
                values,
            }
        })
        .collect();

    Ok(entries)
}

#[tauri::command]
pub async fn clear_log(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut logger = state.data_logger.lock().await;
    logger.clear();
    Ok(())
}

#[tauri::command]
pub async fn save_log(state: tauri::State<'_, AppState>, path: String) -> Result<(), String> {
    let logger = state.data_logger.lock().await;
    let channels = logger.channels();

    // Skip columns that are zero for the entire log: an INI defines far more
    // output channels than the ECU (or demo simulator) actually streams, and
    // those never-seen channels are logged as 0.0. Writing them out buries
    // the real data in hundreds of dead columns.
    let mut has_data = vec![false; channels.len()];
    for entry in logger.entries() {
        for (i, &val) in entry.values.iter().enumerate() {
            if val != 0.0 {
                has_data[i] = true;
            }
        }
    }

    let mut csv = String::new();
    csv.push_str("Time (ms)");
    for (i, channel) in channels.iter().enumerate() {
        if has_data[i] {
            csv.push(',');
            csv.push_str(channel);
        }
    }
    csv.push('\n');

    for entry in logger.entries() {
        csv.push_str(&format!("{}", entry.timestamp.as_millis()));
        for (i, val) in entry.values.iter().enumerate() {
            if has_data[i] {
                csv.push(',');
                csv.push_str(&format!("{:.4}", val));
            }
        }
        csv.push('\n');
    }

    std::fs::write(&path, csv).map_err(|e| format!("Failed to save log: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))
}
