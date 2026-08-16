# Using Dashboards

Detailed guide to viewing and interacting with dashboards.

## Dashboard Layout

Each dashboard consists of:
- **Gauges**: Display numeric values (dials, bars, readouts, stat tiles)
- **Indicators**: Show on/off states (warning lights)
- **Graphs**: Show value history (sparklines, multi-channel trends)
- **Background**: Optional image, dither pattern, or color

## Viewing Data

### Real-Time Updates
When connected to the ECU:
- Gauges update automatically at up to 20 readings per second
- Each gauge animates smoothly toward the latest sample
- Values are color-coded by your warning/danger thresholds

### Value Display
Each gauge shows:
- Current value
- Unit of measurement
- Color-coded status

### Status Colors
- 🟢 **Green/Normal**: Value in safe range
- 🟡 **Yellow/Warning**: Approaching limits
- 🔴 **Red/Danger**: Outside safe range

## Dashboard Selector

Switch between dashboards:
1. Click **Change ▼** in the header
2. Select a dashboard name
3. The view switches immediately

### Categories
Dashboards are grouped in the selector:
- **LibreTune**: Built-in and user-created `.ltdash.xml` dashboards
- **Legacy (TunerStudio)**: Imported `.dash` files
- **Legacy Gauges**: Imported `.gauge` files

## Gauge Demo

Right-click the dashboard and choose **Gauge Demo** to animate every gauge across its full range with independent motion — useful for verifying layouts and painters without an ECU connected. Choose **Stop Gauge Demo** to return to live (or idle) values.

## Syncing Ranges from the INI

Click **Sync Ranges** in the header to adopt the loaded INI's gauge configurations for matching channels: ranges, warning/danger thresholds, and digit counts update, while your dashboard's own titles and units are preserved. Auto-sync on load can be toggled in Settings.

## Validation

The header's **Validate** button (shown with error/warning counts) opens the validation panel — see [Dashboard Validation](validation.md).

## Multi-Monitor

Pop the dashboard out to a separate monitor:
1. Click the pop-out icon (↗️) in the tab bar
2. The dashboard opens in its own window
3. Drag it to the desired monitor
4. Click the dock icon (↙️) to return

The popped-out window follows dashboard switches made in the main window, and vice versa — both always show the same dashboard.

> **Note**: The main Dashboard tab is protected and cannot be accidentally closed. To reopen it if missing, go to **View → Dashboard**.

## Tips

### Reduce Clutter
- Hide rarely-needed components in the designer's layer panel rather than deleting them
- Use larger gauges for critical data
- Group related gauges together

### Optimize for Use Case
- **Driving**: Basic dashboard — big RPM and AFR at a glance
- **Tuning**: Tuning dashboard — corrections and targets
- **Diagnosis**: Telemetry Live — trends and full sensor coverage
