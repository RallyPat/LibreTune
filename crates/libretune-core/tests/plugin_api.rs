//! Plugin API integration tests
//!
//! Tests the WASM host function API surface and permission enforcement.

use libretune_core::plugin_api::*;
use libretune_core::plugin_system::*;

fn create_test_context() -> PluginApiContext {
    let config = PluginConfig {
        data_dir: "/tmp/libretune_plugins".to_string(),
        ecu_type: "Speeduino".to_string(),
        libretune_version: "0.1.0".to_string(),
    };
    let manager = PluginManager::new(config);
    PluginApiContext::new(manager)
}

#[test]
fn test_api_response_serialization() {
    let resp = ApiResponse::ok(vec![1, 2, 3, 4]);
    assert!(resp.success);
    assert_eq!(resp.data.len(), 4);
}

#[test]
fn test_api_response_error_message() {
    let resp = ApiResponse::error("Test error");
    assert!(!resp.success);
    assert_eq!(resp.error, "Test error");
}

#[test]
fn test_permission_denied_response() {
    let resp = ApiResponse::permission_denied("ReadTables");
    assert!(!resp.success);
    assert!(resp.error.contains("ReadTables"));
    assert!(resp.error.contains("Permission denied"));
}

#[test]
fn test_api_context_initialization() {
    let ctx = create_test_context();
    assert_eq!(ctx.plugin_manager.lock().unwrap().count(), 0);
}

#[test]
fn test_api_get_table_data_permission_check() {
    let ctx = create_test_context();

    // Plugin with no permissions should be denied
    let resp = api_get_table_data(&ctx, "restricted_plugin", "veTable1", 0, 0);
    assert!(!resp.success);
    assert!(resp.error.contains("Permission"));
}

#[test]
fn test_api_get_constant_permission_check() {
    let ctx = create_test_context();

    // Plugin with no permissions should be denied
    let resp = api_get_constant(&ctx, "restricted_plugin", "rpmMin");
    assert!(!resp.success);
}

#[test]
fn test_api_set_constant_permission_check() {
    let ctx = create_test_context();

    // Plugin without WriteConstants permission should be denied
    let resp = api_set_constant(&ctx, "readonly_plugin", "rpmMin", &[0, 0, 0, 0]);
    assert!(!resp.success);
    assert!(resp.error.contains("WriteConstants"));
}

#[test]
fn test_api_subscribe_channel_permission_check() {
    let ctx = create_test_context();

    // Plugin without SubscribeChannels permission should be denied
    let resp = api_subscribe_channel(&ctx, "limited_plugin", "RPM");
    assert!(!resp.success);
}

#[test]
fn test_api_get_channel_value_permission_check() {
    let ctx = create_test_context();

    // Plugin without SubscribeChannels permission should be denied
    let resp = api_get_channel_value(&ctx, "limited_plugin", 42);
    assert!(!resp.success);
}

#[test]
fn test_api_execute_action_permission_check() {
    let ctx = create_test_context();

    // Plugin without ExecuteActions permission should be denied
    let resp = api_execute_action(&ctx, "noaccess_plugin", "{}");
    assert!(!resp.success);
    assert!(resp.error.contains("ExecuteActions"));
}

#[test]
fn test_api_log_message_always_allowed() {
    let ctx = create_test_context();

    // Logging should always be allowed, no permission needed
    let resp = api_log_message(&ctx, "test_plugin", 1, "Info message");
    assert!(resp.success);

    let resp = api_log_message(&ctx, "test_plugin", 3, "Error message");
    assert!(resp.success);

    let resp = api_log_message(&ctx, "another", 0, "Debug");
    assert!(resp.success);
}

#[test]
fn test_log_level_all_variants() {
    let levels = vec![
        (0, LogLevel::Debug),
        (1, LogLevel::Info),
        (2, LogLevel::Warn),
        (3, LogLevel::Error),
        (99, LogLevel::Error), // Out of range defaults to Error
    ];

    for (code, expected) in levels {
        assert_eq!(LogLevel::from_code(code), expected);
    }
}

