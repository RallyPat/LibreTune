//! Project struct and management functions

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::properties::Properties;
use crate::tune::TuneFile;

/// Project configuration stored in project.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Config version for migrations
    pub version: String,

    /// Project display name
    pub name: String,

    /// When the project was created
    pub created: String,

    /// When the project was last modified
    pub modified: String,

    /// Relative path to ECU definition (usually projectCfg/definition.ini)
    pub ecu_definition: String,

    /// ECU signature from the INI file
    pub signature: String,

    /// Connection settings
    pub connection: ConnectionSettings,

    /// Project-specific settings
    pub settings: ProjectSettings,

    /// Active dashboard file (relative path)
    pub dashboard: Option<String>,
}

/// Connection/communication settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionSettings {
    /// Serial port name
    pub port: Option<String>,

    /// Baud rate
    pub baud_rate: u32,

    /// Connection timeout in milliseconds
    pub timeout_ms: u32,
}

/// Project behavior settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSettings {
    /// Automatically load CurrentTune.msq on project open
    pub auto_load_tune: bool,

    /// Automatically save tune to CurrentTune.msq on close
    pub auto_save_tune: bool,

    /// Auto-connect on project open
    pub auto_connect: bool,

    /// Maximum number of restore points to keep (0 = unlimited)
    #[serde(default = "default_max_restore_points")]
    pub max_restore_points: u32,
}

fn default_max_restore_points() -> u32 {
    20
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            auto_load_tune: true,
            auto_save_tune: true,
            auto_connect: false,
            max_restore_points: default_max_restore_points(),
        }
    }
}

impl Default for ProjectConfig {
    fn default() -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            version: "1.0".to_string(),
            name: String::new(),
            created: now.clone(),
            modified: now,
            ecu_definition: "projectCfg/definition.ini".to_string(),
            signature: String::new(),
            connection: ConnectionSettings {
                port: None,
                baud_rate: 115200,
                timeout_ms: 1000,
            },
            settings: ProjectSettings::default(),
            dashboard: None,
        }
    }
}

/// A LibreTune project
#[derive(Debug)]
pub struct Project {
    /// Project folder path
    pub path: PathBuf,

    /// Project configuration
    pub config: ProjectConfig,

    /// Currently loaded tune (if any)
    pub current_tune: Option<TuneFile>,

    /// Whether the project has unsaved changes
    pub dirty: bool,
}

