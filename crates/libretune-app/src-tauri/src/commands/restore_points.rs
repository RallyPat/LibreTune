//! Restore point management commands.

use crate::state::AppState;

/// Info about a restore point
#[derive(Debug, Clone, serde::Serialize)]
pub struct RestorePointResponse {
    pub filename: String,
    pub path: String,
    pub created: String,
    pub size_bytes: u64,
}

/// Create a restore point from the current tune
#[tauri::command]
pub async fn create_restore_point(
    state: tauri::State<'_, AppState>,
) -> Result<RestorePointResponse, String> {
    // `Project::create_restore_point` serializes `project.current_tune`, but
    // that field is only refreshed on load/save — every edit command
    // (update_constant, update_table_data and the table-op helpers) writes
    // `state.current_tune`, so the two diverge. Snapshotting the project copy
    // captured the tune as it was when the project was opened, silently losing
    // every edit made since. Observed on a bench ECU: a saved table edit
    // (veTable[0][0]=42) was captured by the restore point as the original 37.
    //
    // Sync the project's tune from the authoritative in-memory tune before
    // snapshotting so the restore point reflects what the user actually has —
    // the same source `save_tune` writes to CurrentTune.msq.
    let current = state.current_tune.lock().await.clone();

    let mut proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_mut()
        .ok_or_else(|| "No project open".to_string())?;

    if let Some(tune) = current {
        project.current_tune = Some(tune);
    }

    let restore_path = project
        .create_restore_point()
        .map_err(|e| format!("Failed to create restore point: {}", e))?;

    // Auto-prune if max_restore_points is set
    let max_points = project.config.settings.max_restore_points;
    if max_points > 0 {
        let _ = project.prune_restore_points(max_points as usize);
    }

    let metadata = std::fs::metadata(&restore_path)
        .map_err(|e| format!("Failed to read restore point metadata: {}", e))?;

    Ok(RestorePointResponse {
        filename: restore_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        path: restore_path.to_string_lossy().to_string(),
        created: chrono::Utc::now().to_rfc3339(),
        size_bytes: metadata.len(),
    })
}

/// List restore points for the current project
#[tauri::command]
pub async fn list_restore_points(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RestorePointResponse>, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let points = project
        .list_restore_points()
        .map_err(|e| format!("Failed to list restore points: {}", e))?;

    Ok(points
        .into_iter()
        .map(|p| RestorePointResponse {
            filename: p.filename,
            path: p.path.to_string_lossy().to_string(),
            created: p.created,
            size_bytes: p.size_bytes,
        })
        .collect())
}

/// Load a restore point as the current tune
///
/// Delegates to `load_tune`, the canonical tune-apply pipeline. This command
/// previously carried a stripped-down copy of that pipeline which (a) had no
/// `Bits` branch and discarded every enum/bits constant via `_ => {}` — after
/// a bad burn, "restore" reported success while every enum setting sat at INI
/// defaults — and (b) never updated `state.current_tune`, so every read path
/// kept serving the pre-restore tune and the next save wrote the old tune
/// back. Observed live against a bench ECU (ledger R8). Routing through
/// `load_tune` gives restore the same semantics as loading any tune: full
/// constant application including bits, state + cache + CurrentTune.msq all
/// updated, `tune:loaded` emitted.
#[tauri::command]
pub async fn load_restore_point(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    filename: String,
) -> Result<(), String> {
    // Resolve the restore point path, then release the project lock before
    // delegating — load_tune takes the same lock to persist CurrentTune.msq.
    let restore_path = {
        let proj_guard = state.current_project.lock().await;
        let project = proj_guard
            .as_ref()
            .ok_or_else(|| "No project open".to_string())?;
        let path = project.restore_points_dir().join(&filename);
        if !path.exists() {
            return Err(format!("Restore point not found: {}", filename));
        }
        path
    };

    crate::commands::load_tune::load_tune(
        state.clone(),
        app,
        restore_path.to_string_lossy().to_string(),
    )
    .await?;

    // Keep the in-memory project's tune in sync with what was just loaded, so
    // project-level flows (save_tune_to_project, use_project_tune) see the
    // restored tune rather than the pre-restore one.
    let restored = state.current_tune.lock().await.clone();
    if let Some(tune) = restored {
        let mut proj_guard = state.current_project.lock().await;
        if let Some(project) = proj_guard.as_mut() {
            project.current_tune = Some(tune);
        }
    }

    // A restored tune differs from what the ECU is running until the user
    // writes/burns it; load_tune resets tune_modified to false, which would
    // hide that.
    *state.tune_modified.lock().await = true;

    Ok(())
}

/// Delete a restore point by filename.
#[tauri::command]
pub async fn delete_restore_point(
    state: tauri::State<'_, AppState>,
    filename: String,
) -> Result<(), String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    project
        .delete_restore_point(&filename)
        .map_err(|e| format!("Failed to delete restore point: {}", e))
}
