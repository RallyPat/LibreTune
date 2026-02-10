//! Plugin API surface - Host functions exposed to WASM plugins.
//!
//! Provides secure, permission-checked API for WASM plugins to:
//! - Read table and constant data
//! - Subscribe to realtime channels
//! - Log messages for debugging
//! - Execute action sequences
//!
//! All functions validate plugin permissions before executing.

use crate::plugin_system::{Permission, PluginManager};
use std::sync::Mutex;

/// Plugin API context shared with WASM host functions.
pub struct PluginApiContext {
    /// Plugin manager reference
    pub plugin_manager: Mutex<PluginManager>,
}

impl PluginApiContext {
    /// Create new API context with plugin manager.
    pub fn new(plugin_manager: PluginManager) -> Self {
        PluginApiContext {
            plugin_manager: Mutex::new(plugin_manager),
        }
    }

    /// Check if plugin has permission (non-panicking).
    fn check_permission(&self, plugin_name: &str, perm: Permission) -> bool {
        if let Ok(manager) = self.plugin_manager.lock() {
            manager.check_permission(plugin_name, perm)
        } else {
            false
        }
    }
}

/// Plugin API response for any function call.
#[derive(Debug, Clone)]
pub struct ApiResponse {
    /// Success or error
    pub success: bool,
    /// Optional data payload (as serialized bytes)
    pub data: Vec<u8>,
    /// Error message if failed
    pub error: String,
}

impl ApiResponse {
    /// Create successful response with data.
    pub fn ok(data: Vec<u8>) -> Self {
        ApiResponse {
            success: true,
            data,
            error: String::new(),
        }
    }

    /// Create success response with empty data.
    pub fn ok_empty() -> Self {
        ApiResponse {
            success: true,
            data: Vec::new(),
            error: String::new(),
        }
    }

    /// Create error response.
    pub fn error(msg: impl Into<String>) -> Self {
        ApiResponse {
            success: false,
            data: Vec::new(),
            error: msg.into(),
        }
    }

    /// Create permission denied response.
    pub fn permission_denied(perm_name: &str) -> Self {
        ApiResponse {
            success: false,
            data: Vec::new(),
            error: format!("Permission denied: {}", perm_name),
        }
    }
}

/// Plugin log level for messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

impl LogLevel {
    /// Convert from numeric code.
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => LogLevel::Debug,
            1 => LogLevel::Info,
            2 => LogLevel::Warn,
            _ => LogLevel::Error,
        }
    }

    /// Convert to string for display.
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

/// Plugin log message with timestamp.
#[derive(Debug, Clone)]
pub struct PluginLogMessage {
    /// Plugin name
    pub plugin_name: String,
    /// Log level
    pub level: LogLevel,
    /// Message text
    pub message: String,
    /// Timestamp in milliseconds since epoch
    pub timestamp_ms: u64,
}

