# LibreTune

[![License: GPL v2](https://img.shields.io/badge/License-GPL_v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)
[![CI](https://github.com/RallyPat/LibreTune/actions/workflows/ci.yml/badge.svg)](https://github.com/RallyPat/LibreTune/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/RallyPat/LibreTune?label=stable)](https://github.com/RallyPat/LibreTune/releases/latest)
[![Nightly](https://img.shields.io/github/v/release/RallyPat/LibreTune?include_prereleases&label=nightly)](https://github.com/RallyPat/LibreTune/releases/tag/nightly)

[![](https://dcbadge.limes.pink/api/server/5X7tFUfqCr)](https://discord.gg/5X7tFUfqCr)


Modern, open-source ECU tuning software for EpicEFI, Speeduino, rusEFI, and other TS format INI compatible aftermarket engine control units.

## Downloads

| Platform | Stable Release | Nightly Build | Notes |
|----------|----------------|---------------|-------|
| **Linux** |  No Stable releases at this time, please use the nightly. | [Nightly](https://github.com/RallyPat/LibreTune/releases/tag/nightly) | AppImage is portable, no install needed |
| **Windows** |  No Stable releases at this time, please use the nightly. | [Portable EXE](https://github.com/RallyPat/LibreTune/releases/tag/nightly) | Nightly is portable, no install needed |
| **macOS** |   No Stable releases at this time, please use the nightly. | [Nightly](https://github.com/RallyPat/LibreTune/releases/tag/nightly) | Separate builds for Apple Silicon and Intel |

> ⚠️ **Nightly builds** are automatically generated from the latest code and may be unstable.

![LibreTune Table Editor](docs/screenshots/table-editor.png)

## Please note that this project is still very early days - these notes are largely agentically written, and may not be 100% accurate at this time.  We welcome public contribution, and want this to be a thriving publically developed effort. 

## ⚠️ Java Plugin System Deprecated

**As of February 4, 2026**, the Java/JVM plugin system (for TunerStudio JAR compatibility) has been deprecated in favor of LibreTune's native WASM plugin architecture. 

- **UI Access Disabled**: The "Plugins..." menu item has been removed
- **Grace Period**: 2-4 releases (~6-12 months) before complete removal
- **Migration Path**: Plugin developers should migrate to WASM (see `DEPRECATION_NOTICE.md`)
- **WASM Plugins**: Native plugin system with sandboxing, permissions, no JRE dependency

For full details, see [DEPRECATION_NOTICE.md](DEPRECATION_NOTICE.md).

## WASM Plugin System

LibreTune's modern **WebAssembly (WASM) plugin system** provides a secure, sandboxed environment for extending functionality:

- **No External Dependencies**: Plugins run in `wasmtime` sandbox, no JRE or runtime required
- **Permission Model**: Plugins declare required permissions (tune access, ECU communication, logging)
- **Stable Host API**: Well-defined Rust interfaces for tune reading/writing, parameter access, ECU commands
- **Plugin Lifecycle**: Plugins load on demand, unload cleanly, no process overhead
- **Developer-Friendly**: Write plugins in Rust, C/C++, or any WASM-compatible language
- **Security by Default**: Sandboxed execution prevents malicious or buggy plugins from crashing the app

See [docs/src/technical/plugin-system.md](docs/src/technical/plugin-system.md) for complete plugin architecture, host functions, and examples.

## Features

### Core Functionality
- **Cross-platform**: Runs on Windows, macOS, and Linux
- **Modern Architecture**: Rust core with native desktop UI via Tauri
- **INI Definition Compatible**: Works with standard ECU INI definition files
- **Real-time Data**: Live sensor display with configurable dashboard gauges
- **Multi-Monitor Support**: Pop-out any tab to its own window with bidirectional sync

### Table Editing
- **2D/3D Table Editors**: Full-featured grid editor with keyboard navigation
- **3D Visualization**: React Three Fiber surface mesh with orbit controls
- **Live Cursor**: Follow mode with inverted triangle indicator and history trail
- **Editing Tools**: Set Equal, Increase/Decrease, Scale, Interpolate, Smooth
- **Re-binning**: Change axis bins with automatic Z-value interpolation
- **Copy/Paste**: Standard clipboard operations for table data
- **Table Comparison**: Side-by-side diff view between tune versions
- **Burn to ECU**: Write changes directly to ECU memory

### Dashboard & Gauges
- **TS-Compatible Dashboards**: Import existing .dash files
- **9 Gauge Types**: Analog dials, bar gauges (horizontal/vertical), digital readouts, sweep gauges, line graphs, histograms, dashed bars
- **Customizable Layout**: Drag-and-drop gauge positioning
- **Designer Mode**: Edit dashboard layouts visually
- **Dashboard Management**: Create, duplicate, rename, delete, export dashboards
- **3 Default Dashboards**: Basic, Racing, and Tuning layouts included

### AutoTune
- **Live Auto-tuning**: Real-time fuel table recommendations based on AFR targets
- **Table Selector**: Choose which table to auto-tune
- **Heat Maps**: Visualize cell weighting and change magnitude
- **Cell Locking**: Lock cells to prevent AutoTune modifications
- **Authority Limits**: Configure maximum adjustment percentages
- **Reference Tables**: Load/save reference CSV files

### Data Logging
- **Configurable Sample Rates**: 1Hz to 100Hz logging
- **Log Playback**: Play/pause, seek slider, variable speed (0.25x-4x)
- **CSV Support**: Load logs from LibreTune or TunerStudio format
- **Channel Selection**: Choose which channels to display

### Diagnostic Tools
- **Tooth Logger**: Crank/cam trigger pattern analysis with RPM detection
- **Composite Logger**: Multi-channel waveform display with sync status
- **CSV Export**: Export diagnostic captures for analysis

### Data Management
- **CSV Export/Import**: Export and import tune data as CSV
- **Reset to Defaults**: Restore all values to INI defaults
- **Restore Points**: Create, load, and manage tune backups
- **TunerStudio Import**: Import existing TunerStudio projects

### Action Scripting
- **Record Actions**: Automatic capture of all table edits and constant adjustments
- **Replay Scripts**: Execute recorded actions on new tunes or ECUs
- **Export/Import**: Share action scripts as JSON for collaboration
- **Conditional Actions**: Execute actions based on constant values
- **Baseline Templates**: Apply proven configurations instantly

### Hardware Configuration
- **Port Editor**: Visual pin assignment for digital outputs, injectors, ignition coils
- **Conflict Detection**: Automatic detection of pin collisions or invalid assignments
- **Per-ECU Configuration**: Save assignments per project with INI compatibility
- **Hardware Pinout**: Grid view of available pins organized by function

### Unit Preferences
- **Temperature**: °C, °F, or Kelvin
- **Pressure**: kPa, PSI, bar, or inHg
- **AFR Display**: AFR or Lambda (with fuel type selection)
- **Speed**: km/h or mph

### Performance Calculator
- **Physics-based HP**: Calculate wheel horsepower from acceleration data
- **Torque Curves**: View estimated torque at different RPMs
- **Acceleration Times**: Estimated 0-60 and quarter mile times
- **Vehicle Specs**: Configure weight, drag coefficient, tire diameter, gear ratios

### Project Management
- **Project-based Workflow**: Organize tunes by vehicle/ECU
- **INI Repository**: Manage ECU definition files with signature matching
- **Online INI Search**: Download INI files from Speeduino and rusEFI GitHub repos
- **Signature Mismatch Detection**: Automatic detection when ECU doesn't match INI

## Screenshots

| Welcome Screen | Settings Dialog | Table Editor |
|:--------------:|:---------------:|:------------:|
| ![Welcome](docs/screenshots/welcome.png) | ![Settings](docs/screenshots/settings-dialog.png) | ![Table Editor](docs/screenshots/table-editor.png) |

## Supported ECUs

### Currently Supported
- **Speeduino** - Full support for INI definition files and serial protocol
- **rusEFI** - Full support for INI definition files and serial protocol
- **EpicEFI** - Full support via standard INI format

### Compatible
- Any ECU using the standard INI definition format (MegaTune/TunerStudio compatible)
- Megasquirt MS2/MS3 (partial support - serial protocol in progress)

**Note**: Trigger pattern support (e.g., "60-2", "36-1", "Nissan QG18") depends on ECU firmware, not LibreTune. See [docs/NISSAN_QG18_TRIGGER_SETUP.md](docs/NISSAN_QG18_TRIGGER_SETUP.md) for details on how trigger patterns work and how to request support for new patterns.

## Quick Start

### Prerequisites

- **Rust 1.75+** - Install via [rustup](https://rustup.rs)
- **Node.js 20+** - For the Tauri frontend

### Build & Run

```bash
# Clone the repository
git clone https://github.com/RallyPat/LibreTune.git
cd LibreTune

# Install frontend dependencies
cd crates/libretune-app
npm install

# Run in development mode
npm run tauri dev
```

### Notes for Windows Users

```bash
# If you are getting a message about link.exe not being found
# you may need to download the Visual Stuido Build Tools 
# until the binaries are ready to be distributed.

# 1. Install Visual Studio Build Tools
# Download from:
# https://visualstudio.microsoft.com/downloads/
#
# Scroll to "Tools for Visual Studio" → "Build Tools for Visual Studio".
# During installation, enable ONLY this workload:
#
#   ✔ Desktop development with C++
#
# This installs:
# - MSVC compiler
# - link.exe
# - Windows 10/11 SDK
# - CMake and Ninja

# 2. Ensure Rust is using the MSVC toolchain
rustup default stable-x86_64-pc-windows-msvc

# 3. Restart your terminal so PATH updates

# 4. Build LibreTune (development mode)
cd crates/libretune-app
npm install
npm run tauri dev
```



### Build for Production

```bash
cd crates/libretune-app
npm run tauri build
```

## Project Structure

```
libretune/
├── crates/
│   ├── libretune-core/    # Core Rust library
│   │   ├── src/
│   │   │   ├── ini/       # INI file parsing
│   │   │   ├── protocol/  # Serial communication
│   │   │   ├── ecu/       # ECU memory model
│   │   │   ├── datalog/   # Data logging
│   │   │   ├── autotune/  # AutoTune algorithms
│   │   │   ├── tune/      # Tune file management
│   │   │   ├── dash/      # Dashboard format parsing
│   │   │   └── project/   # Project & restore points
│   │   └── Cargo.toml
│   └── libretune-app/     # Tauri desktop application
│       ├── src/           # React frontend (TypeScript)
│       │   ├── components/
│       │   │   ├── dashboards/   # Dashboard & gauge rendering
│       │   │   ├── tables/       # 2D/3D table editors
│       │   │   ├── dialogs/      # Modal dialogs
│       │   │   ├── diagnostics/  # Tooth/composite loggers
│       │   │   └── tuner-ui/     # Main UI components
│       │   └── utils/     # Unit conversion, preferences
│       └── src-tauri/     # Tauri backend (Rust)
├── docs/                  # Documentation and screenshots
├── scripts/               # Build and development scripts
└── Cargo.toml             # Workspace root
```

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

### Run Tests

```bash
cargo test --workspace
```

### Run Lints

```bash
cargo clippy --workspace
```

## License

This program is free software; you can redistribute it and/or modify it under
the terms of the GNU General Public License version 2 as published by the Free
Software Foundation.

See [LICENSE](LICENSE) for the full license text.

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting PRs.

## Acknowledgments

LibreTune is an independent open-source project and is not affiliated with EFI Analytics.
