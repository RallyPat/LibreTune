# Customizing Gauges

Configure individual gauge appearance and behavior.

## Accessing Gauge Settings

1. Enter Designer Mode (right-click → Designer Mode)
2. Right-click the gauge
3. Select **Configure Gauge**

## Gauge Properties

### Data Channel
Select which ECU value to display:
- RPM, MAP, TPS, AFR, etc.
- Any channel defined in your INI

### Range Settings

| Property | Description |
|----------|-------------|
| Minimum | Lowest displayable value |
| Maximum | Highest displayable value |
| Low Warning | Threshold for warning zone |
| High Warning | Threshold for warning zone |
| Low Danger | Threshold for danger zone |
| High Danger | Threshold for danger zone |

### Appearance

| Property | Description |
|----------|-------------|
| Gauge Type | Analog, digital, bar, etc. |
| Title | Display name |
| Units | Unit label (RPM, °F, etc.) |
| Decimal Places | Precision of display |

### Colors

| Property | Description |
|----------|-------------|
| Normal Color | Safe range color |
| Warning Color | Warning zone color |
| Danger Color | Danger zone color |
| Background | Gauge background |
| Bezel | Frame color |

## Gauge Types

### Analog Gauge
- Classic round dial with needle
- Best for: RPM, speed, boost
- Shows relative position well

### Digital Readout
- LCD-style numeric display
- Best for: AFR, temperatures, voltages
- Shows precise values

### Bar Gauge
- Horizontal or vertical progress bar
- Best for: Coolant temp, fuel level
- Good for compact layouts

### Sweep Gauge
- Curved arc indicator
- Best for: Stylized displays
- Good visual impact

### Line Graph
- Time-series history
- Best for: Trends, lambda history
- Shows recent changes

## Resizing Gauges

In Designer Mode:
1. Click to select gauge
2. Drag corner handles to resize
3. Maintain aspect ratio with Shift

## Positioning Gauges

In Designer Mode:
1. Click and drag gauge center
2. Snap to grid (optional)
3. Use arrow keys for fine adjustment

## Copying Gauges

1. Right-click gauge
2. Select **Duplicate**
3. New gauge appears offset
4. Modify channel/settings

## Deleting Gauges

1. Select gauge
2. Press Delete key
3. Or right-click → **Remove Gauge**
