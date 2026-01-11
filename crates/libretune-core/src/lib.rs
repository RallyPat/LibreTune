//! # LibreTune Core Library
//!
//! Core functionality for the LibreTune ECU tuning software.

#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

//!
//! This library provides:
//! - INI definition file parsing (standard ECU INI format)
//! - Serial protocol communication with ECUs
//! - ECU memory model and value management
//! - Data logging and playback
//! - Tune file management
//!
//! ## Supported ECUs
//!
//! - Speeduino
//! - EpicEFI
//! - Other INI-compatible ECUs
//!
//! ## Example
//!
//! ```rust,ignore
//! use libretune_core::{ini::EcuDefinition, protocol::Connection};
//!
//! // Load ECU definition from INI file
//! let definition = EcuDefinition::from_file("speeduino.ini")?;
//!
//! // Connect to ECU
//! let mut conn = Connection::open("/dev/ttyUSB0", 115200)?;
//! conn.handshake(&definition)?;
//!
//! // Read real-time data
//! let data = conn.get_realtime_data()?;
//! println!("RPM: {}", data.get("rpm")?);
//! ```

pub mod autotune;
pub mod dash;
pub mod dashboard;
pub mod datalog;
pub mod demo;
pub mod ecu;
pub mod ini;
pub mod plugin;
pub mod project;
pub mod protocol;
pub mod table_ops;
pub mod tune;
pub mod unit_conversion;

/// Re-export commonly used types
pub mod prelude {
    pub use crate::autotune::{
        AutoTuneAuthorityLimits, AutoTuneFilters, AutoTuneRecommendation, AutoTuneSettings,
        AutoTuneState,
    };
    pub use crate::dashboard::{
        get_dashboard_file, get_dashboard_file_path, DashboardLayout, GaugeConfig, GaugeType,
    };
    pub use crate::datalog::{DataLogger, LogEntry};
    pub use crate::ecu::{EcuMemory, Value};
    pub use crate::ini::{Constant, EcuDefinition, OutputChannel, TableDefinition};
    pub use crate::project::{IniEntry, IniRepository, Project, ProjectConfig};
    pub use crate::protocol::{Connection, ConnectionState};
    pub use crate::tune::{PageState, TuneCache, TuneFile};
}

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
