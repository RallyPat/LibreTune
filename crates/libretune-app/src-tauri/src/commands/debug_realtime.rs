//! Single realtime read diagnostic command.

use crate::state::AppState;

/// Retrieves current realtime data from the ECU.
///
/// Diagnostic: perform a single realtime read and return raw + parsed info
#[tauri::command]
pub async fn debug_single_realtime_read(
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let mut report = String::new();

    // 1) Check definition
    {
        let def_guard = state.definition.read().await;
        if let Some(def) = &*def_guard {
            report.push_str(&format!("INI loaded: sig={}\n", def.signature));
            report.push_str(&format!(
                "output_channels count: {}\n",
                def.output_channels.len()
            ));
            report.push_str(&format!(
                "och_get_command: {:?}\n",
                def.protocol.och_get_command
            ));
            report.push_str(&format!(
                "och_block_size: {}\n",
                def.protocol.och_block_size
            ));
            report.push_str(&format!(
                "message_envelope: {:?}\n",
                def.protocol.message_envelope_format
            ));
            report.push_str(&format!("endianness: {:?}\n", def.endianness));
        } else {
            return Ok("ERROR: No definition loaded".to_string());
        }
    }

    // 2) Check connection
    {
        let mut conn_guard = state.connection.lock().await;
        if let Some(conn) = conn_guard.as_mut() {
            report.push_str(&format!(
                "Connected: sig={:?}, modern={}\n",
                conn.signature(),
                conn.is_modern_protocol()
            ));

            // 3) Try to read realtime data
            match conn.get_realtime_data() {
                Ok(raw) => {
                    report.push_str(&format!("Raw data: {} bytes\n", raw.len()));
                    if raw.len() >= 8 {
                        report.push_str(&format!("First 8 bytes: {:02x?}\n", &raw[..8]));
                    }
                }
                Err(e) => {
                    report.push_str(&format!("get_realtime_data ERROR: {:?}\n", e));
                }
            }
        } else {
            report.push_str("ERROR: No connection\n");
        }
    }

    // 4) Check streaming task
    {
        let task_guard = state.streaming_task.lock().await;
        report.push_str(&format!(
            "Streaming task active: {}\n",
            task_guard.is_some()
        ));
    }

    Ok(report)
}
