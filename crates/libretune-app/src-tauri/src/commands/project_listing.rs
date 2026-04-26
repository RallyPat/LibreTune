//! Project listing commands: get path, list available projects.

use libretune_core::project::Project;
use serde::Serialize;

#[derive(Serialize)]
pub struct ProjectInfoResponse {
    pub name: String,
    pub path: String,
    pub signature: String,
    pub modified: String,
}

/// Get the path to the projects directory
#[tauri::command]
pub async fn get_projects_path() -> Result<String, String> {
    let path =
        Project::projects_dir().map_err(|e| format!("Failed to get projects directory: {}", e))?;

    // Create if doesn't exist
    std::fs::create_dir_all(&path)
        .map_err(|e| format!("Failed to create projects directory: {}", e))?;

    Ok(path.to_string_lossy().to_string())
}

/// List all available projects
#[tauri::command]
pub async fn list_projects() -> Result<Vec<ProjectInfoResponse>, String> {
    let projects =
        Project::list_projects().map_err(|e| format!("Failed to list projects: {}", e))?;

    Ok(projects
        .into_iter()
        .map(|p| ProjectInfoResponse {
            name: p.name,
            path: p.path.to_string_lossy().to_string(),
            signature: p.signature,
            modified: p.modified,
        })
        .collect())
}
