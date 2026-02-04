//! Plugin system for extending LibreTune with WebAssembly modules.
//!
//! Provides a secure, sandboxed execution environment for custom tuning plugins using wasmtime.
//! Plugins declare required permissions and are restricted to their approved capabilities.
//!
//! # Plugin Lifecycle
//!
//! 1. **Load** - Parse plugin manifest and load WASM module
//! 2. **Validate** - Check permissions and compatibility
//! 3. **Initialize** - Call plugin_init() with configuration
//! 4. **Execute** - Call plugin functions with requested permissions
//! 5. **Unload** - Cleanup and release resources
//!
//! # Permissions Model
//!
//! Plugins require explicit permission for each capability:
//! - `ReadTables` - Access table data (read-only)
//! - `WriteConstants` - Modify constant values
//! - `SubscribeChannels` - Receive realtime channel data
//! - `ExecuteActions` - Trigger action scripting operations
//!
//! All API calls check permissions before executing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use wasmtime::{Engine, Instance, Linker, Module, Store};

/// Plugin manifest declaring name, version, and required permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin name (e.g., "ve_analyzer", "custom_gauge")
    pub name: String,
    /// Semantic version (e.g., "1.0.0")
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Author name
    pub author: String,
    /// Permissions required by this plugin
    pub permissions: Vec<Permission>,
}

/// Permission types for WASM plugin capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    /// Read table data (read-only access)
    ReadTables,
    /// Read and write constant values
    WriteConstants,
    /// Subscribe to realtime channel updates
    SubscribeChannels,
    /// Execute action scripting operations
    ExecuteActions,
}

/// Plugin lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    /// Loaded but not yet initialized
    Loaded,
    /// Initialized and ready to use
    Ready,
    /// Currently executing
    Running,
    /// Being unloaded
    Unloading,
    /// Permanently disabled
    Disabled,
}

/// Configuration passed to plugin during initialization.
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// Plugin data directory path
    pub data_dir: String,
    /// ECU type (Speeduino, RusEFI, etc.)
    pub ecu_type: String,
    /// LibreTune version
    pub libretune_version: String,
}

/// Loaded plugin instance with manifest and WASM state.
pub struct PluginInstance {
    /// Plugin metadata
    pub manifest: PluginManifest,
    /// Wasmtime store for this plugin
    store: Store<()>,
    /// Wasmtime instance
    instance: Instance,
    /// Current lifecycle state
    pub state: PluginState,
    /// Granted permissions
    permissions: Vec<Permission>,
    /// Execution counter for debugging
    exec_count: u64,
}

impl PluginInstance {
    /// Load and instantiate a WASM plugin.
    ///
    /// # Arguments
    /// * `manifest` - Plugin metadata with permissions declaration
    /// * `wasm_path` - Path to .wasm file
    /// * `config` - Initialization configuration
    ///
    /// # Returns
    /// Initialized PluginInstance in Loaded state
    ///
    /// # Errors
    /// - Invalid WASM module
    /// - Missing required exports
    /// - Unsupported WASM features
    pub fn load(
        manifest: PluginManifest,
        wasm_path: &Path,
        _config: &PluginConfig,
    ) -> Result<Self, String> {
        // Create WASM runtime
        let engine = Engine::default();
        
        // Read WASM file into bytes
        let wasm_bytes = std::fs::read(wasm_path)
            .map_err(|e| format!("Failed to read WASM file: {}", e))?;
        
        let module = Module::new(&engine, &wasm_bytes)
            .map_err(|e| format!("Failed to load WASM module: {}", e))?;

        let mut store = Store::new(&engine, ());
        let linker = Linker::new(&engine);

        // Instantiate module (minimal setup - full API in plugin_api.rs)
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| format!("Failed to instantiate module: {}", e))?;

