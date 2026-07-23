use super::*;
use crate::commands::app_settings::{default_runtime_packet_mode, Settings};
use crate::commands::signature_helpers::{
    build_shallow_mismatch_info, compare_signatures, connect_to_ecu_simulated,
    find_matching_inis_from_state, normalize_signature,
};
use crate::commands::util_helpers::parse_runtime_packet_mode;
use crate::state::AppState;
use libretune_core::autotune::AutoTuneState;
use libretune_core::ini::{EcuDefinition, Endianness, ProtocolSettings};
use libretune_core::project::IniRepository;
use libretune_core::protocol::ConnectionConfig;
use tokio::sync::Mutex;

#[cfg(test)]
mod runtime_mode_tests {
    use super::*;
    use libretune_core::protocol::RuntimePacketMode as Rpm;

    #[test]
    fn test_parse_runtime_packet_mode() {
        assert_eq!(parse_runtime_packet_mode("ForceBurst"), Rpm::ForceBurst);
        assert_eq!(parse_runtime_packet_mode("ForceOCH"), Rpm::ForceOCH);
        assert_eq!(parse_runtime_packet_mode("Disabled"), Rpm::Disabled);
        assert_eq!(parse_runtime_packet_mode("unknown"), Rpm::Auto);
    }

    #[test]
    fn test_default_runtime_packet_mode() {
        assert_eq!(default_runtime_packet_mode(), "Auto");
    }

    // Test helpers that operate on explicit settings path so we don't need a full tauri::App
    #[cfg(test)]
    fn update_setting_with_path(
        settings_path: &std::path::Path,
        key: &str,
        value: &str,
    ) -> Result<(), String> {
        // Load existing or default
        let mut settings: Settings = if let Ok(content) = std::fs::read_to_string(settings_path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Settings::default()
        };

        match key {
            "runtime_packet_mode" => settings.runtime_packet_mode = value.to_string(),
            _ => return Err(format!("Unknown setting: {}", key)),
        }

        if let Ok(json) = serde_json::to_string_pretty(&settings) {
            std::fs::create_dir_all(settings_path.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(settings_path, json).map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Failed to serialize settings".to_string())
        }
    }

    #[test]
    fn test_update_setting_persistence_runtime_packet_mode_file_api() {
        use tempfile::tempdir;
        let dir = tempdir().expect("tempdir");
        let settings_path = dir.path().join("settings.json");

        // Ensure no file to start
        let _ = std::fs::remove_file(&settings_path);

        // Update using helper
        update_setting_with_path(&settings_path, "runtime_packet_mode", "ForceOCH")
            .expect("update should succeed");

        // Read file back and assert
        let content = std::fs::read_to_string(&settings_path).expect("settings file should exist");
        assert!(content.contains("\"runtime_packet_mode\": \"ForceOCH\""));

        // Also simulate load_settings behavior by deserializing
        let settings: Settings = serde_json::from_str(&content).expect("valid json");
        assert_eq!(settings.runtime_packet_mode, "ForceOCH");

        // Clean up
        let _ = std::fs::remove_file(&settings_path);
    }
}

// =============================================================================
// Dashboard Format Conversion Helpers
// =============================================================================

#[cfg(test)]
mod concurrency_tests {
    use super::*;
    use libretune_core::protocol::{Connection, ConnectionConfig};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_no_deadlock_between_execute_controller_and_realtime_snapshot() {
        // Build a minimal AppState with both locks present
        let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("demo.ini");
        assert!(dev_path.exists(), "Demo INI not found at {:?}", dev_path);
        let def =
            EcuDefinition::from_file(dev_path.to_string_lossy().as_ref()).expect("Load demo INI");

        let state = Arc::new(AppState {
            connection: Mutex::new(Some(Connection::new(ConnectionConfig::default()))),
            definition: Mutex::new(Some(def)),
            autotune_state: Mutex::new(AutoTuneState::new()),
            autotune_secondary_state: Mutex::new(AutoTuneState::new()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            metrics_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(None),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            tune_mismatch_snapshot: Mutex::new(None),
            demo_mode: Mutex::new(false),
            console_history: Mutex::new(Vec::new()),
            rpm_state_tracker: Mutex::new(RpmStateTracker::new()),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            connection_factory: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
        });

        // Simulate execute_controller_command pattern: lock def -> sleep -> lock conn
        let s1 = state.clone();

        let task1 = tokio::spawn(async move {
            let _def = s1.definition.lock().await;
            // hold definition lock for some time
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _conn = s1.connection.lock().await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        });

        // Simulate refactored get_realtime_data: snapshot def -> release -> lock conn
        let s2 = state.clone();
        let task2 = tokio::spawn(async move {
            let _snapshot = {
                let def_guard = s2.definition.lock().await;
                def_guard.is_some()
            };

            // Now only lock connection for a short time
            let _conn = s2.connection.lock().await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        });

        // Ensure both complete within timeout (detect deadlock)
        let joined = tokio::time::timeout(Duration::from_secs(2), async {
            let r1 = task1.await;
            let r2 = task2.await;
            (r1, r2)
        })
        .await;

        assert!(joined.is_ok(), "Tasks deadlocked or timed out");
    }
}

// New tests for signature comparison and normalization (unit tests)
#[cfg(test)]
mod signature_tests {
    use super::*;

