//! WASM plugin Tauri commands.
//!
//! Frontend interface to the sandboxed WebAssembly plugin runtime
//! (see `libretune_core::plugin_system`).

use crate::state::AppState;
use libretune_core::plugin_system::{
    Permission as WasmPermission, PluginConfig as WasmPluginConfig, PluginDataSnapshot,
    PluginManager as WasmPluginManager, PluginManifest as WasmPluginManifest,
    PluginProposal as WasmPluginProposal, TableSnapshot as WasmTableSnapshot,
};
use libretune_core::tune::TuneValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parse the frontend's string permission names (e.g. from consent-dialog
/// checkboxes) into typed [`WasmPermission`]s, ignoring anything unrecognized
/// rather than failing the whole load — an unrecognized name just can't be
/// granted, which is the safe default.
fn parse_permissions(names: &[String]) -> Vec<WasmPermission> {
    names
        .iter()
        .filter_map(|n| match n.as_str() {
            "ReadTables" => Some(WasmPermission::ReadTables),
            "WriteConstants" => Some(WasmPermission::WriteConstants),
            "SubscribeChannels" => Some(WasmPermission::SubscribeChannels),
            "ExecuteActions" => Some(WasmPermission::ExecuteActions),
            _ => None,
        })
        .collect()
}

/// Serializable plugin info returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub state: String,
    pub permissions: Vec<String>,
    pub exec_count: u64,
}

/// Ensure the WASM plugin manager is initialized.
fn new_plugin_manager() -> WasmPluginManager {
    WasmPluginManager::new(WasmPluginConfig {
        data_dir: String::new(),
        ecu_type: String::from("Unknown"),
        libretune_version: String::from(env!("CARGO_PKG_VERSION")),
    })
}

/// Load a WASM plugin from a .wasm file.
///
/// # Arguments
/// * `path` - Path to the .wasm plugin file
/// * `manifest_json` - JSON string with plugin manifest (the permissions the
///   plugin is *requesting*)
/// * `approved_permissions` - The permission names the user actually
///   approved (e.g. via a consent dialog listing the manifest's request).
///   Only permissions present in both this list and the manifest are
///   granted — a manifest can never self-grant a permission the user didn't
///   check.
///
/// Returns: Plugin name on success
#[tauri::command]
pub async fn load_wasm_plugin(
    path: String,
    manifest_json: String,
    approved_permissions: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let manifest: WasmPluginManifest = serde_json::from_str(&manifest_json)
        .map_err(|e| format!("Invalid plugin manifest: {}", e))?;

    let wasm_path = std::path::Path::new(&path);
    if !wasm_path.exists() {
        return Err(format!("WASM file not found: {}", path));
    }

    let approved = parse_permissions(&approved_permissions);

    let mut pm_guard = state.wasm_plugin_manager.lock().await;
    let pm = pm_guard.get_or_insert_with(new_plugin_manager);

    let name = pm.load_plugin(manifest, wasm_path, &approved)?;
    Ok(name)
}

/// Unload a WASM plugin by name.
#[tauri::command]
pub async fn unload_wasm_plugin(
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut pm_guard = state.wasm_plugin_manager.lock().await;
    let pm = pm_guard.as_mut().ok_or("Plugin manager not initialized")?;
    pm.unload_plugin(&name)
}

/// List all loaded WASM plugins with their info.
#[tauri::command]
pub async fn list_wasm_plugins(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<WasmPluginInfo>, String> {
    let pm_guard = state.wasm_plugin_manager.lock().await;

    match pm_guard.as_ref() {
        Some(pm) => {
            let list = pm.list_plugins();
            Ok(list
                .iter()
                .map(|(name, stats)| {
                    let (version, description, author, permissions) =
                        if let Some(plugin) = pm.get_plugin(name) {
                            let m = plugin.manifest();
                            (
                                m.version.clone(),
                                m.description.clone(),
                                m.author.clone(),
                                plugin
                                    .granted_permissions()
                                    .iter()
                                    .map(|p| format!("{:?}", p))
                                    .collect(),
                            )
                        } else {
                            (String::new(), String::new(), String::new(), vec![])
                        };

                    WasmPluginInfo {
                        name: name.clone(),
                        version,
                        description,
                        author,
                        state: format!("{:?}", stats.state),
                        permissions,
                        exec_count: stats.exec_count,
                    }
                })
                .collect())
        }
        None => Ok(vec![]),
    }
}

/// A constant a plugin proposed and that was actually applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedConstant {
    pub name: String,
    pub value: f64,
}

/// Result of one `execute_wasm_plugin` call, reported back to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPluginExecutionResult {
    pub exec_count: u64,
    /// The plugin's own `plugin_execute()` return value, if it exported one.
    pub result_code: Option<i32>,
    /// Constants the plugin proposed that were actually written (through the
    /// same path a user-driven edit takes).
    pub applied_constants: Vec<AppliedConstant>,
    /// Action-scripting proposals the plugin made that were *not* applied —
    /// LibreTune's action-scripting engine has no generic "run this one
    /// action now" dispatcher yet, so these are surfaced as raw JSON rather
    /// than silently dropped or half-implemented.
    pub unapplied_actions: Vec<String>,
}

