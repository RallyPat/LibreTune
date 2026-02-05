# Action Scripting

LibreTune includes an **Action Scripting** system that allows you to record, save, and replay tuning actions. This document covers the action types, scripting API, and common workflows.

## Overview

Action Scripting lets you:

- **Record Actions**: Capture table edits, constant adjustments, and ECU commands
- **Playback Scripts**: Replay saved actions on another tune or ECU
- **Share Workflows**: Export/import action scripts for collaboration
- **Automate Tuning**: Create sequences of actions to apply consistently

This is useful for:
- Applying baseline configurations consistently across multiple tune files
- Documenting tuning changes for review and approval
- Sharing tuning methodologies with team members
- Automating repetitive adjustments

## Action Types

### Table Operations

| Action | Description | Parameters |
|--------|-------------|------------|
| `SetTableCell` | Set a single table cell value | `table: string`, `x: u32`, `y: u32`, `value: f64` |
| `SetTableRange` | Set multiple cells in a range | `table: string`, `x_start: u32`, `y_start: u32`, `x_end: u32`, `y_end: u32`, `value: f64` |
| `SetTableRow` | Set all cells in a row | `table: string`, `y: u32`, `values: Vec<f64>` |
| `SetTableColumn` | Set all cells in a column | `table: string`, `x: u32`, `values: Vec<f64>` |
| `ScaleTableRange` | Multiply range by factor | `table: string`, `x_start: u32`, `y_start: u32`, `x_end: u32`, `y_end: u32`, `factor: f64` |
| `InterpolateCells` | Bilinear interpolation | `table: string`, `x1: u32`, `y1: u32`, `x2: u32`, `y2: u32` |
| `SmoothTableRange` | Gaussian smoothing | `table: string`, `x_start: u32`, `y_start: u32`, `x_end: u32`, `y_end: u32`, `kernel_size: u32` |

### Constant Adjustments

| Action | Description | Parameters |
|--------|-------------|------------|
| `SetConstant` | Set scalar constant | `name: string`, `value: f64` |
| `IncrementConstant` | Add value to constant | `name: string`, `delta: f64` |
| `SetConstantBits` | Set bit flags | `name: string`, `bits: Vec<(u32, bool)>` |
| `SetConstantString` | Set string constant | `name: string`, `value: string` |

### ECU Commands

| Action | Description | Parameters |
|--------|-------------|------------|
| `SendCommand` | Send raw ECU command | `command: string` |
| `WriteToECU` | Burn current tune to ECU | `burn_controller: bool` |
| `ReadFromECU` | Sync ECU data to tune | (no parameters) |

### Meta Actions

| Action | Description | Parameters |
|--------|-------------|------------|
| `Label` | Named checkpoint | `text: string` |
| `Delay` | Wait N milliseconds | `duration_ms: u32` |
| `Conditional` | Execute if condition true | `condition: string`, `actions: Vec<Action>` |

## Recording Actions

### Via Context Menu (Automatic)

All table edits via the toolbar are automatically recorded:
1. Click **Set Equal**, **Scale**, **Smooth**, etc.
2. Action is recorded with source and target cells
3. Continue editing normally
4. Actions accumulate in the **Action History** panel

### Manual Event Insertion

To insert custom actions:
1. Open **Tools** → **Action Manager**
2. Click **Insert Action**
3. Choose action type from dropdown
4. Fill in parameters
5. Click **Add** to insert

## Action Manager

The Action Manager panel (View → Action Manager) shows:

### Action List

- **#**: Action number
- **Type**: SetTableCell, ScaleRange, SetConstant, etc.
- **Target**: Table name or constant name
- **Details**: Parameters (cells, value, factor, etc.)
- **Timestamp**: When action was recorded

### Controls

- **Stop Recording**: Pause capture of new actions
- **Clear All**: Remove all recorded actions (with confirmation)
- **Export**: Save actions to `.actions.json` file
- **Import**: Load actions from file
- **Replay**: Execute all actions in sequence

### Filtering

- **Search**: Filter actions by type or target (e.g., "veTable" shows all VE table actions)
- **Type Filter**: Show only specific action types (tables, constants, ECU, etc.)

## Action Replay

### Sequential Replay

```
[User clicks "Replay All"]
  ├─ Action 1: SetTableCell(veTable1, 20, 10, 45.5)
  ├─ Action 2: SetTable Range(veTable1, 15-25, 8-12, 48.0)
  ├─ Action 3: SetConstant(target_afr, 14.5)
  ├─ Action 4: Delay(500ms)
  ├─ Action 5: ScaleTableRange(ignTable, 0-32, 0-16, 1.05)
  └─ All actions completed
```

