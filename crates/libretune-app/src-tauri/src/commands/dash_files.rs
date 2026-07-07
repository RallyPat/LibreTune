//! Dashboard file IO commands: load/save/rename/duplicate/delete + validate + create.

use crate::commands::dash_layout::generate_unique_filename;
use crate::paths::{get_dashboards_dir, get_projects_dir};
use libretune_core::dash::{
    self, create_basic_dashboard, create_racing_dashboard, create_telemetry_compact_dashboard,
    create_telemetry_live_dashboard, create_tuning_dashboard, DashComponent, DashFile, VersionInfo,
};
use libretune_core::ini::EcuDefinition;
use std::path::{Path, PathBuf};

/// Load a TS .dash file and return the full DashFile structure
#[tauri::command]
pub async fn get_dash_file(path: String) -> Result<DashFile, String> {
    println!("[get_dash_file] Loading from: {}", path);

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

    println!(
        "[get_dash_file] Loaded {} components, {} embedded images",
        dash_file.gauge_cluster.components.len(),
        dash_file.gauge_cluster.embedded_images.len()
    );
    Ok(dash_file)
}

/// Validate a dashboard file and return a detailed report
#[tauri::command]
pub async fn validate_dashboard(
    dash_file: DashFile,
    project_name: Option<String>,
    app: tauri::AppHandle,
) -> Result<dash::ValidationReport, String> {
    println!("[validate_dashboard] Validating dashboard");

    // Load ECU definition if project name is provided
    let ecu_def = if let Some(ref proj_name) = project_name {
        let project_dir = get_projects_dir(&app).join(proj_name);
        let ini_path = project_dir.join("definition.ini");

        if ini_path.exists() {
            match EcuDefinition::from_file(ini_path.to_string_lossy().as_ref()) {
                Ok(def) => Some(def),
                Err(e) => {
                    println!(
                        "[validate_dashboard] Warning: Could not load INI for validation: {}",
                        e
                    );
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let report = dash::validate_dashboard(&dash_file, ecu_def.as_ref());

    println!(
        "[validate_dashboard] Validation complete: {} errors, {} warnings",
        report.errors.len(),
        report.warnings.len()
    );

    Ok(report)
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

    let dash_file = match template.as_str() {
        "basic" => create_basic_dashboard(),
        "racing" => create_racing_dashboard(),
        "tuning" => create_tuning_dashboard(),
        "telemetry_live" | "f1_telemetry" => create_telemetry_live_dashboard(),
        "telemetry_compact" => create_telemetry_compact_dashboard(),
        _ => create_basic_dashboard(),
    };

    let target_path = dash_dir.join(&target_name);
    dash::save_dash_file(&dash_file, &target_path)
        .map_err(|e| format!("Failed to write dashboard file: {}", e))?;

    Ok(target_path.to_string_lossy().to_string())
}

/// Rename an existing dashboard file.
#[tauri::command]
pub async fn rename_dashboard(path: String, new_name: String) -> Result<String, String> {
    let source = PathBuf::from(&path);
    let parent = source
        .parent()
        .ok_or_else(|| "Invalid dashboard path".to_string())?
        .to_path_buf();

    let ext = if path.to_lowercase().ends_with(".ltdash.xml") {
        ".ltdash.xml"
    } else if path.to_lowercase().ends_with(".dash") {
        ".dash"
    } else if path.to_lowercase().ends_with(".gauge") {
        ".gauge"
    } else {
        ""
    };

    let mut file_name = new_name.trim().to_string();
    if file_name.is_empty() {
        file_name = "Dashboard".to_string();
    }
    if !ext.is_empty() && !file_name.to_lowercase().ends_with(ext) {
        file_name = format!("{}{}", file_name, ext);
    }

    let target_name = if parent.join(&file_name).exists() {
        generate_unique_filename(&parent, &file_name)
    } else {
        file_name
    };

    let target_path = parent.join(&target_name);
    std::fs::rename(&source, &target_path)
        .map_err(|e| format!("Failed to rename dashboard: {}", e))?;

    Ok(target_path.to_string_lossy().to_string())
}

/// Duplicate a dashboard file.
#[tauri::command]
pub async fn duplicate_dashboard(path: String, new_name: String) -> Result<String, String> {
    let source = PathBuf::from(&path);
    let parent = source
        .parent()
        .ok_or_else(|| "Invalid dashboard path".to_string())?
        .to_path_buf();

    let ext = if path.to_lowercase().ends_with(".ltdash.xml") {
        ".ltdash.xml"
    } else if path.to_lowercase().ends_with(".dash") {
        ".dash"
    } else if path.to_lowercase().ends_with(".gauge") {
        ".gauge"
    } else {
        ""
    };

    let mut file_name = new_name.trim().to_string();
    if file_name.is_empty() {
        file_name = "Dashboard Copy".to_string();
    }
    if !ext.is_empty() && !file_name.to_lowercase().ends_with(ext) {
        file_name = format!("{}{}", file_name, ext);
    }

    let target_name = if parent.join(&file_name).exists() {
        generate_unique_filename(&parent, &file_name)
    } else {
        file_name
    };

    let target_path = parent.join(&target_name);
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
