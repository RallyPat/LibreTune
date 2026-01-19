# Changelog

All notable changes to LibreTune will be documented in this file.

## [Unreleased]

### Added
- Git-based tune versioning with commit history and branches
- Built-in project templates (Speeduino, rusEFI, epicEFI)
- Comprehensive documentation system (API docs + user manual)
- TuneHistoryPanel for viewing and managing tune history
- Version Control settings in Settings dialog
- Auto-sync & reconnect after controller commands (Settings option) and dev-only telemetry for reconnect events

### Changed
- Improved AutoTune with transient filtering and lambda delay compensation
- Enhanced gauge rendering with metallic bezels and 3D effects

### Fixed
- Table operations (scale, smooth, interpolate) now properly connected to backend
- AutoTune Live table lookup for rusEFI/epicEFI INIs

## [0.1.0] - 2026-01-01

### Added
- Initial release
- ECU connection via serial port
- INI definition file parsing
- Table editing (2D/3D)
- Real-time dashboard with customizable gauges
- AutoTune Live with AFR-based VE correction
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
