//! Plugin Manager - handles JVM subprocess lifecycle and communication

use super::bridge::ControllerBridge;
use super::protocol::{
    PluginEvent, PluginInfo, RpcNotification, RpcRequest, RpcResponse, SwingComponent, UiDiff,
};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

/// Plugin instance state
#[derive(Debug)]
pub struct PluginInstance {
    pub info: PluginInfo,
    pub ui_tree: Option<SwingComponent>,
    pub enabled: bool,
}

/// Pending requests shared between main thread and reader thread
type PendingRequests = Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<RpcResponse>>>>;

/// Manages TS-compatible plugin loading and execution
pub struct PluginManager {
    /// Path to plugin-host.jar
    host_jar_path: PathBuf,
    /// JVM process handle
    process: Mutex<Option<Child>>,
    /// Stdin writer to JVM
    stdin: Mutex<Option<ChildStdin>>,
    /// Loaded plugins
    plugins: RwLock<HashMap<String, PluginInstance>>,
    /// Request ID counter
    request_id: AtomicU64,
    /// Pending requests waiting for response (shared with reader thread)
    pending_requests: PendingRequests,
    /// Controller bridge for ECU data
    #[allow(dead_code)]
    bridge: Arc<ControllerBridge>,
    /// Callback for UI updates
    ui_update_callback: RwLock<Option<Box<dyn Fn(String, UiDiff) + Send + Sync>>>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(host_jar_path: PathBuf, bridge: Arc<ControllerBridge>) -> Self {
        Self {
            host_jar_path,
            process: Mutex::new(None),
            stdin: Mutex::new(None),
            plugins: RwLock::new(HashMap::new()),
            request_id: AtomicU64::new(1),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            bridge,
            ui_update_callback: RwLock::new(None),
        }
    }

    /// Check if JRE is available
    pub fn check_jre() -> Result<String, String> {
        let output = Command::new("java")
            .arg("-version")
            .output()
            .map_err(|e| format!("Java not found: {}. Please install JRE 11 or later.", e))?;

        // Java prints version to stderr
        let version = String::from_utf8_lossy(&output.stderr);
        if output.status.success() || version.contains("version") {
            Ok(version.lines().next().unwrap_or("Unknown").to_string())
        } else {
            Err("Java not found. Please install JRE 11 or later.".to_string())
        }
    }

    /// Start the JVM plugin host process
    pub fn start(&self) -> Result<(), String> {
        let mut process_guard = self.process.lock().map_err(|e| e.to_string())?;
        if process_guard.is_some() {
            return Ok(()); // Already running
        }

        // Verify JRE is available
        Self::check_jre()?;

        // Verify plugin-host.jar exists
        if !self.host_jar_path.exists() {
            return Err(format!(
                "Plugin host JAR not found: {}",
                self.host_jar_path.display()
            ));
        }

        // Spawn JVM process
        let mut child = Command::new("java")
            .arg("-jar")
            .arg(&self.host_jar_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("Failed to start plugin host: {}", e))?;

        let stdin = child.stdin.take().ok_or("Failed to get stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to get stdout")?;

        *self.stdin.lock().map_err(|e| e.to_string())? = Some(stdin);
        *process_guard = Some(child);

        // Spawn reader thread for stdout
        self.spawn_reader_thread(stdout);

        Ok(())
    }

    /// Stop the JVM plugin host process
    pub fn stop(&self) -> Result<(), String> {
        // Send shutdown command
        let _ = self.send_notification("shutdown", None);

        // Close stdin to signal EOF
        *self.stdin.lock().map_err(|e| e.to_string())? = None;

        // Wait for process to exit
        if let Some(mut child) = self.process.lock().map_err(|e| e.to_string())?.take() {
            let _ = child.wait();
        }

        // Clear plugins
        self.plugins.write().map_err(|e| e.to_string())?.clear();

        Ok(())
    }

    /// Check if plugin host is running
    pub fn is_running(&self) -> bool {
        self.process.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    /// Load a plugin from JAR file
    pub async fn load_plugin(&self, jar_path: &Path) -> Result<PluginInfo, String> {
        if !self.is_running() {
            self.start()?;
        }

        let response = self
            .send_request(
                "loadPlugin",
                Some(serde_json::json!({
                    "jarPath": jar_path.to_string_lossy()
                })),
            )
            .await?;

        let info: PluginInfo = serde_json::from_value(
            response
                .result
                .ok_or_else(|| response.error.map(|e| e.message).unwrap_or_default())?,
        )
        .map_err(|e| format!("Failed to parse plugin info: {}", e))?;

        // Store plugin
        self.plugins.write().map_err(|e| e.to_string())?.insert(
            info.id.clone(),
            PluginInstance {
                info: info.clone(),
                ui_tree: None,
                enabled: true,
            },
        );

        Ok(info)
    }

    /// Unload a plugin
    pub async fn unload_plugin(&self, plugin_id: &str) -> Result<(), String> {
        self.send_request(
            "unloadPlugin",
            Some(serde_json::json!({ "pluginId": plugin_id })),
        )
        .await?;

        self.plugins
            .write()
            .map_err(|e| e.to_string())?
            .remove(plugin_id);

        Ok(())
    }

    /// Get list of loaded plugins
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins
            .read()
            .map(|g| g.values().map(|p| p.info.clone()).collect())
            .unwrap_or_default()
    }

