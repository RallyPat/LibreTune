# Dashboards

Dashboards display real-time engine data with customizable gauges and indicators.

## Overview

LibreTune dashboards are fully customizable layouts of gauges, indicators, and graphs that show live data from your ECU.

## Default Dashboards

LibreTune includes three professionally designed dashboards:

### Basic Dashboard
Essential monitoring gauges:
- Large analog RPM gauge
- Digital AFR readout
- Coolant and IAT temperature bars
- MAP pressure bar
- Battery, advance, VE, and pulse width readouts

### Racing Dashboard
Track-focused layout:
- Giant center RPM gauge
- Oil pressure and water temp bars
- Speed, AFR, boost, and fuel readouts

### Tuning Dashboard
Calibration-focused layout:
- Mixed gauge types for different data
- Lambda history graph
- Correction factor readouts
- EGT and duty cycle indicators

## Opening a Dashboard

The Dashboard tab is always present when a project is loaded. If it's ever missing:

1. Go to **View → Dashboard** to reopen it, or
2. The Dashboard tab is automatically created when you open a project

> **Note**: The Dashboard tab is protected from accidental closing. The close button (×) is hidden, and middle-click will not close it.

## Switching Dashboards

1. Click the dashboard selector (dropdown in header)
2. Choose from available dashboards
3. The view updates immediately

## Real-Time Data

When connected to an ECU:
- Gauges update automatically (10Hz default)
- Values are color-coded by status (normal/warning/danger)
- Follow mode highlights current operating conditions

## Gauge Types

LibreTune supports all 13 standard gauge styles:

| Type | Description |
|------|-------------|
| **Analog Gauge** | Classic circular dial with metallic bezel and gradient needle |
| **Digital Readout** | LCD-style numeric display with metallic frame |
| **Horizontal Bar Gauge** | Horizontal progress bar with rounded corners and gradient fill |
| **Vertical Bar Gauge** | Vertical progress bar with tick marks and 3D effects |
| **Sweep Gauge** | Curved arc indicator with glowing tip and warning zones |
| **Horizontal Line Gauge** | Horizontal line indicator with gradient track |
| **Vertical Dashed Bar** | Segmented vertical bar with per-segment zone coloring |
| **Line Graph** | Time-series history chart with gradient fill |
| **Histogram** | Distribution bar chart centered on current value |
| **Round Gauge** | Circular gauge with 270° arc and tick marks |
| **Round Dashed Gauge** | Circular gauge with segmented arc |
| **Fuel Meter** | Specialized fuel level gauge |
| **Tachometer** | RPM-specific gauge with redline zone |

## Gauge Zones

Gauges can display warning zones:
- 🟢 **Green**: Normal operating range
- 🟡 **Yellow/Orange**: Warning zone
- 🔴 **Red**: Danger zone

## Designer Mode

To customize dashboard layout:
1. Right-click the dashboard background
2. Select **Designer Mode**
3. Drag gauges to reposition
4. Resize by dragging edges
5. Right-click gauges for options
6. Click **Exit Designer** when done

## Context Menu Options

Right-click any gauge or the background for options:

| Option | Description |
|--------|-------------|
| **Reload Default Gauges** | Reset to default configuration |
| **LibreTune Gauges** | Add gauges from INI definition |
| **Reset Value** | Clear displayed value |
| **Background** | Change background color/image |
| **Antialiasing** | Toggle smooth rendering |
| **Designer Mode** | Enable/disable layout editing |
| **Gauge Demo** | Animate gauges with simulated data |

## Importing Dashboards

LibreTune can import TunerStudio dashboard layouts:
1. Go to **File → Import Dashboard**
2. Select a `.dash` file
3. The layout is converted and displayed

## Creating Custom Dashboards

1. Start from a template or blank dashboard
2. Enter Designer Mode
3. Right-click → **Add Gauge**
4. Select the data channel
5. Configure gauge properties
6. Save the dashboard

## Next Steps

- [Using Dashboards](./dashboards/using.md) - Detailed usage guide
- [Customizing Gauges](./dashboards/customizing.md) - Gauge configuration
- [Creating Dashboards](./dashboards/creating.md) - Build custom layouts
