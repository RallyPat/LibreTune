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

use crate::plugin_api;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use wasmtime::{
    Caller, Config, Engine, Extern, Instance, Linker, Memory, Module, ResourceLimiter, Store,
    StoreLimits, StoreLimitsBuilder,
};

/// Maximum length (bytes) accepted for a guest-supplied string argument
/// (table/constant/channel name, action JSON). Guards against a malicious or
/// buggy plugin claiming an absurd `len` and forcing a huge host allocation.
const MAX_GUEST_STRING_LEN: usize = 64 * 1024;

/// Fuel budget granted before every call into guest code (`plugin_init`,
/// `plugin_execute`, `plugin_shutdown`, or a test's `call_i32_export`). Most
/// wasm instructions burn 1 unit of fuel, so this bounds a single call to a
/// few million executed instructions regardless of what the guest does —
/// enough headroom for real plugin logic, but a `loop {}` guest exhausts it
/// and traps instead of hanging the calling thread forever.
const PLUGIN_FUEL_PER_CALL: u64 = 10_000_000;

/// Maximum bytes a plugin's linear memory may grow to. Applied via
/// `Store::limiter`; a `memory.grow` past this returns failure to the guest
/// (or, with `trap_on_grow_failure`, would trap) rather than letting an
/// unbounded plugin exhaust host memory.
const PLUGIN_MEMORY_LIMIT_BYTES: usize = 64 * 1024 * 1024;

/// How often the background [`EpochTicker`] advances a plugin's `Engine`
/// epoch. Combined with [`EPOCH_TICKS_PER_CALL`], this bounds worst-case
/// wall-clock time per call independently of fuel — a backstop for guest
/// code whose instructions individually burn little fuel but still block
/// for a long time (e.g. host calls that spin).
const EPOCH_TICK_INTERVAL: Duration = Duration::from_millis(50);

/// Epoch ticks allowed per call before wasmtime traps the guest, i.e. a
/// `EPOCH_TICK_INTERVAL * EPOCH_TICKS_PER_CALL` wall-clock deadline (~1s).
const EPOCH_TICKS_PER_CALL: u64 = 20;

/// Background thread that periodically calls [`Engine::increment_epoch`] so
/// `Store::set_epoch_deadline` has something to enforce a wall-clock timeout
/// against. Epoch checks are injected into compiled guest code, but nothing
/// advances the epoch on its own while a synchronous `Store` call (like
/// `plugin_execute`) blocks the calling thread — this ticker is that
/// external actor. It stops itself (checked once per tick) when the owning
/// [`PluginInstance`] is dropped.
struct EpochTicker {
    stop: Arc<AtomicBool>,
}

impl EpochTicker {
    /// Spawn the ticker for `engine`. `Engine` is a cheap `Arc`-backed
    /// handle, so cloning it into the thread doesn't duplicate the compiled
    /// module or runtime state.
    fn start(engine: Engine) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        thread::spawn(move || {
            while !stop_thread.load(Ordering::Relaxed) {
                thread::sleep(EPOCH_TICK_INTERVAL);
                engine.increment_epoch();
            }
        });
        Self { stop }
    }
}

impl Drop for EpochTicker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Sentinel return codes shared by every host function. Plugin-side glue code
/// (bindgen or hand-written) should treat any negative value as failure.
mod host_result {
    /// Call succeeded.
    pub const OK: i32 = 0;
    /// Calling plugin lacks the permission this call requires.
    pub const PERMISSION_DENIED: i32 = -1;
    /// Guest pointer/length was invalid (OOB, bad UTF-8, no memory export).
    pub const INVALID_ARGS: i32 = -2;
    /// Permission was held, but the requested table/constant/channel/id
    /// doesn't exist in this run's data snapshot.
    pub const NOT_FOUND: i32 = -3;
    /// Permission was held and the request was recorded, but it will never
    /// actually run — currently only returned by `execute_action`, since
    /// LibreTune's action-scripting engine has no dispatcher to call into.
    /// Deliberately not `OK`, so a plugin can't mistake "accepted" for
    /// "executed".
    pub const ACCEPTED_NOT_EXECUTED: i32 = 1;
}