impl Project {
    /// Get the default projects directory
    pub fn projects_dir() -> io::Result<PathBuf> {
        let base = dirs::document_dir()
            .or_else(dirs::home_dir)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "Could not find home directory")
            })?;
        Ok(base.join("LibreTuneProjects"))
    }

    /// Create a new project
    ///
    /// # Arguments
    /// * `name` - Project name (used as folder name)
    /// * `ini_source` - Path to INI file to copy into project
    /// * `signature` - ECU signature from the INI
    /// * `parent_dir` - Optional parent directory (defaults to projects_dir)
    pub fn create(
        name: &str,
        ini_source: &Path,
        signature: &str,
        parent_dir: Option<&Path>,
    ) -> io::Result<Self> {
        let parent = match parent_dir {
            Some(p) => p.to_path_buf(),
            None => Self::projects_dir()?,
        };

        // Sanitize project name for filesystem
        let safe_name: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        let project_path = parent.join(&safe_name);

        // Don't overwrite existing project
        if project_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Project '{}' already exists", name),
            ));
        }

        // Create directory structure
        fs::create_dir_all(&project_path)?;
        fs::create_dir_all(project_path.join("projectCfg"))?;
        fs::create_dir_all(project_path.join("datalogs"))?;
        fs::create_dir_all(project_path.join("dashboards"))?;

        // Copy INI file
        let ini_dest = project_path.join("projectCfg").join("definition.ini");
        fs::copy(ini_source, &ini_dest)?;

        // Create project config
        let now = Utc::now().to_rfc3339();
        let config = ProjectConfig {
            version: "1.0".to_string(),
            name: name.to_string(),
            created: now.clone(),
            modified: now,
            ecu_definition: "projectCfg/definition.ini".to_string(),
            signature: signature.to_string(),
            connection: ConnectionSettings::default(),
            settings: ProjectSettings::default(),
            dashboard: None,
        };

        let mut project = Project {
            path: project_path,
            config,
            current_tune: None,
            dirty: false,
        };

        // Save project.json
        project.save_config()?;

        Ok(project)
    }

    /// Open an existing project
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Load project.json
        let config_path = path.join("project.json");
        let config_content = fs::read_to_string(&config_path)?;
        let config: ProjectConfig = serde_json::from_str(&config_content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let mut project = Project {
            path,
            config,
            current_tune: None,
            dirty: false,
        };

        // Auto-load tune if enabled
        if project.config.settings.auto_load_tune {
            let _ = project.load_current_tune(); // Ignore error if no tune exists
        }

        Ok(project)
    }

    /// Import a TunerStudio project into LibreTune format
    ///
    /// Reads project.properties, vehicle.properties, and copies relevant files.
    /// Creates a new LibreTune project in the projects directory.
    ///
    /// # Arguments
    /// * `ts_project_path` - Path to the TunerStudio project folder
    /// * `target_dir` - Optional target directory (defaults to projects_dir)
    pub fn import_tunerstudio<P: AsRef<Path>>(
        ts_project_path: P,
        target_dir: Option<&Path>,
    ) -> io::Result<Self> {
        let ts_path = ts_project_path.as_ref();

        // Look for project.properties in projectCfg subfolder
        let project_props_path = ts_path.join("projectCfg").join("project.properties");
        if !project_props_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Not a valid TS project: project.properties not found",
            ));
        }

        let project_props = Properties::load(&project_props_path)?;

        // Extract project name
        let project_name = project_props
            .get("projectName")
            .cloned()
            .unwrap_or_else(|| {
                ts_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Imported Project".to_string())
            });

        // Find the INI file
        let ini_filename = project_props
            .get("ecuConfigFile")
            .cloned()
            .unwrap_or_else(|| "mainController.ini".to_string());

        let ini_path = ts_path.join("projectCfg").join(&ini_filename);
        if !ini_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("INI file not found: {}", ini_filename),
            ));
        }

        // Read signature from INI file (handle ISO-8859-1 encoding)
        let ini_bytes = fs::read(&ini_path)?;
        let ini_content = match String::from_utf8(ini_bytes.clone()) {
            Ok(s) => s,
            Err(_) => ini_bytes.iter().map(|&b| b as char).collect(),
        };
        let signature = extract_signature(&ini_content).unwrap_or_default();

        // Create the new LibreTune project
        let parent = target_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| Self::projects_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let mut project = Self::create(&project_name, &ini_path, &signature, Some(&parent))?;

        // Copy connection settings
        if let Some(port) = project_props.get("commPort") {
            project.config.connection.port = Some(port.clone());
        }
        if let Some(baud) = project_props.get_i32("baudRate") {
            project.config.connection.baud_rate = baud as u32;
        }

        // Copy CurrentTune.msq if it exists
        let ts_tune_path = ts_path.join("CurrentTune.msq");
        if ts_tune_path.exists() {
            let dest_tune_path = project.current_tune_path();
            fs::copy(&ts_tune_path, &dest_tune_path)?;
            project.load_current_tune()?;
        }

        // Copy pcVariableValues.msq if it exists
        let ts_pc_path = ts_path.join("projectCfg").join("pcVariableValues.msq");
        if ts_pc_path.exists() {
            let dest_pc_path = project.pc_variables_path();
            fs::copy(&ts_pc_path, &dest_pc_path)?;
            // Reload tune to pick up PC variables
            if project.current_tune.is_some() {
                project.load_current_tune()?;
            }
        }

        // Copy restore points if they exist
        let ts_restore_dir = ts_path.join("restorePoints");
        if ts_restore_dir.exists() {
            let dest_restore_dir = project.restore_points_dir();
            fs::create_dir_all(&dest_restore_dir)?;
            for entry in fs::read_dir(&ts_restore_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "msq") {
                    let dest = dest_restore_dir.join(path.file_name().unwrap());
                    fs::copy(&path, &dest)?;
                }
            }
        }

        // Save updated config
        project.save_config()?;

        Ok(project)
    }

    /// Save project configuration
    pub fn save_config(&mut self) -> io::Result<()> {
        self.config.modified = Utc::now().to_rfc3339();

        let config_path = self.path.join("project.json");
        let content = serde_json::to_string_pretty(&self.config)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(config_path, content)?;

        Ok(())
    }

    /// Get the path to the ECU definition INI
    pub fn ini_path(&self) -> PathBuf {
        self.path.join(&self.config.ecu_definition)
    }

    /// Get the path to CurrentTune.msq
    pub fn current_tune_path(&self) -> PathBuf {
        self.path.join("CurrentTune.msq")
    }

    /// Get the path to pcVariableValues.msq
    pub fn pc_variables_path(&self) -> PathBuf {
        self.path.join("projectCfg").join("pcVariableValues.msq")
    }

    /// Load the current tune from disk
    pub fn load_current_tune(&mut self) -> io::Result<()> {
        let tune_path = self.current_tune_path();
        if tune_path.exists() {
            self.current_tune = Some(TuneFile::load(&tune_path)?);

            // Also load PC variables if they exist
            let pc_path = self.pc_variables_path();
            if pc_path.exists() {
                if let Some(ref mut tune) = self.current_tune {
                    let _ = tune.load_pc_variables(&pc_path);
                }
            }
        }
        Ok(())
    }

    /// Save the current tune to disk
    pub fn save_current_tune(&self) -> io::Result<()> {
        if let Some(ref tune) = self.current_tune {
            tune.save(self.current_tune_path())?;

            // Also save PC variables separately
            if !tune.pc_variables.is_empty() {
                let _ = tune.save_pc_variables(self.pc_variables_path(), &self.config.signature);
            }
        }
        Ok(())
    }

    /// Close project (saves if auto-save enabled)
    pub fn close(self) -> io::Result<()> {
        if self.config.settings.auto_save_tune {
            self.save_current_tune()?;
        }
        Ok(())
    }

    /// Get the restore points directory
    pub fn restore_points_dir(&self) -> PathBuf {
        self.path.join("restorePoints")
    }

    /// Create a restore point from the current tune
    ///
    /// Returns the path to the created restore point file.
    /// Format: `{ProjectName}_{YYYY-MM-DD_HH.MM.SS}.msq`
    pub fn create_restore_point(&self) -> io::Result<PathBuf> {
        let Some(ref tune) = self.current_tune else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No tune loaded to create restore point from",
            ));
        };

        let restore_dir = self.restore_points_dir();
        fs::create_dir_all(&restore_dir)?;

        // Generate timestamped filename
        let timestamp = Utc::now().format("%Y-%m-%d_%H.%M.%S");
        let safe_name: String = self
            .config
            .name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        let filename = format!("{}_{}.msq", safe_name, timestamp);
        let restore_path = restore_dir.join(&filename);

        tune.save(&restore_path)?;

        Ok(restore_path)
    }

    /// List all restore points for this project
    pub fn list_restore_points(&self) -> io::Result<Vec<RestorePointInfo>> {
        let restore_dir = self.restore_points_dir();

        if !restore_dir.exists() {
            return Ok(Vec::new());
        }

        let mut points = Vec::new();

        for entry in fs::read_dir(&restore_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map_or(false, |e| e == "msq") {
                let filename = path.file_name().unwrap().to_string_lossy().to_string();
                let metadata = entry.metadata()?;

                // Try to parse timestamp from filename
                let timestamp = parse_restore_point_timestamp(&filename);

                points.push(RestorePointInfo {
                    filename,
                    path: path.clone(),
                    created: timestamp,
                    size_bytes: metadata.len(),
                });
            }
        }

        // Sort by created date, newest first
        points.sort_by(|a, b| b.created.cmp(&a.created));

        Ok(points)
    }

    /// Load a restore point as the current tune
    pub fn load_restore_point(&mut self, filename: &str) -> io::Result<()> {
        let restore_path = self.restore_points_dir().join(filename);

        if !restore_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Restore point not found: {}", filename),
            ));
        }

        self.current_tune = Some(TuneFile::load(&restore_path)?);
        self.dirty = true;

        Ok(())
    }

    /// Delete a restore point
    pub fn delete_restore_point(&self, filename: &str) -> io::Result<()> {
        let restore_path = self.restore_points_dir().join(filename);

        if !restore_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Restore point not found: {}", filename),
            ));
        }

        fs::remove_file(restore_path)
    }

    /// Prune old restore points, keeping only the most recent N
    pub fn prune_restore_points(&self, keep_count: usize) -> io::Result<usize> {
        let points = self.list_restore_points()?;

        if points.len() <= keep_count {
            return Ok(0);
        }

        let mut deleted = 0;
        for point in points.into_iter().skip(keep_count) {
            if self.delete_restore_point(&point.filename).is_ok() {
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    /// List all projects in the default projects directory
    pub fn list_projects() -> io::Result<Vec<ProjectInfo>> {
        let projects_dir = Self::projects_dir()?;

        if !projects_dir.exists() {
            return Ok(Vec::new());
        }

        let mut projects = Vec::new();

        for entry in fs::read_dir(&projects_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let config_path = path.join("project.json");
                if config_path.exists() {
                    if let Ok(content) = fs::read_to_string(&config_path) {
                        if let Ok(config) = serde_json::from_str::<ProjectConfig>(&content) {
                            projects.push(ProjectInfo {
                                name: config.name,
                                path: path.clone(),
                                signature: config.signature,
                                modified: config.modified,
                            });
                        }
                    }
                }
            }
        }

        // Sort by modified date, newest first
        projects.sort_by(|a, b| b.modified.cmp(&a.modified));

        Ok(projects)
    }
}

/// Summary info about a project (for listing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: PathBuf,
    pub signature: String,
    pub modified: String,
}

