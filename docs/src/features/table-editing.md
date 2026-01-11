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

## Undo/Redo

- `Ctrl+Z` - Undo last change
- `Ctrl+Y` or `Ctrl+Shift+Z` - Redo

## Follow Mode

Enable **Follow Mode** to automatically highlight the cell corresponding to current engine operation:
1. Connect to ECU
2. Click the crosshair icon in the toolbar
3. The current operating cell is highlighted

## Next Steps

- [2D Tables](./table-editing/2d-tables.md) - Detailed 2D table editing
- [3D Visualization](./table-editing/3d-visualization.md) - Using the 3D view
- [Keyboard Shortcuts](./table-editing/shortcuts.md) - Complete shortcut reference