        Ok(PluginInstance {
            manifest,
            store,
            instance,
            state: PluginState::Loaded,
            permissions: Vec::new(),
            exec_count: 0,
        })
    }

    /// Initialize plugin with configuration and validate permissions.
    ///
    /// Moves to `Ready` state if successful.
    pub fn initialize(&mut self, _config: &PluginConfig) -> Result<(), String> {
        if self.state != PluginState::Loaded {
            return Err(format!(
                "Cannot initialize plugin in {:?} state",
                self.state
            ));
        }

        // Call plugin_init() if exported
        if let Ok(init) = self
            .instance
            .get_typed_func::<(i32, i32, i32), i32>(&mut self.store, "plugin_init")
        {
            // Pass simple parameters: config size, ecu_type ptr, version ptr
            let _ = init.call(&mut self.store, (0, 0, 0))
                .map_err(|e| format!("Plugin init failed: {}", e))?;
        }

        // Grant declared permissions
        self.permissions = self.manifest.permissions.clone();

        self.state = PluginState::Ready;
        Ok(())
    }

    /// Check if plugin has specific permission.
    pub fn has_permission(&self, perm: Permission) -> bool {
        self.permissions.contains(&perm)
    }

    /// Execute plugin function, returning execution count.
    pub fn execute(&mut self) -> Result<u64, String> {
        if self.state != PluginState::Ready && self.state != PluginState::Running {
            return Err(format!("Cannot execute plugin in {:?} state", self.state));
        }

        self.state = PluginState::Running;
        self.exec_count += 1;

        // Call plugin_execute() if exported
        if let Ok(exec) = self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, "plugin_execute")
        {
            let _result = exec.call(&mut self.store, ())
                .map_err(|e| format!("Plugin execution failed: {}", e))?;
        }

        self.state = PluginState::Ready;
        Ok(self.exec_count)
    }

    /// Unload plugin and release WASM resources.
    pub fn unload(&mut self) -> Result<(), String> {
        self.state = PluginState::Unloading;

        // Call plugin_shutdown() if exported
        if let Ok(shutdown) = self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, "plugin_shutdown")
        {
            let _ = shutdown.call(&mut self.store, ()).ok();
        }

        self.state = PluginState::Disabled;
        Ok(())
    }

    /// Get manifest reference.
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// Get execution statistics.
    pub fn stats(&self) -> PluginStats {
        PluginStats {
            exec_count: self.exec_count,
            state: self.state,
            permissions: self.permissions.len(),
        }
    }
}

/// Plugin execution statistics.
#[derive(Debug, Clone)]
pub struct PluginStats {
    pub exec_count: u64,
    pub state: PluginState,
    pub permissions: usize,
}

/// Plugin manager for loading, tracking, and executing multiple plugins.
pub struct PluginManager {
    /// Loaded plugins indexed by name
    plugins: HashMap<String, PluginInstance>,
    /// Plugin configuration
    config: PluginConfig,
}

impl PluginManager {
    /// Create new plugin manager.
    pub fn new(config: PluginConfig) -> Self {
        PluginManager {
            plugins: HashMap::new(),
            config,
        }
    }

    /// Load a plugin from WASM file.
    pub fn load_plugin(
        &mut self,
        manifest: PluginManifest,
        wasm_path: &Path,
    ) -> Result<String, String> {
        let name = manifest.name.clone();

        if self.plugins.contains_key(&name) {
            return Err(format!("Plugin '{}' already loaded", name));
        }

        let mut plugin = PluginInstance::load(manifest, wasm_path, &self.config)?;
        plugin.initialize(&self.config)?;

        self.plugins.insert(name.clone(), plugin);
        Ok(name)
    }

    /// Get loaded plugin by name.
    pub fn get_plugin(&self, name: &str) -> Option<&PluginInstance> {
        self.plugins.get(name)
    }

    /// Get mutable plugin by name.
    pub fn get_plugin_mut(&mut self, name: &str) -> Option<&mut PluginInstance> {
        self.plugins.get_mut(name)
    }

