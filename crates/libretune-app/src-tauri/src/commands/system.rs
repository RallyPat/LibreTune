//! System / environment Tauri commands.
//!
//! Exposes lightweight info commands like build version and serial port
//! enumeration that don't fit any specific domain.

use libretune_core::protocol::serial::{list_ports, list_ports_unprobed};
use serde::Serialize;

#[derive(Serialize)]
pub struct BuildInfo {
    pub version: String,
    pub build_id: String,
}

/// Get application build information (version + nightly build ID).
#[tauri::command]
pub fn get_build_info(app: tauri::AppHandle) -> BuildInfo {
    let version = app.package_info().version.to_string();
    let build_id = option_env!("LIBRETUNE_BUILD_ID")
        .unwrap_or("unknown")
        .to_string();
    BuildInfo { version, build_id }
}

/// Lists serial ports. `probe` (default true) open-checks Windows ghost COMs
/// for the picker; auto-connect passes false so a new port is seen immediately.
#[tauri::command]
pub async fn get_serial_ports(probe: Option<bool>) -> Result<Vec<String>, String> {
    let ports = if probe.unwrap_or(true) {
        list_ports()
    } else {
        list_ports_unprobed()
    };
    Ok(ports.into_iter().map(|p| p.name).collect())
}
