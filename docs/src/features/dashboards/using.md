# Using Dashboards

Detailed guide to viewing and interacting with dashboards.

## Dashboard Layout

Each dashboard consists of:
- **Gauges**: Display numeric values
- **Indicators**: Show on/off states
- **Graphs**: Show value history
- **Background**: Optional image or color

## Viewing Data

### Real-Time Updates
When connected to ECU:
- All gauges update automatically
- Default rate: 10 updates per second
- Adjust in Settings if needed

### Value Display
Each gauge shows:
- Current value
- Unit of measurement
- Color-coded status

### Status Colors
- 🟢 **Green/Normal**: Value in safe range
- 🟡 **Yellow/Warning**: Approaching limits
- 🔴 **Red/Danger**: Outside safe range

## Gauge Interaction

### Hovering
Hover over a gauge to see:
- Full channel name
- Min/max configured values
- Current value with more precision

### Clicking
Click a gauge to:
- Open related table (if applicable)
- Show value details
- Access gauge settings

### Double-Click
Double-click for full-screen gauge view.

## Dashboard Selector

Switch between dashboards:
1. Click the dropdown in the header
2. Select a dashboard name
3. View switches immediately

### Categories
Dashboards are grouped:
- **Built-in**: LibreTune defaults
- **User**: Your custom dashboards
- **Imported**: From TunerStudio

## Full-Screen Mode

1. Double-click the dashboard background
2. Dashboard fills the screen
3. Press Escape or double-click to exit

## Multi-Monitor

Pop out dashboards to separate monitors:
1. Click the pop-out icon (↗️) in the tab bar
2. Dashboard opens in new window
3. Drag to desired monitor
4. Click dock icon (↙️) to return

## Tips

### Reduce Clutter
- Show only gauges you need
- Use larger gauges for critical data
- Group related gauges together

### Optimize for Use Case
- **Tuning**: Include AFR, VE, corrections
- **Racing**: Large RPM, speed, warnings only
- **Diagnosis**: Include sensors, duty cycles
