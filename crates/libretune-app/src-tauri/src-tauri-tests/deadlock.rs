use super::*;
use std::time::Duration;

#[tokio::test]
async fn test_no_deadlock_between_def_then_conn_and_snapshot_then_conn() {
    // Build a minimal AppState with both locks present
    let state = AppState {
        connection: Mutex::new(Some(libretune_core::protocol::connection::Connection::new(
            libretune_core::project::ConnectionSettings::default().into(),
        ))),
        definition: Mutex::new(Some(
            EcuDefinition::from_str("[Definition]\n").expect("create definition"),
        )),
        autotune_state: Mutex::new(AutoTuneState::new()),
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
        demo_mode: Mutex::new(false),
        plugin_manager: Mutex::new(None),
        controller_bridge: Mutex::new(None),
        migration_report: Mutex::new(None),
    };

    // Simulate execute_controller_command pattern: lock def -> sleep -> lock conn
    let s1 = &state;
    let task1 = tokio::spawn(async move {
        let _def = s1.definition.lock().await;
        // hold definition lock for some time
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _conn = s1.connection.lock().await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        // release
    });

    // Simulate refactored get_realtime_data: snapshot def -> release -> lock conn
    let s2 = &state;
    let task2 = tokio::spawn(async move {
        let (snapshot_present) = {
            let def_guard = s2.definition.lock().await;
            def_guard.is_some()
        };

        if !snapshot_present {
            // fail fast
            return;
        }

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
