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
//! ├── CurrentTune.msq       # Auto-saved current tune (TunerStudio compatible)
//! ├── datalogs/             # Data log files
//! ├── dashboards/           # Dashboard configurations
//! └── projectCfg/
//!     ├── definition.ini    # Copy of ECU definition file
//!     └── custom.ini        # Optional user overrides
//! ```
//!
//! This matches TunerStudio's project layout for familiarity.

mod project;
mod repository;
mod online_repository;

pub use project::{Project, ProjectConfig, ProjectSettings, ConnectionSettings, ProjectInfo};
pub use repository::{IniRepository, IniEntry};
pub use online_repository::{OnlineIniRepository, OnlineIniEntry, IniSource};
