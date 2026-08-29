//! Demo mode commands (extracted from lib.rs).

use crate::AppState;
use libretune_core::ini::EcuDefinition;
use libretune_core::protocol::{Connection, ConnectionConfig};
use libretune_core::simulator::EcuSimulator;
use libretune_core::tune::{TuneCache, TuneFile};
use std::path::PathBuf;
use tauri::{Emitter, Manager};

/// Enable or disable demo mode (simulated ECU for UI testing)
/// When enabled, loads a bundled epicEFI INI and generates simulated sensor data
#[tauri::command]
pub async fn set_demo_mode(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    // Stop any existing streaming first — but only for a transition that
    // actually changes something. Disabling demo mode that is already off
    // must be a no-op, not a way to kill a real ECU's realtime stream.
    let is_transition = enabled || *state.demo_mode.lock().await;
    if is_transition {
        let mut task_guard = state.streaming_task.lock().await;
        if let Some(handle) = task_guard.take() {
            handle.abort();
        }
    }

    if enabled {
        // Disconnect any existing connection to avoid mismatched definitions
        {
            let mut conn_guard = state.connection.lock().await;
            *conn_guard = None;
        }

        // Close and clear any open project/tune to ensure a clean demo state
        {
            let mut proj_guard = state.current_project.lock().await;
            if let Some(project) = proj_guard.take() {
                let _ = project.close();
            }
        }
        {
            let mut tune_guard = state.current_tune.lock().await;
            *tune_guard = None;
        }
        {
            let mut tune_mod_guard = state.tune_modified.lock().await;
            *tune_mod_guard = false;
        }

        // Load the bundled demo INI
        let resource_path = app
            .path()
            .resource_dir()
            .map_err(|e| format!("Failed to get resource dir: {}", e))?
            .join("resources")
            .join("demo.ini");

        // Try resource path first, then development path
        let ini_path = if resource_path.exists() {
            resource_path
        } else {
            // Development fallback: look in src-tauri/resources
            let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("demo.ini");
            if dev_path.exists() {
                dev_path
            } else {
                return Err(format!(
                    "Demo INI not found at {:?} or {:?}",
                    resource_path, dev_path
                ));
            }
        };

        // Load the INI definition
        let def = EcuDefinition::from_file(ini_path.to_string_lossy().as_ref())
            .map_err(|e| format!("Failed to load demo INI: {}", e))?;

        // Initialize TuneCache from definition
        let cache = TuneCache::from_definition(&def);

        // Apply the demo state to the AppState (aborts streaming, clears connection/project/tune and stores def/cache)
        apply_demo_enable(&state, def, cache).await?;

        // Notify frontend that definition/demo mode changed
        let _ = app.emit("demo:changed", true);
        let _ = app.emit("definition:changed", ());

        eprintln!("[DEMO] Demo mode enabled - loaded demo INI and cleared open project/connection");
    } else {
        apply_demo_disable(&state).await?;

        // Notify frontend demo disabled
        let _ = app.emit("demo:changed", false);

        eprintln!("[DEMO] Demo mode disabled");
    }

    Ok(())
}

/// Internal helper: apply demo enable with a provided definition and cache
/// Build a connection onto a simulator backed by `def`, or `None` if the
/// handshake doesn't come up.
fn connect_simulator(def: &EcuDefinition) -> Option<Connection> {
    let simulator = EcuSimulator::from_definition(def);
    // The handshake has to use the INI's own query command and endianness,
    // or it falls back to a generic probe and misreads the reply.
    let mut connection = Connection::with_protocol(
        ConnectionConfig::default(),
        def.protocol.clone(),
        def.endianness,
    );
    match connection.connect_with_channel(Box::new(simulator.channel())) {
        Ok(()) => Some(connection),
        Err(error) => {
            tracing::warn!(
                "demo: simulator handshake failed ({error}); falling back to generated values"
            );
            None
        }
    }
}

