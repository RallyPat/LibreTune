# AutoTune Usage Guide

Step-by-step instructions for using AutoTune to tune your VE table in real-world conditions.

## Quick Start (5 Minutes)

1. **Connect** to your ECU
2. **Go to** Tools → AutoTune (or Ctrl+Shift+A)
3. **Select** your VE table
4. **Click** Start (with default settings)
5. **Drive** normally for 5-10 minutes
6. **Review** the heat map for coverage
7. **Click** Send to ECU to apply changes

## Before You Start

### Prerequisites Checklist
- [ ] Wideband O2 sensor installed and powered on
- [ ] AFR signal wire connected to ECU
- [ ] Engine at operating temperature (180°F+ coolant)
- [ ] Current tune runs without throwing errors
- [ ] No check engine lights for major faults
- [ ] Clear day/controlled conditions if possible

### Safety Considerations
- **Never tune alone** - Have a spotter
- **Test drive familiar roads** - Nothing new or complicated
- **Stay off highways** - Use quiet streets or closed course
- **Keep phone available** - In case of emergency
- **Be ready to disable** - Pull a fuse or kill ignition if needed

## Session Workflow

### Phase 1: Initial Setup (5 minutes)

**Step 1: Open AutoTune**
```
Tools → AutoTune
Or keyboard: Ctrl+Shift+A
```

**Step 2: Select Table**
- Default: VE Table 1 (most common)
- Alternative: Choose different table if your ECU uses different tables
- Secondary: Enable if you want to tune multiple tables simultaneously

**Step 3: Configure Basic Settings**
```
Target AFR: 14.7 (gasoline stoich)
          14.0 (E85)
          13.0 (high boost, rich)
Algorithm: Simple (recommended for beginners)
```

**Load Source (MAP vs MAF)**
- **MAP (Speed Density)**: Default for VE tables
- **MAF**: Use for mass-airflow based tables
- If the table load axis reports MAF, AutoTune auto-switches to **MAF**
- If no MAF channel is detected, AutoTune falls back to **MAP** and shows a hint

**Step 4: Set Authority Limits**
For your FIRST session ever with this tune:
```
Max Increase:   5%
Max Decrease:   5%
Absolute Max:  15%
```

These are conservative. You can increase them later.

**Step 5: Verify Filters**
Default filters work for most cases. Only adjust if:
- Your car idles rough: Raise Min RPM to 1200
- You're tuning a diesel: Adjust temp filters significantly
- You do a track day: Lower Min TPS to 0 for coast-down testing

**Step 6: Click Start**
- AutoTune begins listening to wideband sensor
- Status changes to "Running"
- UI shows "Waiting for data..."

---

### Phase 2: Driving & Data Collection (10-30 minutes)

**Where to Drive**
- Local roads you know well
- Light traffic, predictable conditions
- Mix of steady-state cruise and smooth acceleration
- Don't drive aggressively or make sudden changes

**Driving Pattern 1: Cruise Tuning (Best for city)**
```
1. Drive at 30 mph, constant throttle for 30 seconds
2. Accelerate gently to 40 mph, hold for 30 seconds
3. Cruise at 50 mph for 1 minute
4. Repeat with different throttle positions
5. Vary RPM: 2000, 2500, 3000, 3500 RPM
```

**Driving Pattern 2: Highway Tuning (Best for long highways)**
```
1. Cruise at 60 mph, light throttle, for 2 minutes
2. Slight acceleration to 65 mph, cruise 1 minute
3. Return to 60 mph
4. Repeat with different engine loads
```

**Driving Pattern 3: Combined (Most realistic)**
- Drive like you normally would
- Include both city and highway sections
- Vary speed and load naturally
- Drive for at least 20 minutes

**What to Avoid**
- ❌ Hard acceleration (triggers accel enrichment filter)
- ❌ Coasting with fuel cut (triggers decel filter)
- ❌ Rapid throttle blips (triggers rate filter)
- ❌ Cold engine (min CLT filter rejects it)
- ❌ Idle tuning (min TPS filter ignores it)

**Monitoring While Driving**
- Watch heat map on secondary display if available
- Look for coverage in your normal operating range
- Don't stare at screen - safety first!

---

### Phase 3: Reviewing Data (5 minutes)

**Check Heat Map Coverage**
```
Heat Map View: Cell Weighting (shows data coverage)
```

- 🟢 Bright cells: Good coverage, high confidence
- 🟡 Medium cells: Decent coverage, usable
- 🔴 Dark cells: Little to no data