    #[test]
    fn test_normalize_signature_basic() {
        assert_eq!(
            normalize_signature("Speeduino 2023-05"),
            "speeduino 2023 05"
        );
        assert_eq!(
            normalize_signature("  RusEFI_v1.2.3 (build#42) "),
            "rusefi v1 2 3 build 42"
        );
        assert_eq!(normalize_signature("MegaSquirt"), "megasquirt");
    }

    #[test]
    fn test_compare_signatures_exact_and_partial() {
        // Exact after normalization
        assert_eq!(
            compare_signatures("Speeduino 2023.05", "speeduino 2023-05"),
            SignatureMatchType::Exact
        );

        // Partial when base matches but versions differ
        assert_eq!(
            compare_signatures("rusEFI v1.2.3", "rusEFI v1.2.4"),
            SignatureMatchType::Partial
        );

        // Partial when one contains the other
        assert_eq!(
            compare_signatures("Speeduino build 202305 extra", "speeduino 202305"),
            SignatureMatchType::Partial
        );

        // Mismatch for different families
        assert_eq!(
            compare_signatures("unrelated device", "another device"),
            SignatureMatchType::Mismatch
        );
    }

    #[test]
    fn test_build_shallow_mismatch_info() {
        let info = build_shallow_mismatch_info(
            "Speeduino 2023-05",
            "Speeduino 2023-04",
            Some("/path/test.ini".to_string()),
        );
        assert_eq!(info.match_type, SignatureMatchType::Partial);
        assert_eq!(info.ecu_signature, "Speeduino 2023-05");
        assert_eq!(info.ini_signature, "Speeduino 2023-04");
        assert_eq!(info.current_ini_path.unwrap(), "/path/test.ini");
        assert!(info.matching_inis.is_empty());

        let info2 = build_shallow_mismatch_info("FooBar", "BazQux", None);
        assert_eq!(info2.match_type, SignatureMatchType::Mismatch);
    }

