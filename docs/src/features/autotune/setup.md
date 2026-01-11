# Setting Up AutoTune

Detailed configuration guide for AutoTune Live.

## Prerequisites

Before using AutoTune, ensure:

1. **Wideband O2 sensor** is installed and working
2. **AFR signal** is connected to ECU analog input
3. **AFR target table** is configured in your ECU
4. **Engine is warmed up** to operating temperature
5. **Base tune** is close enough to run safely

## Sensor Configuration

### Wideband Controllers
LibreTune works with any wideband that outputs 0-5V analog:
- AEM UEGO
- Innovate LC-2
- PLX Wideband
- Zeitronix ZT-3

### AFR Curve
Ensure your ECU's AFR curve matches your wideband's output:
1. Check your wideband's voltage-to-AFR table
2. Verify it matches the ECU's `afrTable` constant
3. Adjust if necessary

## AutoTune Settings

### Table Selection
Choose which table to tune:
- **VE Table 1**: Main volumetric efficiency table
- **VE Table 2**: Secondary (if applicable)
- **Other**: Any table that affects AFR

### Target AFR Source

**From AFR Target Table** (Recommended)
- Uses your ECU's configured AFR targets
- Automatically adapts to different conditions
- Supports different targets for WOT vs cruise

**Fixed Target**
- Uses a constant target value
- Simpler but less flexible
- Good for initial testing

### Update Mode

**Manual**
- Review recommendations first
- Click "Send" to apply
- Safest option

**Auto-Send**
- Automatically sends changes
- Set minimum hit count first
- For experienced tuners

## Filter Configuration

### RPM Filter
- **Min RPM**: Ignore idle data (usually 800-1000)
- **Max RPM**: Ignore over-rev data (near redline)

### Temperature Filter
- **Min CLT**: Ignore cold engine data (160°F+)
- Ensures consistent fuel behavior

### Throttle Filter
- **Min TPS**: Ignore closed throttle (1%+)
- Helps avoid decel enleanment data

### Rate Filters
- **Max TPS Rate**: Ignore rapid throttle (10%/sec)
- Filters out transient conditions
- Prevents accel enrichment interference

## Authority Limits

### Maximum Change Per Cell
- **Max Increase**: Largest positive correction (e.g., 15%)
- **Max Decrease**: Largest negative correction (e.g., 15%)

### Cumulative Limit
- **Absolute Max**: Total change from baseline
- Prevents runaway corrections

### Starting Values
| Experience | Max Change | Absolute Max |
|------------|------------|--------------|
| Beginner | 5% | 15% |
| Intermediate | 10% | 25% |
| Expert | 15% | 40% |

## Lambda Delay Compensation

Engine exhaust takes time to reach the O2 sensor. AutoTune compensates:

- **At idle**: ~200ms delay (long runner path)
- **At redline**: ~50ms delay (fast exhaust flow)
- LibreTune interpolates between these values

## Best Practices

1. **Start conservative** with low authority limits
2. **Cover all cells** before applying changes
3. **Let data accumulate** - more hits = better accuracy
4. **Check heat map** for coverage gaps
5. **Validate changes** with a dyno or controlled test
