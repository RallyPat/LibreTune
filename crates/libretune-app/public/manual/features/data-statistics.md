# Data Statistics Guide

The Statistics Panel in the Data Logger provides comprehensive statistical analysis of recorded engine parameters, helping you understand data consistency, identify anomalies, and validate tuning changes.

## Overview

When reviewing logged data, statistics answer critical questions:
- How stable is my AFR? (std dev tells the story)
- What's the typical value? (mean vs median difference reveals outliers)
- Did my tune change actually improve things? (compare before/after)
- Are there any sensor issues? (spikes in max/min reveal glitches)

## Opening the Statistics Panel

### In Data Logger View

1. **Record or load a log** in Data Logger
2. Switch to **Playback mode** (toggle at top of view)
3. Click the **Stats** button to toggle statistics panel
4. Select a channel to analyze or view **multi-channel comparison table**

### Quick Stats Display

The statistics panel shows:
- **Single-channel detail view**: 8-row grid with all statistical measures
- **Multi-channel table view**: Compact table comparing all channels side-by-side

## Statistical Measures Explained

### Mean (Average)

**Formula**: Sum of all values ÷ Number of samples

**What it means**:
- The typical value for this channel
- Useful for baseline comparisons between runs
- Affected by extreme outliers

**Example**: AFR mean = 14.2
- Your lambda control is targeting 14.2 on average
- Compare to your target AFR to see if corrections are needed

**When it matters**: Checking if tune is centered on desired value

---

### Median (50th Percentile)

**What it means**:
- The middle value when all samples are sorted
- Not affected by extreme outliers
- Better representation of "typical" than mean

**Example**: AFR median = 14.1, AFR mean = 14.2
- Median and mean are close → data is symmetric
- Few outliers present

**When it matters**: Understanding the "true center" of your data distribution

---

### Standard Deviation (Std Dev)

**Formula**: Square root of average squared differences from mean

**What it means**:
- Measure of variability or "noise" in the data
- Lower std dev = more stable/consistent
- Higher std dev = noisier/less stable

**Example**: AFR std dev = 0.3 vs 1.2
- 0.3: Very stable AFR control
- 1.2: Significant AFR hunting or sensor noise

**Interpretation**:
- **< 0.2**: Excellent control (narrow band)
- **0.2 - 0.5**: Good control (typical tuned engine)
- **0.5 - 1.0**: Acceptable but could improve
- **> 1.0**: Poor control or noisy sensor

**When it matters**: Validating AutoTune results (should reduce std dev)

---

### Min / Max

**What they mean**:
- Lowest and highest values recorded
- Range = Max - Min
- Helps identify extreme excursions

**Example**: AFR min = 12.5, max = 15.8, range = 3.3
- AFR varies from quite rich to quite lean
- Large range indicates control issues or transient events

**When it matters**: 
- Checking for sensor spikes (unrealistic extremes)
- Identifying worst-case excursions
- Verifying safe operating bounds

---

### Percentiles (P25, P75)

**What they mean**:
- P25: 25% of values fall below this (1st quartile)
- P75: 75% of values fall below this (3rd quartile)
- Describes the "middle 50%" of your data (between P25 and P75)

**Example**: AFR P25 = 13.9, P75 = 14.3
- 50% of AFR readings fall between 13.9 and 14.3
- Shows the core operating range

**When it matters**: Understanding data distribution without outlier distortion

---

### Interquartile Range (IQR)

**Formula**: P75 - P25

**What it means**:
- Spread of the middle 50% of data
- Lower IQR = more concentrated around center
- Higher IQR = more spread out

**Example**: IQR = 0.4 (P75=14.3, P25=13.9)
- Middle 50% spans only 0.4 AFR units
- Tight, consistent data

**When it matters**: Assessing control stability at a glance

---

## Analysis Workflows

### Finding Unstable Channels

**Workflow**: Identify which sensors/controls are noisy

**Look for**:
1. High standard deviation (> 1.0)
2. Large difference between mean and median
3. Large IQR relative to the value
4. Extreme min/max values

**Example**:
```
Channel: IAT (Intake Air Temperature)
Mean: 82°F
Std Dev: 2.5°F  ← Too high for steady state!
Min: 58°F
Max: 95°F
```

**Investigation**:
- IAT sensor is working but very noisy
- Possible causes: loose connector, intake air mixing, sensor failure
- Solution: Check sensor wiring, replace sensor if needed

---

### Comparing Tune Versions (Before/After)

**Workflow**: Validate that your tune changes actually improved things

**Step 1: Record Baseline**
1. Load original tune in ECU
2. Perform identical test drive (same route/conditions)
3. Log data with auto-record
4. Save log: "baseline_tune_log.csv"

**Step 2: Apply Changes & Re-record**
1. Load modified tune in ECU
2. Perform identical test drive again
3. Log data with auto-record
4. Save log: "modified_tune_log.csv"

**Step 3: Compare Statistics**

Open both logs in Data Logger playback:

| Channel | Baseline | Modified | Improvement |
|---------|----------|----------|-------------|
| AFR Mean | 14.5 | 14.2 | On target (14.2) ✓ |
| AFR Std Dev | 0.8 | 0.3 | **More stable!** ✓ |
| AFR IQR | 1.2 | 0.5 | Better control ✓ |
| MAP Std Dev | 2.1 | 1.8 | Slightly better ✓ |
| Timing Std Dev | 3.5 | 2.1 | **Much better!** ✓ |

