//! Git version control Tauri commands.
//!
//! These commands operate on the currently-open project's git repository.

use libretune_core::project::{
    format_commit_message, BranchInfo, CommitDiff, CommitInfo, VersionControl,
};
use serde::Serialize;

use crate::state::AppState;

/// Response type for commit info
#[derive(Debug, Clone, Serialize)]
pub struct CommitInfoResponse {
    sha_short: String,
    sha: String,
    message: String,
    annotation: Option<String>,
    author: String,
    timestamp: String,
    is_head: bool,
}

impl From<CommitInfo> for CommitInfoResponse {
    fn from(info: CommitInfo) -> Self {
        Self {
            sha_short: info.sha_short,
            sha: info.sha,
            message: info.message,
            annotation: info.annotation,
            author: info.author,
            timestamp: info.timestamp,
            is_head: info.is_head,
        }
    }
}

/// Response type for branch info
#[derive(Debug, Clone, Serialize)]
pub struct BranchInfoResponse {
    name: String,
    is_current: bool,
    tip_sha: String,
}

impl From<BranchInfo> for BranchInfoResponse {
    fn from(info: BranchInfo) -> Self {
        Self {
            name: info.name,
            is_current: info.is_current,
            tip_sha: info.tip_sha,
        }
    }
}

/// Initialize git repository for current project
#[tauri::command]
pub async fn git_init_project(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::init(&project.path)
        .map_err(|e| format!("Failed to initialize git: {}", e))?;

    vc.commit("Initial project commit")
        .map_err(|e| format!("Failed to create initial commit: {}", e))?;

    Ok(true)
}

/// Check if current project has git repository
#[tauri::command]
pub async fn git_has_repo(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    Ok(VersionControl::is_git_repo(&project.path))
}

/// Commit current tune with message
#[tauri::command]
pub async fn git_commit(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    message: Option<String>,
    annotation: Option<String>,
) -> Result<String, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    let commit_message = message.unwrap_or_else(|| {
        let format = crate::get_commit_message_format(&app);
        let now = chrono::Local::now();
        format
            .replace("{date}", &now.format("%Y-%m-%d").to_string())
            .replace("{time}", &now.format("%H:%M:%S").to_string())
    });

    let commit_message = format_commit_message(&commit_message, annotation.as_deref());

    let sha = vc
        .commit(&commit_message)
        .map_err(|e| format!("Failed to commit: {}", e))?;

    Ok(sha)
}

/// Get commit history for current project
#[tauri::command]
pub async fn git_history(
    state: tauri::State<'_, AppState>,
    max_count: Option<usize>,
) -> Result<Vec<CommitInfoResponse>, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    let history = vc
        .get_history(max_count.unwrap_or(50))
        .map_err(|e| format!("Failed to get history: {}", e))?;

    Ok(history.into_iter().map(CommitInfoResponse::from).collect())
}

/// Get diff between two commits
#[tauri::command]
pub async fn git_diff(
    state: tauri::State<'_, AppState>,
    from_sha: String,
    to_sha: String,
) -> Result<CommitDiff, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    vc.diff_commits(&from_sha, &to_sha)
        .map_err(|e| format!("Failed to diff commits: {}", e))
}

/// Checkout a specific commit
#[tauri::command]
pub async fn git_checkout(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    sha: String,
) -> Result<(), String> {
    // Checkout writes the historical CurrentTune.msq to the working tree.
    // Resolve the path and release the lock before reloading — the checkout
    // alone previously left every layer above the filesystem untouched
    // (cache, state.current_tune, UI all kept serving the pre-checkout tune,
    // and the next save wrote it back over the restored file; ledger R9,
    // observed live). Reloading through load_tune is what the emitted
    // "tune:loaded" event always assumed had happened.
    let tune_path = {
        let proj_guard = state.current_project.lock().await;
        let project = proj_guard
            .as_ref()
            .ok_or_else(|| "No project open".to_string())?;

        let vc = VersionControl::open(&project.path)
            .map_err(|e| format!("Git repository not initialized: {}", e))?;

        vc.checkout_commit(&sha)
            .map_err(|e| format!("Failed to checkout: {}", e))?;

        project.path.join("CurrentTune.msq")
    };

    crate::commands::load_tune::load_tune(
        state.clone(),
        app,
        tune_path.to_string_lossy().to_string(),
    )
    .await?;

    // The restored tune is not what the ECU is running until written/burned.
    *state.tune_modified.lock().await = true;

    Ok(())
}

/// List all branches in the project's git repository.
#[tauri::command]
pub async fn git_list_branches(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<BranchInfoResponse>, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    let branches = vc
        .list_branches()
        .map_err(|e| format!("Failed to list branches: {}", e))?;

    Ok(branches.into_iter().map(BranchInfoResponse::from).collect())
}

/// Create a new git branch in the project repository.
#[tauri::command]
pub async fn git_create_branch(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    vc.create_branch(&name)
        .map_err(|e| format!("Failed to create branch: {}", e))
}

/// Switch to a different git branch.
#[tauri::command]
pub async fn git_switch_branch(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    // Same shape as git_checkout: the branch switch rewrites CurrentTune.msq
    // on disk, so the in-memory tune must be reloaded or the app keeps
    // serving (and re-saving) the previous branch's tune.
    let tune_path = {
        let proj_guard = state.current_project.lock().await;
        let project = proj_guard
            .as_ref()
            .ok_or_else(|| "No project open".to_string())?;

        let vc = VersionControl::open(&project.path)
            .map_err(|e| format!("Git repository not initialized: {}", e))?;

        vc.switch_branch(&name)
            .map_err(|e| format!("Failed to switch branch: {}", e))?;

        project.path.join("CurrentTune.msq")
    };

    crate::commands::load_tune::load_tune(
        state.clone(),
        app,
        tune_path.to_string_lossy().to_string(),
    )
    .await?;

    *state.tune_modified.lock().await = true;

    Ok(())
}

/// Get the name of the current git branch.
#[tauri::command]
pub async fn git_current_branch(
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    if !VersionControl::is_git_repo(&project.path) {
        return Ok(None);
    }

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    Ok(vc.get_current_branch_name())
}

/// Check if the project has uncommitted git changes.
#[tauri::command]
pub async fn git_has_changes(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    if !VersionControl::is_git_repo(&project.path) {
        return Ok(false);
    }

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    vc.has_changes()
        .map_err(|e| format!("Failed to check changes: {}", e))
}
