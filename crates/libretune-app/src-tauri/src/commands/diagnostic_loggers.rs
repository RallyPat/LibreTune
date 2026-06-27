//! Diagnostic logger Tauri commands (tooth + composite loggers).

use serde::Serialize;
use tauri::Emitter;

use crate::state::AppState;

/// Tooth log entry (single tooth timing)
#[derive(Debug, Clone, Serialize)]
pub struct ToothLogEntry {
    /// Tooth number (0-indexed)
    tooth_number: u16,
    /// Time since last tooth in microseconds
    tooth_time_us: u32,
    /// Crank angle at this tooth (if available)
    crank_angle: Option<f32>,
}

/// Composite log entry (combined tooth + sync)
#[derive(Debug, Clone, Serialize)]
pub struct CompositeLogEntry {
    /// Time in microseconds since start
    time_us: u32,
    /// Primary trigger state (high/low)
    primary: bool,
    /// Secondary trigger state (high/low)  
    secondary: bool,
    /// Sync status
    sync: bool,
    /// Composite voltage (if analog)
    voltage: Option<f32>,
}

/// Tooth logger result
#[derive(Serialize)]
pub struct ToothLogResult {
    /// All captured tooth entries
    teeth: Vec<ToothLogEntry>,
    /// Total capture time in milliseconds
    capture_time_ms: u32,
    /// Detected RPM (if calculable)
    detected_rpm: Option<f32>,
    /// Number of teeth per revolution (if detected)
    teeth_per_rev: Option<u16>,
}

/// Composite logger result  
#[derive(Serialize)]
pub struct CompositeLogResult {
    /// All captured entries
    entries: Vec<CompositeLogEntry>,
    /// Total capture time in milliseconds
    capture_time_ms: u32,
    /// Sample rate in Hz
    sample_rate_hz: u32,
}

