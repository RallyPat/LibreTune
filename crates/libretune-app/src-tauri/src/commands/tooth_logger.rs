//! Tooth logging driven by the INI, and for as long as the user wants.
//!
//! The previous implementation invented its own protocol. For the tooth logger
//! it sent `H` and read the reply as data — but `H` is the INI's *startCommand*,
//! so the reply is empty and the log never begins. For the composite logger it
//! sent `O`, which is not a read command at all: it is the START command of a
//! different logger, `compositeLogger2`. It then decoded whatever came back
//! against a "2-byte count, then 2-byte tooth number, then time in 0.5 µs"
//! layout that appears nowhere in any INI.
//!
//! What the INI actually declares:
//!
//! ```text
//! loggerDef = tooth, "Tooth Logger", tooth
//!    startCommand    = "H"
//!    stopCommand     = "h"
//!    dataReadCommand = "T\$tsCanId\x01\xFC\x00\x01\xFC"
//!    continuousRead  = true
//!    dataReadyCondition = { toothLog1Ready == 1 }
//!    recordDef   = 0, 0, 4
//!    recordField = toothTime, "ToothTime", 0, 32, 1.0, "uS"
//! ```
//!
//! `continuousRead` is the reason TunerStudio logs for minutes and this logged
//! for half a second: 127 is the size of the ECU's buffer, not the length of a
//! log. The firmware refills it and raises `toothLog1Ready` again, so a caller
//! that reads once mistakes one bufferful for the hardware's limit.

use crate::AppState;
use libretune_core::ini::diagnostic_logger::DiagnosticLogger;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;

/// Set while a capture is running; cleared to ask it to stop.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// One decoded record, with its fields under the names the INI gives them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggerRecord {
    /// Sequence number across the whole capture, not just this buffer.
    pub index: u64,
    /// Field name from the INI (`toothTime`, `refTime`, `priLevel`, …) to value,
    /// already scaled.
    pub fields: std::collections::HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub logger: String,
    pub records: u64,
    pub reads: u64,
    pub empty_reads: u64,
    pub running: bool,
    pub note: Option<String>,
}

/// Pick a logger by name, or the first tooth-type one.
fn choose<'a>(loggers: &'a [DiagnosticLogger], want: Option<&str>) -> Option<&'a DiagnosticLogger> {
    match want {
        Some(n) => loggers.iter().find(|l| l.name == n),
        None => loggers
            .iter()
            .find(|l| l.kind == "tooth")
            .or_else(|| loggers.first()),
    }
}

/// Start a capture and keep reading until `stop_tooth_capture` is called.
///
/// Records are emitted in batches on the `tooth-log-records` event as they
/// arrive, so a long capture does not have to be held in memory here or waited
/// on by the caller.
#[tauri::command]
pub async fn start_tooth_capture(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    logger_name: Option<String>,
    max_seconds: Option<u64>,
) -> Result<CaptureStatus, String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("A capture is already running".into());
    }
    let result = run_capture(&app, &state, logger_name, max_seconds).await;
    RUNNING.store(false, Ordering::SeqCst);
    result
}

/// Ask a running capture to finish. It stops after the read in flight.
#[tauri::command]
pub fn stop_tooth_capture() -> bool {
    RUNNING.swap(false, Ordering::SeqCst)
}

async fn run_capture(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, AppState>,
    logger_name: Option<String>,
    max_seconds: Option<u64>,
) -> Result<CaptureStatus, String> {
    let (logger, start_cmd, read_cmd, stop_cmd) = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let logger = choose(&def.diagnostic_loggers, logger_name.as_deref())
            .ok_or("This INI declares no diagnostic loggers")?
            .clone();
        if logger.data_read_command.is_empty() {
            return Err(format!(
                "'{}' declares no dataReadCommand; there is nothing to read with",
                logger.name
            ));
        }
        let vals = std::collections::HashMap::new();
        let start =
            crate::commands::tune_io::parse_command_string(def, &logger.start_command, &vals)?;
        let read =
            crate::commands::tune_io::parse_command_string(def, &logger.data_read_command, &vals)?;
        let stop =
            crate::commands::tune_io::parse_command_string(def, &logger.stop_command, &vals)?;
        (logger, start, read, stop)
    };

    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    tracing::info!(
        logger = %logger.name,
        continuous = logger.continuous_read,
        record_len = logger.record_len,
        "tooth capture starting"
    );
    conn.send_raw_bytes(&start_cmd)
        .map_err(|e| format!("Failed to start '{}': {e}", logger.name))?;

    let timeout = std::time::Duration::from_millis(logger.data_read_timeout_ms.max(500));
    let deadline =
        max_seconds.map(|s| std::time::Instant::now() + std::time::Duration::from_secs(s));
    let (mut records, mut reads, mut empty) = (0u64, 0u64, 0u64);
    let mut batch: Vec<LoggerRecord> = Vec::new();
    let mut note = None;

    loop {
        if !RUNNING.load(Ordering::SeqCst) {
            break;
        }
        if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
            note = Some(format!(
                "stopped at the {}s limit",
                max_seconds.unwrap_or(0)
            ));
            break;
        }

        let payload = match conn.send_raw_bytes_with_response(&read_cmd, timeout) {
            Ok(p) => p,
            Err(e) => {
                note = Some(format!("read failed after {reads} reads: {e}"));
                break;
            }
        };
        reads += 1;

        let n = logger.record_count(payload.len());
        if n == 0 {
            empty += 1;
            // The ECU refills between reads; an empty buffer means "not ready
            // yet", not "finished". Give it a moment rather than spinning.
            if empty > 200 {
                note = Some("no data after 200 empty reads - is the engine running?".into());
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
            continue;
        }
        empty = 0;

        for i in 0..n {
            let off = logger.header_len + i * logger.record_len;
            let rec = &payload[off..off + logger.record_len];
            batch.push(LoggerRecord {
                index: records,
                fields: logger.decode(rec).into_iter().collect(),
            });
            records += 1;
        }
        if batch.len() >= 256 {
            let _ = app.emit("tooth-log-records", &batch);
            batch.clear();
        }

        // A logger that does not refill has given everything it has.
        if !logger.continuous_read {
            note = Some("logger does not declare continuousRead; one buffer only".into());
            break;
        }
    }

    if !batch.is_empty() {
        let _ = app.emit("tooth-log-records", &batch);
    }
    if !stop_cmd.is_empty() {
        let _ = conn.send_raw_bytes(&stop_cmd);
    }
    tracing::info!(records, reads, "tooth capture finished");

    Ok(CaptureStatus {
        logger: logger.name,
        records,
        reads,
        empty_reads: empty,
        running: false,
        note,
    })
}

/// What this INI offers, so a UI can present the choice rather than guess.
#[tauri::command]
pub async fn list_diagnostic_loggers(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<DiagnosticLogger>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    Ok(def.diagnostic_loggers.clone())
}
