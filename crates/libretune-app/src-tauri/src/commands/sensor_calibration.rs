//! Sensor calibration write commands (Speeduino calibration space).
//!
//! These drive the dedicated `t` calibration command implemented in
//! `libretune_core::protocol::calibration` — the CLT/IAT thermistor curves
//! and the O2/AFR transfer table live outside the tune pages, so ordinary
//! page writes can never reach them.

use crate::state::AppState;
use libretune_core::protocol::calibration::{
    temperature_calibration_bins, O2_CALIBRATION_WIRE_BYTES, TEMP_CALIBRATION_POINTS,
};
use libretune_core::protocol::CalibrationTable;

fn parse_sensor(sensor: &str) -> Result<CalibrationTable, String> {
    match sensor.to_ascii_lowercase().as_str() {
        "clt" => Ok(CalibrationTable::Clt),
        "iat" => Ok(CalibrationTable::Iat),
        other => Err(format!(
            "unknown temperature sensor '{}' (expected 'clt' or 'iat')",
            other
        )),
    }
}

/// The ADC bins the connected ECU will assign to a 32-point temperature
/// calibration, so the frontend can sample its sensor curve at exactly the
/// points the firmware will use. The two protocol paths differ (legacy x*32,
/// CRC x*33); disconnected callers get the legacy bins for preview purposes.
#[derive(serde::Serialize)]
pub struct CalibrationBinsInfo {
    pub bins: Vec<u16>,
    pub modern_protocol: bool,
    pub connected: bool,
}

#[tauri::command]
pub async fn get_temperature_calibration_bins(
    state: tauri::State<'_, AppState>,
) -> Result<CalibrationBinsInfo, String> {
    let conn_guard = state.connection.lock().await;
    let modern = conn_guard
        .as_ref()
        .map(|c| c.is_modern_protocol())
        .unwrap_or(false);
    Ok(CalibrationBinsInfo {
        bins: temperature_calibration_bins(modern).to_vec(),
        modern_protocol: modern,
        connected: conn_guard.is_some(),
    })
}

/// Write a 32-point CLT or IAT calibration curve to the ECU.
///
/// `temps_c` are °C at the bins reported by
/// [`get_temperature_calibration_bins`].
#[tauri::command]
pub async fn write_temperature_calibration(
    state: tauri::State<'_, AppState>,
    sensor: String,
    temps_c: Vec<f64>,
) -> Result<(), String> {
    let table = parse_sensor(&sensor)?;
    let temps: [f64; TEMP_CALIBRATION_POINTS] = temps_c.try_into().map_err(|v: Vec<f64>| {
        format!(
            "expected {} temperatures, got {}",
            TEMP_CALIBRATION_POINTS,
            v.len()
        )
    })?;

    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;
    conn.write_temperature_calibration(table, &temps)
        .map_err(|e| e.to_string())?;
    tracing::info!("sensor calibration written: {} (32 points)", sensor);
    Ok(())
}

/// Write the 1024-entry O2/AFR transfer curve (AFR per 10-bit ADC count).
#[tauri::command]
pub async fn write_afr_calibration(
    state: tauri::State<'_, AppState>,
    afr_values: Vec<f64>,
) -> Result<(), String> {
    let afr: Box<[f64; O2_CALIBRATION_WIRE_BYTES]> = afr_values
        .into_boxed_slice()
        .try_into()
        .map_err(|v: Box<[f64]>| {
            format!(
                "expected {} AFR values, got {}",
                O2_CALIBRATION_WIRE_BYTES,
                v.len()
            )
        })?;

    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;
    conn.write_o2_calibration(&afr).map_err(|e| e.to_string())?;
    tracing::info!("AFR sensor calibration written (1024 points)");
    Ok(())
}