**Areas to Cover**
- **Idle**: 800-1000 RPM, 0-5% TPS
- **Cruise**: 2000-3500 RPM, 10-30% TPS
- **Highway**: 3000-4000 RPM, 20-40% TPS
- **Acceleration**: 2000-5000 RPM, 50-100% TPS (if possible)

**Quality Check**
```
Switch to: Cell Change (shows correction magnitude)
```

- Uniform color = good, consistent corrections
- Wildly varying colors = inconsistent data or sensor issues
- All blue = running lean
- All red = running rich
- Mixed = good blend of conditions

---

### Phase 4: Applying Changes (3 minutes)

**Review Recommendations**
```
1. Switch back to Primary table view
2. Look at recommended values (light blue/red overlay)
3. Scan for any extreme values that look wrong
4. Check locked cells (if any)
```

**Send to ECU**
```
Button: Send to ECU (applies without saving)
```

This sends changes to ECU RAM. They take effect immediately.

**Drive Test (5 minutes)**
- Take a short drive (just around the block)
- Feel throttle response - is it improved?
- Listen for any pinging or hesitation
- Watch wideband if you can - should be closer to target

**If Good: Save the Tune**
```
File → Save Tune (Ctrl+S)
```

**If Not Ready: Continue Tuning**
```
Click: Start again (with same or adjusted authority limits)
```

---

### Phase 5: Iterative Refinement

**First Session Results**
- Usually rough (5-10% AFR deviation)
- Gives you a foundation to build on
- Don't expect perfection yet

**Second Session (Next day)**
```
1. Increase authority limits slightly:
   Max Increase:  10%
   Max Decrease:  10%
   Absolute Max:  30%

2. Tune same areas again (30 minutes)
3. Send to ECU, verify improvement
4. Save tune
```

**Progressive Strategy**
| Session | Authority | Goal | Comments |
|---------|-----------|------|----------|
| 1 | Conservative (5%) | Get basic coverage | Slow, safe adjustments |
| 2 | Moderate (10%) | Fill gaps, refine | Verify stability |
| 3 | Aggressive (15%) | Final polish | Most cells converged |
| 4+ | Fine-tuning (5-10%) | Optimize specific regions | Minor corrections |

---

## Advanced Techniques

### Multi-Table Tuning

Tune fuel and ignition tables together for best results:

```
1. Enable "Secondary Table"
2. Select AFR Target table
3. Primary: VE Table 1
4. Secondary: AFR Table 1

Drive normally - both tables get tuned simultaneously
```

### Cell Locking

Lock cells you want to protect:

**Before Tuning**
```
1. Right-click cell in grid
2. Select "Lock Selected Cells"
3. Locked cells show 🔒 icon
4. AutoTune will skip them
```

**Use Cases**
- Lock idle cells if they're dialed in
- Lock high-boost cells to prevent over-correction
- Lock cells with marginal data quality

### Zoom & Navigation

Work with specific regions:

```
1. Zoom in on low-RPM/low-load region
2. Tune that section intensely for 10 minutes
3. Zoom to different region
4. Repeat for each section
```

### CSV Export

Save recommendations for analysis:

```
File → Export AutoTune Data
Opens save dialog
Examine in spreadsheet software
Compare before/after values
```

---

## Troubleshooting

### "No Data" / Gray Heat Map

**Possible Causes**
- Engine not running (obvious!)
- Wideband sensor not working
- AFR signal not connected to ECU
- Filters too restrictive

**Solution**
```
1. Verify wideband powers on
2. Check sensor LED - should blink
3. Test AFR input in ECU diagnostics screen
4. Loosen filters: Lower Min RPM, Min TPS
5. Verify ECU is receiving AFR channel
```

### All Cells Show Blue (Very Lean)

**This Is Bad** - Do not ignore!

**Causes**
- Fuel pressure too low
- Fuel pump failing
- Injector problem
- Sensor calibrated wrong

**Solution**
```
1. STOP AutoTune immediately
2. Check fuel pressure gauge (should be 43-45 PSI)
3. Verify fuel pump operation
4. Check injector connections
5. Do NOT drive aggressively until resolved
```

### All Cells Show Red (Very Rich)

**Less critical than lean**, but still wrong.

**Causes**
- Fuel pressure too high
- Regulator stuck open
- Sensor calibrated high
- Global fuel scaling off

**Solution**
```
1. Check fuel pressure (might be 50+ PSI)
2. Verify sensor zero point
3. Reduce global fuel trim if available
4. Continue tuning - it will converge
```

### Heat Map Has Gaps

**This is normal** - you can't cover every cell.

**Solution**
```
1. Identify which cells are missing
2. Plan driving pattern to hit those cells
3. Next session, focus on those areas
4. You can leave some cells untouched
```

### Wideband Reading Bounces Around

