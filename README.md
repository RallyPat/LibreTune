# LibreTune

[![License: GPL v2](https://img.shields.io/badge/License-GPL_v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)
[![CI](https://github.com/RallyPat/LibreTune/actions/workflows/ci.yml/badge.svg)](https://github.com/RallyPat/LibreTune/actions/workflows/ci.yml)

Modern, open-source ECU tuning software for EpicEFI, Speeduino, rusEFI, and other TS format INI compatible aftermarket engine control units.

![LibreTune Table Editor](docs/screenshots/table-editor.png)

## Please note that this project is still very early days - these notes are largely agentically written, and may not be 100% accurate at this time.  We welcome public contribution, and want this to be a thriving publically developed effort. 

## Features

### Core Functionality
- **Cross-platform**: Runs on Windows, macOS, and Linux
- **Modern Architecture**: Rust core with native desktop UI via Tauri
- **INI Definition Compatible**: Works with standard ECU INI definition files
- **Real-time Data**: Live sensor display with configurable dashboard gauges

### Table Editing
- **2D/3D Table Editors**: Full-featured grid editor with keyboard navigation
- **Editing Tools**: Set Equal, Increase/Decrease, Scale, Interpolate, Smooth
- **Re-binning**: Change axis bins with automatic Z-value interpolation
- **Copy/Paste**: Standard clipboard operations for table data
- **Burn to ECU**: Write changes directly to ECU memory

### Dashboard & Gauges
- **TS-Compatible Dashboards**: Import existing .dash files
- **Multiple Gauge Types**: Analog dials, bar gauges, digital readouts, sweep gauges
- **Customizable Layout**: Drag-and-drop gauge positioning
- **Designer Mode**: Edit dashboard layouts visually

### AutoTune
- **Live Auto-tuning**: Real-time fuel table recommendations based on AFR targets
- **Heat Maps**: Visualize cell weighting and change magnitude
- **Cell Locking**: Lock cells to prevent AutoTune modifications
- **Authority Limits**: Configure maximum adjustment percentages

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
- **Speeduino** - Full support for INI definition files
- **rusEFI** - Full support for INI definition files

### Planned
- Megasquirt MS1/MS2
- Megasquirt MS3
- Other INI-compatible ECUs

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
│   │   │   └── tune/      # Tune file management
│   │   └── Cargo.toml
│   └── libretune-app/     # Tauri desktop application
│       ├── src/           # React frontend (TypeScript)
│       └── src-tauri/     # Tauri backend (Rust)
├── docs/                  # Documentation and screenshots
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
