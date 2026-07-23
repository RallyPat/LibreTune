# Understanding Recommendations

How AutoTune calculates correction recommendations.

## The Correction Formula

For every sample attributed to a VE cell, AutoTune computes the required VE
that would have produced the target AFR:

```
required_ve = current_ve × (actual_afr / target_afr)
```

- **Lean reading** (`actual_afr > target_afr`) → `required_ve` is higher than
  the current VE → recommendation adds fuel.
- **Rich reading** (`actual_afr < target_afr`) → `required_ve` is lower than
  the current VE → recommendation removes fuel.

### Example
- Current VE: 75
- Target AFR: 14.7
- Actual AFR: 15.5 (lean)
- Required VE: 75 × (15.5 / 14.7) ≈ **79.1**

The recommendation is the cumulative moving average of these raw required VE
values, then clamped by authority limits for display and application.

## Cumulative Moving Average (CMA)

AutoTune does **not** apply each sample directly. It keeps a running average of
the *raw* required VE for each cell:

```
raw_cma += (required_ve - raw_cma) / hit_count
```

Authority limits are applied **after** the average is updated. This means a
single clamped value cannot permanently bias the underlying average; the
average continues to converge toward the true required VE.

## Hit Count and Hit Percentage

### Hit Count
Number of accepted samples that have contributed to a cell's recommendation.

### Hit Percentage
The share of *all* accepted samples that fell into this cell:

```
hit_percentage = (cell_hit_count / total_accepted_samples) × 100
```

This gives a realistic picture of how much driving time was spent in each
operating region. A cell with 50 hits out of 1000 total samples has a 5% hit
percentage, which is very different from a cell with 50 hits out of 200 total
samples (25%).

### Visual Confidence
- **1–5 hits**: Preliminary (gray)
- **5–20 hits**: Developing (light color)
- **20+ hits**: Confident (full color)

## Color Coding

### Recommendation Grid
- 🔵 **Blue**: Increase fuel (running lean)
- 🔴 **Red**: Decrease fuel (running rich)
- ⬜ **Gray**: No data or insufficient hits

### Intensity
- **Bright**: Large correction needed
- **Dim**: Small correction
- **Neutral**: On target

## Heat Map Views

### Cell Weighting Map
Shows data coverage:
- **Bright cells**: Many data points (high confidence)
- **Dim cells**: Few data points (low confidence)
- **Dark cells**: No data

### Cell Change Map
Shows correction magnitude:
- **Bright cells**: Large recommended change
- **Dim cells**: Small recommended change

## Filtering Effects

Samples are rejected if they fail any configured filter:
- RPM outside filter range
- Load outside y-axis bounds
- Coolant temp too low
- TPS changing too fast
- Accel enrichment active
- Custom expression evaluates to false

Filtered samples do **not** count toward `total_accepted_samples` and do not
contribute to recommendations.

## Authority Limiting

Recommendations are clamped to authority limits before display/application.
Two limits are enforced together:

```
delta = raw_cma - beginning_value
clamped_delta = clamp(delta, -max_value_change, +max_value_change)
final_delta = clamp(clamped_delta,
                    -beginning_value × max_percent_change/100,
                    +beginning_value × max_percent_change/100)
final_ve = beginning_value + final_delta
```

If the final value is clamped, the cell shows a warning icon.

## Interpreting Results

### Uniform Lean/Rich
All cells the same direction suggests:
- Fuel pressure issue
- Sensor calibration error
- Global offset needed

### Specific Regions
Localized corrections suggest:
- Normal VE variations
- Proper table tuning needed

### Erratic Recommendations
Random patterns suggest:
- Noisy sensor
- Unstable engine
- Bad data (check filters)

## When to Apply

Apply recommendations when:
- ✅ Good coverage across the table
- ✅ Consistent readings per cell
- ✅ Heat map shows sufficient hits
- ✅ No warning indicators

Wait for more data when:
- ⚠️ Sparse coverage
- ⚠️ Recommendations bouncing
- ⚠️ Authority limits frequently hit