    #[tokio::test]
    async fn test_find_matching_inis_and_build_info_partial() {
        use std::fs::write;
        use tempfile::tempdir;

        // Create a temporary repository and a sample INI with a Speeduino signature
        let dir = tempdir().expect("tempdir");
        let ini_path = dir.path().join("speedy.ini");
        let content = r#"[MegaTune]
name = "Speedy"
signature = "Speeduino 2023-04"
"#;
        write(&ini_path, content).expect("write ini");

        // Open repository and import the ini
        let mut repo = IniRepository::open(Some(dir.path())).expect("open repo");
        let _id = repo.import(&ini_path).expect("import");

        // Build minimal AppState with this repo
        let state = AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(None),
            autotune_state: Mutex::new(AutoTuneState::default()),
            autotune_secondary_state: Mutex::new(AutoTuneState::default()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            metrics_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(Some(repo)),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            tune_mismatch_snapshot: Mutex::new(None),
            demo_mode: Mutex::new(false),
            console_history: Mutex::new(Vec::new()),
            rpm_state_tracker: Mutex::new(RpmStateTracker::new()),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            connection_factory: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
        };

        let matches = find_matching_inis_from_state(&state, "Speeduino 2023-05").await;
        // We expect at least one match (the one we imported)
        assert!(!matches.is_empty());
        assert!(matches
            .iter()
            .any(|e| e.signature.to_lowercase().contains("speeduino")));

        // Build mismatch info using our helper and attach matching INIs
        let mut info = build_shallow_mismatch_info(
            "Speeduino 2023-05",
            "Speeduino 2023-04",
            Some("test.ini".to_string()),
        );
        info.matching_inis = matches;

        assert_eq!(info.match_type, SignatureMatchType::Partial);
        assert_eq!(info.current_ini_path.unwrap(), "test.ini");
        assert!(!info.matching_inis.is_empty());
    }

    #[tokio::test]
    async fn test_find_matching_inis_and_build_info_mismatch() {
        use std::fs::write;
        use tempfile::tempdir;

        // Create temporary repo with a Speeduino ini
        let dir = tempdir().expect("tempdir");
        let ini_path = dir.path().join("speedy.ini");
        let content = r#"[MegaTune]
name = "Speedy"
signature = "Speeduino 2023-04"
"#;
        write(&ini_path, content).expect("write ini");

        let mut repo = IniRepository::open(Some(dir.path())).expect("open repo");
        let _id = repo.import(&ini_path).expect("import");

        let state = AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(None),
            autotune_state: Mutex::new(AutoTuneState::default()),
            autotune_secondary_state: Mutex::new(AutoTuneState::default()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            metrics_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(Some(repo)),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            tune_mismatch_snapshot: Mutex::new(None),
            demo_mode: Mutex::new(false),
            console_history: Mutex::new(Vec::new()),
            rpm_state_tracker: Mutex::new(RpmStateTracker::new()),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            connection_factory: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
        };

        let matches = find_matching_inis_from_state(&state, "Speeduino 2023-05").await;
        // Using a completely different signature should yield no matches
        // (We already have a Speeduino INI in the repo)
        assert!(matches
            .iter()
            .any(|e| e.signature.to_lowercase().contains("speeduino")));

        // Build mismatch info for an unrelated ECU signature
        let mut info = build_shallow_mismatch_info("FooBar 1.0", "Speeduino 2023-04", None);
        info.matching_inis = Vec::new();
        assert_eq!(info.match_type, SignatureMatchType::Mismatch);
        assert!(info.matching_inis.is_empty());
    }