**Interpretation**: All metrics improved → tune changes were successful!

---

### Identifying Outliers

**Workflow**: Spot bad data points or transient events

**Method 1: Compare Mean vs Median**
```
Channel: Lambda
Mean: 1.003
Median: 0.998
Difference: 0.005  ← Significant!
```
**Interpretation**: Mean is pulled up by some high values
- Outliers present, probably during acceleration
- Median (0.998) is the "true center"

**Method 2: Check P25 and P75**
```
Channel: CLT (Coolant Temperature)
Min: 155°F
P25: 182°F  ← 25% of data
Median: 185°F
P75: 187°F  ← 75% of data
Max: 210°F  ← Spike!
```
**Interpretation**: 
- Typical coolant is 182-187°F (tight range)
- Spike to 210°F is an outlier
- Might be startup transient or sensor glitch

---

## Real-World Examples

### Example 1: Good Tuned Engine

```
AFR Statistics (stock fuel map):
Mean: 14.0
Median: 14.1
Std Dev: 0.25
Min: 13.2
Max: 14.8
P25: 13.9
P75: 14.2
IQR: 0.3
```

**What this tells us**:
- ✓ AFR is centered on target (14.0)
- ✓ Very stable (std dev = 0.25)
- ✓ Tight distribution (IQR = 0.3)
- ✓ Min/max are reasonable (0.8 unit range)

**Conclusion**: Excellent closed-loop fuel control

---

### Example 2: Boost Control Issue

```
MAP (Manifold Air Pressure):
Mean: 12.5 psi
Std Dev: 2.8 psi  ← High!
Min: 8.2
Max: 18.5  ← Big spike!
P25: 11.2
P75: 13.8
IQR: 2.6
```

**What this tells us**:
- ✗ Boost control is unstable (std dev = 2.8)
- ✗ Large swings (IQR = 2.6)
- ✗ Spike to 18.5 suggests over-boost event

**Diagnosis**: 
- Wastegate spring might be weak
- Boost target PID needs tuning
- Check for boost leak

**Action**: Adjust boost control parameters and re-test

---

### Example 3: AFR Sensor Issue

```
AFR (from sensor):
Mean: 14.1
Median: 13.95  ← Different from mean!
Std Dev: 0.6
Min: 11.2  ← Very lean
Max: 17.8  ← Very rich
P25: 13.7
P75: 14.5
IQR: 0.8
```

**What this tells us**:
- ✗ Mean/median different → outliers present
- ✗ High std dev (0.6)
- ✗ Unrealistic min (11.2) and max (17.8)
- Possible sensor failure or wiring intermittent

**Diagnosis**: AFR readings are unstable with extreme spikes

**Action**: 
- Check sensor connector and wiring
- Test sensor on bench
- Replace if faulty

---

## Tips for Data Interpretation

### "Std Dev is too high" - What Now?

1. **Check for transient events**
   - Peak and dips during acceleration/deceleration expected
   - Log steady-state only to verify sensor health
   
2. **Cross-check multiple sensors**
   - If AFR is noisy but CLT is stable, sensor-specific issue
   - If multiple channels noisy, could be EMI/wiring issue

3. **Compare to baseline**
   - If baseline was also noisy, it's not a regression
   - If baseline was clean, something changed

### "Mean vs Median are far apart" - What Now?

1. **Investigate the outliers**
   - Where in the log do they occur? (startup? WOT?)
   - Are they expected for that driving condition?

2. **Slice the log**
   - Log steady-state cruise separately
   - Log acceleration separately
   - Each should have tighter stats

3. **Accept or reject outliers**
   - Cold start transients are normal
   - Fuel cut on deceleration is normal
   - Sensor spikes are not normal

### "Statistics look good but car doesn't feel tuned" - Why?

Possible reasons:
- **Time window matters**: One 5-minute log might not be representative
  - Log at different times of day, temperatures
  - Log during actual driving conditions you care about

- **Channel selection**: Might be logging wrong channels
  - Verify you're looking at the right sensor input
  - Check scaling and units

- **Driving technique**: Same tune, different results
  - Consistent throttle inputs produce consistent logs
  - Aggressive throttle causes larger swings

---

## Integration with Other Features

### With Smart Recording

1. **Enable auto-record** in Data Logger
2. **Drive your normal route** without manual start/stop
3. **Multiple passes** of the same route
4. **Compare statistics across passes**
   - Consistent stats = reproducible tune
   - Varying stats = driving variable (temp, traffic, etc.)

---

### With AutoTune

1. **Log baseline run** with original tune
2. **Review baseline statistics** (std dev, mean, range)
3. **Run AutoTune session** and apply recommendations
4. **Log same drive again** with tuned version
5. **Compare statistics**:
   - AFR std dev should decrease
   - AFR mean should match target
   - Overall stability should improve

---

### With Performance Calculator

1. **Calculate expected power/torque** from tune parameters
2. **Log a dyno pull or acceleration run**
3. **Review statistics for that load condition**
4. **Verify tune matches intended parameters**
   - AFR should match stoichiometric target
   - Timing should match curve
   - Boost should match target (if turbocharged)

---

## See Also
- [Data Logging](datalog.md) - Data logging overview
- [Smart Recording](smart-recording.md) - Automatic key-on/off logging
- [Diagnostic Loggers](diagnostic-loggers.md) - Tooth and Composite logging
- [AutoTune Usage](autotune/usage-guide.md) - Using logs for AutoTune analysis
