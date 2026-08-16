# Dashboards

Dashboards display real-time engine data with customizable gauges and indicators.

## Overview

LibreTune dashboards are fully customizable layouts of gauges, indicators, and graphs that show live data from your ECU. Live data never flows through the interface layer — each gauge reads the realtime stream directly and animates on its own canvas, so even dense dashboards stay smooth.

## Default Dashboards

LibreTune includes three built-in dashboards, created automatically on first run:

### Basic Dashboard
A widescreen (16:9) driver cluster for everyday monitoring:
- Large RPM tachometer and AFR dial on the flanks
- MAP, coolant, and throttle bar gauges in the center column
- Battery, intake temperature, ignition timing, and injector pulse-width readout cards along the top

### Tuning Dashboard
Calibration-focused layout for tuning sessions:
- Mixed gauge types for fuel and ignition data
- Lambda history graph
- Correction factor readouts
- EGT and duty cycle indicators

### Telemetry Live
A dense, Grafana-style operations view:
- Top strip of key readouts (RPM, MAP, TPS, AFR, lambda, temperatures, speed, battery, duty)
- Left column of physical-sensor stat tiles (pulse width, timing, boost, baro, oil pressure/temperature, EGT, fuel level)
- Four multi-series trend charts (engine dynamics, fuel & AFR, temperatures, pressures) with live legend values under each graph
- A wall of scrolling single-channel sparkline charts

> **Note**: Default dashboards are only written when missing — your edits to them are never overwritten. To restore a default, delete its file from the dashboards folder (or use **Reset to Defaults** in the dashboard selector, which deletes *all* dashboards including custom ones).

## Opening a Dashboard

The Dashboard tab is always present when a project is loaded. If it's ever missing:

1. Go to **View → Dashboard** to reopen it, or
2. The Dashboard tab is automatically created when you open a project

> **Note**: The Dashboard tab is protected from accidental closing. The close button (×) is hidden, and middle-click will not close it.

## Switching Dashboards

1. Click **Change ▼** in the dashboard header
2. Choose from available dashboards, grouped by category (LibreTune defaults, legacy TunerStudio imports, gauge files)
3. The view updates immediately

Your selection is remembered between sessions, and if you pop the dashboard out to its own window, both windows follow each other's dashboard switches.

## Startup Sweep

When a dashboard loads while the engine is not running, all gauges perform a sports-car-style sweep to their maximum and back, then rest at their minimums for a moment before live data eases in.

## Real-Time Data

While connected, gauges update at up to 20 readings per second. Each gauge animates its needle or value smoothly toward the latest sample and colors it by your warning/danger thresholds:

- 🟢 **Normal**: Value in safe range
- 🟡 **Warning**: Approaching limits
- 🔴 **Danger**: Outside safe range

## Gauge Types

LibreTune supports 20 gauge styles:

| Type | Description |
|------|-------------|
| **Analog Gauge** | Classic circular dial with metallic bezel and gradient needle (aliases: Basic/Circle Analog) |
| **Digital Readout** | LCD-style numeric display with metallic frame |
| **Horizontal Bar Gauge** | Horizontal progress bar with rounded corners and gradient fill |
| **Vertical Bar Gauge** | Vertical progress bar with tick marks and 3D effects |
| **Horizontal/Vertical Dashed Bar** | Segmented bars with per-segment zone coloring |
| **Sweep Gauge** | Curved arc indicator with glowing tip and warning zones |
| **Horizontal Line Gauge** | Horizontal line indicator with gradient track |
| **Line Graph** | Time-series history chart with gradient fill |
| **Histogram** | Distribution bar chart centered on current value |
| **Round Gauge** | Circular gauge with 270° arc and tick marks |
| **Round Dashed Gauge** | Circular gauge with segmented arc |
| **Analog Bar / Analog Moving Bar** | Arc-scale bar variants |
| **Fuel Meter** | Specialized fuel level gauge |
| **Tachometer** | RPM-specific gauge with redline zone |
| **Telemetry Stat** | Flat stat tile with colored value and range bar (LibreTune-native) |
| **Multi-Channel Trend** | Multiple channels overlaid on one chart with a live legend (LibreTune-native) |

## Designer Mode

To customize the layout, right-click the dashboard and select **Designer Mode**. The designer is fully WYSIWYG — gauges render live while you edit. See [Customizing Gauges](dashboards/customizing.md) for details.

## Context Menu Options

Right-click any gauge or the background (works in both live and designer modes):

| Option | Description |
|--------|-------------|
| **Reload Default Gauges** | Re-read the dashboard file from disk, discarding unsaved edits |
| **TS/LibreTune Gauges** | Replace the clicked gauge with any gauge configuration from the loaded INI (designer mode) |
| **Background** | Set the cluster background or dither color |
| **Antialiasing** | Toggle gauge antialiasing |
| **Designer Mode** | Enter/exit the editor |
| **Gauge Demo** | Animate all gauges across their ranges (no ECU needed) |

## Dashboard Files

Dashboards are stored as `.ltdash.xml` files in the app data dashboards folder. TunerStudio `.dash` and `.gauge` files can be imported and keep working as-is — unknown properties are preserved losslessly, so files round-trip through LibreTune without losing TunerStudio-specific data.
