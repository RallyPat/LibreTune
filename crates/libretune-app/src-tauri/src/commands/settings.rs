//! Application settings commands.

use crate::{load_settings, save_settings, Settings};
use tauri::Emitter;

/// Get application settings
#[tauri::command]
pub async fn get_settings(app: tauri::AppHandle) -> Result<Settings, String> {
    Ok(load_settings(&app))
}

/// Update a single setting
#[tauri::command]
pub async fn update_setting(
    app: tauri::AppHandle,
    key: String,
    value: String,
) -> Result<(), String> {
    let mut settings = load_settings(&app);

    match key.as_str() {
        "units_system" => settings.units_system = value,
        "auto_burn_on_close" => {
            settings.auto_burn_on_close = value.parse().map_err(|_| "Invalid boolean value")?
        }
        "gauge_snap_to_grid" => {
            settings.gauge_snap_to_grid = value.parse().map_err(|_| "Invalid boolean value")?
        }
        "gauge_free_move" => {
            settings.gauge_free_move = value.parse().map_err(|_| "Invalid boolean value")?
        }
        "gauge_lock" => settings.gauge_lock = value.parse().map_err(|_| "Invalid boolean value")?,
        "auto_sync_gauge_ranges" => {
            settings.auto_sync_gauge_ranges = value.parse().map_err(|_| "Invalid boolean value")?
        }
        "indicator_column_count" => settings.indicator_column_count = value,
        "indicator_fill_empty" => {
            settings.indicator_fill_empty = value.parse().map_err(|_| "Invalid boolean value")?
        }
        "indicator_text_fit" => settings.indicator_text_fit = value,
        // Status bar channels (JSON array)
        "status_bar_channels" => {
            settings.status_bar_channels = serde_json::from_str(&value)
                .map_err(|e| format!("Invalid JSON for status_bar_channels: {}", e))?
        }
        // Heatmap scheme settings
        "heatmap_value_scheme" => settings.heatmap_value_scheme = value,
        "heatmap_change_scheme" => settings.heatmap_change_scheme = value,
        "heatmap_coverage_scheme" => settings.heatmap_coverage_scheme = value,
        // Help icon visibility
        "show_all_help_icons" => {
            settings.show_all_help_icons = value.parse().map_err(|_| "Invalid boolean value")?
        }
        // Alert rules settings
        "alert_large_change_enabled" => {
            settings.alert_large_change_enabled =
                value.parse().map_err(|_| "Invalid boolean value")?
        }
        "alert_large_change_abs" => {
            settings.alert_large_change_abs = value.parse().map_err(|_| "Invalid number value")?
        }
        "alert_large_change_percent" => {
            settings.alert_large_change_percent =
                value.parse().map_err(|_| "Invalid number value")?
        }
        "runtime_packet_mode" => {
            settings.runtime_packet_mode = value;
        }
        "last_serial_port" => {
            settings.last_serial_port = if value.is_empty() { None } else { Some(value) }
        }
        "auto_reconnect_after_controller_command" => {
            settings.auto_reconnect_after_controller_command =
                value.parse().map_err(|_| "Invalid boolean value")?
        }
        "auto_reconnect_after_firmware" => {
            settings.auto_reconnect_after_firmware =
                value.parse().map_err(|_| "Invalid boolean value")?
        }
        "onboarding_completed" => {
            settings.onboarding_completed = value.parse().map_err(|_| "Invalid boolean value")?
        }
        // Session persistence
        "last_project_path" => {
            settings.last_project_path = if value.is_empty() { None } else { Some(value) }
        }
        "last_active_tab" => {
            settings.last_active_tab = if value.is_empty() { None } else { Some(value) }
        }
        "language" => settings.language = if value.is_empty() { None } else { Some(value) },
        _ => return Err(format!("Unknown setting: {}", key)),
    }

    save_settings(&app, &settings);
    let _ = app.emit("settings:changed", key.clone());
    Ok(())
}

/// Update custom heatmap color stops for a context
#[tauri::command]
pub async fn update_heatmap_custom_stops(
    app: tauri::AppHandle,
    context: String,
    stops: Vec<String>,
) -> Result<(), String> {
    let mut settings = load_settings(&app);

    match context.as_str() {
        "value" => settings.heatmap_value_custom = stops,
        "change" => settings.heatmap_change_custom = stops,
        "coverage" => settings.heatmap_coverage_custom = stops,
        _ => return Err(format!("Unknown heatmap context: {}", context)),
    }

    save_settings(&app, &settings);
    Ok(())
}
