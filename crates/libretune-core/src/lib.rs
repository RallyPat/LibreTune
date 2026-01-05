//! # LibreTune Core Library
//!
//! Core functionality for the LibreTune ECU tuning software.
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

pub mod ini;
pub mod protocol;
pub mod ecu;
pub mod datalog;
pub mod tune;
pub mod autotune;
pub mod dashboard;
pub mod dash;
pub mod table_ops;
pub mod project;
pub mod demo;
pub mod unit_conversion;

/// Re-export commonly used types
pub mod prelude {
    pub use crate::ini::{EcuDefinition, Constant, OutputChannel, TableDefinition};
    pub use crate::protocol::{Connection, ConnectionState};
    pub use crate::ecu::{EcuMemory, Value};
    pub use crate::datalog::{DataLogger, LogEntry};
    pub use crate::autotune::{
        AutoTuneState, AutoTuneSettings, AutoTuneFilters, 
        AutoTuneAuthorityLimits, AutoTuneRecommendation
    };
    pub use crate::dashboard::{
        DashboardLayout, GaugeConfig, GaugeType,
        get_dashboard_file, get_dashboard_file_path
    };
    pub use crate::tune::{TuneFile, TuneCache, PageState};
    pub use crate::project::{Project, ProjectConfig, IniRepository, IniEntry};
}

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