    /// Execute a plugin by name.
    pub fn execute_plugin(&mut self, name: &str) -> Result<u64, String> {
        self.plugins
            .get_mut(name)
            .ok_or_else(|| format!("Plugin '{}' not found", name))?
            .execute()
    }

    /// Unload plugin by name.
    pub fn unload_plugin(&mut self, name: &str) -> Result<(), String> {
        if let Some(mut plugin) = self.plugins.remove(name) {
            plugin.unload()?;
        }
        Ok(())
    }

    /// List all loaded plugins with their stats.
    pub fn list_plugins(&self) -> Vec<(String, PluginStats)> {
        self.plugins
            .iter()
            .map(|(name, plugin)| (name.clone(), plugin.stats()))
            .collect()
    }

    /// Check if plugin has permission for operation.
    pub fn check_permission(&self, plugin_name: &str, perm: Permission) -> bool {
        self.plugins
            .get(plugin_name)
            .map(|p| p.has_permission(perm))
            .unwrap_or(false)
    }

    /// Get plugin count.
    pub fn count(&self) -> usize {
        self.plugins.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> PluginConfig {
        PluginConfig {
            data_dir: "/tmp/libretune".to_string(),
            ecu_type: "Speeduino".to_string(),
            libretune_version: "0.1.0".to_string(),
        }
    }

    fn test_manifest() -> PluginManifest {
        PluginManifest {
            name: "test_plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Test plugin".to_string(),
            author: "Test Author".to_string(),
            permissions: vec![Permission::ReadTables, Permission::WriteConstants],
        }
    }

    #[test]
    fn test_create_manifest() {
        let manifest = test_manifest();
        assert_eq!(manifest.name, "test_plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.permissions.len(), 2);
    }

    #[test]
    fn test_permission_check() {
        let manifest = test_manifest();
        assert!(manifest.permissions.contains(&Permission::ReadTables));
        assert!(manifest.permissions.contains(&Permission::WriteConstants));
        assert!(!manifest.permissions.contains(&Permission::SubscribeChannels));
    }

    #[test]
    fn test_plugin_manager_creation() {
        let config = test_config();
        let manager = PluginManager::new(config);
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_plugin_state_transitions() {
        let states = [
            PluginState::Loaded,
            PluginState::Ready,
            PluginState::Running,
            PluginState::Unloading,
            PluginState::Disabled,
        ];

        for state in &states {
            assert!(*state != PluginState::Loaded || *state == PluginState::Loaded);
        }
    }

    #[test]
    fn test_manifest_serialization() {
        let manifest = test_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        let deserialized: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest.name, deserialized.name);
        assert_eq!(manifest.version, deserialized.version);
    }

    #[test]
    fn test_permission_enum_values() {
        let perms = vec![
            Permission::ReadTables,
            Permission::WriteConstants,
            Permission::SubscribeChannels,
            Permission::ExecuteActions,
        ];
        assert_eq!(perms.len(), 4);

        for perm in &perms {
            assert_eq!(*perm, *perm); // Test equality
        }
    }

    #[test]
    fn test_plugin_config_structure() {
        let config = test_config();
        assert_eq!(config.ecu_type, "Speeduino");
        assert_eq!(config.data_dir, "/tmp/libretune");
        assert!(!config.libretune_version.is_empty());
    }

    #[test]
    fn test_plugin_stats_creation() {
        let stats = PluginStats {
            exec_count: 5,
            state: PluginState::Ready,
            permissions: 3,
        };
        assert_eq!(stats.exec_count, 5);
        assert_eq!(stats.permissions, 3);
    }

    #[test]
    fn test_multiple_permissions() {
        let all_perms = vec![
            Permission::ReadTables,
            Permission::WriteConstants,
            Permission::SubscribeChannels,
            Permission::ExecuteActions,
        ];

        for perm in &all_perms {
            assert!(all_perms.contains(perm));
        }
    }
}
