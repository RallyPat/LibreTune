//! Project struct and management functions

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            auto_load_tune: true,
            auto_save_tune: true,
            auto_connect: false,
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

    /// Load the current tune from disk
    pub fn load_current_tune(&mut self) -> io::Result<()> {
        let tune_path = self.current_tune_path();
        if tune_path.exists() {
            self.current_tune = Some(TuneFile::load(&tune_path)?);
        }
        Ok(())
    }

    /// Save the current tune to disk
    pub fn save_current_tune(&self) -> io::Result<()> {
        if let Some(ref tune) = self.current_tune {
            tune.save(self.current_tune_path())?;
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