/// Fill `cache` from the pages the simulator already holds, and build the
/// matching [`TuneFile`].
///
/// The simulator boots on a seeded tune, but `TuneCache::from_definition`
/// is all zeroes. Leaving the two unreconciled gives demo mode a split
/// brain: the physics answer from the seeded veTable while AutoTune reads
/// zeroes out of the cache, and the table editors refuse to open at all
/// for want of a `current_tune`.
async fn sync_tune_from_simulator(
    state: &AppState,
    def: &EcuDefinition,
    cache: &mut TuneCache,
) -> Option<TuneFile> {
    let mut conn_guard = state.connection.lock().await;
    let connection = conn_guard.as_mut()?;
    let mut tune = TuneFile::new(&def.signature);

    for page in 0..def.n_pages {
        // A zero-length page is declared but has nothing to read; the real
        // sync path records it as empty rather than failing.
        if def.page_sizes.get(page as usize).copied().unwrap_or(0) == 0 {
            tune.pages.insert(page, Vec::new());
            cache.load_page(page, Vec::new());
            continue;
        }
        match connection.read_page(page) {
            Ok(data) => {
                cache.load_page(page, data.clone());
                tune.pages.insert(page, data);
            }
            Err(error) => {
                tracing::warn!("demo: page {page} did not sync from the simulator: {error}");
            }
        }
    }

    Some(tune)
}

pub(crate) async fn apply_demo_enable(
    state: &AppState,
    def: EcuDefinition,
    cache: TuneCache,
) -> Result<(), String> {
    // Stop any existing streaming task first
    {
        let mut task_guard = state.streaming_task.lock().await;
        if let Some(handle) = task_guard.take() {
            handle.abort();
        }
    }

    // Replace any existing connection with one onto the in-process
    // simulator, so demo mode reads pages, writes and burns through the same
    // protocol path as real hardware instead of side-stepping it. If the
    // simulator can't be reached for any reason, demo mode still runs — the
    // realtime stream falls back to generating values directly.
    {
        let mut conn_guard = state.connection.lock().await;
        *conn_guard = connect_simulator(&def);
    }

    // Close and clear any open project to ensure a clean demo state
    {
        let mut proj_guard = state.current_project.lock().await;
        if let Some(project) = proj_guard.take() {
            let _ = project.close();
        }
    }

    // The tune comes off the simulator itself, so the cache the editors read
    // and the memory the physics answer from are the same bytes.
    let mut cache = cache;
    let tune = sync_tune_from_simulator(state, &def, &mut cache).await;

    {
        let mut tune_guard = state.current_tune.lock().await;
        *tune_guard = tune;
    }

    {
        let mut tune_mod_guard = state.tune_modified.lock().await;
        *tune_mod_guard = false;
    }

    // Store the provided cache and definition
    {
        let mut cache_guard = state.tune_cache.lock().await;
        *cache_guard = Some(cache);
    }

    {
        let mut def_guard = state.definition.lock().await;
        *def_guard = Some(def);
    }
    crate::commands::data_logging::stop_recording_on_definition_change(state).await;
    crate::commands::realtime_stream::stop_streaming_on_definition_change(state).await;

    // Set demo mode flag
    {
        let mut demo_guard = state.demo_mode.lock().await;
        *demo_guard = true;
    }

    Ok(())
}

pub(crate) async fn apply_demo_disable(state: &AppState) -> Result<(), String> {
    // Idempotent: a duplicate or stale "disable demo" must not touch a
    // connection it never owned. Only the simulator installed by
    // `apply_demo_enable` is torn down here, and only while demo mode is
    // actually on — otherwise this would disconnect real hardware.
    {
        let mut demo_guard = state.demo_mode.lock().await;
        if !*demo_guard {
            return Ok(());
        }
        *demo_guard = false;
    }

    // The simulator connection belongs to demo mode. Leaving it installed
    // makes connection status report a connected ECU that is not there, and
    // every later command keeps targeting the simulator.
    let mut conn_guard = state.connection.lock().await;
    if let Some(mut connection) = conn_guard.take() {
        connection.disconnect();
    }
    Ok(())
}

/// Check if demo mode is currently enabled.
///
/// Demo mode simulates ECU data for testing without a real connection.
///
/// Returns: True if demo mode is active
#[tauri::command]
pub async fn get_demo_mode(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let demo_guard = state.demo_mode.lock().await;
    Ok(*demo_guard)
}

