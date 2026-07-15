//! start_realtime_stream and feed_autotune_data (extracted from lib.rs).

use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    get_conn_lock_holder, load_settings, set_conn_lock_holder, stream_log, AppState,
    AutoTuneLoadSource, RpmState, StreamStats,
};
use libretune_core::autotune::VEDataPoint;
use libretune_core::demo::DemoSimulator;
use tauri::{Emitter, Manager};

/// Canonical dashboard channel names → ECU-specific output channel names.
/// Only inserts when the canonical name is absent from `data`.
const REALTIME_CHANNEL_ALIASES: &[(&str, &[&str])] = &[
    (
        "rpm",
        &["RPMValue", "rpm", "RPM", "engineSpeed", "rpmSensor"],
    ),
    ("afr", &["AFRValue", "RealAFRValue", "afr", "AFR", "afr1"]),
    (
        "coolant",
        &["coolant", "CLTValue", "clt", "CLT", "coolantTemp"],
    ),
    (
        "map",
        &["MAPValue", "map", "MAP", "manifoldPressure", "fuelLoad"],
    ),
    (
        "tps",
        &["TPSValue", "tps", "TPS", "throttlePosition", "throttle"],
    ),
    (
        "battery",
        &[
            "VBatt",
            "vBatt",
            "battery",
            "Battery",
            "vbatt",
            "batteryVoltage",
        ],
    ),
    (
        "iat",
        &["IATValue", "iat", "IAT", "intakeAirTemp", "intake"],
    ),
    (
        "advance",
        &[
            "correctedIgnitionAdvance",
            "baseIgnitionAdvance",
            "runningAdvance",
            "SA",
            "advance",
            "timing",
            "ignitionAdvance",
            "ignAdv",
            "Advance",
        ],
    ),
    (
        "ve",
        &[
            "veValue", "VE1", "ve1", "veMain", "VEValue", "ve", "VE", "veCurr",
        ],
    ),
    (
        "boost",
        &[
            "boostControlTarget",
            "boostOutput",
            "boostPressure",
            "boost",
            "Boost",
        ],
    ),
    (
        "speed",
        &["vehicleSpeedKph", "speed", "Speed", "wheelSpeed"],
    ),
    ("oilPressure", &["oilPressure", "OilPressure", "oilpress"]),
    (
        "fuelLevel",
        &["fuelLevel", "FuelLevel", "fuel", "fuelTankLevel"],
    ),
    (
        "pulseWidth",
        &[
            "actualLastInjection",
            "pulseWidth1",
            "pulseWidth",
            "pw1",
            "PW1",
        ],
    ),
    (
        "dutyCycle",
        &["injectorDutyCycle", "dutyCycle", "injDuty", "InjectorDuty"],
    ),
    ("lambda", &["lambda", "Lambda", "lambdaValue", "wbo2"]),
    (
        "dwell",
        &[
            "sparkDwell",
            "sparkDwellValue",
            "dwell",
            "Dwell",
            "dwellAngle",
            "baseDwell",
        ],
    ),
    ("baro", &["baroPressure", "baro", "BaroPressure"]),
    (
        "afrTarget",
        &["targetAFR", "targetAfr", "afrTarget", "AFRTarget"],
    ),
    (
        "egt",
        &[
            "egt1", "egt2", "egt3", "egt4", "egt5", "egt6", "egt7", "egt8", "egt",
        ],
    ),
    (
        "correction",
        &[
            "stftCorrection1",
            "stftCorrection2",
            "ltftCorrection1",
            "correction",
            "fuelCorrection",
            "egoCorrection",
        ],
    ),
    (
        "sync",
        &["isMapValid", "sync", "engineSync", "hasSync", "triggerSync"],
    ),
];

/// Map ECU-specific output channel names to the canonical names used by default
/// dashboards. Also applies derived aliases (e.g. totalFuelCorrection → correction %).
pub(crate) fn apply_channel_aliases(data: &mut HashMap<String, f64>) {
    for (alias, candidates) in REALTIME_CHANNEL_ALIASES {
        if data.contains_key(*alias) {
            continue;
        }
        for &candidate in *candidates {
            if let Some(&val) = data.get(candidate) {
                data.insert(alias.to_string(), val);
                break;
            }
        }
    }

    // rusEFI/FOME: totalFuelCorrection is a multiplier (1.0 = 100%); dashboards expect %.
    if !data.contains_key("correction") {
        if let Some(&v) = data.get("totalFuelCorrection") {
            data.insert("correction".to_string(), v * 100.0);
        }
    }
}

