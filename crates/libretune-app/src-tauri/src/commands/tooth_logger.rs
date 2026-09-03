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
//!
//! # This perturbs a running engine
//!
//! Tested on a Speeduino 202501 / ATmega2560: starting the tooth logger makes a
//! running engine misfire. The cost is in the firmware, not the host - `H` puts
//! it into logging mode and it then writes a record inside the trigger ISR on
//! every tooth, which delays the ignition and injection scheduling that shares
//! that path. Pacing the host reads was tried and does not help, because the
//! work happens whether anything reads or not.
//!
//! So this is a stationary diagnostic. It is genuinely useful for checking
//! trigger patterns, decoder setup and missing-tooth alignment while cranking
//! or at a steady idle, and it should not be used to chase a fault that only
//! appears under load - it will add a misfire to whatever is being hunted.
//! `start_tooth_capture` refuses above a conservative rpm for that reason.

use crate::AppState;
use libretune_core::ini::diagnostic_logger::DiagnosticLogger;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;

/// Above this the logger is refused ON A LEGACY CONNECTION ONLY.
///
/// The misfire this guarded against was never the per-tooth ISR work - a bench
/// sweep held sync to 6,061 rpm with zero losses. It was the legacy transmit:
/// an unframed `T` reaches `sendToothLog_legacy`, which the firmware itself
/// marks `/* Blocking */` and which stalls the main loop for a measured 45 ms
/// per read - and ignition is scheduled from that loop. The CRC protocol's
/// framed path yields to the main loop every four bytes
/// (`SERIAL_TRANSMIT_TOOTH_INPROGRESS`), so it has no such stall and gets no
/// RPM limit, same as the reference tuning software.
const MAX_SAFE_RPM_LEGACY: f64 = 1500.0;

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

    // Refuse on a spinning engine. Logging is done inside the trigger ISR, so
    // it delays spark and injection scheduling - measured as misfiring on a
    // running car. Cranking and idle are fine and are where this is useful.
    {
        let rt = crate::commands::realtime_get::get_realtime_data(state.clone())
            .await
            .unwrap_or_default();
        let rpm = rt.get("rpm").copied().unwrap_or(0.0);
        let modern = {
            let conn_guard = state.connection.lock().await;
            conn_guard.as_ref().is_some_and(|c| c.is_modern_protocol())
        };
        if !modern && rpm > MAX_SAFE_RPM_LEGACY {
            return Err(format!(
                "Refusing to start the tooth logger at {rpm:.0} rpm on the legacy                  protocol: its blocking transmit stalls the ECU main loop ~45 ms                  per read, and ignition is scheduled from that loop - a measured                  misfire mechanism. Reconnect with the CRC protocol to log at any                  RPM, or stay below {MAX_SAFE_RPM_LEGACY:.0} rpm on legacy."
            ));
        }
    }

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
    // PACE THE READS. Without this the loop re-reads as fast as the serial
    // round-trip allows, which on a 16 MHz ATmega2560 starves the main loop
    // that schedules ignition and injection - observed as misfiring on a
    // running engine, which is not an acceptable cost for a diagnostic.
    //
    // The buffer holds `record_len`-sized records and fills at the tooth rate;
    // at idle that is about two seconds. Reading several times a second gains
    // nothing and costs the ECU real time, so wait between reads and let the
    // buffer do its job.
    let min_interval = std::time::Duration::from_millis(250);
    let mut last_read = std::time::Instant::now() - min_interval;
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

        let since = last_read.elapsed();
        if since < min_interval {
            std::thread::sleep(min_interval - since);
        }
        last_read = std::time::Instant::now();

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
            continue;
        }
        empty = 0;

        let mut real = 0usize;
        for i in 0..n {
            let off = logger.header_len + i * logger.record_len;
            let rec = &payload[off..off + logger.record_len];
            // A zero-filled record is the firmware saying "nothing logged",
            // not a tooth that took no time. Keeping them reports thousands of
            // captured teeth from a stationary engine.
            if logger.is_empty_record(rec) {
                continue;
            }
            batch.push(LoggerRecord {
                index: records,
                fields: logger.decode(rec).into_iter().collect(),
            });
            records += 1;
            real += 1;
        }
        if real == 0 {
            // A full buffer of zeros counts as not-ready, same as a short read.
            empty += 1;
            if empty > 200 {
                note = Some(
                    "buffer keeps coming back empty - the crank must be turning                      for the logger to record anything"
                        .into(),
                );
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
            continue;
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
