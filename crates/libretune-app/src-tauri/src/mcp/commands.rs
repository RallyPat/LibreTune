//! Tauri command layer for the local MCP server.
//!
//! Everything that needs a live `AppHandle` lives here, so
//! [`super::server`] and [`super::handler`] stay testable without a running
//! app.
//!
//! The MCP toggle and port are NOT routed through the generic
//! `update_setting` command: flipping them has to reconcile a real socket,
//! and that is async work the batch settings path has no way to await. The
//! Settings dialog calls these commands directly instead.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;

use libretune_core::agent::orchestrator::ReadToolExecutor;

use crate::commands::agent::LiveReadExecutor;
use crate::mcp::handler::ExecutorFactory;
use crate::mcp::server::{
    reconcile_mcp_server, regenerate_and_restart, McpServerState, MIN_MCP_PORT,
};
use crate::mcp::token::load_or_create_token;
use crate::paths::get_app_data_dir;

/// Where the bearer token lives. Same directory as `settings.json`.
fn config_dir(app: &tauri::AppHandle) -> PathBuf {
    get_app_data_dir(app)
}

/// Build the per-session executor factory.
///
/// `unit_prefs: None` on purpose — an external agent gets raw ECU units, not
/// whatever the user happens to have the UI set to. Display conversion is a
/// human-facing concern, and a silently °F-converted coolant reading would
/// be worse than useless to a model doing arithmetic on it.
fn executor_factory(app: &tauri::AppHandle) -> ExecutorFactory {
    let app = app.clone();
    Arc::new(move || {
        Arc::new(LiveReadExecutor::new(app.clone(), None)) as Arc<dyn ReadToolExecutor>
    })
}

/// Server status for the Settings UI. `port` is the *bound* port while
/// running (meaningful even when the configured port was 0) and 0 when
/// stopped.
#[derive(Debug, Clone, Serialize)]
pub struct McpStatus {
    pub running: bool,
    pub port: u16,
}

fn status_of(state: &McpServerState) -> McpStatus {
    match state.local_addr() {
        Some(addr) => McpStatus {
            running: true,
            port: addr.port(),
        },
        None => McpStatus {
            running: false,
            port: 0,
        },
    }
}

#[tauri::command]
pub async fn mcp_status(state: tauri::State<'_, McpServerState>) -> Result<McpStatus, String> {
    Ok(status_of(&state))
}

/// Enable or disable the server, persisting the choice and reconciling the
/// socket in one step.
///
/// A failed reconcile (a taken port, most often) rolls the persisted setting
/// back — otherwise the app would keep believing MCP is on, retry the same
/// doomed bind on every launch, and show a checkbox that disagrees with the
/// status line beside it.
#[tauri::command]
pub async fn mcp_set_enabled(
    app: tauri::AppHandle,
    state: tauri::State<'_, McpServerState>,
    enabled: bool,
) -> Result<McpStatus, String> {
    let (previous, port) = crate::with_settings(&app, |s| {
        let previous = s.mcp_enabled;
        s.mcp_enabled = enabled;
        (previous, s.mcp_port)
    });

    if let Err(e) = reconcile_mcp_server(
        &state,
        executor_factory(&app),
        config_dir(&app),
        enabled,
        port,
    )
    .await
    {
        crate::with_settings(&app, |s| s.mcp_enabled = previous);
        return Err(e);
    }
    Ok(status_of(&state))
}

/// Change the listening port. Restarts the server when it is running, and
/// rolls the setting back if the new port cannot be bound.
#[tauri::command]
pub async fn mcp_set_port(
    app: tauri::AppHandle,
    state: tauri::State<'_, McpServerState>,
    port: u16,
) -> Result<McpStatus, String> {
    if port < MIN_MCP_PORT {
        return Err(format!("Port must be {MIN_MCP_PORT} or higher"));
    }
    let (previous, enabled) = crate::with_settings(&app, |s| {
        let previous = s.mcp_port;
        s.mcp_port = port;
        (previous, s.mcp_enabled)
    });

    if let Err(e) = reconcile_mcp_server(
        &state,
        executor_factory(&app),
        config_dir(&app),
        enabled,
        port,
    )
    .await
    {
        crate::with_settings(&app, |s| s.mcp_port = previous);
        return Err(e);
    }
    Ok(status_of(&state))
}

/// The current bearer token, minting one on first read.
#[tauri::command]
pub async fn mcp_get_token(app: tauri::AppHandle) -> Result<String, String> {
    load_or_create_token(&config_dir(&app))
}

/// Mint a fresh token, invalidating the old one immediately.
#[tauri::command]
pub async fn mcp_regenerate_token(
    app: tauri::AppHandle,
    state: tauri::State<'_, McpServerState>,
) -> Result<String, String> {
    regenerate_and_restart(&state, executor_factory(&app), config_dir(&app)).await
}

/// Start the server at launch when it was left enabled. A no-op — not an
/// error — when disabled, which is every default install.
pub async fn start_on_boot(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    let settings = crate::load_settings(app);
    if !settings.mcp_enabled {
        return Ok(());
    }
    let state = app.state::<McpServerState>();
    reconcile_mcp_server(
        &state,
        executor_factory(app),
        config_dir(app),
        true,
        settings.mcp_port,
    )
    .await
}
