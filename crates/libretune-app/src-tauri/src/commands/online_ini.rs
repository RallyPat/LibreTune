//! Online INI repository Tauri commands.

use libretune_core::project::{IniSource, OnlineIniEntry};
use serde::Serialize;

use crate::paths::get_definitions_dir;
use crate::state::AppState;

/// Serializable version of OnlineIniEntry for the frontend
#[derive(Serialize)]
pub struct OnlineIniEntryResponse {
    source: String,
    name: String,
    signature: Option<String>,
    download_url: String,
    repo_path: String,
    size: Option<u64>,
}

impl From<OnlineIniEntry> for OnlineIniEntryResponse {
    fn from(entry: OnlineIniEntry) -> Self {
        OnlineIniEntryResponse {
            source: entry.source.display_name().to_string(),
            name: entry.name,
            signature: entry.signature,
            download_url: entry.download_url,
            repo_path: entry.repo_path,
            size: entry.size,
        }
    }
}

/// Check if we have internet connectivity
#[tauri::command]
pub async fn check_internet_connectivity(
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let repo = state.online_ini_repository.lock().await;
    Ok(repo.check_connectivity().await)
}

/// Search for INI files online matching a signature.
/// If signature is None, returns all available INIs.
#[tauri::command]
pub async fn search_online_inis(
    state: tauri::State<'_, AppState>,
    signature: Option<String>,
) -> Result<Vec<OnlineIniEntryResponse>, String> {
    let mut repo = state.online_ini_repository.lock().await;

    let results = repo
        .search(signature.as_deref())
        .await
        .map_err(|e| format!("Failed to search online INIs: {}", e))?;

    Ok(results.into_iter().map(|e| e.into()).collect())
}

/// Download an INI file from online repository
#[tauri::command]
pub async fn download_ini(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    download_url: String,
    name: String,
    source: String,
) -> Result<String, String> {
    let repo = state.online_ini_repository.lock().await;

    let source_enum = match source.to_lowercase().as_str() {
        "speeduino" => IniSource::Speeduino,
        "rusefi" => IniSource::RusEFI,
        "fome" => IniSource::Fome,
        "epicefi" => IniSource::EpicEFI,
        _ => IniSource::Custom,
    };

    let entry = OnlineIniEntry {
        source: source_enum,
        name: name.clone(),
        signature: None,
        download_url,
        repo_path: name.clone(),
        size: None,
    };

    let definitions_dir = get_definitions_dir(&app);

    let downloaded_path = repo
        .download(&entry, &definitions_dir)
        .await
        .map_err(|e| format!("Failed to download INI: {}", e))?;

    drop(repo);
    let mut local_repo_guard = state.ini_repository.lock().await;
    if let Some(ref mut local_repo) = *local_repo_guard {
        let _ = local_repo.import(&downloaded_path);
    }

    Ok(downloaded_path.to_string_lossy().to_string())
}
