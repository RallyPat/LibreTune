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
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let channels: Vec<String> = def.output_channels.keys().cloned().collect();

    let mut logger = state.data_logger.lock().await;
    *logger = DataLogger::new(channels);
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

    let mut csv = String::new();
    csv.push_str("Time (ms)");
    for channel in logger.channels() {
        csv.push(',');
        csv.push_str(channel);
    }
    csv.push('\n');

    for entry in logger.entries() {
        csv.push_str(&format!("{}", entry.timestamp.as_millis()));
        for val in &entry.values {
            csv.push(',');
            csv.push_str(&format!("{:.4}", val));
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
