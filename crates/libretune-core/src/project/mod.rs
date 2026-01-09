//! Project Management
//!
//! Handles LibreTune projects which bundle an ECU definition (INI file),
//! tune files, data logs, and project settings together.
//!
//! ## Project Structure
//!
//! A project is a folder containing:
//! ```text
//! [ProjectName]/
//! ├── project.json          # Project metadata and settings
//! ├── CurrentTune.msq       # Auto-saved current tune (TS-compatible)
//! ├── datalogs/             # Data log files
//! ├── dashboards/           # Dashboard configurations
//! ├── restorePoints/        # Timestamped tune backups
//! └── projectCfg/
//!     ├── definition.ini    # Copy of ECU definition file
//!     ├── pcVariableValues.msq  # PC variable values
//!     └── custom.ini        # Optional user overrides
//! ```
//!
//! This matches TS project layout for familiarity.

mod online_repository;
mod project;
mod properties;
mod repository;

pub use online_repository::{IniSource, OnlineIniEntry, OnlineIniRepository};
pub use project::{
    ConnectionSettings, Project, ProjectConfig, ProjectInfo, ProjectSettings, RestorePointInfo,
};
pub use properties::Properties;
pub use repository::{IniEntry, IniRepository};
