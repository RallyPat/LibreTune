//! connect_to_ecu command (extracted from lib.rs).

use crate::commands::metrics::start_metrics_task;
use crate::{
    call_connection_factory_and_build_result, find_matching_inis_internal,
    load_settings, parse_runtime_packet_mode, AppState, ConnectResult, SignatureMatchType,
    SignatureMismatchInfo,
};
use libretune_core::protocol::{Connection, ConnectionConfig};
use tauri::Emitter;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn connect_to_ecu(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    port_name: String,
    baud_rate: u32,
    timeout_ms: Option<u64>,
    runtime_packet_mode: Option<String>,
    connection_type: Option<String>,
    tcp_host: Option<String>,
    tcp_port: Option<u16>,
) -> Result<ConnectResult, String> {
    use libretune_core::protocol::ConnectionType;

    let conn_type = match connection_type.as_deref() {
        Some(t) if t.eq_ignore_ascii_case("tcp") => ConnectionType::Tcp,
        _ => ConnectionType::Serial,
    };

    let mut config = ConnectionConfig {
        connection_type: conn_type,
        port_name: port_name.clone(),
        tcp_host,
        tcp_port,
        ..Default::default()
    };

    // Apply runtime_packet_mode override if provided
    if let Some(mode) = runtime_packet_mode {
        config.runtime_packet_mode = parse_runtime_packet_mode(&mode);
    }

    // Validate baud rate passed from UI: guard against 0.
    if baud_rate == 0 {
        eprintln!(
            "[WARN] connect_to_ecu: received baud_rate 0, defaulting to {}",
            libretune_core::protocol::DEFAULT_BAUD_RATE
        );
        config.baud_rate = libretune_core::protocol::DEFAULT_BAUD_RATE;
    } else {
        config.baud_rate = baud_rate;
    }

    // Log resolved configuration for diagnostics
    eprintln!(
        "[INFO] connect_to_ecu: type={:?} port='{}' baud={} tcp={:?}:{:?} timeout_ms={}",
        config.connection_type,
        config.port_name,
        config.baud_rate,
        config.tcp_host,
        config.tcp_port,
        config.timeout_ms
    );

    // Get protocol settings from loaded definition if available
    let def_guard = state.definition.read().await;
    let protocol_settings = def_guard.as_ref().map(|d| d.protocol.clone());
    let endianness = def_guard.as_ref().map(|d| d.endianness).unwrap_or_default();
    let expected_signature = def_guard.as_ref().map(|d| d.signature.clone());
    let expected_prefix = def_guard.as_ref().and_then(|d| d.signature_prefix.clone());
    drop(def_guard);

    // If a test connection factory is installed, use helper to obtain a signature without opening a port
    if state.connection_factory.lock().await.is_some() {
        let res = call_connection_factory_and_build_result(&state, config.clone()).await?;

        // Start metrics task (no connection available, metrics will skip if needed)
        start_metrics_task(app.clone(), state.clone()).await;

        return Ok(res);
    }

    // If a timeout was provided by the UI, apply it
    if let Some(t) = timeout_ms {
        eprintln!("[INFO] connect_to_ecu: using timeout_ms={} from UI", t);
        config.timeout_ms = t;
    }

    // Create connection in a dedicated OS thread (not Tokio's spawn_blocking)
    // Use catch_unwind to capture panics and send them back as errors.
    // Capture a small copy of the connection parameters for post-mortem logging
    let log_port = config.port_name.clone();
    let log_baud = config.baud_rate;
    let log_timeout = config.timeout_ms;

    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let send_err = |s: String| {
            let _ = tx.send(Err(s));
        };

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut conn = if let Some(protocol) = protocol_settings {
                Connection::with_protocol(config, protocol, endianness)
            } else {
                Connection::new(config)
            };

            match conn.connect() {
                Ok(_) => Ok(conn),
                Err(e) => Err(e.to_string()),
            }
        }));

        match res {
            Ok(Ok(conn)) => {
                let _ = tx.send(Ok(conn));
            }
            Ok(Err(e)) => send_err(e),
            Err(panic_info) => {
                let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                send_err(format!("Connection thread panicked: {}", panic_msg));
            }
        }
    });

    // Wait for result with a longer timeout to account for USB latency
    let result = match rx.recv_timeout(std::time::Duration::from_secs(15)) {
        Ok(r) => r,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            Err("Connection timed out after 15 seconds".to_string())
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err("Connection thread crashed or disconnected".to_string())
        }
    };

    match result {
        Ok(conn) => {
            let signature = conn.signature().unwrap_or("Unknown").to_string();

            // Check signature match and build mismatch info if needed
            let mismatch_info = if let Some(ref expected) = expected_signature {
                let match_type = crate::commands::signature_helpers::compare_signatures_with_prefix(
                    &signature,
                    expected,
                    expected_prefix.as_deref(),
                );

                if match_type != SignatureMatchType::Exact {
                    // Log the mismatch
                    eprintln!(
                        "Warning: ECU signature '{}' {} INI signature '{}'",
                        signature,
                        if match_type == SignatureMatchType::Partial {
                            "partially matches"
                        } else {
                            "does not match"
                        },
                        expected
                    );

                    // Find matching INIs from repository
                    let matching_inis = find_matching_inis_internal(&state, &signature).await;

                    // Get current INI path from settings
                    let current_ini_path = {
                        let settings = load_settings(&app);
                        settings.last_ini_path.clone()
                    };

                    let info = SignatureMismatchInfo {
                        ecu_signature: signature.clone(),
                        ini_signature: expected.clone(),
                        match_type,
                        current_ini_path,
                        matching_inis,
                    };

                    // Also emit event for backward compatibility
                    let _ = app.emit("signature:mismatch", &info);

                    Some(info)
                } else {
                    None
                }
            } else {
                None
            };

            let mut guard = state.connection.lock().await;
            *guard = Some(conn);

            // Start periodic metrics emission task
            start_metrics_task(app.clone(), state.clone()).await;

            Ok(ConnectResult {
                signature,
                mismatch_info,
            })
        }
        Err(e) => {
            eprintln!(
                "[ERROR] connect_to_ecu failed: {} (port='{}' baud={} timeout_ms={})",
                e, log_port, log_baud, log_timeout
            );
            Err(e)
        }
    }
}