**Normal behavior** - sensors are noisy.

**Why AutoTune Works**
- Weighted averaging filters noise
- Accumulates many samples
- Natural filtering built-in

**If Bouncing Excessively**
- Check sensor wiring (loose connections?)
- Check for EMI (spark plug wires near sensor wire?)
- Sensor might be failing

### Corrections Don't Match Actual AFR

**Cause**: Lambda delay compensation or sensor lag

**Explanation**
- Exhaust takes 50-200ms to reach sensor
- AutoTune correlates AFR to the VE cell from that time ago
- Slight mismatch normal

**Solution**
- Trust the algorithm - it's designed for this
- More data samples improve correlation
- Second tuning session often improves results

---

## Real-World Examples

### Scenario 1: NA Gasoline Engine

**Goal**: Dial in VE table for smooth cruise

```
Target AFR:     14.7
Session 1:      5% authority, 20 minutes cruise
Result:         Heat map shows green cruising range
Send Changes:   Yes
Session 2:      10% authority, add acceleration data
Result:         Better coverage through RPM range
Send Changes:   Yes
Session 3:      10% authority, fine-tune rough spots
Result:         Smooth AFR across table
Final:          Save and burn to flash
```

**Outcomes**
- Good idle quality ✓
- Smooth throttle response ✓
- Consistent AFR ✓
- Better fuel economy ✓

### Scenario 2: Turbocharged Engine

**Goal**: Tune for safety and drivability on pump gas

```
Target AFR:     13.5 (rich, boost safety)
Authority:      Conservative (5-8%)
Zones to Cover: 
  - Idle/cruise (below 1.0 bar)
  - Mid-boost (1.0-1.3 bar)
  - High-boost (1.3+ bar) - only if safe
Session Time:   30 minutes, mixed driving
Lock High-Boost: Yes (until proven safe)
Send Changes:   Yes with verification drive
```

**Caution**
- Don't boost hard until tune verified
- Watch IAT (intake air temperature)
- Watch knock detection
- Conservative is better than aggressive

### Scenario 3: E85 Ethanol Blend

**Goal**: Tune for high-octane, cold-start friendly

```
Target AFR:     12.0 (E85 stoich is lower)
Expect:         Richer than gasoline
Session Time:   45 minutes (more data needed)
Special Notes:  
  - E85 needs more fuel than gasoline
  - Cold-start might need separate table
  - Winter/summer E85 blends differ
Verify:         Fuel consumption should be higher
```

---

## Tips & Tricks

### Get the Best Results
1. **More data = better results** - Longer sessions always win
2. **Consistent conditions** - Cold day vs hot day affects things
3. **Known road** - Predictable speeds help data quality
4. **Second and third sessions** - First session is foundation only
5. **Backup your tune** - Save before AutoTune, save after

### Keyboard Shortcuts
```
Ctrl+Shift+A     Open AutoTune
Ctrl+S           Save tune
Ctrl+Shift+E     Export data
Tab              Switch between tables
F5               Refresh heat map
```

### When to Stop Tuning
- Heat map looks good in your normal driving range
- AFR stays within ±1.0 of target
- No more significant changes happening
- You feel confident in the tune
- Multiple drives show consistent results

### When to Continue Tuning
- Major gaps in heat map coverage
- AFR swinging ±2.0 or more
- Specific regions show inconsistent data
- You just started (first session always rough)
- You changed major engine components

---

## Common Mistakes

### ❌ Pushing Authority Too High Early
**Wrong**: 50% max change on first session
**Why**: Huge, uncontrolled changes = danger
**Right**: Start 5%, increase gradually

### ❌ Tuning Only One Condition
**Wrong**: Only highway cruising for 10 minutes
**Why**: Poor coverage, unbalanced tune
**Right**: Mix of conditions for 30+ minutes

### ❌ Ignoring Bad Data
**Wrong**: Heat map all blue, keep tuning
**Why**: Sign of sensor/mechanical problem
**Right**: Diagnose and fix before continuing

### ❌ Not Verifying After Sending Changes
**Wrong**: Send to ECU, immediately save
**Why**: What if results are wrong?
**Right**: Drive 5 minutes, check AFR target, then save

### ❌ One-and-Done Tuning
**Wrong**: One 15-minute session, call it done
**Why**: Tune isn't stable until validated
**Right**: Multiple short sessions for confidence

---

## See Also

- [Setting Up AutoTune](./setup.md) - Hardware and ECU configuration
- [Understanding Filters](./filters.md) - Data quality control
- [Understanding Recommendations](./recommendations.md) - How corrections are calculated
- [AutoTune Overview](../autotune.md) - Feature overview

