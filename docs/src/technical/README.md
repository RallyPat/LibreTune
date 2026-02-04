# Technical Reference

This section provides in-depth technical documentation for LibreTune's core algorithms, data structures, and protocols. It's intended for developers, advanced users, and anyone wanting to understand how LibreTune works under the hood.

## Contents

### Algorithms
- **[AutoTune Algorithm](./autotune-algorithm.md)** - Lambda delay compensation, transient filtering, hit weighting, and recommendation calculation
- **[Table Operations](./table-operations.md)** - Interpolation, smoothing, scaling, and re-binning algorithms
- **[Gauge Rendering](./gauge-rendering.md)** - Canvas-based rendering techniques, color spaces, and visual effects

### Data Formats
- **[INI Parser](./ini-parser.md)** - ECU definition file parsing, expression evaluation, and validation
- **[MSQ File Format](./msq-format.md)** - Tune file structure, XML schema, and serialization
- **[Dashboard Format](./dashboard-format.md)** - TunerStudio-compatible XML format for gauges and layouts

### Communication
- **[ECU Protocol](./ecu-protocol.md)** - Binary and text-based communication protocols, command structure, and error handling
- **[Realtime Streaming](./realtime-streaming.md)** - Event-based data streaming architecture and performance optimization

### Core Systems
- **[Version Control](./version-control.md)** - Git integration, tune fingerprinting, and migration detection
- **[Lua Scripting](./lua-scripting.md)** - Sandboxed runtime, API reference, and security model

## Philosophy

LibreTune's technical implementation follows these principles:

1. **Type Safety** - Extensive use of Rust's type system to prevent errors at compile time
2. **Error Handling** - All operations return `Result<T, E>` for explicit error handling
3. **Performance** - Zero-copy parsing, efficient memory usage, and async I/O
4. **Compatibility** - Support for multiple ECU platforms while maintaining clean abstractions
5. **Testability** - Unit tests for all critical algorithms and data structures

## Source Code Organization

```
crates/
├── libretune-core/          # Pure Rust library (no UI dependencies)
│   ├── src/
│   │   ├── autotune.rs      # AutoTune algorithm implementation
│   │   ├── ini/             # INI parser and data structures
│   │   ├── protocol/        # ECU communication protocols
│   │   ├── table_ops.rs     # Table manipulation algorithms
│   │   ├── dash/            # Dashboard format parser/writer
│   │   ├── project/         # Project and version control
│   │   └── lua/             # Lua scripting runtime
│   └── tests/               # Unit and integration tests
└── libretune-app/           # Tauri desktop application
    ├── src-tauri/           # Rust backend (Tauri commands)
    └── src/                 # React frontend (TypeScript)
```

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| INI parsing | O(n) | Single-pass parser with minimal allocations |
| Table interpolation | O(1) | Bilinear interpolation for 2D tables |
| AutoTune recommendation | O(k) | k = number of data points in buffer |
| Dashboard rendering | O(g) | g = number of gauges, 60 FPS target |
| Git commit | O(n) | n = number of changed pages |

## Thread Safety

LibreTune uses Rust's ownership system and tokio async runtime for safe concurrent operations:

- **ECU Communication** - Single connection per project, guarded by `Arc<Mutex<Connection>>`
- **Realtime Streaming** - Background tokio task emits events to frontend
- **AutoTune State** - Mutex-protected state in AppState struct
- **Settings** - Atomic file writes with temporary files and rename

## See Also

- [CONTRIBUTING.md](../contributing.md) - Developer setup and contribution guidelines
- [INI File Format](../reference/ini-format.md) - User-facing INI format documentation
- [Supported ECUs](../reference/supported-ecus.md) - ECU platform compatibility matrix
