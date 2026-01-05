//! Data Logging
//!
//! Records and plays back ECU real-time data.

mod recorder;
mod format;
mod playback;

pub use recorder::DataLogger;
pub use format::LogFormat;
pub use playback::LogPlayer;

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A single log entry with timestamp and values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp from start of logging
    pub timestamp: Duration,
    /// Channel values (in order of datalog definition)
    pub values: Vec<f64>,
}

impl LogEntry {
    /// Create a new log entry
    pub fn new(timestamp: Duration, values: Vec<f64>) -> Self {
        Self { timestamp, values }
    }
}