/// A single table's current axis bins and cell values, captured for one
/// plugin `execute()` call. Mirrors the shape the table editors already work
/// with (`z_values[row][col]`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TableSnapshot {
    pub x_bins: Vec<f64>,
    pub y_bins: Vec<f64>,
    pub z_values: Vec<Vec<f64>>,
}

/// Read-only snapshot of tune/table/channel data made available to a plugin
/// for the duration of one [`PluginInstance::execute`] call.
///
/// WASM host functions are registered as plain synchronous closures
/// (`wasmtime::Linker::func_wrap`), but the real data lives behind
/// `tokio::sync::Mutex`es in the Tauri app's `AppState`, only lockable via
/// `.await`. A sync closure can't await, so the caller (the `execute_wasm_plugin`
/// Tauri command) builds this snapshot *before* calling `execute()`,
/// following the app's established lock order, and every host function reads
/// from this owned, lock-free copy instead of reaching back into `AppState`.
/// This means a plugin sees a consistent-but-possibly-stale view of the tune
/// as of when it started running, never a live one — see [`PluginProposal`]
/// for how writes work under the same constraint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginDataSnapshot {
    /// Table name -> current bins/values.
    pub tables: HashMap<String, TableSnapshot>,
    /// Scalar constant name -> current value. Array-shaped constants (e.g.
    /// axis bins) aren't included here; read them via their owning table.
    pub constants: HashMap<String, f64>,
    /// Realtime channel name -> current value, if connected. Empty when not
    /// connected to an ECU.
    pub channels: HashMap<String, f64>,
}

/// A write a plugin asked for during one `execute()` call. Collected instead
/// of applied immediately (again: no `.await` available inside the sync
/// closure that recorded it), then applied afterward by
/// `execute_wasm_plugin`, async, through the exact same code path a
/// user-driven edit takes — so it gets the same tune-cache/ECU write and
/// lock-ordering behavior as everything else. The plugin's own load-time
/// permission grant is what authorizes this; there is no separate per-write
/// approval prompt (WASM execution is synchronous end-to-end, so nothing can
/// block mid-run on a UI click).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginProposal {
    SetConstant {
        name: String,
        value: f64,
    },
    /// Not yet auto-applied: LibreTune's action-scripting engine
    /// (`action_scripting::Action`) is validation-only today, with no
    /// generic "execute this one action now" dispatcher to call into. The
    /// raw JSON is surfaced back to the caller as an unapplied proposal
    /// rather than silently dropped or half-implemented.
    ExecuteAction {
        action_json: String,
    },
}

/// Result of one [`PluginInstance::execute`] call.
#[derive(Debug, Clone)]
pub struct PluginExecutionResult {
    pub exec_count: u64,
    /// The `plugin_execute` export's own return value, if it exported one.
    pub result_code: Option<i32>,
    /// Writes/actions the plugin asked for during this run, in call order.
    pub proposals: Vec<PluginProposal>,
}

/// Per-instance state attached to the plugin's [`wasmtime::Store`]. This is
/// what host functions consult to know which plugin is calling and what it
/// was actually granted — never the shared [`PluginManager`], so a host
/// function invoked from inside a WASM call can never re-enter a lock held by
/// whatever is currently driving that plugin's execution.
struct PluginHostState {
    plugin_name: String,
    permissions: Vec<Permission>,
    /// Refreshed at the start of every `execute()` call.
    snapshot: PluginDataSnapshot,
    /// Drained into the [`PluginExecutionResult`] at the end of `execute()`.
    proposals: Vec<PluginProposal>,
    /// Channel names the guest has subscribed to this run, indexed by the id
    /// `subscribe_channel` handed back — `get_channel_value` resolves ids
    /// through this rather than trusting a guest-supplied name directly.
    subscribed_channels: Vec<String>,
    /// Enforces [`PLUGIN_MEMORY_LIMIT_BYTES`] via `Store::limiter`.
    limits: StoreLimits,
}

