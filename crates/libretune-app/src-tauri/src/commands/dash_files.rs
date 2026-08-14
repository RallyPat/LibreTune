//! Dashboard file IO commands: load/save/rename/duplicate/delete + validate + create.

use crate::commands::dash_layout::{generate_unique_filename, template_by_id};
use crate::paths::get_dashboards_dir;
use crate::state::AppState;
use libretune_core::dash::{self, DashComponent, DashFile, VersionInfo};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Load a TS .dash file and return the full DashFile structure
#[tauri::command]
pub async fn get_dash_file(path: String) -> Result<DashFile, String> {
    debug!("Loading dashboard from: {}", path);

    let lower = path.to_lowercase();

    let dash_file = if lower.ends_with(".gauge") {
        let gauge_file = dash::load_gauge_file(Path::new(&path))
            .map_err(|e| format!("Failed to parse gauge XML: {}", e))?;

        let mut dash_file = DashFile {
            bibliography: gauge_file.bibliography,
            version_info: gauge_file.version_info,
            ..Default::default()
        };
        dash_file.gauge_cluster.embedded_images = gauge_file.embedded_images;
        dash_file
            .gauge_cluster
            .components
            .push(DashComponent::Gauge(Box::new(gauge_file.gauge)));
        dash_file
    } else {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read dashboard file: {}", e))?;

        dash::parse_dash_file(&content)
            .map_err(|e| format!("Failed to parse dashboard XML: {}", e))?
    };

    debug!(
        "Loaded dashboard: {} components, {} embedded images",
        dash_file.gauge_cluster.components.len(),
        dash_file.gauge_cluster.embedded_images.len()
    );
    Ok(dash_file)
}

/// Validate a dashboard file against the loaded INI definition and return
/// a detailed report.
#[tauri::command]
pub async fn validate_dashboard(
    dash_file: DashFile,
    state: tauri::State<'_, AppState>,
) -> Result<dash::ValidationReport, String> {
    let def_guard = state.definition.lock().await;
    let report = dash::validate_dashboard(&dash_file, def_guard.as_ref());

    info!(
        "Dashboard validation complete: {} errors, {} warnings",
        report.errors.len(),
        report.warnings.len()
    );

    Ok(report)
}

/// Suggest output-channel remaps for dashboard components whose channels
/// are unknown in the loaded INI (cross-firmware synonym table plus
/// guarded fuzzy matching).
#[tauri::command]
pub async fn suggest_channel_remaps(
    dash_file: DashFile,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<dash::ChannelRemap>, String> {
    let def_guard = state.definition.lock().await;
    let Some(ecu_def) = def_guard.as_ref() else {
        return Ok(Vec::new());
    };
    Ok(dash::suggest_channel_remaps(&dash_file, ecu_def))
}

/// Save a TS .dash or .gauge file directly to a path
#[tauri::command]
pub async fn save_dash_file(path: String, dash_file: DashFile) -> Result<(), String> {
    let lower = path.to_lowercase();
    let path_buf = PathBuf::from(&path);

    if lower.ends_with(".gauge") {
        let gauge = dash_file
            .gauge_cluster
            .components
            .iter()
            .find_map(|comp| match comp {
                DashComponent::Gauge(g) => Some((**g).clone()),
                _ => None,
            })
            .ok_or_else(|| "Gauge file must contain a gauge component".to_string())?;

        let gauge_file = dash::GaugeFile {
            bibliography: dash_file.bibliography.clone(),
            version_info: VersionInfo {
                file_format: "1.0".to_string(),
                firmware_signature: dash_file.version_info.firmware_signature.clone(),
            },
            embedded_images: dash_file.gauge_cluster.embedded_images.clone(),
            gauge,
        };

        dash::save_gauge_file(&gauge_file, &path_buf)
            .map_err(|e| format!("Failed to write gauge file: {}", e))?;
    } else {
        dash::save_dash_file(&dash_file, &path_buf)
            .map_err(|e| format!("Failed to write dashboard file: {}", e))?;
    }

    Ok(())
}

/// Create a new dashboard file from a template in the user dashboards directory.
#[tauri::command]
pub async fn create_new_dashboard(
    app: tauri::AppHandle,
    name: String,
    template: String,
) -> Result<String, String> {
    let dash_dir = get_dashboards_dir(&app);
    if !dash_dir.exists() {
        std::fs::create_dir_all(&dash_dir)
            .map_err(|e| format!("Failed to create dashboards directory: {}", e))?;
    }

    let mut file_name = name.trim().to_string();
    if file_name.is_empty() {
        file_name = "Dashboard".to_string();
    }
    if !file_name.to_lowercase().ends_with(".ltdash.xml") {
        file_name = format!("{}.ltdash.xml", file_name);
    }

    let target_name = if dash_dir.join(&file_name).exists() {
        generate_unique_filename(&dash_dir, &file_name)
    } else {
        file_name
    };

    let dash_file = template_by_id(&template)
        .map(|spec| (spec.builder)())
        .unwrap_or_else(libretune_core::dash::create_basic_dashboard);

    let target_path = dash_dir.join(&target_name);
    dash::save_dash_file(&dash_file, &target_path)
        .map_err(|e| format!("Failed to write dashboard file: {}", e))?;

    Ok(target_path.to_string_lossy().to_string())
}

/// The dashboard file extension carried by `path` ("" when unrecognized).
fn dashboard_extension(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".ltdash.xml") {
        ".ltdash.xml"
    } else if lower.ends_with(".dash") {
        ".dash"
    } else if lower.ends_with(".gauge") {
        ".gauge"
    } else {
        ""
    }
}

