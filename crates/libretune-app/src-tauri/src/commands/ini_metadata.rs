//! INI / protocol metadata Tauri commands.

use libretune_core::ini::{IniCapabilities, VeAnalyzeConfig};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct ProtocolDefaults {
    pub default_baud_rate: u32,
    pub inter_write_delay: u32,
    pub delay_after_port_open: u32,
    pub message_envelope_format: Option<String>,
    pub page_activation_delay: u32,
    /// Suggested read timeout for UI (ms)
    pub timeout_ms: u32,
}

/// Get protocol timing defaults from the loaded INI definition.
#[tauri::command]
pub async fn get_protocol_defaults(
    state: tauri::State<'_, AppState>,
) -> Result<ProtocolDefaults, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let proto = def.protocol.clone();
    Ok(ProtocolDefaults {
        default_baud_rate: proto.default_baud_rate,
        inter_write_delay: proto.inter_write_delay,
        delay_after_port_open: proto.delay_after_port_open,
        message_envelope_format: proto.message_envelope_format.clone(),
        page_activation_delay: proto.page_activation_delay,
        timeout_ms: proto.block_read_timeout,
    })
}

#[derive(Serialize)]
pub struct ProtocolCapabilities {
    pub supports_och: bool,
}

/// Return derived protocol capabilities from the loaded INI definition.
#[tauri::command]
pub async fn get_protocol_capabilities(
    state: tauri::State<'_, AppState>,
) -> Result<ProtocolCapabilities, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let proto = &def.protocol;
    Ok(ProtocolCapabilities {
        supports_och: proto.och_get_command.is_some() && proto.och_block_size > 0,
    })
}

/// Return the parsed [VeAnalyze] configuration if present.
#[tauri::command]
pub async fn get_ve_analyze_config(
    state: tauri::State<'_, AppState>,
) -> Result<Option<VeAnalyzeConfig>, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    Ok(def.ve_analyze.clone())
}

/// Return INI-derived feature capabilities for UI gating.
#[tauri::command]
pub async fn get_ini_capabilities(
    state: tauri::State<'_, AppState>,
) -> Result<IniCapabilities, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    Ok(def.capabilities())
}
