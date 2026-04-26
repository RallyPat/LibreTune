//! get_available_inis command (extracted from lib.rs).

use crate::paths::get_definitions_dir;

/// Lists all available ECU INI definition files in the definitions directory.
///
/// Scans the app's definitions directory for .ini files that describe ECU protocols.
///
/// Returns: Sorted vector of INI filenames
#[tauri::command]
pub async fn get_available_inis(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let mut inis = Vec::new();
    let definitions_dir = get_definitions_dir(&app);
    println!("Scanning for INIs in: {:?}", definitions_dir);

    // Ensure definitions directory exists
    if !definitions_dir.exists() {
        let _ = std::fs::create_dir_all(&definitions_dir);
        println!("Created definitions directory: {:?}", definitions_dir);
        return Ok(inis); // Return empty list for new install
    }

    match std::fs::read_dir(&definitions_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext.to_string_lossy().to_lowercase() == "ini" {
                        if let Some(name) = entry.file_name().to_str() {
                            inis.push(name.to_string());
                        }
                    }
                }
            }
            println!("Found {} INI files", inis.len());
        }
        Err(e) => {
            println!("Failed to read definitions directory: {}", e);
            return Err(format!("Failed to read definitions directory: {}", e));
        }
    }
    inis.sort();
    Ok(inis)
}
