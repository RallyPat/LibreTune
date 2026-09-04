//! Reading and writing a user-chosen text file.
//!
//! `AutoTune.tsx` has called `read_file_contents` and `write_file_contents`
//! since it was written, and neither command existed - Load Ref and Save Ref
//! both failed with "Command not found". A tuning session's reference table
//! could be built but never kept.
//!
//! This is the *only* general file read/write pair exposed to the webview. A
//! second, identical pair (`read_text_file` / `write_text_file`) used to live in
//! `data_logging.rs`; it was deleted and its callers pointed here so there is
//! one place to enforce the fence below.
//!
//! Paths are supposed to come from the native file dialog, so they are the
//! user's own choice rather than anything this code invents - but nothing on the
//! Tauri invoke boundary enforces that, and `tauri.conf.json` sets `"csp": null`,
//! so any script that reaches the webview could otherwise read `~/.ssh/id_rsa`
//! or overwrite a shell profile. [`resolve_user_path`] confines both commands to
//! the directories a file dialog would realistically land in.

use std::path::{Component, Path, PathBuf};

use libretune_core::project::Project;

/// Directories a webview-supplied path is allowed to touch.
///
/// Deliberately not the home directory: that would readmit `~/.ssh`, `~/.aws`
/// and every dotfile. These are where a save/open dialog actually lands.
fn allowed_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(projects) = Project::projects_dir() {
        roots.push(projects);
    }
    roots.extend(
        [
            dirs::document_dir(),
            dirs::download_dir(),
            dirs::desktop_dir(),
            // The app's own data dir, where settings/dashboards/logs live.
            dirs::data_dir().map(|d| d.join("LibreTune")),
        ]
        .into_iter()
        .flatten(),
    );
    // Tests write to a scratch directory rather than the user's real Documents.
    #[cfg(test)]
    roots.push(std::env::temp_dir());

    roots
        .into_iter()
        .map(|r| r.canonicalize().unwrap_or(r))
        .collect()
}

/// Resolve `path` far enough to prefix-check it, without requiring it to exist.
///
/// Canonicalises the deepest ancestor that does exist (which resolves symlinks
/// and `.`), then re-appends the remainder. `..` is rejected outright rather
/// than resolved, so a component can never climb out of a root after the check.
fn resolve_for_check(path: &Path) -> Result<PathBuf, String> {
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(format!("Path may not contain '..': {}", path.display()));
    }

    let mut existing = path;
    let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
    loop {
        if existing.exists() {
            break;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name);
                existing = parent;
            }
            _ => return Err(format!("Path does not resolve: {}", path.display())),
        }
    }

    let mut resolved = existing
        .canonicalize()
        .map_err(|e| format!("Path does not resolve: {}: {e}", path.display()))?;
    for name in tail.iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

/// Reject a path outside every [`allowed_roots`] entry.
pub(crate) fn resolve_user_path(path: &str) -> Result<PathBuf, String> {
    let requested = PathBuf::from(path);
    if !requested.is_absolute() {
        return Err(format!("Path must be absolute: {path}"));
    }
    let resolved = resolve_for_check(&requested)?;

    let roots = allowed_roots();
    if roots.iter().any(|root| resolved.starts_with(root)) {
        return Ok(resolved);
    }

    Err(format!(
        "Refusing to touch {path}: LibreTune only reads and writes files under \
         your projects, Documents, Downloads, Desktop or app-data folders."
    ))
}

/// Write `content` to `path`, creating or replacing it.
#[tauri::command]
pub async fn write_file_contents(path: String, content: String) -> Result<(), String> {
    let p = resolve_user_path(&path)?;
    // Create the directory rather than failing on a path the user picked from a
    // dialog that let them type a new folder name.
    if let Some(dir) = p.parent() {
        if !dir.as_os_str().is_empty() && !dir.exists() {
            std::fs::create_dir_all(dir).map_err(|e| format!("create {dir:?}: {e}"))?;
        }
    }
    std::fs::write(&p, content).map_err(|e| format!("write {path}: {e}"))?;
    tracing::info!(path = %path, "file written");
    Ok(())
}

/// Read `path` as UTF-8 text.
#[tauri::command]
pub async fn read_file_contents(path: String) -> Result<String, String> {
    let p = resolve_user_path(&path)?;
    std::fs::read_to_string(&p).map_err(|e| format!("read {path}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("libretune_file_io_test")
            .join(name)
    }

    #[tokio::test]
    async fn a_file_round_trips() {
        let dir = std::env::temp_dir().join("libretune_file_io_test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = scratch("nested").join("ref.csv");
        let p = path.to_string_lossy().to_string();

        // The nested directory does not exist yet: a dialog can name a folder
        // that has not been created, and failing there loses the user's work.
        write_file_contents(p.clone(), "rpm,ve\n1000,45\n".into())
            .await
            .expect("write creates missing directories");
        let back = read_file_contents(p).await.expect("read back");
        assert_eq!(back, "rpm,ve\n1000,45\n");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_missing_file_reports_the_path() {
        let p = scratch("no-such-file.csv").to_string_lossy().to_string();
        let err = read_file_contents(p.clone()).await.expect_err("must fail");
        assert!(
            err.contains("no-such-file.csv"),
            "the message should name the path: {err}"
        );
    }

    /// The invoke boundary is reachable by anything running in the webview, and
    /// `tauri.conf.json` sets no CSP. These commands must not be a general
    /// filesystem.
    #[tokio::test]
    async fn a_path_outside_every_allowed_root_is_refused() {
        let err = read_file_contents("/etc/passwd".into())
            .await
            .expect_err("reading a system file must be refused");
        assert!(err.contains("Refusing to touch"), "{err}");

        let err = write_file_contents("/etc/libretune-owned".into(), "x".into())
            .await
            .expect_err("writing a system file must be refused");
        assert!(err.contains("Refusing to touch"), "{err}");
    }

    #[tokio::test]
    async fn traversal_out_of_an_allowed_root_is_refused() {
        let escape = std::env::temp_dir().join("..").join("..").join("etc");
        let err = read_file_contents(escape.join("passwd").to_string_lossy().to_string())
            .await
            .expect_err("'..' must not be resolved into an allowed root");
        assert!(err.contains(".."), "{err}");
    }

    #[tokio::test]
    async fn a_relative_path_is_refused() {
        let err = read_file_contents("ref.csv".into())
            .await
            .expect_err("a relative path resolves against the app's cwd, not the user's");
        assert!(err.contains("must be absolute"), "{err}");
    }
}
