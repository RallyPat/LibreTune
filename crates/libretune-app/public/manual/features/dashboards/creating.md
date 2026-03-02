# Creating Dashboards

Build custom dashboard layouts from scratch.

## Starting a New Dashboard

1. Go to **View → Dashboard → New Dashboard**
2. Or right-click dashboard selector → **New**
3. Choose:
   - **Blank**: Start empty
   - **From Template**: Use a preset layout
   - **Copy Current**: Clone existing dashboard

## Designer Mode

Enter Designer Mode to edit layout:
1. Right-click dashboard background
2. Select **Designer Mode**
3. Border indicates edit mode is active

## Adding Gauges

### From Context Menu
1. Right-click empty area
2. Select **Add Gauge**
3. Choose gauge type
4. Select data channel
5. Gauge appears at click location

### Drag and Drop from Sidebar
1. Make sure **Designer Mode** is active
2. In the sidebar channel list, find the channel you want to display
3. **Drag** the channel name onto the dashboard canvas
4. **Drop** it where you want the gauge to appear
5. The gauge is created automatically with the correct channel, units, min/max, and warning zones from the INI definition

Dropped gauges automatically:
- Snap to the dashboard grid (if snap is enabled)
- Inherit INI metadata (units, min, max, warning thresholds)
- Get added to the undo/redo history so you can undo the placement

### From Gauge Browser
1. Open gauge browser (View → Gauge Browser)
2. Drag gauge onto dashboard
3. Drop to place

## Gauge Categories

Gauges are organized by type:
- **Engine**: RPM, MAP, TPS
- **Fuel**: AFR, pulse width, duty
- **Ignition**: Advance, dwell
- **Sensors**: Temperatures, pressures
- **Calculated**: Power, torque, speed

## Layout Tips

### Grid Alignment
- Enable **Snap to Grid** for clean layouts
- Grid size configurable in Settings

### Grouping
Place related gauges together:
- Fuel gauges in one area
- Temperature gauges together
- Diagnostic gauges separated

### Hierarchy
- Largest gauge = most important
- Center = primary focus
- Edges = secondary info

## Setting Background

1. Right-click dashboard
2. Select **Background**
3. Choose:
   - **Solid Color**: Pick a color
   - **Gradient**: Two-color blend
   - **Image**: Load image file

## Saving Dashboards

1. Click **Exit Designer** or press Escape
2. Dashboard saves automatically
3. Or right-click → **Save Dashboard As...**

### Dashboard Files
Dashboards are saved as `.ltdash.xml`:
- In your project folder
- Or global dashboards folder

## Sharing Dashboards

Export dashboards to share:
1. Right-click dashboard selector
2. Select **Export Dashboard**
3. Choose save location

Import shared dashboards:
1. File → Import Dashboard
2. Select `.ltdash.xml` or `.dash` file
3. Dashboard added to selector

## Dashboard Templates

Create templates for reuse:
1. Design your layout
2. Save As → enter template name
3. Check "Save as Template"
4. Available in New Dashboard dialog
