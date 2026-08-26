//! Online INI repository Tauri commands.

use libretune_core::project::{IniSource, OnlineIniEntry};
use serde::Serialize;
use std::path::PathBuf;

use crate::paths::{get_app_data_dir, get_definitions_dir};
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

/// Search / refresh results plus cache metadata for display.
#[derive(Serialize)]
pub struct OnlineIniListResponse {
    pub entries: Vec<OnlineIniEntryResponse>,
    /// RFC 3339 timestamp of the last successful network refresh.
    pub last_updated: Option<String>,
    /// True when this call performed a network refresh (vs. serving cache).
    pub refreshed: bool,
}

/// Path of the persisted online-INI listing cache.
fn online_cache_path(app: &tauri::AppHandle) -> PathBuf {
    get_app_data_dir(app).join("online_ini_cache.json")
}

/// Ensure the repository holds a usable listing: load the persisted cache on
/// first use, then refresh from the network when it is stale (or missing).
/// A stale cache survives a failed refresh so offline use stays possible.
/// Returns whether a network refresh happened.
async fn ensure_repo_cache(
    repo: &mut libretune_core::project::OnlineIniRepository,
    app: &tauri::AppHandle,
) -> Result<bool, String> {
    if repo.entries().is_empty() {
        match repo.load_cache(&online_cache_path(app)) {
            Ok(_) => {}
            Err(e) => eprintln!("Warning: online INI cache unreadable: {e}"),
        }
    }
    if !repo.is_stale() {
        return Ok(false);
    }
    match repo.refresh().await {
        Ok(()) => {
            if let Err(e) = repo.save_cache(&online_cache_path(app)) {
                eprintln!("Warning: failed to persist online INI cache: {e}");
            }
            Ok(true)
        }
        Err(e) => {
            if repo.entries().is_empty() {
                // Nothing to fall back on — surface the failure.
                return Err(format!("Failed to refresh online INIs: {e}"));
            }
            eprintln!("Warning: online INI refresh failed, serving stale cache: {e}");
            Ok(false)
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
/// If signature is None, returns all available INIs. Serves the persisted
/// cache when fresh; refreshes from the network when stale or missing.
#[tauri::command]
pub async fn search_online_inis(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    signature: Option<String>,
) -> Result<OnlineIniListResponse, String> {
    let mut repo = state.online_ini_repository.lock().await;
    let refreshed = ensure_repo_cache(&mut repo, &app).await?;

    let results = repo
        .search(signature.as_deref())
        .await
        .map_err(|e| format!("Failed to search online INIs: {}", e))?;

    Ok(OnlineIniListResponse {
        entries: results.into_iter().map(|e| e.into()).collect(),
        last_updated: repo.last_updated_rfc3339(),
        refreshed,
    })
}

/// Force a network refresh of the online INI listing, bypassing the cache
/// TTL (the dialog's manual Refresh button). Falls back to the stale cache
/// only when the refresh fails and one exists.
#[tauri::command]
pub async fn refresh_online_inis(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<OnlineIniListResponse, String> {
    let mut repo = state.online_ini_repository.lock().await;

    let refreshed = match repo.refresh().await {
        Ok(()) => {
            if let Err(e) = repo.save_cache(&online_cache_path(&app)) {
                eprintln!("Warning: failed to persist online INI cache: {e}");
            }
            true
        }
        Err(e) => {
            if repo.entries().is_empty() {
                return Err(format!("Failed to refresh online INIs: {e}"));
            }
            eprintln!("Warning: online INI refresh failed, keeping cache: {e}");
            false
        }
    };

    let results = repo
        .search(None)
        .await
        .map_err(|e| format!("Failed to list online INIs: {}", e))?;

    Ok(OnlineIniListResponse {
        entries: results.into_iter().map(|e| e.into()).collect(),
        last_updated: repo.last_updated_rfc3339(),
        refreshed,
    })
}

/// Refresh the online INI listing only when the cached one has expired past
/// its TTL. Intended for a periodic background timer: a no-op while fresh.
#[tauri::command]
pub async fn refresh_online_inis_if_stale(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<OnlineIniListResponse, String> {
    let mut repo = state.online_ini_repository.lock().await;
    if repo.entries().is_empty() {
        if let Err(e) = repo.load_cache(&online_cache_path(&app)) {
            eprintln!("Warning: online INI cache unreadable: {e}");
        }
    }
    if !repo.is_stale() {
        let results = repo
            .search(None)
            .await
            .map_err(|e| format!("Failed to list online INIs: {}", e))?;
        return Ok(OnlineIniListResponse {
            entries: results.into_iter().map(|e| e.into()).collect(),
            last_updated: repo.last_updated_rfc3339(),
            refreshed: false,
        });
    }
    drop(repo);
    refresh_online_inis(app, state).await
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
