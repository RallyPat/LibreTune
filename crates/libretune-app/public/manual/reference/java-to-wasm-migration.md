# Java → WASM Plugin Migration Guide

**Status**: Java/JVM plugin system is deprecated (Feb 4, 2026).  
**Target**: LibreTune WASM plugin system (native, sandboxed, no JRE dependency).

## 1) Overview

LibreTune’s Java plugin system provided TunerStudio JAR compatibility through a JVM subprocess and Swing UI introspection. The native WASM system replaces this with a secure, permissioned, in-process runtime that is faster to load and easier to distribute.

This guide explains how to migrate Java/Swing plugins to WASM plugins.

## 2) Key Differences

| Area | Java Plugins (Deprecated) | WASM Plugins (Current) |
|------|---------------------------|-------------------------|
| Runtime | External JVM (JRE 11+) | Built-in (wasmtime) |
| UI | Swing (introspected) | Native LibreTune UI or headless |
| Security | Full JVM permissions | Explicit permissions |
| Packaging | JAR + manifest | .wasm + manifest |
| Transport | JSON-RPC over stdin/stdout | Host function calls |

## 3) Migration Checklist

1. **Inventory**: List all data access, UI components, and callbacks in your Java plugin.
2. **Choose Language**: Rust is recommended for WASM plugins (best tooling).
3. **Define Permissions**: Identify exactly what the plugin needs: `ReadTables`, `WriteConstants`, `SubscribeChannels`, `ExecuteActions`.
4. **Rewrite Logic**: Move data processing and ECU logic into WASM functions.
5. **Replace UI**: Swap Swing UI with native LibreTune UI or run headless.
6. **Test**: Load in the LibreTune Plugin Manager and validate outputs.

## 4) Permissions Mapping

| Java API Capability | WASM Permission |
|---------------------|-----------------|
| Read table data | `ReadTables` |
| Write constant | `WriteConstants` |
| Subscribe channels | `SubscribeChannels` |
| Execute actions | `ExecuteActions` |

## 5) API Mapping

### Java (Deprecated)
Common Java plugin API usage:
- `getParameters()` → List constants
- `setParameter()` → Change constant values
- `getRealtimeData()` → Channel access
- `getUI()` → Swing UI

### WASM (Current)
Common WASM host functions:
- `get_table_data(table_name)`
- `get_constant(constant_name)`
- `set_constant(constant_name, value)`
- `subscribe_channel(channel_name)`
- `get_channel_value(channel_name)`
- `execute_action(action_json)`
- `log_message(message)`

## 6) UI Migration Strategy

Java plugins used Swing components such as `JPanel`, `JTextField`, and `JButton`. WASM plugins should use one of the following approaches:

**Option A — Headless Plugins (Recommended)**
- Compute logic only
- No UI
- Use existing LibreTune UI components for output

**Option B — Native UI Integration**
- Add a new LibreTune UI panel or dialog (React/TypeScript)
- Connect to the plugin via events or shared data
- Best for complex UI workflows

## 7) Example Migration Outline

**Java plugin behavior**:
- Read VE table
- Apply smoothing algorithm
- Write modified table
- Provide a simple UI for smoothing factor

**WASM migration**:
1. `get_table_data("veTable1Tbl")`
2. Apply smoothing in WASM
3. `set_constant()` or future `set_table_data()` (if exposed)
4. Use a LibreTune dialog for the smoothing factor, then call plugin execution

## 8) Packaging

### Java (Deprecated)
- JAR file
- Manifest with `TunerStudio-Plugin` or `Main-Class`

### WASM (Current)
- `.wasm` binary
- `plugin.toml` manifest (name, version, permissions)

## 9) Testing

Use LibreTune’s Plugin Manager to:
1. Load the `.wasm` file
2. Verify permission prompts
3. Run `execute()` or entrypoint
4. Validate outputs or ECU changes

## 10) Common Pitfalls

- **UI rewrite is required**: Swing UIs do not migrate automatically.
- **Permissions must be explicit**: missing permissions will block host functions.
- **WASM is sandboxed**: file system access is not allowed unless explicitly supported.

## 11) Support

- **Docs**: `SPRINT_3_SUMMARY.md`
- **Issue Tracker**: Use GitHub issues for migration help
- **Community**: Provide feedback during the deprecation grace period

## 12) Next Steps

If you need help migrating a specific plugin, open a GitHub issue with:
- Plugin name
- Java API usage summary
- Required UI features
- Desired output format
