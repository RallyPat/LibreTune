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

## INI-Driven Availability

Action Scripting is strictly driven by the loaded ECU INI definition:

- **Tables**: Actions only appear and execute for tables defined in `[TableEditor]` or `[CurveEditor]`.
- **Constants**: Actions only apply to constants defined in `[Constants]` (and `[ConstantsExtensions]`).
- **ECU Commands**: Raw command actions are only available if the INI defines `[ControllerCommands]`.

If a tune does not provide a section or entity, related actions are hidden or skipped rather than guessed or synthesized.

## Action Types

The Action Engine supports the following atomic operations. All actions are validated against the currently loaded INI definition before execution.

### Table Operations

| Action | Description | Parameters |
|--------|-------------|------------|
| `TableEdit` | Modify a single cell | `table_name: string`, `x_index: usize`, `y_index: usize`, `new_value: f64`, `old_value: Option<f64>` |
| `BulkOperation` | Apply operation to multiple cells | `operation: string` (set, scale, smooth, offset, interpolate, rebin), `table_name: string`, `cells: Vec<(usize, usize)>`, `parameters: Map<String, f64>` |

### Constant Adjustments

| Action | Description | Parameters |
|--------|-------------|------------|
| `ConstantChange` | Modify a scalar constant | `constant_name: string`, `new_value: f64`, `old_value: Option<f64>` |

### ECU Commands

| Action | Description | Parameters |
|--------|-------------|------------|
| `SendCommand` | Execute an INI-defined controller command | `command: string` |
| `Pause` | Wait for a specified duration | `duration_ms: u32` |
| `ExecuteLuaScript` | Run a custom Lua script snippet | `script: string` |

## Validation Logic

Before an action set is executed, the `validate_action_set` function checks:
1.  **Table Existence**: Ensures `table_name` exists in `[TableEditor]`.
2.  **Constant Existence**: Ensures `constant_name` exists in `[Constants]`.
3.  **Command Validity**: Ensures `command` in `SendCommand` is defined in `[ControllerCommands]` of the INI.
4.  **Value Safety**: Checks for NaN or infinite values.

If any check fails, the execution is aborted to prevent indeterminate ECU states.

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

Conditions only reference constants or channels that exist in the loaded INI. If a referenced item does not exist, the condition evaluates to false.

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