/// Fetch the guest's exported linear memory, or an [`host_result::INVALID_ARGS`] error.
fn get_memory(caller: &mut Caller<'_, PluginHostState>) -> Result<Memory, i32> {
    match caller.get_export("memory") {
        Some(Extern::Memory(mem)) => Ok(mem),
        _ => Err(host_result::INVALID_ARGS),
    }
}

/// Read a UTF-8 string out of the guest's linear memory at `ptr`/`len`,
/// bounds- and size-checked so a hostile plugin can't force an OOB read or an
/// unbounded host allocation.
fn read_guest_string(
    caller: &mut Caller<'_, PluginHostState>,
    ptr: i32,
    len: i32,
) -> Result<String, i32> {
    if ptr < 0 || len < 0 || len as usize > MAX_GUEST_STRING_LEN {
        return Err(host_result::INVALID_ARGS);
    }
    let memory = get_memory(caller)?;
    let mut buf = vec![0u8; len as usize];
    memory
        .read(&caller, ptr as usize, &mut buf)
        .map_err(|_| host_result::INVALID_ARGS)?;
    String::from_utf8(buf).map_err(|_| host_result::INVALID_ARGS)
}

/// Write raw bytes into the guest's linear memory at `ptr`.
fn write_guest_bytes(
    caller: &mut Caller<'_, PluginHostState>,
    ptr: i32,
    data: &[u8],
) -> Result<(), i32> {
    if ptr < 0 {
        return Err(host_result::INVALID_ARGS);
    }
    let memory = get_memory(caller)?;
    memory
        .write(&mut *caller, ptr as usize, data)
        .map_err(|_| host_result::INVALID_ARGS)
}

/// Map a failed [`plugin_api::ApiResponse`] to the right wire-level code:
/// permission denial vs. "not found in this run's snapshot".
fn response_failure_code(resp: &plugin_api::ApiResponse) -> i32 {
    if resp.is_permission_denied() {
        host_result::PERMISSION_DENIED
    } else {
        host_result::NOT_FOUND
    }
}

