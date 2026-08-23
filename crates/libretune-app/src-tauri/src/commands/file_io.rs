//! Reading and writing a user-chosen text file.
//!
//! `AutoTune.tsx` has called `read_file_contents` and `write_file_contents`
//! since it was written, and neither command existed - Load Ref and Save Ref
//! both failed with "Command not found". A tuning session's reference table
//! could be built but never kept.
//!
//! Paths come from the native file dialog, so they are the user's own choice
//! rather than anything this code invents.

/// Write `content` to `path`, creating or replacing it.
#[tauri::command]
pub async fn write_file_contents(path: String, content: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    // Create the directory rather than failing on a path the user picked from a
    // dialog that let them type a new folder name.
    if let Some(dir) = p.parent() {
        if !dir.as_os_str().is_empty() && !dir.exists() {
            std::fs::create_dir_all(dir).map_err(|e| format!("create {dir:?}: {e}"))?;
        }
    }
    std::fs::write(p, content).map_err(|e| format!("write {path}: {e}"))?;
    tracing::info!(path = %path, "file written");
    Ok(())
}

/// Read `path` as UTF-8 text.
#[tauri::command]
pub async fn read_file_contents(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("read {path}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_file_round_trips() {
        let dir = std::env::temp_dir().join("libretune_file_io_test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("nested").join("ref.csv");
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
        let err = read_file_contents("no/such/file.csv".into())
            .await
            .expect_err("must fail");
        assert!(
            err.contains("no/such/file.csv"),
            "the message should name the path: {err}"
        );
    }
}