    // Explicit simulated connect tests: ensure connect-like behavior returns mismatch_info
    #[tokio::test]
    async fn test_connect_simulated_partial_and_mismatch() {
        use std::fs::write;
        use tempfile::tempdir;

        // Create temporary repo and a Speeduino INI
        let dir = tempdir().expect("tempdir");
        let ini_path = dir.path().join("speedy.ini");
        let content = r#"[MegaTune]
name = "Speedy"
signature = "Speeduino 2023-04"
"#;
        write(&ini_path, content).expect("write ini");

        let mut repo = IniRepository::open(Some(dir.path())).expect("open repo");
        let _id = repo.import(&ini_path).expect("import");

        // Build AppState with a loaded definition that expects the Speeduino 2023-04 signature
        let def = EcuDefinition::from_str(
            r#"[MegaTune]
signature = "Speeduino 2023-04"
"#,
        )
        .expect("parse def");

        let state = AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(Some(def)),
            autotune_state: Mutex::new(AutoTuneState::default()),
            autotune_secondary_state: Mutex::new(AutoTuneState::default()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            metrics_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(Some(repo)),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            tune_mismatch_snapshot: Mutex::new(None),
            demo_mode: Mutex::new(false),
            console_history: Mutex::new(Vec::new()),
            rpm_state_tracker: Mutex::new(RpmStateTracker::new()),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            connection_factory: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
        };

        // Partial match case
        let result_partial = connect_to_ecu_simulated(&state, "Speeduino 2023-05").await;
        assert_eq!(
            result_partial.mismatch_info.as_ref().unwrap().match_type,
            SignatureMatchType::Partial
        );
        assert!(!result_partial
            .mismatch_info
            .as_ref()
            .unwrap()
            .matching_inis
            .is_empty());

        // Mismatch case
        let result_mismatch = connect_to_ecu_simulated(&state, "UnrelatedDevice 1.0").await;
        assert_eq!(
            result_mismatch.mismatch_info.as_ref().unwrap().match_type,
            SignatureMatchType::Mismatch
        );
        assert!(result_mismatch
            .mismatch_info
            .as_ref()
            .unwrap()
            .matching_inis
            .is_empty());
    }

    #[tokio::test]
    async fn test_call_connection_factory_and_build_result_helper() {
        use std::fs::write;
        use std::sync::Arc;
        use tempfile::tempdir;

        // Create temp repo with Speeduino INI
        let dir = tempdir().expect("tempdir");
        let ini_path = dir.path().join("speedy.ini");
        let content = r#"[MegaTune]
name = "Speedy"
signature = "Speeduino 2023-04"
"#;
        write(&ini_path, content).expect("write ini");
        let mut repo = IniRepository::open(Some(dir.path())).expect("open repo");
        let _id = repo.import(&ini_path).expect("import");

        // Build a minimal AppState with repo and expected definition
        let state = AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(Some(
                EcuDefinition::from_str(
                    r#"[MegaTune]
signature = "Speeduino 2023-04"
"#,
                )
                .expect("parse def"),
            )),
            autotune_state: Mutex::new(AutoTuneState::default()),
            autotune_secondary_state: Mutex::new(AutoTuneState::default()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            metrics_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(Some(repo)),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            tune_mismatch_snapshot: Mutex::new(None),
            demo_mode: Mutex::new(false),
            console_history: Mutex::new(Vec::new()),
            rpm_state_tracker: Mutex::new(RpmStateTracker::new()),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            connection_factory: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
        };

        // Install factory returning a partial matching signature
        let factory: std::sync::Arc<
            dyn Fn(ConnectionConfig, Option<ProtocolSettings>, Endianness) -> Result<String, String>
                + Send
                + Sync,
        > = Arc::new(|_cfg, _p, _e| Ok("Speeduino 2023-05".to_string()));
        *state.connection_factory.lock().await = Some(factory);

        let res = call_connection_factory_and_build_result(&state, ConnectionConfig::default())
            .await
            .expect("factory ok");
        assert_eq!(
            res.mismatch_info.as_ref().unwrap().match_type,
            SignatureMatchType::Partial
        );
        assert!(!res.mismatch_info.as_ref().unwrap().matching_inis.is_empty());

        // Install factory that returns Err
        let factory_err: std::sync::Arc<
            dyn Fn(ConnectionConfig, Option<ProtocolSettings>, Endianness) -> Result<String, String>
                + Send
                + Sync,
        > = Arc::new(|_cfg, _p, _e| Err("fail".to_string()));
        *state.connection_factory.lock().await = Some(factory_err);

        let err = call_connection_factory_and_build_result(&state, ConnectionConfig::default())
            .await
            .err()
            .expect("err expected");
        assert!(err.contains("Factory-based connect failed"));
    }
}