/// Feed the current realtime snapshot to the data logger when a recording is
/// active. The logger applies its own sample-rate limiting in `record()`.
pub(crate) async fn feed_data_logger(app_state: &AppState, data: &HashMap<String, f64>) {
    // try_lock: never stall the stream tick on logger contention
    let mut logger = match app_state.data_logger.try_lock() {
        Ok(l) => l,
        Err(_) => return,
    };
    if !logger.is_recording() {
        return;
    }
    let values: Vec<f64> = logger
        .channels()
        .iter()
        .map(|name| data.get(name).copied().unwrap_or(0.0))
        .collect();
    logger.record(values);
}

pub(crate) async fn feed_autotune_data(
    app_state: &AppState,
    data: &HashMap<String, f64>,
    current_time_ms: u64,
) {
    // Check if AutoTune is running
    let autotune_guard = app_state.autotune_state.lock().await;
    if !autotune_guard.is_running {
        return;
    }
    drop(autotune_guard);

    // Get the config
    let mut config_guard = app_state.autotune_config.lock().await;
    let config = match config_guard.as_mut() {
        Some(c) => c,
        None => return,
    };

    // Extract channel values (try common channel names)
    let rpm = data
        .get("rpm")
        .or_else(|| data.get("RPM"))
        .or_else(|| data.get("rpmValue"))
        .copied()
        .unwrap_or(0.0);

    let map = data
        .get("map")
        .or_else(|| data.get("MAP"))
        .or_else(|| data.get("mapValue"))
        .or_else(|| data.get("fuelingLoad"))
        .copied()
        .unwrap_or(0.0);

    let maf_value = data
        .get("maf")
        .or_else(|| data.get("MAF"))
        .or_else(|| data.get("mafValue"))
        .or_else(|| data.get("airMass"))
        .or_else(|| data.get("airMassFlow"))
        .or_else(|| data.get("airflow"))
        .or_else(|| data.get("airFlow"))
        .copied()
        .unwrap_or(0.0);

    let load_value = match config.load_source {
        AutoTuneLoadSource::Map => map,
        AutoTuneLoadSource::Maf => {
            if maf_value > 0.0 {
                maf_value
            } else {
                map
            }
        }
    };

    let afr = data
        .get("afr")
        .or_else(|| data.get("AFR"))
        .or_else(|| data.get("afr1"))
        .or_else(|| data.get("AFRValue"))
        .or_else(|| data.get("lambda1"))
        .map(|v| if *v < 2.0 { *v * 14.7 } else { *v }) // Convert lambda to AFR
        .unwrap_or(14.7);

    let ve = data
        .get("ve")
        .or_else(|| data.get("VE"))
        .or_else(|| data.get("veValue"))
        .or_else(|| data.get("VEtable"))
        .copied()
        .unwrap_or(0.0);

    let clt = data
        .get("clt")
        .or_else(|| data.get("CLT"))
        .or_else(|| data.get("coolant"))
        .or_else(|| data.get("coolantTemperature"))
        .copied()
        .unwrap_or(0.0);

    let tps = data
        .get("tps")
        .or_else(|| data.get("TPS"))
        .or_else(|| data.get("tpsValue"))
        .copied()
        .unwrap_or(0.0);

    // Calculate TPS rate (%/sec) based on time delta
    let tps_rate =
        if let (Some(last_tps), Some(last_ts)) = (config.last_tps, config.last_timestamp_ms) {
            let dt_sec = (current_time_ms.saturating_sub(last_ts)) as f64 / 1000.0;
            if dt_sec > 0.001 {
                (tps - last_tps) / dt_sec
            } else {
                0.0
            }
        } else {
            0.0
        };

    // Update last values for next iteration
    config.last_tps = Some(tps);
    config.last_timestamp_ms = Some(current_time_ms);

    // Check for accel enrichment flag
    let accel_enrich_active = data
        .get("accelEnrich")
        .or_else(|| data.get("accelEnrichActive"))
        .or_else(|| data.get("tpsAE"))
        .map(|v| *v > 0.5);

    // Create the data point
    let data_point = VEDataPoint {
        rpm,
        map,
        maf: maf_value,
        load: load_value,
        afr,
        ve,
        clt,
        tps,
        tps_rate,
        accel_enrich_active,
        timestamp_ms: current_time_ms,
    };

    // Clone the config values before we release the guard
    let x_bins = config.x_bins.clone();
    let y_bins = config.y_bins.clone();
    let secondary_x_bins = config.secondary_x_bins.clone();
    let secondary_y_bins = config.secondary_y_bins.clone();
    let settings = config.settings.clone();
    let filters = config.filters.clone();
    let authority = config.authority_limits.clone();
    drop(config_guard);

    // Feed to AutoTune
    let mut autotune_guard = app_state.autotune_state.lock().await;
    autotune_guard.add_data_point(
        data_point.clone(),
        &x_bins,
        &y_bins,
        &settings,
        &filters,
        &authority,
    );

    if let (Some(sec_x_bins), Some(sec_y_bins)) = (secondary_x_bins, secondary_y_bins) {
        let mut secondary_guard = app_state.autotune_secondary_state.lock().await;
        secondary_guard.add_data_point(
            data_point,
            &sec_x_bins,
            &sec_y_bins,
            &settings,
            &filters,
            &authority,
        );
    }
}

