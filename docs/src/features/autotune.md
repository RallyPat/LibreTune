# AutoTune Live

AutoTune Live automatically generates tuning recommendations based on wideband O2 sensor feedback.

## Overview

AutoTune monitors your engine in real-time and suggests VE table corrections to achieve target AFR values. It's the fastest way to get a basic tune dialed in.

## Prerequisites

- Wideband O2 sensor connected to ECU
- AFR target table configured in ECU
- Engine running and at operating temperature

## Starting AutoTune

1. Connect to your ECU
2. Go to **Tools → AutoTune Live** or press `Ctrl+Shift+A`
3. Select the table to tune (usually VE Table 1)
4. Configure settings
5. Click **Start**

## AutoTune Interface

| Section | Description |
|---------|-------------|
| **Current Table** | Your existing VE values |
| **Recommendations** | Suggested corrections |
| **Heat Map** | Tuning coverage visualization |
| **Statistics** | Hit counts and data quality |

## Settings

### Target AFR Source
- **From Table**: Use ECU's AFR target table
- **Fixed Value**: Use a constant target (e.g., 14.7:1)

### Update Controller
When enabled, recommendations are automatically sent to the ECU.

### Authority Limits

| Setting | Description |
|---------|-------------|
| **Max Increase** | Maximum % to increase any cell |
| **Max Decrease** | Maximum % to decrease any cell |
| **Absolute Max** | Maximum absolute change per update |

## Filters

Filters prevent bad data from affecting recommendations:

| Filter | Description | Default |
|--------|-------------|---------|
| **Min RPM** | Ignore data below this RPM | 800 |
| **Max RPM** | Ignore data above this RPM | 6500 |
| **Min CLT** | Ignore when engine is cold | 160°F |
| **Min TPS** | Ignore at closed throttle | 1% |
| **Max TPS Rate** | Ignore rapid throttle changes | 10%/sec |
| **Exclude Accel Enrich** | Ignore during acceleration enrichment | Yes |

## Understanding Recommendations

### Color Coding
- 🔵 **Blue**: Recommendation to add fuel (running lean)
- 🔴 **Red**: Recommendation to remove fuel (running rich)
- ⚪ **Gray**: Insufficient data

### Heat Map Views

#### Cell Weighting
Shows how much data has been collected per cell:
- Bright = lots of data (high confidence)
- Dim = little data (low confidence)

#### Cell Change
Shows the magnitude of recommended changes:
- Bright = large correction needed
- Dim = minimal correction

## Workflow

1. **Warm up** the engine to operating temperature
2. **Start AutoTune** with conservative authority limits
3. **Drive normally** covering various RPM/load points
4. **Monitor** the heat map for coverage
5. **Apply** recommendations when confident
6. **Iterate** with higher authority as the tune improves

## Tips

### Get Good Coverage
- Drive at steady state in each cell for several seconds
- Cover all RPM/load combinations you'll use
- Include both acceleration and cruise conditions

### Start Conservative
- Begin with low authority limits (5-10%)
- Increase as the base tune improves
- Large corrections suggest the base tune needs work

### Watch for Outliers
- Occasional spikes are filtered automatically
- Consistent lean/rich readings indicate real issues
- Check for vacuum leaks if all cells read lean

## Applying Changes

1. Review recommendations in the grid
2. Click **Send to ECU** to apply without saving
3. Or click **Apply & Save** to update the tune file
4. **Burn** to make changes permanent in ECU flash

## Cell Locking

Lock cells you don't want AutoTune to modify:
1. Select cells in the Recommendations grid
2. Right-click → **Lock Selected Cells**
3. Locked cells show a 🔒 icon

## Next Steps

- [Setting Up AutoTune](./autotune/setup.md) - Detailed configuration
- [Understanding Recommendations](./autotune/recommendations.md) - How corrections are calculated
- [Filters and Authority](./autotune/filters.md) - Tuning data quality