    /// Get plugin UI tree
    pub fn get_plugin_ui(&self, plugin_id: &str) -> Option<SwingComponent> {
        self.plugins.read().ok()?.get(plugin_id)?.ui_tree.clone()
    }

    /// Send an event to a plugin (user interaction)
    pub async fn send_plugin_event(
        &self,
        plugin_id: &str,
        event: PluginEvent,
    ) -> Result<(), String> {
        self.send_request(
            "pluginEvent",
            Some(serde_json::json!({
                "pluginId": plugin_id,
                "event": event
            })),
        )
        .await?;
        Ok(())
    }

    /// Set callback for UI updates
    pub fn set_ui_update_callback<F>(&self, callback: F)
    where
        F: Fn(String, UiDiff) + Send + Sync + 'static,
    {
        if let Ok(mut cb) = self.ui_update_callback.write() {
            *cb = Some(Box::new(callback));
        }
    }

    /// Send JSON-RPC request and wait for response
    async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<RpcResponse, String> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = RpcRequest::new(id, method, params);

        // Create response channel
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending_requests
            .lock()
            .map_err(|e| e.to_string())?
            .insert(id, tx);

        // Send request
        self.write_message(&serde_json::to_string(&request).map_err(|e| e.to_string())?)?;

        // Wait for response (with timeout)
        tokio::time::timeout(std::time::Duration::from_secs(30), rx)
            .await
            .map_err(|_| "Request timeout".to_string())?
            .map_err(|_| "Response channel closed".to_string())
    }

    /// Send JSON-RPC notification (no response expected)
    fn send_notification(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), String> {
        let notification = RpcNotification::new(method, params);
        self.write_message(&serde_json::to_string(&notification).map_err(|e| e.to_string())?)
    }

    /// Write a message to JVM stdin
    fn write_message(&self, message: &str) -> Result<(), String> {
        let mut stdin_guard = self.stdin.lock().map_err(|e| e.to_string())?;
        let stdin = stdin_guard.as_mut().ok_or("Plugin host not running")?;

        writeln!(stdin, "{}", message)
            .map_err(|e| format!("Failed to write to plugin host: {}", e))?;
        stdin
            .flush()
            .map_err(|e| format!("Failed to flush stdin: {}", e))?;

        Ok(())
    }

    /// Spawn thread to read JVM stdout
    fn spawn_reader_thread(&self, stdout: ChildStdout) {
        let pending = self.pending_requests.clone();

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(msg) => {
                        // Try to parse as RPC response first
                        if let Ok(response) = serde_json::from_str::<RpcResponse>(&msg) {
                            // Route response to waiting caller via oneshot channel
                            let id = response.id;
                            if let Ok(mut map) = pending.lock() {
                                if let Some(tx) = map.remove(&id) {
                                    let _ = tx.send(response);
                                }
                            }
                        } else if let Ok(notification) =
                            serde_json::from_str::<RpcNotification>(&msg)
                        {
                            // Handle notification (UI updates, etc.)
                            eprintln!("[Plugin] Notification: {:?}", notification);
                        } else {
                            // Unknown message format - log for debugging
                            eprintln!("[Plugin] Unknown message: {}", msg);
                        }
                    }
                    Err(e) => {
                        eprintln!("[Plugin] Error reading from plugin host: {}", e);
                        break;
                    }
                }
            }
            eprintln!("[Plugin] Reader thread exiting");
        });
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Scan a directory for plugin JAR files
pub fn scan_plugins_dir(dir: &Path) -> Vec<PathBuf> {
    if !dir.is_dir() {
        return vec![];
    }

    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .map(|ext| ext.eq_ignore_ascii_case("jar"))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_plugins_dir_empty() {
        let plugins = scan_plugins_dir(Path::new("/nonexistent"));
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_jre_check() {
        // This test may fail if Java is not installed
        match PluginManager::check_jre() {
            Ok(version) => {
                assert!(
                    version.contains("version")
                        || version.contains("openjdk")
                        || version.contains("java")
                );
            }
            Err(e) => {
                assert!(e.contains("not found") || e.contains("Java"));
            }
        }
    }
}