/// Build a read-only snapshot of current tune/table/channel data for a
/// plugin's `execute()` call.
///
/// Table and constant reads are offline-only — sourced from the synced tune
/// cache via [`crate::get_table_data_internal`] and [`TuneFile::get_value`],
/// never a live ECU read — matching how the table editors already treat
/// synced data as authoritative, and sidestepping any risk of holding a lock
/// across ECU I/O while building this. Channel data comes from a single
/// best-effort realtime poll; if not connected, `channels` is simply empty
/// rather than failing the whole snapshot.
async fn build_plugin_data_snapshot(state: &tauri::State<'_, AppState>) -> PluginDataSnapshot {
    let table_names: Vec<String> = {
        let def_guard = state.definition.lock().await;
        def_guard
            .as_ref()
            .map(|d| d.tables.values().map(|t| t.name.clone()).collect())
            .unwrap_or_default()
    };

    let mut tables = HashMap::new();
    for name in &table_names {
        if let Ok(t) = crate::get_table_data_internal(state, name).await {
            tables.insert(
                name.clone(),
                WasmTableSnapshot {
                    x_bins: t.x_bins,
                    y_bins: t.y_bins,
                    z_values: t.z_values,
                },
            );
        }
    }

    let constant_names: Vec<String> = {
        let def_guard = state.definition.lock().await;
        def_guard
            .as_ref()
            .map(|d| d.constants.keys().cloned().collect())
            .unwrap_or_default()
    };
    let mut constants = HashMap::new();
    {
        let tune_guard = state.current_tune.lock().await;
        if let Some(tune) = tune_guard.as_ref() {
            for name in &constant_names {
                match tune.get_value(name) {
                    Some(TuneValue::Scalar(v)) => {
                        constants.insert(name.clone(), *v);
                    }
                    Some(TuneValue::Bool(b)) => {
                        constants.insert(name.clone(), if *b { 1.0 } else { 0.0 });
                    }
                    _ => {}
                }
            }
        }
    }

    let channels = crate::commands::realtime_get::get_realtime_data(state.clone())
        .await
        .unwrap_or_default();

    PluginDataSnapshot {
        tables,
        constants,
        channels,
    }
}

/// Execute a WASM plugin by name against a fresh snapshot of current
/// tune/table/channel data. Any `set_constant` proposal the plugin made
/// (only reachable if `WriteConstants` was granted at load time) is applied
/// afterward via the same [`crate::commands::constant_update::update_constant`]
/// path a user-driven edit takes; `execute_action` proposals are returned
/// unapplied.
#[tauri::command]
pub async fn execute_wasm_plugin(
    name: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<WasmPluginExecutionResult, String> {
    let snapshot = build_plugin_data_snapshot(&state).await;

    let result = {
        let mut pm_guard = state.wasm_plugin_manager.lock().await;
        let pm = pm_guard.as_mut().ok_or("Plugin manager not initialized")?;
        pm.execute_plugin(&name, snapshot)?
    };

    let mut applied_constants = Vec::new();
    let mut unapplied_actions = Vec::new();
    for proposal in result.proposals {
        match proposal {
            WasmPluginProposal::SetConstant {
                name: const_name,
                value,
            } => {
                match crate::commands::constant_update::update_constant(
                    state.clone(),
                    app.clone(),
                    const_name.clone(),
                    value,
                )
                .await
                {
                    Ok(()) => applied_constants.push(AppliedConstant {
                        name: const_name,
                        value,
                    }),
                    Err(e) => eprintln!(
                        "[WARN] plugin '{}' proposed constant '{}' = {} but the write failed: {}",
                        name, const_name, value, e
                    ),
                }
            }
            WasmPluginProposal::ExecuteAction { action_json } => {
                unapplied_actions.push(action_json);
            }
        }
    }

    Ok(WasmPluginExecutionResult {
        exec_count: result.exec_count,
        result_code: result.result_code,
        applied_constants,
        unapplied_actions,
    })
}

/// Get info about a specific WASM plugin.
#[tauri::command]
pub async fn get_wasm_plugin_info(
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<WasmPluginInfo, String> {
    let pm_guard = state.wasm_plugin_manager.lock().await;
    let pm = pm_guard.as_ref().ok_or("Plugin manager not initialized")?;

    let plugin = pm
        .get_plugin(&name)
        .ok_or_else(|| format!("Plugin '{}' not found", name))?;

    let stats = plugin.stats();
    let manifest = plugin.manifest();

    Ok(WasmPluginInfo {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        description: manifest.description.clone(),
        author: manifest.author.clone(),
        state: format!("{:?}", stats.state),
        permissions: plugin
            .granted_permissions()
            .iter()
            .map(|p| format!("{:?}", p))
            .collect(),
        exec_count: stats.exec_count,
    })
}
