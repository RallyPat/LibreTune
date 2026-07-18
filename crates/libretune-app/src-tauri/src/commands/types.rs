//! Public(crate) type definitions exposed by Tauri commands.

use libretune_core::protocol::ConnectionState;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct ConnectionStatus {
    pub state: ConnectionState,
    pub signature: Option<String>,
    pub has_definition: bool,
    pub ini_name: Option<String>,
    pub demo_mode: bool,
}

/// Signature match type for comparing ECU and INI signatures
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SignatureMatchType {
    /// Signatures match exactly
    Exact,
    /// Signatures match partially (one contains the other, version diff)
    Partial,
    /// Signatures do not match
    Mismatch,
}

/// Information about a signature mismatch for the frontend
#[derive(Serialize, Clone)]
pub(crate) struct SignatureMismatchInfo {
    /// The signature reported by the ECU
    pub ecu_signature: String,
    /// The signature expected by the loaded INI file
    pub ini_signature: String,
    /// How closely the signatures match
    pub match_type: SignatureMatchType,
    /// Path to the currently loaded INI
    pub current_ini_path: Option<String>,
    /// List of INIs that might match the ECU signature
    pub matching_inis: Vec<MatchingIniInfo>,
}

/// Information about an INI that matches the ECU signature
#[derive(Serialize, Clone)]
pub(crate) struct MatchingIniInfo {
    /// Path to the INI file
    pub path: String,
    /// Display name of the INI
    pub name: String,
    /// Signature from this INI
    pub signature: String,
    /// How well it matches (exact or partial)
    pub match_type: SignatureMatchType,
}

/// Result of ECU connection attempt
#[derive(Serialize)]
pub(crate) struct ConnectResult {
    /// The signature reported by the ECU
    pub signature: String,
    /// Mismatch info if signatures don't match exactly
    pub mismatch_info: Option<SignatureMismatchInfo>,
}

/// Result of ECU sync operation
#[derive(Serialize)]
pub(crate) struct SyncResult {
    /// Whether all pages synced successfully
    pub success: bool,
    /// Number of pages successfully synced
    pub pages_synced: u8,
    /// Number of pages that failed to sync
    pub pages_failed: u8,
    /// Total number of pages attempted
    pub total_pages: u8,
    /// Error messages for failed pages (for logging)
    pub errors: Vec<String>,
}

/// Extended constant info for frontend with value_type field
#[derive(Serialize)]
pub(crate) struct ConstantInfo {
    pub name: String,
    pub label: Option<String>,
    pub units: String,
    pub digits: u8,
    pub min: f64,
    pub max: f64,
    pub value_type: String, // "scalar", "string", "bits", "array"
    pub bit_options: Vec<String>,
    pub help: Option<String>,
    pub visibility_condition: Option<String>, // Expression for when field should be visible
    pub is_pc_variable: bool,
}
/// Sync response with progress information
#[derive(Serialize)]
pub(crate) struct SyncProgress {
    pub current_page: u8,
    pub total_pages: u8,
    pub bytes_read: usize,
    pub total_bytes: usize,
    pub complete: bool,
    /// Optional: page that just failed (for partial sync indication)
    pub failed_page: Option<u8>,
}

#[derive(Serialize)]
pub(crate) struct CurrentProjectInfo {
    pub name: String,
    pub path: String,
    pub signature: String,
    pub has_tune: bool,
    pub tune_modified: bool,
    pub connection: ConnectionSettingsResponse,
}

#[derive(Serialize)]
pub(crate) struct ConnectionSettingsResponse {
    pub port: Option<String>,
    pub baud_rate: u32,
    pub auto_connect: bool,
}
