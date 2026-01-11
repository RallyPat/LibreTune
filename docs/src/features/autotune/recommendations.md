# Understanding Recommendations

How AutoTune calculates correction recommendations.

## The Basic Algorithm

AutoTune compares actual AFR to target AFR and calculates a correction:

```
Error = (Actual AFR - Target AFR) / Target AFR
Correction = Current VE × (1 - Error)
```

### Example
- Target AFR: 14.7
- Actual AFR: 15.5 (lean)
- Error: (15.5 - 14.7) / 14.7 = 5.4%
- If VE is 75, recommendation: 75 × 1.054 = 79.1

## Weighted Averaging

AutoTune doesn't use single readings. It accumulates data:

1. **Hit count**: Number of samples in this cell
2. **Weighted average**: Recent samples weighted more
3. **Confidence**: More hits = more confidence

The recommendation shown is a weighted average of all samples.

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

## Hit Count

**Minimum hits** before recommendation is trusted:
- **1-5 hits**: Preliminary (gray)
- **5-20 hits**: Developing (light color)
- **20+ hits**: Confident (full color)

Configure minimum hit threshold in settings.

## Filtering Effects

Samples are rejected if:
- RPM outside filter range
- Coolant temp too low
- TPS changing too fast
- Accel enrichment active

Filtered samples don't contribute to recommendations.

## Authority Limiting

Recommendations are clamped to authority limits:

```
Final = clamp(Recommendation, 
              Current - MaxDecrease, 
              Current + MaxIncrease)
```

If recommendation exceeds limits, the cell shows a warning icon.

## Interpreting Results

### Uniform Lean/Rich
All cells same direction suggests:
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
- ✅ Good coverage across table
- ✅ Consistent readings per cell
- ✅ Heat map shows sufficient hits
- ✅ No warning indicators

Wait for more data when:
- ⚠️ Sparse coverage
- ⚠️ Recommendations bouncing
- ⚠️ Authority limits frequently hit
