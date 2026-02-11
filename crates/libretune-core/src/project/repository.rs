//! INI Repository Management
//!
//! Manages a local repository of ECU definition (INI) files,
//! similar to TS firmware folder.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// An entry in the INI repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IniEntry {
    /// Unique ID (filename without extension)
    pub id: String,

    /// Display name (from INI [MegaTune] or filename)
    pub name: String,

    /// ECU signature
    pub signature: String,

    /// File path relative to repository
    pub path: String,

    /// When this INI was imported
    pub imported: String,

    /// Original source path (for reference)
    pub source: Option<String>,
}

/// Repository of ECU definition files
pub struct IniRepository {
    /// Repository directory path
    pub path: PathBuf,

    /// Cached list of entries (loaded from index.json)
    entries: Vec<IniEntry>,
}

impl IniRepository {
    /// Get the default repository directory (in app data)
    pub fn default_path() -> io::Result<PathBuf> {
        let base = dirs::data_local_dir()
            .or_else(dirs::home_dir)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "Could not find app data directory")
            })?;
        Ok(base.join("LibreTune").join("definitions"))
    }

    /// Open or create the repository
    pub fn open(path: Option<&Path>) -> io::Result<Self> {
        let repo_path = match path {
            Some(p) => p.to_path_buf(),
            None => Self::default_path()?,
        };

        // Create directory if it doesn't exist
        fs::create_dir_all(&repo_path)?;

        // Load or create index
        let mut repo = IniRepository {
            path: repo_path,
            entries: Vec::new(),
        };

        repo.load_index()?;

        Ok(repo)
    }

    /// Load the index from disk
    fn load_index(&mut self) -> io::Result<()> {
        let index_path = self.path.join("index.json");

        if index_path.exists() {
            let content = fs::read_to_string(&index_path)?;
            self.entries = serde_json::from_str(&content)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        } else {
            self.entries = Vec::new();
        }

        Ok(())
    }

    /// Save the index to disk
    fn save_index(&self) -> io::Result<()> {
        let index_path = self.path.join("index.json");
        let content = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(index_path, content)
    }

    /// List all INI files in the repository
    pub fn list(&self) -> &[IniEntry] {
        &self.entries
    }

    /// Import an INI file into the repository
    ///
    /// Returns the entry ID
    pub fn import(&mut self, source_path: &Path) -> io::Result<String> {
        // Read and parse the INI to get signature
        let content = Self::read_ini_file(source_path)?;

        let (name, signature) = Self::extract_ini_info(&content)?;

        // Generate unique ID from signature
        let id = Self::generate_id(&signature);

        // Check if already exists
        if self.entries.iter().any(|e| e.id == id) {
            return Ok(id); // Already imported
        }

        // Copy file to repository
        let dest_filename = format!("{}.ini", id);
        let dest_path = self.path.join(&dest_filename);
        fs::write(&dest_path, &content)?;

        // Add entry
        let entry = IniEntry {
            id: id.clone(),
            name,
            signature,
            path: dest_filename,
            imported: Utc::now().to_rfc3339(),
            source: Some(source_path.to_string_lossy().to_string()),
        };

        self.entries.push(entry);
        self.save_index()?;

        Ok(id)
    }

    /// Read an INI file, handling encoding issues
    fn read_ini_file(path: &Path) -> io::Result<String> {
        // Try UTF-8 first
        match fs::read_to_string(path) {
            Ok(content) => Ok(content),
            Err(_) => {
                // Try reading as bytes and converting from Windows-1252
                let bytes = fs::read(path)?;

                // Use lossy conversion - replace invalid chars with ?
                // This handles Windows-1252 and other encodings
                Ok(String::from_utf8_lossy(&bytes).into_owned())
            }
        }
    }

    /// Extract the content between the first pair of double quotes in a value string.
    /// Handles INI comments (;) that may appear after the closing quote.
    /// e.g., `"rusEFI master.2026.01.08" ; comment` -> `rusEFI master.2026.01.08`
    fn extract_quoted_value(value: &str) -> String {
        let value = value.trim();

        // Find the first opening quote
        if let Some(start) = value.find('"') {
            // Find the closing quote after the opening quote
            if let Some(end) = value[start + 1..].find('"') {
                return value[start + 1..start + 1 + end].to_string();
            }
        }

        // Fallback: strip comment (anything after unquoted ';'), then trim quotes
        value
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches('"')
            .to_string()
    }

    /// Extract name and signature from INI content
    fn extract_ini_info(content: &str) -> io::Result<(String, String)> {
        let mut name = String::new();
        let mut signature = String::new();
        let mut in_megatune = false;

        for line in content.lines() {
            let line = line.trim();

            if line.starts_with('[') {
                // Check for [MegaTune] section start (case-insensitive)
                // Handle trailing comments: [MegaTune] ; comment
                let header_part = line.split(';').next().unwrap_or("").trim();
                in_megatune = header_part
                    .trim_matches(|c| c == '[' || c == ']')
                    .eq_ignore_ascii_case("MegaTune");
                continue;
            }

            if !in_megatune {
                continue;
            }

            // Look for signature in [MegaTune] section
            if line.to_lowercase().starts_with("signature") {
                if let Some(val) = line.splitn(2, '=').nth(1) {
                    signature = Self::extract_quoted_value(val);
                }
            }

            // Look for nEmu in [MegaTune] section for display name
            if line.to_lowercase().starts_with("nemu") {
                if let Some(val) = line.splitn(2, '=').nth(1) {
                    name = Self::extract_quoted_value(val);
                }
            }
        }

        if signature.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Could not find signature in [MegaTune] section of INI file",
            ));
        }

        // Use signature as name if nEmu not found
        if name.is_empty() {
            name = signature.clone();
        }

        Ok((name, signature))
    }

    /// Generate a filesystem-safe ID from signature
    fn generate_id(signature: &str) -> String {
        signature
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }

    /// Get an entry by ID
    pub fn get(&self, id: &str) -> Option<&IniEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Get the full path to an INI file
    pub fn get_path(&self, id: &str) -> Option<PathBuf> {
        self.get(id).map(|e| self.path.join(&e.path))
    }

    /// Remove an INI from the repository
    pub fn remove(&mut self, id: &str) -> io::Result<()> {
        if let Some(entry) = self.entries.iter().find(|e| e.id == id) {
            let file_path = self.path.join(&entry.path);
            if file_path.exists() {
                fs::remove_file(file_path)?;
            }
        }

        self.entries.retain(|e| e.id != id);
        self.save_index()
    }

    /// Scan for INI files in a directory and import them
    pub fn scan_directory(&mut self, dir: &Path) -> io::Result<Vec<String>> {
        let mut imported = Vec::new();

        if !dir.exists() {
            return Ok(imported);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext
                        .to_str()
                        .map(|s| s.eq_ignore_ascii_case("ini"))
                        .unwrap_or(false)
                    {
                        match self.import(&path) {
                            Ok(id) => imported.push(id),
                            Err(e) => {
                                eprintln!("Warning: Failed to import {:?}: {}", path, e);
                            }
                        }
                    }
                }
            }
        }

        Ok(imported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_ini_repository() {
        let temp = temp_dir().join("libretune_test_repo");
        let _ = fs::remove_dir_all(&temp);

        // Create test INI
        let ini_path = temp.join("source").join("test.ini");
        fs::create_dir_all(ini_path.parent().unwrap()).unwrap();
        fs::write(
            &ini_path,
            r#"
[MegaTune]
signature = "TestECU 1.0"
nEmu = "Test Engine Control Unit"
        "#,
        )
        .unwrap();

        // Open repository
        let repo_path = temp.join("repo");
        let mut repo = IniRepository::open(Some(&repo_path)).unwrap();

        // Import INI
        let id = repo.import(&ini_path).unwrap();
        assert_eq!(id, "TestECU_1.0");

        // Verify entry
        let entry = repo.get(&id).unwrap();
        assert_eq!(entry.name, "Test Engine Control Unit");
        assert_eq!(entry.signature, "TestECU 1.0");

        // Cleanup
        let _ = fs::remove_dir_all(&temp);
    }
}