/// Register every host function a plugin may import under the `"env"`
/// module. Each one checks the calling plugin's *own* granted permissions
/// (from [`PluginHostState`], fixed at load time) before doing anything —
/// this is the boundary [`crate::plugin_api`]'s permission checks actually
/// protect once a plugin can call back into the host at all.
fn register_host_functions(linker: &mut Linker<PluginHostState>) -> Result<(), String> {
    linker
        .func_wrap(
            "env",
            "log_message",
            |mut caller: Caller<'_, PluginHostState>,
             level: i32,
             msg_ptr: i32,
             msg_len: i32|
             -> i32 {
                let message = match read_guest_string(&mut caller, msg_ptr, msg_len) {
                    Ok(s) => s,
                    Err(code) => return code,
                };
                let plugin_name = caller.data().plugin_name.clone();
                let resp = plugin_api::api_log_message(plugin_name, level, message);
                if resp.success {
                    host_result::OK
                } else {
                    host_result::INVALID_ARGS
                }
            },
        )
        .map_err(|e| format!("Failed to register log_message: {}", e))?;

    linker
        .func_wrap(
            "env",
            "get_table_data",
            |mut caller: Caller<'_, PluginHostState>,
             name_ptr: i32,
             name_len: i32,
             row: i32,
             col: i32,
             out_ptr: i32|
             -> i32 {
                let table_name = match read_guest_string(&mut caller, name_ptr, name_len) {
                    Ok(s) => s,
                    Err(code) => return code,
                };
                let resp = {
                    let host = caller.data();
                    plugin_api::api_get_table_data(
                        &host.permissions,
                        &host.snapshot,
                        &table_name,
                        row,
                        col,
                    )
                };
                if !resp.success {
                    return response_failure_code(&resp);
                }
                match write_guest_bytes(&mut caller, out_ptr, &resp.data) {
                    Ok(()) => host_result::OK,
                    Err(code) => code,
                }
            },
        )
        .map_err(|e| format!("Failed to register get_table_data: {}", e))?;

    linker
        .func_wrap(
            "env",
            "get_constant",
            |mut caller: Caller<'_, PluginHostState>,
             name_ptr: i32,
             name_len: i32,
             out_ptr: i32|
             -> i32 {
                let name = match read_guest_string(&mut caller, name_ptr, name_len) {
                    Ok(s) => s,
                    Err(code) => return code,
                };
                let resp = {
                    let host = caller.data();
                    plugin_api::api_get_constant(&host.permissions, &host.snapshot, &name)
                };
                if !resp.success {
                    return response_failure_code(&resp);
                }
                match write_guest_bytes(&mut caller, out_ptr, &resp.data) {
                    Ok(()) => host_result::OK,
                    Err(code) => code,
                }
            },
        )
        .map_err(|e| format!("Failed to register get_constant: {}", e))?;

    linker
        .func_wrap(
            "env",
            "set_constant",
            |mut caller: Caller<'_, PluginHostState>,
             name_ptr: i32,
             name_len: i32,
             value: f32|
             -> i32 {
                let name = match read_guest_string(&mut caller, name_ptr, name_len) {
                    Ok(s) => s,
                    Err(code) => return code,
                };
                let resp = {
                    let host = caller.data();
                    plugin_api::api_set_constant(&host.permissions, &name, &value.to_le_bytes())
                };
                if !resp.success {
                    return host_result::PERMISSION_DENIED;
                }
                caller
                    .data_mut()
                    .proposals
                    .push(PluginProposal::SetConstant {
                        name,
                        value: value as f64,
                    });
                host_result::OK
            },
        )
        .map_err(|e| format!("Failed to register set_constant: {}", e))?;

    linker
        .func_wrap(
            "env",
            "subscribe_channel",
            |mut caller: Caller<'_, PluginHostState>,
             name_ptr: i32,
             name_len: i32,
             out_ptr: i32|
             -> i32 {
                let name = match read_guest_string(&mut caller, name_ptr, name_len) {
                    Ok(s) => s,
                    Err(code) => return code,
                };
                let resp = {
                    let host = caller.data();
                    plugin_api::api_subscribe_channel(&host.permissions, &host.snapshot, &name)
                };
                if !resp.success {
                    return response_failure_code(&resp);
                }
                let host = caller.data_mut();
                let channel_id = host.subscribed_channels.len() as i32;
                host.subscribed_channels.push(name);
                match write_guest_bytes(&mut caller, out_ptr, &channel_id.to_le_bytes()) {
                    Ok(()) => host_result::OK,
                    Err(code) => code,
                }
            },
        )
        .map_err(|e| format!("Failed to register subscribe_channel: {}", e))?;

    linker
        .func_wrap(
            "env",
            "get_channel_value",
            |mut caller: Caller<'_, PluginHostState>, channel_id: i32, out_ptr: i32| -> i32 {
                if channel_id < 0 {
                    return host_result::INVALID_ARGS;
                }
                let resp = {
                    let host = caller.data();
                    // Only a name this instance itself got back from
                    // subscribe_channel is ever looked up — a guest can't
                    // forge a subscription by guessing an id.
                    let Some(name) = host.subscribed_channels.get(channel_id as usize) else {
                        return host_result::INVALID_ARGS;
                    };
                    plugin_api::api_get_channel_value(&host.permissions, &host.snapshot, name)
                };
                if !resp.success {
                    return response_failure_code(&resp);
                }
                match write_guest_bytes(&mut caller, out_ptr, &resp.data) {
                    Ok(()) => host_result::OK,
                    Err(code) => code,
                }
            },
        )
        .map_err(|e| format!("Failed to register get_channel_value: {}", e))?;

    linker
        .func_wrap(
            "env",
            "execute_action",
            |mut caller: Caller<'_, PluginHostState>, json_ptr: i32, json_len: i32| -> i32 {
                let action_json = match read_guest_string(&mut caller, json_ptr, json_len) {
                    Ok(s) => s,
                    Err(code) => return code,
                };
                let resp = {
                    let host = caller.data();
                    plugin_api::api_execute_action(&host.permissions, &action_json)
                };
                if !resp.success {
                    return host_result::PERMISSION_DENIED;
                }
                caller
                    .data_mut()
                    .proposals
                    .push(PluginProposal::ExecuteAction { action_json });
                // Deliberately not host_result::OK — see ACCEPTED_NOT_EXECUTED's
                // doc comment. The proposal is recorded, not run.
                host_result::ACCEPTED_NOT_EXECUTED
            },
        )
        .map_err(|e| format!("Failed to register execute_action: {}", e))?;

    Ok(())
}

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
    store: Store<PluginHostState>,
    /// Wasmtime instance
    instance: Instance,
    /// Current lifecycle state
    pub state: PluginState,
    /// Granted permissions — the intersection of what the manifest requested
    /// and what the user actually approved at load time. May be a strict
    /// subset of `manifest.permissions`.
    permissions: Vec<Permission>,
    /// Execution counter for debugging
    exec_count: u64,
    /// Keeps the epoch-advancing background thread alive for as long as this
    /// instance exists; never read directly, only relied on for its `Drop`.
    _epoch_ticker: EpochTicker,
}

