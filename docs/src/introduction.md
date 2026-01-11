# Introduction

Welcome to **LibreTune**, a modern, open-source ECU tuning application for Speeduino, EpicEFI, rusEFI, and other compatible aftermarket engine control units.

![LibreTune Welcome Screen](../screenshots/welcome.png)

## What is LibreTune?

LibreTune is a desktop application that allows you to:

- **Connect** to your ECU via USB serial connection
- **Monitor** real-time engine data with customizable dashboards
- **Edit** fuel, ignition, and other calibration tables
- **AutoTune** your engine using live AFR feedback
- **Save and manage** multiple tune versions with Git-based history
- **Log data** for later analysis and playback

## Key Features

### Modern Interface
LibreTune provides a clean, intuitive interface with dark theme support, keyboard navigation, and multi-monitor support.

### Professional Table Editing
Edit 2D and 3D tables with professional tools:
- Set Equal, Scale, Smooth, Interpolate operations
- Copy/paste with smart cell selection
- Re-bin tables with automatic Z-value interpolation
- 3D visualization for better understanding

### AutoTune Live
Automatically adjust VE and other tables based on wideband O2 sensor feedback:
- Real-time recommendations with authority limits
- Transient filtering to ignore bad data
- Heat map visualization of tuning coverage

### TunerStudio Compatibility
- Import existing TunerStudio projects
- Use standard INI definition files
- Import TunerStudio dashboard layouts

## Supported ECUs

LibreTune works with any ECU that uses the standard INI definition format:

- **Speeduino** - All versions
- **rusEFI** - All board variants
- **EpicEFI** - All versions
- **MegaSquirt** - MS2, MS3 (compatibility mode)

## Getting Started

Ready to start tuning? Head to the [Installation](./getting-started/installation.md) guide to get LibreTune set up on your system.

## Getting Help

- **In-app Help**: Press `F1` or click the `?` button on any dialog
- **GitHub Issues**: [Report bugs or request features](https://github.com/RallyPat/LibreTune/issues)
- **Community**: Join discussions in the GitHub repository

## License

LibreTune is open-source software licensed under the [GPL-2.0 License](https://github.com/RallyPat/LibreTune/blob/main/LICENSE).
