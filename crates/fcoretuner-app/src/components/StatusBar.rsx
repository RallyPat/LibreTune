//! Status Bar Component
//!
//! Displays connection status, ECU signature, and operational state.
//! Modeled after TunerStudio status bar.

use serde::{Serialize, Deserialize};

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ConnectionState {
    #[serde(rename = "disconnected")]
    Disconnected,
    #[serde(rename = "connecting")]
    Connecting,
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "error")]
    Error,
}

/// Status bar component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusBar {
    /// Current connection state
    pub connection_state: ConnectionState,
    /// ECU signature (if connected)
    pub signature: Option<String>,
    /// Currently loaded INI name
    pub ini_name: Option<String>,
    /// Error message (if any)
    pub error_message: Option<String>,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            connection_state: ConnectionState::Disconnected,
            signature: None,
            ini_name: None,
            error_message: None,
        }
    }
}