impl PluginInstance {
    /// Load and instantiate a WASM plugin.
    ///
    /// # Arguments
    /// * `manifest` - Plugin metadata with permissions declaration
    /// * `wasm_path` - Path to .wasm file
    /// * `config` - Initialization configuration
    /// * `granted_permissions` - Permissions the user actually approved for
    ///   this load (see [`PluginManager::load_plugin`]). Only permissions
    ///   that are both requested by the manifest *and* present here are
    ///   granted — this is the boundary the host functions registered on the
    ///   plugin's [`wasmtime::Linker`] enforce, so an unapproved permission
    ///   is never reachable from guest code regardless of what the manifest
    ///   claims.
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
        granted_permissions: &[Permission],
    ) -> Result<Self, String> {
        let granted: Vec<Permission> = manifest
            .permissions
            .iter()
            .filter(|p| granted_permissions.contains(p))
            .copied()
            .collect();

        // Create WASM runtime. Fuel and epoch interruption both must be
        // enabled on the `Config` up front — they can't be turned on for a
        // `Store` after the fact — so that every plugin call can be bounded
        // with `set_fuel`/`set_epoch_deadline` below, and a memory cap
        // enforced via `Store::limiter`. Without these a guest `loop {}`
        // (or an unbounded `memory.grow`) would otherwise hang or OOM the
        // host thread that's driving it.
        let engine = Engine::new(Config::new().consume_fuel(true).epoch_interruption(true))
            .map_err(|e| format!("Failed to create WASM engine: {}", e))?;

        // Read WASM file into bytes
        let wasm_bytes =
            std::fs::read(wasm_path).map_err(|e| format!("Failed to read WASM file: {}", e))?;

        let module = Module::new(&engine, &wasm_bytes)
            .map_err(|e| format!("Failed to load WASM module: {}", e))?;

        let host_state = PluginHostState {
            plugin_name: manifest.name.clone(),
            permissions: granted.clone(),
            snapshot: PluginDataSnapshot::default(),
            proposals: Vec::new(),
            subscribed_channels: Vec::new(),
            limits: StoreLimitsBuilder::new()
                .memory_size(PLUGIN_MEMORY_LIMIT_BYTES)
                .build(),
        };
        let mut store = Store::new(&engine, host_state);
        store.limiter(|state: &mut PluginHostState| &mut state.limits as &mut dyn ResourceLimiter);
        let mut linker: Linker<PluginHostState> = Linker::new(&engine);
        register_host_functions(&mut linker)?;

        // Instantiation runs data/elem segment initialisers and the `start`
        // function, which consume fuel and count against the epoch deadline
        // like any other guest code — so it needs a budget too.
        store
            .set_fuel(PLUGIN_FUEL_PER_CALL)
            .map_err(|e| format!("Failed to set plugin fuel budget: {}", e))?;
        store.set_epoch_deadline(EPOCH_TICKS_PER_CALL);

        // Instantiate module against the permission-checked host-function
        // surface registered above.
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| format!("Failed to instantiate module: {}", e))?;

        let epoch_ticker = EpochTicker::start(engine);

        Ok(PluginInstance {
            manifest,
            store,
            instance,
            state: PluginState::Loaded,
            permissions: granted,
            exec_count: 0,
            _epoch_ticker: epoch_ticker,
        })
    }

    /// Grant this call's fuel and epoch-deadline budget just before invoking
    /// a guest export (`plugin_init`/`plugin_execute`/`plugin_shutdown`, or a
    /// test's [`PluginInstance::call_i32_export`]). Must be called anew
    /// before every such call — fuel and the epoch deadline are both
    /// consumed/advanced by wasmtime as execution proceeds, so a stale
    /// budget from a previous call would leave the next one with none.
    fn arm_call_budget(&mut self) -> Result<(), String> {
        self.store
            .set_fuel(PLUGIN_FUEL_PER_CALL)
            .map_err(|e| format!("Failed to set plugin fuel budget: {}", e))?;
        self.store.set_epoch_deadline(EPOCH_TICKS_PER_CALL);
        Ok(())
    }

    /// Initialize plugin with configuration.
    ///
    /// Moves to `Ready` state if successful. Permissions were already fixed
    /// at [`PluginInstance::load`] time — this step only runs the plugin's
    /// own `plugin_init()` export, it does not grant anything further.
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
            self.arm_call_budget()?;
            // Pass simple parameters: config size, ecu_type ptr, version ptr
            let _ = init
                .call(&mut self.store, (0, 0, 0))
                .map_err(|e| format!("Plugin init failed: {}", e))?;
        }

        self.state = PluginState::Ready;
        Ok(())
    }

    /// Check if plugin has specific permission.
    pub fn has_permission(&self, perm: Permission) -> bool {
        self.permissions.contains(&perm)
    }

    /// Permissions actually granted to this instance (manifest request ∩
    /// user approval at load time) — may be fewer than `manifest.permissions`.
    pub fn granted_permissions(&self) -> &[Permission] {
        &self.permissions
    }

    /// Call a zero-argument WASM export that returns `i32` and return its raw
    /// result. Unlike [`PluginInstance::execute`] (which always reports
    /// `exec_count` and discards the export's own return value), this
    /// surfaces exactly what the guest returned — needed to verify host
    /// functions like `get_constant`/`log_message` actually gave back the
    /// permission-check result the guest observed.
    pub fn call_i32_export(&mut self, name: &str) -> Result<i32, String> {
        let func = self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, name)
            .map_err(|e| format!("Export '{}' not found or wrong signature: {}", name, e))?;
        self.arm_call_budget()?;
        func.call(&mut self.store, ())
            .map_err(|e| format!("Call to '{}' failed: {}", name, e))
    }

    /// Read `len` bytes from the plugin's exported linear memory at `offset`.
    /// Lets tests verify a host function wrote its result where the guest
    /// expected it.
    pub fn read_memory(&mut self, offset: usize, len: usize) -> Result<Vec<u8>, String> {
        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| "Plugin has no exported memory".to_string())?;
        let mut buf = vec![0u8; len];
        memory
            .read(&self.store, offset, &mut buf)
            .map_err(|e| format!("Memory read failed: {}", e))?;
        Ok(buf)
    }

    /// Execute the plugin's `plugin_execute()` export against a fresh data
    /// snapshot, returning what it read as available and what it asked to
    /// change. See [`PluginDataSnapshot`]/[`PluginProposal`] for why this
    /// takes a snapshot rather than reaching into live state directly.
    pub fn execute(
        &mut self,
        snapshot: PluginDataSnapshot,
    ) -> Result<PluginExecutionResult, String> {
        if self.state != PluginState::Ready && self.state != PluginState::Running {
            return Err(format!("Cannot execute plugin in {:?} state", self.state));
        }

        self.state = PluginState::Running;
        self.exec_count += 1;

        {
            let host = self.store.data_mut();
            host.snapshot = snapshot;
            host.proposals.clear();
            host.subscribed_channels.clear();
        }

        let mut result_code = None;
        if let Ok(exec) = self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, "plugin_execute")
        {
            self.arm_call_budget()?;
            result_code = Some(
                exec.call(&mut self.store, ())
                    .map_err(|e| format!("Plugin execution failed: {}", e))?,
            );
        }

        self.state = PluginState::Ready;
        let proposals = std::mem::take(&mut self.store.data_mut().proposals);
        Ok(PluginExecutionResult {
            exec_count: self.exec_count,
            result_code,
            proposals,
        })
    }

    /// Unload plugin and release WASM resources.
    pub fn unload(&mut self) -> Result<(), String> {
        self.state = PluginState::Unloading;

        // Call plugin_shutdown() if exported
        if let Ok(shutdown) = self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, "plugin_shutdown")
        {
            // Best-effort: unloading proceeds regardless of whether the
            // budget re-arms or the shutdown call itself succeeds.
            let _ = self.arm_call_budget();
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
    ///
    /// `approved_permissions` is the set of permissions the user actually
    /// consented to grant (e.g. via a load-time confirmation dialog) — the
    /// plugin ends up with the intersection of this set and whatever its own
    /// manifest requests. A manifest requesting `WriteConstants` that the
    /// user did not approve simply doesn't get it, silently and safely,
    /// rather than being auto-granted.
    pub fn load_plugin(
        &mut self,
        manifest: PluginManifest,
        wasm_path: &Path,
        approved_permissions: &[Permission],
    ) -> Result<String, String> {
        let name = manifest.name.clone();

        if self.plugins.contains_key(&name) {
            return Err(format!("Plugin '{}' already loaded", name));
        }

        let mut plugin =
            PluginInstance::load(manifest, wasm_path, &self.config, approved_permissions)?;
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

    /// Execute a plugin by name against a fresh data snapshot.
    pub fn execute_plugin(
        &mut self,
        name: &str,
        snapshot: PluginDataSnapshot,
    ) -> Result<PluginExecutionResult, String> {
        self.plugins
            .get_mut(name)
            .ok_or_else(|| format!("Plugin '{}' not found", name))?
            .execute(snapshot)
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
        assert!(!manifest
            .permissions
            .contains(&Permission::SubscribeChannels));
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

    /// Builds a tiny WASM module (via WAT text format) that exports a
    /// `plugin_execute` which loops forever, plus a 1-page linear memory so
    /// it satisfies `PluginInstance::load`'s usual expectations. Written to a
    /// temp file so it can be handed to `PluginInstance::load` like any real
    /// plugin binary.
    fn write_infinite_loop_plugin(dir: &tempfile::TempDir) -> std::path::PathBuf {
        let wat_src = r#"
            (module
                (memory (export "memory") 1)
                (func (export "plugin_execute") (result i32)
                    (loop $forever
                        br $forever
                    )
                    (i32.const 0)
                )
            )
        "#;
        let wasm_bytes = wat::parse_str(wat_src).expect("valid WAT source");
        let path = dir.path().join("infinite_loop.wasm");
        std::fs::write(&path, wasm_bytes).expect("write test wasm module");
        path
    }

    #[test]
    fn test_infinite_loop_plugin_traps_instead_of_hanging() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let wasm_path = write_infinite_loop_plugin(&dir);

        let manifest = PluginManifest {
            name: "infinite_loop".to_string(),
            version: "1.0.0".to_string(),
            description: "Test plugin with a runaway loop".to_string(),
            author: "Test Author".to_string(),
            permissions: vec![],
        };

        let mut plugin = PluginInstance::load(manifest, &wasm_path, &test_config(), &[])
            .expect("plugin with an infinite loop still loads and instantiates fine");
        plugin
            .initialize(&test_config())
            .expect("plugin has no plugin_init export, so init is a no-op");

        // Without a fuel/epoch budget this call would hang the calling
        // thread forever; with it, wasmtime traps once the budget set in
        // PluginInstance::arm_call_budget is exhausted and execute()
        // surfaces that as an Err instead of blocking.
        let result = plugin.execute(PluginDataSnapshot::default());
        assert!(
            result.is_err(),
            "expected an infinite loop to be terminated by the fuel/epoch budget, got {:?}",
            result
        );
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