/// Start the tooth logger and capture data
///
/// ECU Protocol Commands:
/// - Speeduino: 'H' to get tooth log (blocking), 'T' for timing pattern, 'h' for tooth times
/// - rusEFI: 'l\x01' start tooth logger, 'l\x02' get data, 'l\x03' stop
/// - MS2/MS3: Page 0xf0-0xf1 fetch tooth log data
#[tauri::command]
pub async fn start_tooth_logger(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ToothLogResult, String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.read().await;

    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;
    let _def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // Detect ECU type from signature
    let signature = conn.signature().unwrap_or_default().to_lowercase();

    let teeth: Vec<ToothLogEntry>;

    if signature.contains("speeduino") || signature.contains("202") {
        // Speeduino protocol: Send 'H' command for tooth log
        // Response format: 2-byte count (little-endian) + (count * 4-byte entries)
        // Each entry: 2 bytes tooth number (LE) + 2 bytes time in 0.5µs units (LE)
        eprintln!("[Tooth Logger] Starting Speeduino tooth capture...");

        let response = conn
            .send_raw_bytes_with_response(b"H", std::time::Duration::from_millis(2000))
            .map_err(|e| format!("Failed to get tooth log data: {}", e))?;

        if response.len() < 2 {
            return Err("Tooth logger returned no data (ECU may not support this command)".into());
        }

        // Parse 2-byte tooth count
        let tooth_count = u16::from_le_bytes([response[0], response[1]]) as usize;
        eprintln!("[Tooth Logger] ECU reports {} teeth", tooth_count);

        let expected_len = 2 + tooth_count * 4;
        if response.len() < expected_len {
            eprintln!(
                "[Tooth Logger] Warning: expected {} bytes but got {}. Parsing available data.",
                expected_len,
                response.len()
            );
        }

        let available_teeth = (response.len().saturating_sub(2)) / 4;
        let parse_count = available_teeth.min(tooth_count);

        teeth = (0..parse_count)
            .map(|i| {
                let offset = 2 + i * 4;
                let tooth_num = u16::from_le_bytes([response[offset], response[offset + 1]]);
                // Time is in 0.5µs units, convert to µs
                let raw_time = u16::from_le_bytes([response[offset + 2], response[offset + 3]]);
                let tooth_time_us = raw_time as u32 / 2;
                ToothLogEntry {
                    tooth_number: tooth_num,
                    tooth_time_us,
                    crank_angle: None, // Speeduino doesn't provide angle in this response
                }
            })
            .collect();

        eprintln!("[Tooth Logger] Parsed {} teeth from response", teeth.len());
    } else if signature.contains("rusefi") || signature.contains("fome") {
        // rusEFI protocol: Binary commands
        // 'l\x01' = start tooth logger
        // 'l\x02' = get tooth data
        // 'l\x03' = stop tooth logger
        // Response to 'l\x02': 2-byte count (BE) + (count * 4-byte entries)
        // Each entry: 4 bytes time in µs (big-endian, u32)
        eprintln!("[Tooth Logger] Starting rusEFI tooth capture...");

        // Start logger
        conn.send_raw_bytes(&[b'l', 0x01])
            .map_err(|e| format!("Failed to start tooth logger: {}", e))?;

        // Wait for capture
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Get data
        let response = conn
            .send_raw_bytes_with_response(&[b'l', 0x02], std::time::Duration::from_millis(2000))
            .map_err(|e| format!("Failed to get tooth data: {}", e))?;

        // Stop logger
        let _ = conn.send_raw_bytes(&[b'l', 0x03]);

        if response.len() < 2 {
            return Err("Tooth logger returned no data".into());
        }

        // rusEFI uses big-endian 2-byte count
        let tooth_count = u16::from_be_bytes([response[0], response[1]]) as usize;
        eprintln!("[Tooth Logger] ECU reports {} teeth", tooth_count);

        let available_teeth = (response.len().saturating_sub(2)) / 4;
        let parse_count = available_teeth.min(tooth_count);

        teeth = (0..parse_count)
            .map(|i| {
                let offset = 2 + i * 4;
                let tooth_time_us = u32::from_be_bytes([
                    response[offset],
                    response[offset + 1],
                    response[offset + 2],
                    response[offset + 3],
                ]);
                ToothLogEntry {
                    tooth_number: i as u16,
                    tooth_time_us,
                    crank_angle: None,
                }
            })
            .collect();

        eprintln!("[Tooth Logger] Parsed {} teeth from response", teeth.len());
    } else if signature.contains("ms2") || signature.contains("ms3") || signature.contains("mega") {
        // Megasquirt protocol: Read tooth log page
        // MS2/MS3 uses page 0xF0 for tooth log data
        // Response: raw bytes, each 2-byte pair is tooth time in µs (big-endian)
        eprintln!("[Tooth Logger] Starting Megasquirt tooth capture...");

        let response = conn
            .read_page(0xF0)
            .map_err(|e| format!("Failed to read tooth log page: {}", e))?;

        if response.is_empty() {
            return Err("Tooth logger returned no data".into());
        }

        // MS tooth log: each entry is 2 bytes (big-endian), tooth time in µs
        let tooth_count = response.len() / 2;
        teeth = (0..tooth_count)
            .filter_map(|i| {
                let offset = i * 2;
                let raw_time = u16::from_be_bytes([response[offset], response[offset + 1]]);
                // Skip zero entries (unused slots)
                if raw_time == 0 {
                    return None;
                }
                Some(ToothLogEntry {
                    tooth_number: i as u16,
                    tooth_time_us: raw_time as u32,
                    crank_angle: None,
                })
            })
            .collect();

        eprintln!("[Tooth Logger] Parsed {} teeth from response", teeth.len());
    } else {
        // Unknown ECU - return placeholder indicating feature not available
        return Err(format!(
            "Tooth logger not supported for this ECU type (signature: {})",
            signature
        ));
    }

    // Calculate RPM from tooth times (if we have enough data)
    let detected_rpm = if teeth.len() >= 2 {
        let total_time: u32 = teeth.iter().map(|t| t.tooth_time_us).sum();
        let avg_tooth_time_us = total_time as f32 / teeth.len() as f32;
        // Assuming standard trigger wheel (36-1 teeth = 35 actual teeth per rev)
        let teeth_per_rev = if teeth.len() > 30 {
            36
        } else {
            teeth.len() as u16
        };
        let rev_time_us = avg_tooth_time_us * teeth_per_rev as f32;
        let rpm = 60_000_000.0 / rev_time_us;
        Some(rpm)
    } else {
        None
    };

    // Emit event to frontend
    let _ = app.emit("tooth_logger:data", &teeth);

    Ok(ToothLogResult {
        teeth,
        capture_time_ms: 500,
        detected_rpm,
        teeth_per_rev: Some(36),
    })
}

/// Stops the tooth logger capture.
///
/// Sends the appropriate stop command based on ECU type.
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn stop_tooth_logger(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;

    if let Some(conn) = conn_guard.as_mut() {
        let signature = conn.signature().unwrap_or_default().to_lowercase();

        if signature.contains("rusefi") || signature.contains("fome") {
            // rusEFI: Send stop command
            conn.send_raw_bytes(&[b'l', 0x03])
                .map_err(|e| format!("Failed to stop tooth logger: {}", e))?;
        }
        // Speeduino and MS don't need explicit stop
    }

    Ok(())
}