#[test]
fn test_log_level_display_strings() {
    assert_eq!(LogLevel::Debug.as_str(), "DEBUG");
    assert_eq!(LogLevel::Info.as_str(), "INFO");
    assert_eq!(LogLevel::Warn.as_str(), "WARN");
    assert_eq!(LogLevel::Error.as_str(), "ERROR");
}

#[test]
fn test_plugin_log_message_timestamp() {
    let msg = PluginLogMessage::new("plugin1", LogLevel::Info, "message");
    assert!(msg.timestamp_ms > 0);

    let msg2 = PluginLogMessage::new("plugin2", LogLevel::Error, "error");
    assert!(msg2.timestamp_ms > 0);
    assert!(msg2.timestamp_ms >= msg.timestamp_ms); // Later message should have >= timestamp
}

#[test]
fn test_plugin_log_message_formatting() {
    let msg = PluginLogMessage::new("my_plugin", LogLevel::Warn, "This is a warning");
    let formatted = msg.format_display();

    assert!(formatted.contains("my_plugin"));
    assert!(formatted.contains("WARN"));
    assert!(formatted.contains("This is a warning"));
}

#[test]
fn test_api_get_plugin_info_not_found() {
    let ctx = create_test_context();

    // Plugin doesn't exist
    let resp = api_get_plugin_info(&ctx, "nonexistent_plugin");
    assert!(!resp.success);
    assert!(resp.error.contains("not found"));
}

#[test]
fn test_api_response_empty_data() {
    let resp = ApiResponse::ok_empty();
    assert!(resp.success);
    assert!(resp.data.is_empty());
    assert!(resp.error.is_empty());
}

#[test]
fn test_api_response_large_data() {
    let large_data = vec![0u8; 65536]; // 64KB
    let resp = ApiResponse::ok(large_data.clone());
    assert!(resp.success);
    assert_eq!(resp.data.len(), 65536);
}

#[test]
fn test_plugin_log_message_different_levels() {
    let debug = PluginLogMessage::new("p", LogLevel::Debug, "debug");
    let info = PluginLogMessage::new("p", LogLevel::Info, "info");
    let warn = PluginLogMessage::new("p", LogLevel::Warn, "warn");
    let error = PluginLogMessage::new("p", LogLevel::Error, "error");

    assert_eq!(debug.level, LogLevel::Debug);
    assert_eq!(info.level, LogLevel::Info);
    assert_eq!(warn.level, LogLevel::Warn);
    assert_eq!(error.level, LogLevel::Error);
}

#[test]
fn test_api_response_error_only() {
    let resp = ApiResponse::error("Critical failure");
    assert!(!resp.success);
    assert!(resp.data.is_empty());
    assert_eq!(resp.error, "Critical failure");
}

#[test]
fn test_multiple_log_messages() {
    let ctx = create_test_context();

    // Multiple logs should all succeed
    for i in 0..5 {
        let msg = format!("Log message {}", i);
        let resp = api_log_message(&ctx, "test", 1, msg);
        assert!(resp.success);
    }
}

#[test]
fn test_api_permission_enforcement_consistency() {
    let ctx = create_test_context();

    // Same plugin should get consistent permission denied response
    let resp1 = api_get_table_data(&ctx, "test_plugin", "table1", 0, 0);
    let resp2 = api_get_table_data(&ctx, "test_plugin", "table2", 5, 5);

    assert!(!resp1.success);
    assert!(!resp2.success);
    assert!(resp1.error.contains("Permission"));
    assert!(resp2.error.contains("Permission"));
}

#[test]
fn test_log_message_multiple_plugins() {
    let ctx = create_test_context();

    // Different plugins can log independently
    let resp1 = api_log_message(&ctx, "plugin_a", 0, "From A");
    let resp2 = api_log_message(&ctx, "plugin_b", 1, "From B");
    let resp3 = api_log_message(&ctx, "plugin_c", 2, "From C");

    assert!(resp1.success);
    assert!(resp2.success);
    assert!(resp3.success);
}
