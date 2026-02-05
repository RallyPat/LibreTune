# WASM Plugin System

LibreTune uses a native **WebAssembly (WASM)** plugin architecture to extend functionality dynamically. This system replaced the legacy Java/JRE plugin support as of February 2026.

## Overview

The WASM plugin system provides:

- **Sandboxed Execution**: Plugins run in isolation with a controlled API surface
- **No External Dependencies**: No JRE installation required
- **Permission Model**: Fine-grained control over what plugins can access
- **Host Functions**: Stable API for tuning data, ECU communication, and UI events
- **Lightweight**: Minimal memory footprint and startup time

## Architecture

```
┌─────────────────────────────────────┐
│      LibreTune Application          │
│  (Rust + React + Tauri Frontend)    │
└────────────────┬────────────────────┘
                 │
         ┌───────▼────────┐
         │  WASM Runtime  │
         │   (wasmtime)   │
         └───────┬────────┘
                 │
      ┌──────────┼──────────┐
      ▼          ▼          ▼
  ┌────────┐ ┌────────┐ ┌────────┐
  │Plugin1 │ │Plugin2 │ │Plugin3 │
  │ (WASM) │ │ (WASM) │ │ (WASM) │
  └────────┘ └────────┘ └────────┘
```

Each plugin is a compiled WASM binary (.wasm file) with:
- Entry points for lifecycle events (init, destroy, etc.)
- Implementations of plugin hooks
- Calls to host functions through the plugin API

## Plugin Manifest

Plugins declare their capabilities via a `plugin.json` manifest:

```json
{
  "name": "example-plugin",
  "version": "1.0.0",
  "description": "Example plugin",
  "author": "Your Name",
  "permissions": ["read_tune", "read_ecu_data"],
  "exports": {
    "init": "plugin_init",
    "on_tune_loaded": "plugin_on_tune_loaded"
  }
}
```

### Manifest Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Plugin identifier (kebab-case) |
| `version` | string | Semantic version (e.g., "1.0.0") |
| `description` | string | Human-readable description |
| `author` | string | Plugin author name |
| `permissions` | array | Required permissions (see table below) |
| `exports` | object | WASM function exports for hooks |

## Permissions

Plugins must declare all required permissions. Accessing unpermitted resources raises an error.

| Permission | Access |
|------------|--------|
| `read_tune` | Read-only access to tune constants and tables |
| `write_tune` | Modify tune constants and tables |
| `read_ecu_data` | Read ECU output channels and realtime data |
| `send_ecu_command` | Send commands to the ECU |
| `read_logs` | Access data logging and diagnostic output |
| `ui_events` | Receive UI events (tab changes, etc.) |

## Host Functions

Plugins interact with LibreTune through host functions. These are Rust functions exported to WASM.

### Tune Access

```rust
// Read a constant value (scalar)
host_read_constant(name_ptr: i32, name_len: i32) -> f64

// Read a table cell value
host_read_table_cell(table_ptr: i32, table_len: i32, 
                     x: i32, y: i32) -> f64

// Write a table cell value  
host_write_table_cell(table_ptr: i32, table_len: i32,
                      x: i32, y: i32, value: f64) -> i32
```

### ECU Communication

```rust
// Send a command to ECU
host_send_ecu_command(cmd_ptr: i32, cmd_len: i32) -> i32

// Get realtime channel value
host_get_realtime_value(channel_ptr: i32, channel_len: i32) -> f64

// Get current connection status
host_get_connection_status() -> i32  // 0=disconnected, 1=connected
```

### Logging

```rust
// Log a debug message
host_log_debug(msg_ptr: i32, msg_len: i32)

// Log an error message
host_log_error(msg_ptr: i32, msg_len: i32)
```

## Plugin Lifecycle

```
1. Plugin Loaded
   ├─ Manifest validated
   ├─ Permissions checked
   └─ WASM module instantiated

2. Plugin Initialized
   ├─ host_init() called
   └─ Plugin ready for events

3. Plugin Active
   ├─ Hook functions called on events
   ├─ Host functions available
   └─ State maintained across calls

4. Plugin Unloaded
   ├─ host_destroy() called
   ├─ Cleanup performed
   └─ Resources released
```

## Plugin Hooks

Plugins can export functions that LibreTune calls at specific events:

| Hook | Signature | Trigger |
|------|-----------|---------|
| `init` | `() -> i32` | Plugin loaded, before any events |
| `destroy` | `() -> void` | Plugin being unloaded |
| `on_tune_loaded` | `() -> void` | Tune file opened/loaded |
| `on_tune_updated` | `() -> void` | Tune constants modified |
| `on_ecu_connected` | `(connection_info_ptr: i32) -> void` | ECU connected |
| `on_realtime_update` | `() -> void` | Realtime data arrived (100ms) |
| `on_table_painted` | `(table_name_ptr: i32, table_name_len: i32) -> void` | Table rendered to screen |

## Example Plugin (Rust + WASM)

```rust
// Example: Simple VE table analyzer plugin
use std::panic;

#[panic_handler]
fn handle_panic(_: &panic::PanicInfo) -> ! {
    loop {}
}

extern "C" {
    fn host_read_table_cell(table_ptr: i32, table_len: i32, x: i32, y: i32) -> f64;
    fn host_log_debug(msg_ptr: i32, msg_len: i32);
}

#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
    let msg = b"VE Analyzer Plugin 1.0 loaded";
    unsafe {
        host_log_debug(msg.as_ptr() as i32, msg.len() as i32);
    }
    0  // Success
}

#[no_mangle]
pub extern "C" fn plugin_on_table_painted(table_ptr: i32, table_len: i32) -> () {
    // Analyze table whenever it's displayed
    unsafe {
        let value = host_read_table_cell(table_ptr, table_len, 0, 0);
        // Process value...
    }
}
```

## Building Plugins

### Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

### Build

```bash
wasm-pack build --target web --release
```

### Plugin Distribution

1. Create `plugin.json` manifest
2. Compile WASM binary with `wasm-pack`
3. Package as `.wasm` file
4. Users load via LibreTune UI or config directory

## Security Considerations

- **Sandboxing**: WASM runtime is isolated from host system
- **Permission Checking**: Violations result in runtime errors
- **Resource Limits**: Plugins have configurable memory limits
- **No File Access**: Plugins cannot directly access filesystem
- **No Network Access**: Plugins cannot make external connections

## Debugging Plugins

Enable debug logging:

```bash
RUST_LOG=libretune_core::plugin_system=debug ./libretune
```

View plugin logs in the application log output or console.

## See Also

- [Java to WASM Migration Guide](../reference/java-to-wasm-migration.md)
- [Plugin Development Tutorial](./plugin-development.md) (coming soon)