/// Start the composite logger and capture data
#[tauri::command]
pub async fn start_composite_logger(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<CompositeLogResult, String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.read().await;

    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;
    let _def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let signature = conn.signature().unwrap_or_default().to_lowercase();

    let entries: Vec<CompositeLogEntry>;

    if signature.contains("speeduino") || signature.contains("202") {
        // Speeduino composite logger commands:
        // 'J' = Start composite logger
        // 'O' = Get composite data
        // 'X' = Stop composite logger
        // Response to 'O': Raw bytes, each entry is 1 byte of packed flags:
        //   bit 0: primary trigger state
        //   bit 1: secondary trigger state
        //   bit 2: sync status
        // Entries are captured at ~10kHz (100µs intervals)
        eprintln!("[Composite Logger] Starting Speeduino composite capture...");

        conn.send_raw_bytes(b"J")
            .map_err(|e| format!("Failed to start composite logger: {}", e))?;

        std::thread::sleep(std::time::Duration::from_millis(500));

        let response = conn
            .send_raw_bytes_with_response(b"O", std::time::Duration::from_millis(2000))
            .map_err(|e| format!("Failed to get composite data: {}", e))?;

        if response.is_empty() {
            return Err("Composite logger returned no data".into());
        }

        // Each byte is a packed status entry at ~100µs intervals
        entries = response
            .iter()
            .enumerate()
            .map(|(i, &byte)| CompositeLogEntry {
                time_us: (i as u32) * 100, // 100µs per sample = 10kHz
                primary: (byte & 0x01) != 0,
                secondary: (byte & 0x02) != 0,
                sync: (byte & 0x04) != 0,
                voltage: None,
            })
            .collect();

        // Send stop
        let _ = conn.send_raw_bytes(b"X");

        eprintln!(
            "[Composite Logger] Parsed {} entries from response",
            entries.len()
        );
    } else if signature.contains("rusefi") || signature.contains("fome") {
        // rusEFI: 'l\x04' start, 'l\x05' get, 'l\x06' stop
        // Response to 'l\x05': 2-byte count (BE) + (count * 5-byte entries)
        // Each entry: 4 bytes time_us (BE u32) + 1 byte flags
        //   flags bit 0: primary, bit 1: secondary, bit 2: sync
        eprintln!("[Composite Logger] Starting rusEFI composite capture...");

        conn.send_raw_bytes(&[b'l', 0x04])
            .map_err(|e| format!("Failed to start composite logger: {}", e))?;

        std::thread::sleep(std::time::Duration::from_millis(500));

        let response = conn
            .send_raw_bytes_with_response(&[b'l', 0x05], std::time::Duration::from_millis(2000))
            .map_err(|e| format!("Failed to get composite data: {}", e))?;

        let _ = conn.send_raw_bytes(&[b'l', 0x06]);

        if response.len() < 2 {
            return Err("Composite logger returned no data".into());
        }

        let entry_count = u16::from_be_bytes([response[0], response[1]]) as usize;
        let available = (response.len().saturating_sub(2)) / 5;
        let parse_count = available.min(entry_count);

        entries = (0..parse_count)
            .map(|i| {
                let offset = 2 + i * 5;
                let time_us = u32::from_be_bytes([
                    response[offset],
                    response[offset + 1],
                    response[offset + 2],
                    response[offset + 3],
                ]);
                let flags = response[offset + 4];
                CompositeLogEntry {
                    time_us,
                    primary: (flags & 0x01) != 0,
                    secondary: (flags & 0x02) != 0,
                    sync: (flags & 0x04) != 0,
                    voltage: None,
                }
            })
            .collect();

        eprintln!(
            "[Composite Logger] Parsed {} entries from response",
            entries.len()
        );
    } else if signature.contains("ms2") || signature.contains("ms3") || signature.contains("mega") {
        // Megasquirt: Page 0xF2 for composite log data
        // Response: raw bytes, each entry is 6 bytes:
        //   4 bytes time_us (BE u32), 1 byte flags, 1 byte voltage (0-255 mapped to 0-5V)
        eprintln!("[Composite Logger] Starting Megasquirt composite capture...");

        let response = conn
            .read_page(0xF2)
            .map_err(|e| format!("Failed to read composite log page: {}", e))?;

        if response.is_empty() {
            return Err("Composite logger returned no data".into());
        }

        let entry_count = response.len() / 6;
        entries = (0..entry_count)
            .filter_map(|i| {
                let offset = i * 6;
                if offset + 5 >= response.len() {
                    return None;
                }
                let time_us = u32::from_be_bytes([
                    response[offset],
                    response[offset + 1],
                    response[offset + 2],
                    response[offset + 3],
                ]);
                // Skip zero-time entries (unused)
                if time_us == 0 {
                    return None;
                }
                let flags = response[offset + 4];
                let raw_voltage = response[offset + 5];
                Some(CompositeLogEntry {
                    time_us,
                    primary: (flags & 0x01) != 0,
                    secondary: (flags & 0x02) != 0,
                    sync: (flags & 0x04) != 0,
                    voltage: Some(raw_voltage as f32 * 5.0 / 255.0),
                })
            })
            .collect();

        eprintln!(
            "[Composite Logger] Parsed {} entries from response",
            entries.len()
        );
    } else {
        return Err(format!(
            "Composite logger not supported for this ECU type (signature: {})",
            signature
        ));
    }

    let _ = app.emit("composite_logger:data", &entries);

    Ok(CompositeLogResult {
        entries,
        capture_time_ms: 500,
        sample_rate_hz: 10000,
    })
}

/// Stops the composite logger capture.
///
/// Sends the appropriate stop command based on ECU type.
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn stop_composite_logger(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;

    if let Some(conn) = conn_guard.as_mut() {
        let signature = conn.signature().unwrap_or_default().to_lowercase();

        if signature.contains("rusefi") || signature.contains("fome") {
            conn.send_raw_bytes(&[b'l', 0x06])
                .map_err(|e| format!("Failed to stop composite logger: {}", e))?;
        }
    }

    Ok(())
}
