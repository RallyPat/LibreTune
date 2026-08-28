# Table Editing

LibreTune provides powerful tools for editing calibration tables.

## Overview

ECU calibration data is stored in tables:
- **1D Tables**: Single row of values (e.g., warmup enrichment by temperature)
- **2D Tables**: Grid of values indexed by two axes (e.g., VE table by RPM and MAP)

## Opening a Table

1. Navigate the menu tree in the sidebar
2. Click on a table name (e.g., **Fuel → VE Table 1**)
3. The table editor opens in a new tab

![Table Editor](../../screenshots/table-editor.png)

## Table Editor Interface

The table editor consists of:

| Area | Description |
|------|-------------|
| **Toolbar** | Editing operations and view options |
| **Axis Labels** | RPM (columns) and Load/MAP (rows) |
| **Cell Grid** | Editable values |
| **3D View** | Optional 3D visualization |

## Selecting Cells

| Action | Result |
|--------|--------|
| Click | Select single cell |
| Shift+Click | Extend selection |
| Ctrl+Click | Toggle cell in selection |
| Click+Drag | Select rectangular region |
| Ctrl+A | Select all cells |

## Editing Values

### Direct Entry
1. Select a cell
2. Type a new value
3. Press Enter to confirm

### Increment/Decrement
- Press `+` or `>` to increase by step value
- Press `-` or `<` to decrease by step value
- Hold Shift for larger steps (10x)

### Bulk Operations

See [Keyboard Shortcuts](./table-editing/shortcuts.md) for the full list.

| Shortcut | Operation |
|----------|-----------|
| `=` | Set selected cells to their average |
| `*` | Scale selected cells by a factor |
| `/` | Interpolate between corner cells |
| `S` | Smooth selected cells |

## Toolbar Operations

### Set Equal (`=`)
Sets all selected cells to their average value.

### Scale (`*`)
Multiplies selected cells by a factor:
- Enter `1.1` to increase by 10%
- Enter `0.9` to decrease by 10%

### Smooth (`S`)
Applies Gaussian smoothing to reduce abrupt transitions. Higher factors blend values more.

### Interpolate (`/`)
Creates a smooth gradient between corner values:
1. Select a rectangular region
2. Press `/`
3. Values are interpolated bilinearly

### Re-bin
Changes the axis values and interpolates Z values automatically. Useful for adjusting RPM or load breakpoints.

## Copy and Paste

- `Ctrl+C` - Copy selected cells
- `Ctrl+V` - Paste values
- `Ctrl+Shift+V` - Paste with options (add, multiply, etc.)

## Table File Import/Export (.table)

Individual tables can be saved to and loaded from TunerStudio-compatible
`.table` files — handy for sharing a single table (a VE table, an ignition
map, injector dead-times) between projects or with other tuners, without
touching the rest of the tune.

- **Save Table to File** — the download icon in the table toolbar writes the
  table's current X/Y axis bins and value grid to a `.table` file.
- **Load Table from File** — the upload icon reads a `.table` file back into
  the open table, replacing its axis bins and values.

Notes:

- `.table` files interoperate with TunerStudio's "Save Table to File" /
  "Load Table from File".
- The file's dimensions must match the table's **current** size exactly.
  Unlike TunerStudio, LibreTune does not resample a mismatched grid onto the
  table's axes — resize (re-bin) the table first if the sizes differ, so a
  file can never silently apply at the wrong scale.
- Importing overwrites the table's axis bins and values; the rest of the tune
  is untouched. Burn to ECU when you're happy with the result.

## Undo/Redo

- `Ctrl+Z` - Undo last change
- `Ctrl+Y` or `Ctrl+Shift+Z` - Redo

## Follow Mode

Enable **Follow Mode** to automatically highlight the cell corresponding to current engine operation:
1. Connect to ECU
2. Click the crosshair icon in the toolbar
3. The current operating cell is highlighted

On top of the highlight, a **trace line** is drawn through the cells visited
recently, ending in a bright dot on the current cell — an at-a-glance answer
to "where is the engine operating right now?". It is drawn in both the
standalone table view and tables embedded in dialogs.

- Older segments fade out over time; the fade duration follows the
  *Trail fade* setting (`table_trail_fade_sec`, 0 = never fade)
- The position comes from the table's own axis channels; if the INI declares
  none, the standard `rpm` / `map` channels are used
- Toggle with `F`; trail and cursor colors follow the *Cursor color* /
  *Trail color* settings

## Editing Curves

1D curves (for example, warmup enrichment or fan PWM vs. temperature) open in
the curve editor, which shows the values as a draggable line chart alongside a
data table.

- **Drag to edit** — Click anywhere on the chart to grab the nearest point and
  drag it up or down. You can also grab a point marker directly. The drag keeps
  tracking even if the cursor leaves the chart area and commits when you release.
- **Type exact values** — Double-click any cell in the data table to enter a
  precise number.
- **Undo/Redo** — `Ctrl+Z` / `Ctrl+Y` work the same as in table editing.

## Next Steps

- [2D Tables](./table-editing/2d-tables.md) - Detailed 2D table editing
- [3D Visualization](./table-editing/3d-visualization.md) - Using the 3D view
- [Keyboard Shortcuts](./table-editing/shortcuts.md) - Complete shortcut reference