### Conditional Replay

Actions can have conditions:

```json
{
  "type": "SetTableRange",
  "table": "boostTable",
  "condition": "constant('hasBoost') == 1",
  "x_range": [0, 32],
  "y_range": [0, 16],
  "value": 5.0
}
```

If the condition is false, the action is skipped.

## Action Script Format

Actions are exported as JSON for version control and sharing:

```json
{
  "version": "1.0",
  "name": "Speeduino NA Baseline",
  "description": "Base fuel and timing tables for naturally aspirated 4-cyl",
  "created": "2026-02-05T12:34:56Z",
  "actions": [
    {
      "id": 1,
      "type": "SetTableRange",
      "table": "veTable1Tbl",
      "x_start": 0,
      "y_start": 0,
      "x_end": 32,
      "y_end": 16,
      "value": 50.0,
      "description": "Base VE table to 50%",
      "timestamp": "2026-02-05T12:34:57Z"
    },
    {
      "id": 2,
      "type": "SetTableRange",
      "table": "ignTable1Tbl",
      "x_start": 0,
      "y_start": 0,
      "x_end": 32,
      "y_end": 16,
      "value": 5.0,
      "description": "Safe advance to 5°",
      "timestamp": "2026-02-05T12:34:58Z"
    },
    {
      "id": 3,
      "type": "SetConstant",
      "constant": "target_afr",
      "value": 14.5,
      "description": "Target AFR for NA engine",
      "timestamp": "2026-02-05T12:34:59Z"
    }
  ]
}
```

## Common Workflows

### Apply Baseline Configuration

1. Record actions (or import from template):
   ```
   SetTableRange(veTable, 0-32, 0-16, 50%)
   SetTableRange(ignTable, 0-32, 0-16, 5°)
   SetConstant(target_afr, 14.5)
   SetConstant(rev_limit, 7000)
   ```

2. Save to `baseline.actions.json`
3. For new tune:
   - Open action manager
   - Import `baseline.actions.json`
   - Click **Replay**
   - Baseline applied instantly

### Progressive Tuning

Record your tuning session:
1. Edit VE table cells
2. Adjust ignition
3. Tweak constants
4. All actions recorded automatically

Export to document your process:
- Share with other tuners
- Refer back to your methodology
- Version control your approach

### Collaboration

1. Tuner A creates `na-baseline.actions.json` with proven setup
2. Tuner B imports into new project
3. Replays to get fast-track baseline
4. Continues manual tuning from baseline

## API Reference

### Action Recorder (Rust)

```rust
pub struct ActionRecorder {
    actions: Vec<Action>,
    recording: bool,
    // ...
}

impl ActionRecorder {
    pub fn new() -> Self { /* ... */ }
    pub fn record(&mut self, action: Action) { /* ... */ }
    pub fn pause(&mut self) { /* ... */ }
    pub fn resume(&mut self) { /* ... */ }
    pub fn clear(&mut self) { /* ... */ }
    pub fn export_json(&self) -> Result<String, Error> { /* ... */ }
    pub fn import_json(&mut self, json: &str) -> Result<(), Error> { /* ... */ }
}
```

### Tauri Commands

| Command | Purpose |
|---------|---------|
| `get_action_history()` | Get list of recorded actions |
| `clear_action_history()` | Clear all recorded actions |
| `export_action_script(filename)` | Export actions to JSON file |
| `import_action_script(filename)` | Import actions from JSON file |
| `replay_actions()` | Execute all actions in sequence |
| `replay_action(index)` | Execute single action by index |

## Limitations

- **No Nested Conditions**: Conditions cannot reference other conditions
- **No Loops**: Actions execute in linear sequence (no `for` loops)
- **No Branching**: Cannot use `if-else` logic (only conditional actions)
- **Sync Only**: Actions sync to local tune cache, not live ECU data
- **No Undo on Replay**: Actions cannot be undone after replay (use undo manually)

## Troubleshooting

### Actions not recording

**Cause**: Recording is paused

**Solution**: Click **Resume** in Action Manager

### Replay fails with "Table not found"

**Cause**: Target table name different in new INI version

**Solution**: 
1. Check table names in new INI
2. Edit `.actions.json` to use correct names
3. Re-import and replay

### Conditional action skipped

**Cause**: Condition expression evaluated to false

**Solution**:
1. Check constant values
2. Verify condition syntax
3. Remove condition to force execution

## See Also

- [Action Management](../features/tools.md#action-manager) - UI guide
- [Table Operations](./table-operations.md) - Cell editing details
- [Contributing](../contributing.md) - How to extend action types
