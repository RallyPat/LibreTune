# Customizing Gauges

Configure individual gauge appearance and behavior in the WYSIWYG designer.

## Designer Mode

Right-click the dashboard (anywhere — background or gauge) and select **Designer Mode**.

The designer is fully **WYSIWYG**: gauges render live exactly as on the dashboard — same painters, same realtime data — with selection outlines and resize handles overlaid. What you edit is what you get.

Components hidden from the live dashboard via the layer panel appear dimmed in the designer so you can still select and edit them.

## Editing Basics

| Action | How |
|--------|-----|
| Select | Click a component (or pick it in the layer panel) |
| Move | Drag the component |
| Resize | Drag one of the 8 handles on a selected component |
| Delete | Select, then press **Delete** (or **Backspace**) |
| Copy / Paste | **Ctrl+C** / **Ctrl+V** (pasted components get a fresh id and a small offset) |
| Undo / Redo | **Ctrl+Z** / **Ctrl+Shift+Z** (or **Ctrl+Y**) |
| Deselect | **Escape** |
| Save | **Ctrl+S** or the toolbar **Save** button |
| Align to canvas | Toolbar align buttons (left / center / right / top / middle / bottom) |

Edits are held in memory until you save — the dashboard file on disk is only written when you do.

## Grid and Snapping

The toolbar toggles the grid overlay and the snap step (in percent). With snap enabled, moves and resizes round to the grid.

## Adding Components

Two ways, both drop onto the canvas in designer mode:

- **Drag a channel from the sidebar** onto the canvas — creates a gauge bound to that channel, inheriting units, range, and warning thresholds from the INI definition
- **Drag a tile from the Gauge Palette** — creates a gauge of that painter type with no channel assigned yet; bind a channel in the property editor

## Layer Panel

The right rail lists every component top-down (top of list = top of the visual stack):

- Click to select
- **▲ / ▼** reorder the stack
- **● / ○** hide or show a component on the live dashboard (hidden components stay visible, dimmed, in the designer)
- **✕** delete

## Property Editor

With a component selected, the property editor exposes its full configuration:

### Data Channel
Select which ECU value to display — any channel defined in your INI.

### Range Settings

| Property | Description |
|----------|-------------|
| Minimum / Maximum | Display range |
| Low / High Warning | Threshold for the warning zone |
| Low / High Danger | Threshold for the danger zone |

### Appearance

| Property | Description |
|----------|-------------|
| Gauge Type | Any of the 20 painter styles (see the gauge types table on the [Dashboards](../dashboards.md) page) |
| Title | Display name |
| Units | Unit label (°C, kPa, %, …) |
| Decimal Places | Value precision |
| Colors | Value, warning, danger, background, bezel |

### Conditional Visibility

The **Enabled Condition** field accepts an expression (e.g. `rpm > 0` or `hasLambdaSensor`); the component only renders on the live dashboard while the expression holds true.

## Replacing a Gauge from the INI

In designer mode, right-click a gauge and open the **TS/LibreTune Gauges** submenu: it lists the loaded INI's gauge configurations grouped by category. Choosing one keeps the gauge's position and size but rebinds its channel, title, units, and range to the INI definition.

## Embedded Assets

The **Asset Manager** (bottom of the right rail) embeds images and fonts into the dashboard file itself, so custom backgrounds, needle images, and indicator art travel with the file.
