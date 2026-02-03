# Tools

Advanced features for comparing tunes, managing actions, and tuning workflows.

## Table Comparison

Compare tables side-by-side to see differences between tune versions.

### Overview

Table Comparison helps you:
- See exact cell-by-cell differences
- Identify which cells changed between versions
- Understand tune progression through iterations
- Measure correction magnitude
- Export comparison reports

### Opening Table Comparison

1. **Go to** Tools → Table Comparison
2. **Select tables** to compare
3. **Choose comparison type** (values, percentages, or differences)
4. **View results**

### Comparison Types

#### Cell Values
Shows actual numerical values in each table:
```
         Baseline    Tuned      Difference
RPM 3000  
Load 50%    75.2       82.1        +6.9
```

#### Percentages
Shows percentage change from baseline:
```
         Baseline    Tuned      % Change
RPM 3000  
Load 50%    75.2       82.1       +9.2%
```

#### Heatmap
Color-coded visualization:
- 🟦 Blue = Richer (added fuel)
- 🟥 Red = Leaner (removed fuel)
- ⬜ Gray = Unchanged

### Workflow

**Step 1: Load Baseline Tune**
- Click "Load Baseline"
- Select first tune file
- Choose table to compare

**Step 2: Load Tuned Version**
- Click "Load Comparison"
- Select second tune file
- Same table selected

**Step 3: Review Results**
- Scan for large differences
- Identify correction patterns
- Check consistency

**Step 4: Export**
- Click "Export Comparison"
- Choose format (PDF, Excel, CSV)
- File saved with timestamp

### Use Cases

#### Verifying AutoTune Results
```
Load tune from BEFORE AutoTune
Load same tune AFTER AutoTune session
Compare to see recommendations that were applied
Verify changes make sense
```

#### Documenting Tune Progression
```
Baseline tune (stock)
    ↓ compare
Version 1 (first session)
    ↓ compare
Version 2 (second session)
    ↓ compare
Final tune
```

#### Identifying Problem Areas
```
Load reference tune (from dyno)
Load your tune
Compare to find deviations
Investigate large differences
```

#### Before/After Validation
```
Before modification:  Stock tune
After modification:   Tuned version
Compare to prove changes
Document results
```

### Example Output

```
=== VE TABLE COMPARISON ===
Tune A: baseline-stock.msq
Tune B: final-tuned.msq

Total cells: 256
Cells changed: 98 (38%)
Max increase: +12.4 (RPM 3000, Load 80%)
Max decrease: -3.1 (RPM 2000, Load 10%)
Average change: +2.1

Changed Regions:
  ✓ Idle to 2000 RPM: +4% average (richer for stability)
  ✓ 2000-4000 RPM cruise: +1% average (light optimization)
  ✓ 4000+ RPM WOT: +8% average (aggressive lean-out)

Unchanged Regions:
  ✓ Cold start cell (locked during tuning)
  ✓ High-boost cells (limit set too low for testing)
```

---

## Action Manager

Record and replay tuning actions for documentation and reproducibility.

### Overview

Action Manager captures:
- Table edits (cell changes)
- AutoTune recommendations
- Tune saves
- ECU burns
- Settings changes

### When to Use

- **Document changes**: Keep record of what was tuned
- **Replay actions**: Re-apply same changes to new tune
- **Collaboration**: Share tuning steps with others
- **Training**: Show how a tune was developed
- **Analysis**: Understand tune development history

### How to Use

#### Recording Actions

1. **Open Action Manager**
   ```
   Tools → Action Manager
   ```

2. **Click Record**
   - Manager listens to all changes
   - Status shows "Recording..."
   - Continue with normal tuning

3. **Perform Tuning**
   - Edit table cells
   - Run AutoTune
   - Save tunes
   - Burn to ECU
   - All actions recorded

4. **Stop Recording**
   - Click **Stop** when done
   - Actions saved with timestamp
   - Can name the session

#### Replaying Actions

1. **Load action file**
   - Click **Load Actions**
   - Select .actions file
   - Session loads

2. **Review actions**
   - See list of recorded actions
   - Verify order and details
   - Can skip unwanted steps

3. **Play back**
   - Click **Play** to start
   - Actions apply automatically
   - Pause/resume available
   - Can repeat if needed

### Action Types

| Action | Description | Details |
|--------|-------------|---------|
| Cell Edit | Single cell changed | RPM, Load, Old Value, New Value |
| Table Operation | Bulk edit (scale, smooth) | Table, Operation, Parameters |
| AutoTune | Recommendations applied | Table, # Cells Changed |
| Save | Tune file saved | Filename, Timestamp |
| Burn | Sent to ECU | ECU Type, Success/Failure |
| Setting | Option changed | Setting Name, Old Value, New Value |

### Workflow Examples

#### Example 1: Document Tuning Session
```
1. Open Action Manager → Record
2. Open AutoTune, run tuning session
3. Apply recommendations
4. Save tune
5. Stop recording
6. Name it: "2024-02-03_autotune_session"
7. Export for sharing
```

#### Example 2: Create Tuning Template
```
1. Record reference tuning session
2. Save actions as template
3. Load template on new tune
4. Play back all actions
5. Customizations applied automatically
```

#### Example 3: Audit Changes
```
1. Load action file from tuning session
2. Review all changes made
3. Calculate total modification
4. Document results
5. Share methodology with others
```

### Exporting Actions

**Formats available:**
- **.actions** - Proprietary format (can be replayed)
- **.txt** - Human-readable log
- **.pdf** - Formatted report with analysis

**Export steps:**
1. Actions recorded/loaded
2. Click **Export**
3. Choose format
4. Click **Save**

### Action Settings

**Auto-record mode:**
- Enable to start recording automatically
- Useful for long tuning sessions
- Can toggle on/off as needed

**Skip confirmation:**
- When replaying, automatically apply all actions
- Can slow down playback to review

**Save history:**
- Keep old action files
- Compare different tuning approaches
- Learn from past sessions

---

## Reset to Defaults

Quickly reset tune to factory defaults for a clean start.

### Overview

This tool resets either:
- **Current table**: Single table to INI defaults
- **Entire tune**: All constants to INI defaults

### When to Use

- **Start over**: Bad tune, need to reset
- **Reference baseline**: Compare current vs stock
- **Testing**: Isolate specific table changes
- **Emergency**: Worst-case fallback

### How to Use

1. **Go to** Tools → Reset to Defaults
2. **Choose scope**:
   - Reset current table only
   - Reset all tables
3. **Confirm** you want to reset
4. **Done** - changes take effect immediately

### Warning

⚠️ **This action cannot be undone!**
- Make sure tune is saved first
- Keep backup copy before resetting
- Changes are immediate

### After Reset

The tune is now:
- Back to INI stock values
- All edits since loading are lost
- Ready for fresh tuning
- Can be saved as new file

---

## See Also

- [AutoTune Usage Guide](./autotune/usage-guide.md) - Tuning best practices
- [Table Editing](./table-editing.md) - Manual table adjustments
- [Data Logging](./datalog.md) - Recording engine data for analysis
- [Troubleshooting](../reference/troubleshooting.md) - Common issues