/// Info about a restore point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePointInfo {
    /// Filename of the restore point
    pub filename: String,
    /// Full path to the file
    pub path: PathBuf,
    /// When the restore point was created (parsed from filename or file metadata)
    pub created: String,
    /// File size in bytes
    pub size_bytes: u64,
}

/// Parse timestamp from restore point filename
///
/// Expected format: `{Name}_{YYYY-MM-DD_HH.MM.SS}.msq`
fn parse_restore_point_timestamp(filename: &str) -> String {
    // Try to find the timestamp pattern at the end
    // Look for _YYYY-MM-DD_HH.MM.SS.msq
    let without_ext = filename.strip_suffix(".msq").unwrap_or(filename);

    // Find the last underscore that starts a date pattern
    if let Some(pos) = without_ext.rfind('_') {
        let date_part = &without_ext[..pos];
        if let Some(date_pos) = date_part.rfind('_') {
            // Extract YYYY-MM-DD_HH.MM.SS
            let timestamp = &without_ext[date_pos + 1..];
            // Convert from YYYY-MM-DD_HH.MM.SS to ISO format
            if timestamp.len() >= 19 {
                let date = &timestamp[0..10];
                let time = timestamp[11..].replace('.', ":");
                return format!("{}T{}Z", date, time);
            }
        }
    }

    // Fallback: use current time
    Utc::now().to_rfc3339()
}

/// Extract the signature from an INI file content
fn extract_signature(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.to_lowercase().starts_with("signature") {
            if let Some(eq_pos) = line.find('=') {
                let value = line[eq_pos + 1..].trim();
                // Remove quotes
                let value = value.trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_project_creation() {
        let temp = temp_dir().join("libretune_test_projects");
        let _ = fs::remove_dir_all(&temp);

        // Create a fake INI file
        let ini_path = temp.join("test.ini");
        fs::create_dir_all(&temp).unwrap();
        fs::write(&ini_path, "[MegaTune]\nsignature = \"TestECU 1.0\"").unwrap();

        // Create project
        let project =
            Project::create("Test Project", &ini_path, "TestECU 1.0", Some(&temp)).unwrap();

        assert_eq!(project.config.name, "Test Project");
        assert_eq!(project.config.signature, "TestECU 1.0");
        assert!(project.path.join("project.json").exists());
        assert!(project.path.join("projectCfg/definition.ini").exists());

        // Cleanup
        let _ = fs::remove_dir_all(&temp);
    }
}