#[cfg(test)]
mod demo_mode_tests {
    use super::*;
    use crate::state::{AppState, RpmStateTracker, StreamStats};
    use libretune_core::autotune::AutoTuneState;
    use libretune_core::datalog::DataLogger;
    use libretune_core::project::OnlineIniRepository;
    use std::path::PathBuf;
    use tokio::sync::Mutex;

    /// A bare [`AppState`] with every field at its empty default.
    fn test_state() -> AppState {
        AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(None),
            autotune_state: Mutex::new(AutoTuneState::new()),
            autotune_secondary_state: Mutex::new(AutoTuneState::new()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
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
            // Background task for connection metrics emission (added recently)
            metrics_task: Mutex::new(None),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            connection_factory: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
            agent_task: Mutex::new(None),
            app_start_epoch: AppState::process_start_epoch(),
            inc_table_cache: AppState::new_inc_table_cache(),
        }
    }

    /// The bundled demo INI, the one demo mode actually loads.
    fn demo_def() -> EcuDefinition {
        let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("demo.ini");
        assert!(dev_path.exists(), "Demo INI not found at {:?}", dev_path);
        EcuDefinition::from_file(dev_path.to_string_lossy().as_ref()).expect("Load demo INI")
    }

    #[tokio::test]
    async fn test_apply_demo_enable_and_disable() {
        let state = test_state();

        let def = demo_def();
        let cache = TuneCache::from_definition(&def);

        // initial state
        assert!(!*state.demo_mode.lock().await);
        assert!(state.definition.lock().await.is_none());
        assert!(state.tune_cache.lock().await.is_none());

        apply_demo_enable(&state, def.clone(), cache)
            .await
            .expect("apply enable");
        assert!(*state.demo_mode.lock().await);
        assert!(state.definition.lock().await.is_some());
        assert!(state.tune_cache.lock().await.is_some());
        assert!(
            state.connection.lock().await.is_some(),
            "demo mode must install the simulator connection, or the realtime \
             stream silently falls back to the generator"
        );

        // The cache the editors read and the memory the physics answer from
        // have to be the same bytes. A zero-filled cache beside a seeded
        // simulator is a split brain: AutoTune samples nothing usable and the
        // table editors report "No tune is loaded".
        {
            let tune_guard = state.current_tune.lock().await;
            let tune = tune_guard
                .as_ref()
                .expect("demo mode must load a tune off the simulator");
            assert_eq!(
                tune.pages.len(),
                def.n_pages as usize,
                "every declared page must be synced"
            );
            assert!(
                tune.pages.values().any(|page| page.iter().any(|b| *b != 0)),
                "a tune of nothing but zeroes means the sync did not happen"
            );
        }
        {
            let cache_guard = state.tune_cache.lock().await;
            let cache = cache_guard.as_ref().expect("the cache is installed");
            assert!(
                (0..def.n_pages).any(|page| cache
                    .get_page(page)
                    .is_some_and(|bytes| bytes.iter().any(|b| *b != 0))),
                "the tune cache must hold the simulator's pages, not zeroes"
            );
        }

        apply_demo_disable(&state).await.expect("apply disable");
        assert!(!*state.demo_mode.lock().await);
        assert!(
            state.connection.lock().await.is_none(),
            "leaving the simulator connected reports a phantom ECU once demo \
             mode is off"
        );
    }

    #[tokio::test]
    async fn disabling_demo_mode_that_is_already_off_leaves_the_connection_alone() {
        // `set_demo_mode(false)` fired twice, or from a stale UI action, used
        // to disconnect whatever was plugged in — including real hardware.
        let state = test_state();
        let def = demo_def();
        let cache = TuneCache::from_definition(&def);
        apply_demo_enable(&state, def, cache)
            .await
            .expect("apply enable");
        apply_demo_disable(&state).await.expect("first disable");

        // Stand in for a real ECU connected after demo mode was turned off.
        {
            let mut conn_guard = state.connection.lock().await;
            *conn_guard = Some(Connection::new(ConnectionConfig::default()));
        }

        apply_demo_disable(&state).await.expect("second disable");

        assert!(
            state.connection.lock().await.is_some(),
            "disabling demo mode it never enabled must not disconnect anything"
        );
    }
}