/// Resolve the destination for a rename/duplicate: keep the source's
/// extension (unless `new_name` already carries one) and de-conflict
/// against existing files in the same directory.
fn resolve_copy_target(
    source: &Path,
    new_name: &str,
    default_name: &str,
) -> Result<PathBuf, String> {
    let parent = source
        .parent()
        .ok_or_else(|| "Invalid dashboard path".to_string())?
        .to_path_buf();

    let ext = dashboard_extension(&source.to_string_lossy());
    let mut file_name = new_name.trim().to_string();
    if file_name.is_empty() {
        file_name = default_name.to_string();
    }
    if !ext.is_empty() && !file_name.to_lowercase().ends_with(ext) {
        file_name = format!("{}{}", file_name, ext);
    }

    let target_name = if parent.join(&file_name).exists() {
        generate_unique_filename(&parent, &file_name)
    } else {
        file_name
    };

    Ok(parent.join(target_name))
}

/// Rename an existing dashboard file.
#[tauri::command]
pub async fn rename_dashboard(path: String, new_name: String) -> Result<String, String> {
    let source = PathBuf::from(&path);
    let target_path = resolve_copy_target(&source, &new_name, "Dashboard")?;

    std::fs::rename(&source, &target_path)
        .map_err(|e| format!("Failed to rename dashboard: {}", e))?;

    Ok(target_path.to_string_lossy().to_string())
}

/// Duplicate a dashboard file.
#[tauri::command]
pub async fn duplicate_dashboard(path: String, new_name: String) -> Result<String, String> {
    let source = PathBuf::from(&path);
    let target_path = resolve_copy_target(&source, &new_name, "Dashboard Copy")?;

    std::fs::copy(&source, &target_path)
        .map_err(|e| format!("Failed to duplicate dashboard: {}", e))?;

    Ok(target_path.to_string_lossy().to_string())
}

/// Export a dashboard to a specific path.
#[tauri::command]
pub async fn export_dashboard(path: String, dash_file: DashFile) -> Result<(), String> {
    save_dash_file(path, dash_file).await
}

/// Delete a dashboard file.
#[tauri::command]
pub async fn delete_dashboard(path: String) -> Result<(), String> {
    let path_buf = PathBuf::from(&path);
    if !path_buf.exists() {
        return Err("Dashboard file not found".to_string());
    }
    std::fs::remove_file(&path_buf).map_err(|e| format!("Failed to delete dashboard: {}", e))?;
    Ok(())
}