#[tauri::command]
pub async fn start_realtime_stream(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    interval_ms: Option<u64>,
) -> Result<(), String> {
    let interval = interval_ms.unwrap_or(100);
    let is_demo = *state.demo_mode.lock().await;

    // In demo mode, we only need the definition
    // In real mode, we need both connection and definition. Avoid holding both locks at
    // the same time to prevent potential deadlocks with other commands that lock in the
    // opposite order.
    if !is_demo {
        {
            let def_guard = state.definition.lock().await;
            if def_guard.is_none() {
                return Err("Connection or definition missing".to_string());
            }
        }
        {
            let conn_guard = state.connection.lock().await;
            if conn_guard.is_none() {
                return Err("Connection or definition missing".to_string());
            }
        }
    } else {
        let def_guard = state.definition.lock().await;
        if def_guard.is_none() {
            return Err("Definition not loaded for demo mode".to_string());
        }
    }

    // Always replace old task: previous stop_realtime_stream (fire-and-forget from
    // React cleanup) may not have completed yet.  If we return early here,
    // the deferred stop will abort the only task, leaving the stream dead.
    let mut task_guard = state.streaming_task.lock().await;
    if let Some(old_handle) = task_guard.take() {
        stream_log("start: aborting old task");
        old_handle.abort();
    }
    stream_log(&format!(
        "start: spawning new task (interval={}ms)",
        interval
    ));

    let app_handle = app.clone();

    let handle = tokio::spawn(async move {
        let app_state = app_handle.state::<AppState>();
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(interval));

        // For demo mode, create a simulator
        let mut demo_simulator: Option<DemoSimulator> = None;
        let start_time = std::time::Instant::now();

        // Cache output channels + endianness once before the loop.
        // These don't change during a session so there's no need to re-lock every tick.
        let cached_def_data: Option<(
            Arc<HashMap<String, libretune_core::ini::OutputChannel>>,
            libretune_core::ini::Endianness,
        )> = {
            // Step A: clone the Arc from cache (lock, clone, release)
            let cached_ch: Option<Arc<HashMap<String, libretune_core::ini::OutputChannel>>>;
            {
                let channels_cache = app_state.cached_output_channels.lock().await;
                cached_ch = channels_cache.as_ref().map(Arc::clone);
            } // lock released

            // Step B: get endianness from definition (separate lock)
            if let Some(ch) = cached_ch {
                let def_guard = app_state.definition.lock().await;
                let endianness = def_guard
                    .as_ref()
                    .map(|d| d.endianness)
                    .unwrap_or(libretune_core::ini::Endianness::Little);
                Some((ch, endianness))
            } else {
                let def_guard = app_state.definition.lock().await;
                def_guard
                    .as_ref()
                    .map(|def| (Arc::new(def.output_channels.clone()), def.endianness))
            }
        };
        stream_log(&format!(
            "task started, cached_def_data={}",
            cached_def_data.is_some()
        ));

        // Cache app settings once — load_settings() reads from disk and must not run every tick.
        let rpm_settings = load_settings(&app_handle);
        let rpm_key_on_threshold = rpm_settings.key_on_threshold_rpm;
        let rpm_key_off_timeout = rpm_settings.key_off_timeout_sec;

        // Determine transfer mode once and initialize stream stats
        {
            let (mode_label, mode_reason) = {
                let conn_guard = app_state.connection.lock().await;
                if let Some(conn) = conn_guard.as_ref() {
                    let (fetch, reason) = conn.choose_runtime_command();
                    let label = match &fetch {
                        libretune_core::protocol::RuntimeFetch::Burst(_) => "Burst".to_string(),
                        libretune_core::protocol::RuntimeFetch::OCH(_) => "OCH".to_string(),
                    };
                    (label, reason)
                } else {
                    ("Demo".to_string(), "demo mode".to_string())
                }
            };
            let mut stats = app_state.stream_stats.lock().await;
            *stats = StreamStats {
                ticks_total: 0,
                ticks_success: 0,
                ticks_skipped: 0,
                ticks_error: 0,
                transfer_mode: mode_label,
                transfer_reason: mode_reason,
                interval_ms: interval,
                started_at_ms: chrono::Utc::now().timestamp_millis(),
            };
        }

        let mut tick_count: u64 = 0;
        // Local stream stat counters (flushed to shared state periodically)
        let mut local_ticks_total: u64 = 0;
        let mut local_ticks_success: u64 = 0;
        let mut local_ticks_skipped: u64 = 0;
        let mut local_ticks_error: u64 = 0;
        loop {
            ticker.tick().await;
            tick_count += 1;
            local_ticks_total += 1;

            // Trace: log which phase we're in so we can find deadlocks
            if tick_count <= 25 || tick_count.is_multiple_of(20) {
                stream_log(&format!("tick #{}: T1-demo_mode", tick_count));
            }
            let is_demo = match app_state.demo_mode.try_lock() {
                Ok(guard) => *guard,
                Err(_) => {
                    // demo_mode lock busy — skip tick
                    continue;
                }
            };
            let current_time_ms = start_time.elapsed().as_millis() as u64;

            if is_demo {
                // Demo mode: generate simulated data
                if demo_simulator.is_none() {
                    demo_simulator = Some(DemoSimulator::new());
                }

                if let Some(ref mut sim) = demo_simulator {
                    let elapsed_ms = start_time.elapsed().as_millis() as u64;
                    let mut data = sim.update(elapsed_ms);

                    // User Math Channels Evaluation (Demo)
                    {
                        let mut channels_guard = app_state.math_channels.lock().await;
                        for channel in channels_guard.iter_mut() {
                            if channel.cached_ast.is_none() {
                                let _ = channel.compile();
                            }
                            if let Some(expr) = &channel.cached_ast {
                                if let Ok(val) =
                                    libretune_core::ini::expression::evaluate_simple(expr, &data)
                                {
                                    data.insert(channel.name.clone(), val.as_f64());
                                }
                            }
                        }
                    }

                    apply_channel_aliases(&mut data);

                    // Sanitize NaN/Infinity — serde_json cannot serialize these,
                    // which would silently break app_handle.emit().
                    for v in data.values_mut() {
                        if !v.is_finite() {
                            *v = 0.0;
                        }
                    }

                    if let Err(e) = app_handle.emit("realtime:update", &data) {
                        stream_log(&format!("emit FAILED (demo): {}", e));
                    }

                    // Check for RPM state transitions (key-on/off detection)
                    {
                        let rpm = data
                            .get("rpm")
                            .or_else(|| data.get("RPM"))
                            .copied()
                            .unwrap_or(0.0);

                        let mut tracker = app_state.rpm_state_tracker.lock().await;

                        if let Some(new_state) =
                            tracker.update(rpm, rpm_key_on_threshold, rpm_key_off_timeout)
                        {
                            // Emit event when state changes
                            let state_str = match new_state {
                                RpmState::On => "on",
                                RpmState::Off => "off",
                            };
                            let _ = app_handle.emit("realtime:key_state_changed", &state_str);
                        }
                    }

                    // Feed data to AutoTune if running
                    feed_autotune_data(&app_state, &data, current_time_ms).await;

                    // Feed data to the data logger if recording
                    feed_data_logger(&app_state, &data).await;

                    local_ticks_success += 1;
                }
            } else {
                // Real ECU mode: read from connection
                demo_simulator = None; // Clear simulator if we switch modes

                // Phase 1: Get raw data from ECU (hold connection lock only during I/O)
                // Use try_lock() to avoid blocking forever if another command
                // (e.g. get_all_constant_values) is holding the connection lock.
                if tick_count <= 25 || tick_count.is_multiple_of(20) {
                    stream_log(&format!("tick #{}: T2-conn_lock", tick_count));
                }
                let raw_result: Result<Vec<u8>, String>;
                {
                    match app_state.connection.try_lock() {
                        Ok(mut conn_guard) => {
                            set_conn_lock_holder("stream_loop");
                            if let Some(conn) = conn_guard.as_mut() {
                                raw_result = conn.get_realtime_data().map_err(|e| e.to_string());
                            } else {
                                raw_result = Err("No connection".to_string());
                            }
                            set_conn_lock_holder("(none)");
                        }
                        Err(_) => {
                            // Connection lock is busy (another command is using it) — skip this tick
                            if tick_count <= 25 || tick_count.is_multiple_of(20) {
                                let holder = get_conn_lock_holder();
                                stream_log(&format!(
                                    "tick #{}: conn_lock busy (held by: {}), skipping",
                                    tick_count, holder
                                ));
                            }
                            local_ticks_skipped += 1;
                            // Flush stats periodically even on skips
                            if local_ticks_total.is_multiple_of(20) {
                                if let Ok(mut stats) = app_state.stream_stats.try_lock() {
                                    stats.ticks_total = local_ticks_total;
                                    stats.ticks_success = local_ticks_success;
                                    stats.ticks_skipped = local_ticks_skipped;
                                    stats.ticks_error = local_ticks_error;
                                }
                            }
                            continue;
                        }
                    }
                } // conn lock released via try_lock drop

                // Diagnostic logging for raw result
                match &raw_result {
                    Ok(raw) => {
                        static STREAM_LOG_COUNTER: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let count =
                            STREAM_LOG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if count < 5 || count.is_multiple_of(100) {
                            eprintln!(
                                "[DEBUG] stream tick #{}: got {} raw bytes",
                                count,
                                raw.len()
                            );
                        }
                    }
                    Err(e) => {
                        static ERR_LOG_COUNTER: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let count =
                            ERR_LOG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if count < 10 || count.is_multiple_of(50) {
                            eprintln!(
                                "[ERROR] stream tick #{}: get_realtime_data failed: {}",
                                count, e
                            );
                        }
                    }
                }

                // Phase 2: Use pre-cached output channels and endianness (no locks needed)
                if tick_count <= 25 || tick_count.is_multiple_of(20) {
                    stream_log(&format!("tick #{}: T3-phase2(cached)", tick_count));
                }
                let def_data = &cached_def_data;

                // Phase 3: Process data outside of any mutex locks
                match (&raw_result, def_data) {
                    (Ok(raw), Some((output_channels, endianness))) => {
                        // Two-pass approach for computed channels:
                        // Pass 1: Parse all non-computed channels
                        let mut data: HashMap<String, f64> = HashMap::new();
                        let mut computed_channels = Vec::new();

                        for (name, channel) in output_channels.iter() {
                            if channel.is_computed() {
                                computed_channels.push((name.clone(), channel.clone()));
                            } else if let Some(val) = channel.parse(raw, *endianness) {
                                data.insert(name.clone(), val);
                            }
                        }

                        // Pass 2: Evaluate computed channels using parsed values as context
                        for (name, channel) in computed_channels {
                            if let Some(val) = channel.parse_with_context(raw, *endianness, &data) {
                                data.insert(name, val);
                            }
                        }

                        // Pass 3: User Math Channels Evaluation
                        if tick_count <= 25 || tick_count.is_multiple_of(20) {
                            stream_log(&format!("tick #{}: T4-math_ch", tick_count));
                        }
                        if let Ok(mut channels_guard) = app_state.math_channels.try_lock() {
                            for channel in channels_guard.iter_mut() {
                                if channel.cached_ast.is_none() {
                                    let _ = channel.compile();
                                }
                                if let Some(expr) = &channel.cached_ast {
                                    if let Ok(val) =
                                        libretune_core::ini::expression::evaluate_simple(
                                            expr, &data,
                                        )
                                    {
                                        data.insert(channel.name.clone(), val.as_f64());
                                    }
                                }
                            }
                        }

                        apply_channel_aliases(&mut data);

                        // Sanitize NaN/Infinity — serde_json cannot serialize these,
                        // which would silently break app_handle.emit().
                        for v in data.values_mut() {
                            if !v.is_finite() {
                                *v = 0.0;
                            }
                        }

                        if let Err(e) = app_handle.emit("realtime:update", &data) {
                            stream_log(&format!("emit FAILED (real): {}", e));
                        }

                        // Log parsed channel count — every tick for the first 30, then every 20th (~1/sec)
                        {
                            static EMIT_LOG_COUNTER: std::sync::atomic::AtomicU64 =
                                std::sync::atomic::AtomicU64::new(0);
                            let count =
                                EMIT_LOG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if count < 30 || count.is_multiple_of(20) {
                                let rpm = data
                                    .get("rpm")
                                    .or_else(|| data.get("RPM"))
                                    .copied()
                                    .unwrap_or(-1.0);
                                stream_log(&format!(
                                    "emit #{}: {} ch, rpm={:.0}",
                                    count,
                                    data.len(),
                                    rpm
                                ));
                            }
                        }

                        // Check for RPM state transitions (key-on/off detection)
                        if tick_count <= 25 || tick_count.is_multiple_of(20) {
                            stream_log(&format!("tick #{}: T5-rpm_state", tick_count));
                        }
                        {
                            let rpm = data
                                .get("rpm")
                                .or_else(|| data.get("RPM"))
                                .copied()
                                .unwrap_or(0.0);

                            if let Ok(mut tracker) = app_state.rpm_state_tracker.try_lock() {
                                if let Some(new_state) =
                                    tracker.update(rpm, rpm_key_on_threshold, rpm_key_off_timeout)
                                {
                                    let state_str = match new_state {
                                        RpmState::On => "on",
                                        RpmState::Off => "off",
                                    };
                                    let _ =
                                        app_handle.emit("realtime:key_state_changed", &state_str);
                                }
                            }
                        }

                        // Feed data to AutoTune if running
                        if tick_count <= 25 || tick_count.is_multiple_of(20) {
                            stream_log(&format!("tick #{}: T6-autotune", tick_count));
                        }
                        feed_autotune_data(&app_state, &data, current_time_ms).await;

                        // Feed data to the data logger if recording
                        feed_data_logger(&app_state, &data).await;

                        local_ticks_success += 1;
                    }
                    (Err(e), _) => {
                        // Log errors to stream log so we can see Phase 1 failures
                        {
                            static ERR_STREAM_LOG: std::sync::atomic::AtomicU64 =
                                std::sync::atomic::AtomicU64::new(0);
                            let n =
                                ERR_STREAM_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if n < 10 || n.is_multiple_of(50) {
                                stream_log(&format!("stream error #{}: {}", n, e));
                            }
                        }
                        let _ = app_handle.emit("realtime:error", &e);
                        local_ticks_error += 1;
                    }
                    _ => {}
                }
            }

            // Flush local stats to shared state every ~1s (20 ticks at 50ms)
            if local_ticks_total.is_multiple_of(20) {
                if let Ok(mut stats) = app_state.stream_stats.try_lock() {
                    stats.ticks_total = local_ticks_total;
                    stats.ticks_success = local_ticks_success;
                    stats.ticks_skipped = local_ticks_skipped;
                    stats.ticks_error = local_ticks_error;
                }
            }
        }
    });

    *task_guard = Some(handle);
    Ok(())
}