impl PluginLogMessage {
    /// Create new log message.
    pub fn new(
        plugin_name: impl Into<String>,
        level: LogLevel,
        message: impl Into<String>,
    ) -> Self {
        PluginLogMessage {
            plugin_name: plugin_name.into(),
            level,
            message: message.into(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        }
    }

    /// Format log message for display.
    pub fn format_display(&self) -> String {
        format!(
            "[{}] {} [{}]: {}",
            self.timestamp_ms,
            self.plugin_name,
            self.level.as_str(),
            self.message
        )
    }
}

/// Host function: Get table data (readonly).
///
/// # Permissions
/// Requires `ReadTables` permission.
///
/// # Arguments
/// * `plugin_name` - Name of calling plugin
/// * `table_name` - Name of table to read
/// * `row`, `col` - Cell coordinates (-1 for header)
///
/// # Returns
/// ApiResponse with value as bytes or error
pub fn api_get_table_data(
    _ctx: &PluginApiContext,
    plugin_name: &str,
    _table_name: &str,
    _row: i32,
    _col: i32,
) -> ApiResponse {
    // Permission check
    if !_ctx.check_permission(plugin_name, Permission::ReadTables) {
        return ApiResponse::permission_denied("ReadTables");
    }

    // Implementation would fetch from ECU memory model
    // For now, return placeholder response
    ApiResponse::ok(vec![0u8; 4]) // 4 bytes for f32 value
}

/// Host function: Get constant value.
///
/// # Permissions
/// Requires `ReadTables` permission (constants are table-like data).
///
/// # Arguments
/// * `plugin_name` - Name of calling plugin
/// * `constant_name` - Name of constant
///
/// # Returns
/// ApiResponse with value as bytes or error
pub fn api_get_constant(
    _ctx: &PluginApiContext,
    plugin_name: &str,
    _constant_name: &str,
) -> ApiResponse {
    // Permission check
    if !_ctx.check_permission(plugin_name, Permission::ReadTables) {
        return ApiResponse::permission_denied("ReadTables");
    }

    // Implementation would fetch from tune cache
    ApiResponse::ok(vec![0u8; 4]) // 4 bytes for value
}

/// Host function: Set constant value.
///
/// # Permissions
/// Requires `WriteConstants` permission.
///
/// # Arguments
/// * `plugin_name` - Name of calling plugin
/// * `constant_name` - Name of constant
/// * `value_bytes` - Raw bytes to write
///
/// # Returns
/// ApiResponse with success or error
pub fn api_set_constant(
    _ctx: &PluginApiContext,
    plugin_name: &str,
    _constant_name: &str,
    _value_bytes: &[u8],
) -> ApiResponse {
    // Permission check
    if !_ctx.check_permission(plugin_name, Permission::WriteConstants) {
        return ApiResponse::permission_denied("WriteConstants");
    }

    // Implementation would write to tune cache
    ApiResponse::ok_empty()
}

/// Host function: Subscribe to realtime channel.
///
/// # Permissions
/// Requires `SubscribeChannels` permission.
///
/// # Arguments
/// * `plugin_name` - Name of calling plugin
/// * `channel_name` - Name of channel (e.g., "RPM", "AFR")
///
/// # Returns
/// ApiResponse with channel ID or error
pub fn api_subscribe_channel(
    _ctx: &PluginApiContext,
    plugin_name: &str,
    _channel_name: &str,
) -> ApiResponse {
    // Permission check
    if !_ctx.check_permission(plugin_name, Permission::SubscribeChannels) {
        return ApiResponse::permission_denied("SubscribeChannels");
    }

    // Implementation would register channel subscription
    // Return channel ID as bytes
    ApiResponse::ok(vec![0u8; 4]) // Channel ID
}

/// Host function: Get realtime value for subscribed channel.
///
/// # Permissions
/// Requires `SubscribeChannels` permission.
///
/// # Arguments
/// * `plugin_name` - Name of calling plugin
/// * `channel_id` - ID from subscribe call
///
/// # Returns
/// ApiResponse with current value or error
pub fn api_get_channel_value(
    _ctx: &PluginApiContext,
    plugin_name: &str,
    _channel_id: u32,
) -> ApiResponse {
    // Permission check
    if !_ctx.check_permission(plugin_name, Permission::SubscribeChannels) {
        return ApiResponse::permission_denied("SubscribeChannels");
    }

    // Implementation would fetch current value
    ApiResponse::ok(vec![0u8; 4]) // f32 value as bytes
}

/// Host function: Log a message from plugin.
///
/// No special permissions required (logging is always allowed).
///
/// # Arguments
/// * `plugin_name` - Name of calling plugin
/// * `level` - Log level (0=Debug, 1=Info, 2=Warn, 3=Error)
/// * `message` - Message text
pub fn api_log_message(
    _ctx: &PluginApiContext,
    plugin_name: impl Into<String>,
    level: i32,
    message: impl Into<String>,
) -> ApiResponse {
    let log_level = LogLevel::from_code(level);
    let log_msg = PluginLogMessage::new(plugin_name, log_level, message);

    // In real implementation, would write to log file or channel
    eprintln!("{}", log_msg.format_display());

    ApiResponse::ok_empty()
}

/// Host function: Execute action sequence.
///
/// # Permissions
/// Requires `ExecuteActions` permission.
///
/// # Arguments
/// * `plugin_name` - Name of calling plugin
/// * `action_json` - JSON-encoded action data
///
/// # Returns
/// ApiResponse with execution result or error
pub fn api_execute_action(
    _ctx: &PluginApiContext,
    plugin_name: &str,
    _action_json: &str,
) -> ApiResponse {
    // Permission check
    if !_ctx.check_permission(plugin_name, Permission::ExecuteActions) {
        return ApiResponse::permission_denied("ExecuteActions");
    }

    // Implementation would parse JSON and execute action
    ApiResponse::ok_empty()
}

/// Host function: Get plugin info.
///
/// Returns plugin manifest and current stats.
///
/// # Arguments
/// * `plugin_name` - Name of plugin to query
///
/// # Returns
/// ApiResponse with JSON-encoded plugin info
pub fn api_get_plugin_info(_ctx: &PluginApiContext, plugin_name: &str) -> ApiResponse {
    if let Ok(manager) = _ctx.plugin_manager.lock() {
        if let Some(plugin) = manager.get_plugin(plugin_name) {
            let manifest = plugin.manifest();
            let stats = plugin.stats();

            let info = format!(
                r#"{{"name":"{}","version":"{}","state":"{}","exec_count":{}}}"#,
                manifest.name,
                manifest.version,
                match stats.state {
                    crate::plugin_system::PluginState::Loaded => "loaded",
                    crate::plugin_system::PluginState::Ready => "ready",
                    crate::plugin_system::PluginState::Running => "running",
                    crate::plugin_system::PluginState::Unloading => "unloading",
                    crate::plugin_system::PluginState::Disabled => "disabled",
                },
                stats.exec_count
            );

            return ApiResponse::ok(info.into_bytes());
        }
    }

    ApiResponse::error("Plugin not found")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_system::{PluginConfig, PluginManager};

    fn create_test_context() -> PluginApiContext {
        let config = PluginConfig {
            data_dir: "/tmp".to_string(),
            ecu_type: "Speeduino".to_string(),
            libretune_version: "0.1.0".to_string(),
        };
        let manager = PluginManager::new(config);
        PluginApiContext::new(manager)
    }

    #[test]
    fn test_api_response_ok() {
        let resp = ApiResponse::ok(vec![1, 2, 3]);
        assert!(resp.success);
        assert_eq!(resp.data, vec![1, 2, 3]);
        assert!(resp.error.is_empty());
    }

    #[test]
    fn test_api_response_ok_empty() {
        let resp = ApiResponse::ok_empty();
        assert!(resp.success);
        assert!(resp.data.is_empty());
    }

    #[test]
    fn test_api_response_error() {
        let resp = ApiResponse::error("test error");
        assert!(!resp.success);
        assert_eq!(resp.error, "test error");
    }

    #[test]
    fn test_api_response_permission_denied() {
        let resp = ApiResponse::permission_denied("WriteConstants");
        assert!(!resp.success);
        assert!(resp.error.contains("WriteConstants"));
    }

    #[test]
    fn test_log_level_from_code() {
        assert_eq!(LogLevel::from_code(0), LogLevel::Debug);
        assert_eq!(LogLevel::from_code(1), LogLevel::Info);
        assert_eq!(LogLevel::from_code(2), LogLevel::Warn);
        assert_eq!(LogLevel::from_code(3), LogLevel::Error);
        assert_eq!(LogLevel::from_code(99), LogLevel::Error);
    }

    #[test]
    fn test_log_level_as_str() {
        assert_eq!(LogLevel::Debug.as_str(), "DEBUG");
        assert_eq!(LogLevel::Info.as_str(), "INFO");
        assert_eq!(LogLevel::Warn.as_str(), "WARN");
        assert_eq!(LogLevel::Error.as_str(), "ERROR");
    }

    #[test]
    fn test_plugin_log_message_creation() {
        let msg = PluginLogMessage::new("test_plugin", LogLevel::Info, "test message");
        assert_eq!(msg.plugin_name, "test_plugin");
        assert_eq!(msg.level, LogLevel::Info);
        assert_eq!(msg.message, "test message");
        assert!(msg.timestamp_ms > 0);
    }

    #[test]
    fn test_plugin_log_message_format() {
        let msg = PluginLogMessage::new("test", LogLevel::Error, "failure");
        let formatted = msg.format_display();
        assert!(formatted.contains("test"));
        assert!(formatted.contains("ERROR"));
        assert!(formatted.contains("failure"));
    }

    #[test]
    fn test_plugin_api_context_creation() {
        let ctx = create_test_context();
        assert_eq!(ctx.plugin_manager.lock().unwrap().count(), 0);
    }

    #[test]
    fn test_api_get_table_data_no_permission() {
        let ctx = create_test_context();
        let resp = api_get_table_data(&ctx, "unknown_plugin", "veTable1", 0, 0);
        assert!(!resp.success);
        assert!(resp.error.contains("Permission denied"));
    }

    #[test]
    fn test_api_get_constant_no_permission() {
        let ctx = create_test_context();
        let resp = api_get_constant(&ctx, "unknown_plugin", "rpm");
        assert!(!resp.success);
    }

    #[test]
    fn test_api_set_constant_no_permission() {
        let ctx = create_test_context();
        let resp = api_set_constant(&ctx, "unknown_plugin", "rpm", &[0, 0, 0, 0]);
        assert!(!resp.success);
    }

    #[test]
    fn test_api_subscribe_channel_no_permission() {
        let ctx = create_test_context();
        let resp = api_subscribe_channel(&ctx, "unknown_plugin", "RPM");
        assert!(!resp.success);
    }

    #[test]
    fn test_api_get_channel_value_no_permission() {
        let ctx = create_test_context();
        let resp = api_get_channel_value(&ctx, "unknown_plugin", 0);
        assert!(!resp.success);
    }

    #[test]
    fn test_api_execute_action_no_permission() {
        let ctx = create_test_context();
        let resp = api_execute_action(&ctx, "unknown_plugin", "{}");
        assert!(!resp.success);
    }

    #[test]
    fn test_api_log_message_always_allowed() {
        let ctx = create_test_context();
        let resp = api_log_message(&ctx, "test_plugin", 1, "test log");
        assert!(resp.success);
    }

    #[test]
    fn test_api_get_plugin_info_not_found() {
        let ctx = create_test_context();
        let resp = api_get_plugin_info(&ctx, "nonexistent");
        assert!(!resp.success);
    }

    #[test]
    fn test_api_response_data_integrity() {
        let data = vec![1, 2, 3, 4, 5];
        let resp = ApiResponse::ok(data.clone());
        assert_eq!(resp.data, data);
    }

    #[test]
    fn test_log_level_ordering() {
        // Verify numeric ordering matches severity
        assert!((LogLevel::Debug as i32) < (LogLevel::Info as i32));
        assert!((LogLevel::Info as i32) < (LogLevel::Warn as i32));
        assert!((LogLevel::Warn as i32) < (LogLevel::Error as i32));
    }
}
