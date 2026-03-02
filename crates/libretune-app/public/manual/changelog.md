# Changelog

All notable changes to LibreTune will be documented in this file.

## [Unreleased]

### Added
- **Base Map Generator** — Create safe, driveable starting tunes from engine specifications (cylinder count, displacement, injector size, fuel type, aspiration, etc.). Generates VE, ignition, AFR tables plus enrichment curves. Accessible from Welcome View and Tools menu.
- **File-centric workflow** — Projects are now created automatically when you open a tune file. The Welcome View shows three main actions: Open Tune File, Connect to ECU, and Import TS Project.
- **Open Tune File dialog** — Browse for .msq/.xml files with automatic INI signature matching, preview panel, and one-click project creation.
- **ECU Definitions management** — New Settings tab for viewing, importing, and deleting INI definition files.
- **Project deletion** — Delete projects directly from the Welcome View's recent projects list (click-to-confirm safety).
- **ECU Console** — Text-based command interface for rusEFI, FOME, and epicEFI ECUs. Accessible from Tools → ECU Console when connected to a supported ECU. Includes command history navigation, color-coded output, and FOME fast comms optimization.
- **Pop-out Windows** — Pop any tab out to its own standalone window for multi-monitor setups. Bidirectional sync keeps realtime data, table edits, and connection status synchronized across all windows.
- **Tune Migration** — Automatic INI version tracking and migration report when opening tunes saved with a different firmware version. Shows severity-coded change categories (type changes, scale changes, added/removed constants).
- **Drag-and-drop gauge creation** — Drag channels from the sidebar onto the dashboard in Designer Mode to create gauges with auto-populated INI metadata (units, min/max, warnings).
- **Smart Recording** — Automatic data log recording triggered by configurable conditions (RPM threshold, TPS activation, etc.).
- **Data Statistics** — Statistical analysis of logged data with min/max/average/standard deviation per channel.
- **Alert Rules** — Configurable alert conditions that trigger notifications or actions when channel values exceed thresholds.
- **Lua Scripting** — Embedded Lua scripting engine for custom calculations, automated testing sequences, and advanced data processing.
- **Math Channels** — Expression engine for creating calculated channels from ECU data (e.g., AFR error, VE efficiency, boost delta).
- **Performance Calculator** — Physics-based horsepower and torque estimation from vehicle specs and acceleration data.
- **Diagnostic Loggers** — Tooth logger for crank/cam trigger pattern analysis and composite logger for multi-channel waveform visualization with sync status detection.
- **Hardware Configuration** — Visual port/pin editor for assigning digital outputs, injectors, and ignition coils with conflict detection.
- **Action Scripting** — Record, replay, and share tuning action sequences as JSON scripts. Supports conditional execution and baseline templates.
- **Change Annotations** — Annotate individual tune changes with notes explaining the purpose of each modification.
- **WASM Plugin System** — Secure, sandboxed plugin architecture using WebAssembly. Plugins declare permissions and run without external dependencies.
- **Dashboard Validation** — Automated validation of dashboard configurations against INI channel definitions.
- Git-based tune versioning with commit history, branches, and auto-commit settings
- Comprehensive documentation system (API docs + user manual)
- TuneHistoryPanel for viewing and managing tune history
- Version Control settings in Settings dialog
- Auto-sync & reconnect after controller commands (Settings option)
- Dynamic window title showing current project name
- Project name displayed in sidebar header
- Build number display in About dialog (YYYY.MM.DD+g<sha> format)
- FOME fast comms with intelligent fallback for optimized console communication
- Dashboard tab protection (cannot be accidentally closed; recoverable via View → Dashboard)
- Configurable status bar channels (Settings → Status Bar Channels, max 8)
- INI signature mismatch dialog with local and online INI search
- Online INI repository search from Speeduino and rusEFI GitHub repos
- Resilient ECU sync with partial failure handling and status bar indicator
- CSV export/import for tune data
- Reset tune to defaults
- Runtime Packet Mode setting (Auto / Force Burst / Force OCH / Disabled)

### Changed
- Replaced template-based "New Project" flow with file-centric "Open Tune File" workflow
- Welcome screen redesigned with three primary actions and recent projects list
- File menu: "New Project" renamed to "Open Tune File" (Ctrl+N); "Open Project" removed
- INI file import moved from File menu to Settings → ECU Definitions tab
- Improved AutoTune with transient filtering, lambda delay compensation, and authority limits enforcement
- Enhanced gauge rendering with metallic bezels, 3D effects, and gradient fills across all 13 gauge types
- Table editor uses flat CSS grid for pixel-perfect axis alignment
- Realtime data streaming switched from polling to event-based architecture (50ms intervals)
- Connection lock diagnostics with lock-holder tracking for debugging

### Removed
- Built-in project templates (Speeduino, rusEFI, epicEFI) — replaced by Base Map Generator
- "New Project" and "Open Project" dialogs — replaced by Open Tune File and Welcome View
- Java/JVM plugin system UI (deprecated; see DEPRECATION_NOTICE.md)

### Fixed
- Table operations (scale, smooth, interpolate, set equal, rebin) now properly connected to backend
- AutoTune table lookup for rusEFI/epicEFI INIs (veTable1Map → veTable1Tbl resolution)
- Realtime stream lock contention from get_all_constant_values blocking serial connection
- Dashboard tab can no longer be accidentally closed by middle-click or pop-out
- Table axis label alignment in embedded table editor (flat grid approach)
- lastOffset keyword handling in INI constant parsing
- std_separator showing as menu item instead of divider
- PcVariables not available to dialogs (now parsed as full Constants)
- Smooth table weight array indexing bug

## [0.1.0] - 2026-01-01

### Added
- Initial release
- ECU connection via serial port
- INI definition file parsing
- Table editing (2D/3D)
- Real-time dashboard with customizable gauges
- AutoTune with AFR-based VE correction
- Data logging and playback
- TunerStudio project import
- Restore points system
- Online INI repository search
- Multi-monitor pop-out windows
- Unit conversion (temperature, pressure, AFR/Lambda)

### Supported ECUs
- Speeduino
- rusEFI
- epicEFI
- MegaSquirt MS2/MS3 (compatibility mode)
