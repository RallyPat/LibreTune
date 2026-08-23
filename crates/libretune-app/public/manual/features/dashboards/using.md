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

> **Note**: The main Dashboard tab is protected and cannot be popped out or accidentally closed. To reopen it if missing, go to **View → Dashboard**.

## Performance & Smoothing

Dashboards redraw each gauge on a capped, phase-staggered timer instead of
redrawing on every incoming sample. This keeps CPU (and laptop battery) use
predictable even when the ECU streams faster than the display needs.

### Gauge Refresh Rate

**Settings → General → Dashboard → Gauge refresh rate** caps how often each
gauge redraws:

- **10 Hz (battery saver)** — street tuning on battery power
- **15–25 Hz** — balanced
- **30 Hz (default)** — smoothest motion

Redraw timers are staggered so gauges don't all paint in the same frame, and
needle motion uses a time-based EMA (exponential moving average), so motion
looks identical at any refresh rate — only the step cadence changes.

### Transient Spike Highlight

Short pulses that would be smoothed away are not lost: when the raw value
jumps far from the smoothed reading, the value readout briefly flashes in the
critical color, and line graphs draw a colored dot at the raw position. This
makes brief excursions (e.g. a lean spike) visible without making the trace
look like an audio waveform.

### Right-Align Gauge Values

**Settings → General → Dashboard → Right-align gauge values** keeps numeric
readouts steady when the digit count or sign changes (e.g. timing crossing
0°). Digital and bar gauges anchor values to a fixed right edge; dial gauges
pad values to a fixed width so they stay centered but never shift.

## Tips

### Reduce Clutter
- Show only gauges you need
- Use larger gauges for critical data
- Group related gauges together

### Optimize for Use Case
- **Tuning**: Include AFR, VE, corrections
- **Racing**: Large RPM, speed, warnings only
- **Diagnosis**: Include sensors, duty cycles
