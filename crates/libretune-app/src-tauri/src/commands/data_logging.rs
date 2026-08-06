//! Data logging Tauri commands.

use libretune_core::datalog::DataLogger;
use serde::Serialize;
use std::collections::HashMap;

use crate::state::AppState;

/// If `logger` is actively recording, stops it and returns `true`. Pulled
/// out of `stop_recording_on_definition_change` so the actual stop/preserve
/// behavior is unit-testable without constructing a full `AppState`.
fn stop_if_recording(logger: &mut DataLogger) -> bool {
    if logger.is_recording() {
        logger.stop();
        true
    } else {
        false
    }
}

/// Stops any in-progress recording. Call this from every place that
/// overwrites `state.definition` (reconnect to a different ECU, load a
/// different INI, toggle demo mode, open a different project). A running
/// recording's channel list was resolved against the OLD definition;
/// continuing to record against a new one can silently record all-zero
/// columns for channels that no longer exist, or worse, real values from a
/// same-named channel with different units/scale into the same CSV column
/// with no indication anything changed. Fail closed instead: stop the
/// recording (preserving what was already collected) rather than let it
/// silently keep going against data it was never validated against.
pub(crate) async fn stop_recording_on_definition_change(state: &AppState) {
    let mut logger = state.data_logger.lock().await;
    if stop_if_recording(&mut logger) {
        eprintln!(
            "[WARN] Data logging stopped: ECU definition changed mid-recording. \
             Existing entries are preserved; start a new recording to continue."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_an_active_recording() {
        let mut logger = DataLogger::new(vec!["rpm".to_string()]);
        logger.start();
        assert!(logger.is_recording());

        assert!(stop_if_recording(&mut logger));
        assert!(!logger.is_recording());
    }

    #[test]
    fn no_op_when_not_recording() {
        let mut logger = DataLogger::new(vec!["rpm".to_string()]);
        assert!(!logger.is_recording());

        assert!(!stop_if_recording(&mut logger));
        assert!(!logger.is_recording());
    }

    #[test]
    fn preserves_already_recorded_entries() {
        let mut logger = DataLogger::new(vec!["rpm".to_string()]);
        logger.start();
        logger.record(vec![1234.0]);
        assert_eq!(logger.entry_count(), 1);

        stop_if_recording(&mut logger);
        assert_eq!(logger.entry_count(), 1);
    }
}

#[derive(Serialize)]
pub struct LoggingStatus {
    is_recording: bool,
    entry_count: usize,
    duration_ms: u64,
    channels: Vec<String>,
    /// Oldest samples dropped because the in-memory buffer hit its ceiling.
    /// Nonzero means the log no longer covers the whole session (D7).
    discarded_count: u64,
    /// Path of the file the log is being streamed to (saved continuously),
    /// or null when logging only to memory.
    stream_path: Option<String>,
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

    // The log is streamed continuously to a timestamped file in the project's
    // datalogs/ folder (TunerStudio-style: saved the whole time). Grab the dir
    // before taking the logger lock to avoid holding two locks at once.
    let stream_dir = {
        let proj = state.current_project.lock().await;
        proj.as_ref().map(|p| p.path.join("datalogs"))
    };

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

    // Open a fresh timestamped file and stream to it (matches TunerStudio's
    // YYYY-MM-DD_HH.MM.SS naming). Streaming failure is non-fatal — recording
    // still works in memory and can be saved manually.
    if let Some(dir) = stream_dir {
        let name = chrono::Local::now().format("%Y-%m-%d_%H.%M.%S").to_string();
        let path = dir.join(format!("{name}.csv"));
        match logger.start_streaming(&path) {
            Ok(()) => tracing::info!("streaming datalog to {}", path.display()),
            Err(e) => tracing::warn!("could not stream datalog to {}: {e}", path.display()),
        }
    }

    // Reset the dropped-sample counter for this session and mark recording
    // active so the stream tick counts (rather than silently swallows) any
    // sample it can't hand to the logger while the lock is busy (D10).
    crate::state::LOGGER_SAMPLES_DROPPED.store(0, std::sync::atomic::Ordering::Relaxed);
    crate::state::LOGGER_RECORDING.store(true, std::sync::atomic::Ordering::Relaxed);

    Ok(())
}

#[tauri::command]
pub async fn stop_logging(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut logger = state.data_logger.lock().await;
    logger.stop();
    crate::state::LOGGER_RECORDING.store(false, std::sync::atomic::Ordering::Relaxed);
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
        discarded_count: logger.discarded_count(),
        stream_path: logger.stream_path().map(|p| p.display().to_string()),
    })
}

#[tauri::command]
pub async fn get_log_entries(
    state: tauri::State<'_, AppState>,
    start_index: Option<usize>,
    count: Option<usize>,
    channels: Option<Vec<String>>,
) -> Result<Vec<LogEntryData>, String> {
    let logger = state.data_logger.lock().await;
    let all_channels = logger.channels();

    // Only serialize the requested channels: an INI defines 1000+ output
    // channels and shipping all of them per entry over IPC at high sample
    // rates (100 Hz) is several MB per poll — enough to OOM the webview.
    let selected: Vec<(usize, &String)> = match &channels {
        Some(filter) => {
            let wanted: std::collections::HashSet<&str> =
                filter.iter().map(|s| s.as_str()).collect();
            all_channels
                .iter()
                .enumerate()
                .filter(|(_, name)| wanted.contains(name.as_str()))
                .collect()
        }
        None => all_channels.iter().enumerate().collect(),
    };

    let start = start_index.unwrap_or(0);
    let max_count = count.unwrap_or(1000);

    let entries: Vec<LogEntryData> = logger
        .entries()
        .skip(start)
        .take(max_count)
        .map(|entry| {
            let mut values = HashMap::with_capacity(selected.len());
            for (i, channel) in &selected {
                if let Some(&val) = entry.values.get(*i) {
                    values.insert((*channel).clone(), val);
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
    // Build the CSV string while holding the lock (needed to read the
    // logger), then drop it before the blocking disk write below — holding
    // it across std::fs::write stalls every other data-logging command
    // (stop/status/clear, or a UI status poll) for however long the write
    // takes.
    let csv = {
        let logger = state.data_logger.lock().await;
        let channels = logger.channels();

        // Skip columns that are zero for the entire log: an INI defines far
        // more output channels than the ECU (or demo simulator) actually
        // streams, and those never-seen channels are logged as 0.0. Writing
        // them out buries the real data in hundreds of dead columns.
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

        csv
    };

    std::fs::write(&path, csv).map_err(|e| format!("Failed to save log: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))
}

#[tauri::command]
pub async fn write_text_file(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|e| format!("Failed to write file: {}", e))
}
