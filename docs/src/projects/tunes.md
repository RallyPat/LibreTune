# Managing Tunes

Save, load, and organize your tune files.

## Tune File Format

LibreTune uses the standard MSQ format:
- XML-based tune storage
- Compatible with TunerStudio
- Human-readable with any text editor

## Saving Tunes

### Quick Save
- Press `Ctrl+S` or click **Save**
- Overwrites current tune file
- Creates restore point (if enabled)

### Save As
- **File → Save Tune As...**
- Choose new filename
- Keeps original unchanged

### Auto-Save
Configure in Settings:
- **Interval**: How often to auto-save
- **Location**: Where to save backups
- **Keep**: How many backups to retain

## Loading Tunes

### From Project
1. **File → Load Tune**
2. Select MSQ file
3. Values loaded into current project

### From Another Project
1. Browse to any project folder
2. Select the tune file
3. Values merged with current INI

## Burn to ECU

Write tune to ECU's permanent flash:

1. Connect to ECU
2. Make your changes
3. **File → Burn to ECU** or `Ctrl+B`
4. Confirm the burn
5. Wait for completion

### When to Burn

**Do burn when:**
- Changes are tested and verified
- You want changes to survive power cycle
- Switching to street driving

**Don't burn when:**
- Still testing changes
- Experimenting with values
- Not yet validated

### Burn Safety

LibreTune warns before burning:
- Shows changed values count
- Asks for confirmation
- Cannot be undone (use restore points!)

## Comparing Tunes

Compare two tune files:
1. **Tools → Compare Tunes**
2. Select first tune (or use current)
3. Select second tune
4. Differences shown in grid

### Difference View
- **Green**: Values in second tune only
- **Red**: Values in first tune only
- **Yellow**: Different values

## Resetting to Defaults

Reset all values to INI defaults:
1. **Tools → Reset to Defaults**
2. Confirm the reset
3. All constants reset to default values

**Warning**: This cannot be undone! Create a restore point first.

## Exporting/Importing

### Export as CSV
1. **File → Export Tune as CSV**
2. All constants exported with metadata
3. Useful for analysis or backup

### Import from CSV
1. **File → Import Tune from CSV**
2. Values validated against INI
3. Applied to current tune
