# 2D Tables

Detailed guide to editing 2D calibration tables.

## Understanding 2D Tables

A 2D table maps two input variables to an output value:
- **X-axis (columns)**: Usually RPM
- **Y-axis (rows)**: Usually load (MAP, TPS, etc.)
- **Z-values (cells)**: The calibrated output

## Cell Selection

### Mouse Selection
- **Click**: Select single cell
- **Click + Drag**: Select rectangular region
- **Ctrl + Click**: Add/remove cell from selection
- **Shift + Click**: Extend selection to clicked cell

### Keyboard Selection
- **Arrow keys**: Move selection
- **Shift + Arrow**: Extend selection
- **Ctrl + A**: Select all cells

## Editing Cells

### Single Cell Edit
1. Select a cell
2. Type a number
3. Press Enter

### Increment/Decrement
- `+` or `>`: Increase by step
- `-` or `<`: Decrease by step
- Hold **Shift** for 10x step

### Bulk Operations

| Key | Operation | Description |
|-----|-----------|-------------|
| `=` | Set Equal | Average of selected cells |
| `*` | Scale | Multiply by factor |
| `/` | Interpolate | Bilinear interpolation |
| `S` | Smooth | Gaussian blur |

## Copy and Paste

### Basic Copy/Paste
1. Select cells
2. `Ctrl+C` to copy
3. Select destination
4. `Ctrl+V` to paste

### Paste Special
`Ctrl+Shift+V` opens paste options:
- **Replace**: Normal paste
- **Add**: Add pasted values to existing
- **Multiply**: Multiply by pasted values
- **Average**: Average with existing values

## Cell Colors

Cells are color-coded by value:
- **Dark blue**: Low values
- **Green/Yellow**: Mid values
- **Red**: High values

The color scale is based on the table's min/max range.

## Follow Mode

When connected to ECU:
1. Click the crosshair icon
2. Current operating cell is highlighted
3. Values update in real-time

## Tips

### Smoothing Strategy
1. Get the general shape correct first
2. Use Smooth on rough transitions
3. Fine-tune individual cells

### Scaling Strategy
1. Select entire table or region
2. Scale by percentage (e.g., 1.05 for +5%)
3. Useful for global adjustments

### Interpolation Strategy
1. Set corner/edge cells to known-good values
2. Select the region between
3. Interpolate to fill in
