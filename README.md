# LibreTune

[![License: GPL v2](https://img.shields.io/badge/License-GPL_v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)
[![CI](https://github.com/RallyPat/LibreTune/actions/workflows/ci.yml/badge.svg)](https://github.com/RallyPat/LibreTune/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/RallyPat/LibreTune?label=stable)](https://github.com/RallyPat/LibreTune/releases/latest)
[![Nightly](https://img.shields.io/github/v/release/RallyPat/LibreTune?include_prereleases&label=nightly)](https://github.com/RallyPat/LibreTune/releases/tag/nightly)

[![](https://dcbadge.limes.pink/api/server/5X7tFUfqCr)](https://discord.gg/5X7tFUfqCr)

Modern, open-source ECU tuning software for Speeduino, rusEFI, EpicEFI, and other INI-compatible aftermarket engine control units. Built with a Rust core and a native desktop UI via Tauri.

## Please note that this project is still very early days - these notes are largely agentically written, and may not be 100% accurate at this time.  We welcome public contribution, and want this to be a thriving publically developed effort. 

## Downloads

| Platform | Stable Release | Nightly Build | Notes |
|----------|----------------|---------------|-------|
| **Linux** |  No Stable releases at this time, please use the nightly. | [Nightly](https://github.com/RallyPat/LibreTune/releases/tag/nightly) | AppImage is portable, no install needed |
| **Windows** |  No Stable releases at this time, please use the nightly. | [Portable EXE](https://github.com/RallyPat/LibreTune/releases/tag/nightly) | Nightly is portable, no install needed |
| **macOS** |   No Stable releases at this time, please use the nightly. | [Nightly](https://github.com/RallyPat/LibreTune/releases/tag/nightly) | Separate builds for Apple Silicon and Intel |

> ⚠️ **Nightly builds** are automatically generated from the latest code and may be unstable.

![LibreTune Table Editor](docs/screenshots/table-editor.png)

## Features

### Table Editing
Full-featured 2D grid editor with 3D surface visualization, keyboard navigation, and live ECU cursor tracking. Includes Set Equal, Scale, Interpolate, Smooth, Re-bin, Copy/Paste, table comparison, and direct Burn to ECU.

### Dashboards & Gauges
Import existing .dash files or build layouts from scratch with 13 gauge types (analog dials, bar gauges, sweep gauges, digital readouts, line graphs, histograms, tachometers, and more). Drag-and-drop designer mode with three default dashboards included.

### AutoTune
Live fuel table recommendations based on AFR targets with heat map visualization, cell locking, authority limits, transient filtering, and lambda delay compensation.

### Diagnostics & Logging
Data logger with configurable sample rates, playback controls, CSV import/export, math channels, and alert rules. Tooth logger and composite logger for trigger pattern analysis. Text-based ECU console for rusEFI/FOME/epicEFI.

### Data Management
Project-based workflow with restore points, Git-based tune versioning, CSV export/import, TunerStudio project import, INI version tracking with automatic tune migration, and change annotations. Online INI search from Speeduino and rusEFI GitHub repos.

### Additional
- **Multi-monitor**: Pop out any tab to its own window with bidirectional sync
- **Unit preferences**: Temperature (°C/°F/K), pressure (kPa/PSI/bar/inHg), AFR/Lambda
- **Performance calculator**: Estimated HP/torque curves and acceleration times
- **Action scripting**: Record and replay tuning actions across tunes
- **Extensible**: WASM plugin system with sandboxing and permission model

For full documentation, see the [User Manual](https://rallypat.github.io/LibreTune/).

## Screenshots

| Welcome Screen | Settings Dialog | Table Editor |
|:--------------:|:---------------:|:------------:|
| ![Welcome](docs/screenshots/welcome.png) | ![Settings](docs/screenshots/settings-dialog.png) | ![Table Editor](docs/screenshots/table-editor.png) |

## Supported ECUs

| ECU | INI Support | Serial Protocol |
|-----|:-----------:|:---------------:|
| **Speeduino** | ✅ | ✅ |
| **rusEFI** | ✅ | ✅ |
| **FOME** | ✅ | ✅ |
| **EpicEFI** | ✅ | ✅ |
| **MegaSquirt MS2/MS3** | ✅ | Partial |

Any ECU using the standard INI definition format should be compatible.

## Quick Start

### Prerequisites

- **Rust 1.75+** — Install via [rustup](https://rustup.rs)
- **Node.js 20+** — For the Tauri frontend

### Build & Run

```bash
git clone https://github.com/RallyPat/LibreTune.git
cd LibreTune

cd crates/libretune-app
npm install
npm run tauri dev
```

<details>
<summary><strong>Notes for Windows Users</strong></summary>

If `link.exe` is not found, install the Visual Studio Build Tools:

1. Download from [visualstudio.microsoft.com/downloads](https://visualstudio.microsoft.com/downloads/) → "Tools for Visual Studio" → "Build Tools for Visual Studio"
2. During installation, enable **Desktop development with C++**
3. Set the Rust toolchain: `rustup default stable-x86_64-pc-windows-msvc`
4. Restart your terminal, then build as above

</details>

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
│   │   └── src/
│   │       ├── ini/       # INI file parsing
│   │       ├── protocol/  # Serial communication
│   │       ├── ecu/       # ECU memory model
│   │       ├── datalog/   # Data logging
│   │       ├── autotune/  # AutoTune algorithms
│   │       ├── tune/      # Tune file management
│   │       ├── dash/      # Dashboard format parsing
│   │       └── project/   # Project & restore points
│   └── libretune-app/     # Tauri desktop application
│       ├── src/           # React frontend (TypeScript)
│       └── src-tauri/     # Tauri backend (Rust)
├── docs/                  # User manual (mdBook)
├── scripts/               # Build and development scripts
└── Cargo.toml             # Workspace root
```

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

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
