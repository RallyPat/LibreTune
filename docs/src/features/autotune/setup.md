# Setting Up AutoTune

Detailed configuration guide for AutoTune.

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

**Auto-Discovered AFR Target Table** (Recommended)
- LibreTune reads the ECU's AFR target table and uses a per-cell target for
  each VE cell.
- Automatically adapts to different conditions (cruise vs. WOT, etc.).
- If a cell has no valid target, the fixed fallback value is used.

**Fixed Target**
- Uses a constant target value for the whole table.
- Simpler but less flexible; good for initial testing or ECUs without a target
  AFR table.

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
- **Min RPM**: Ignore idle data (default 1000)
- **Max RPM**: Ignore over-rev data (near redline, default 7000)

### Load (Y-Axis) Filter
- **Min Y Axis**: Ignore cells below this load (e.g., very low MAP)
- **Max Y Axis**: Ignore cells above this load (e.g., boost if not tuning boost)

Useful when you want to restrict tuning to a specific load range without
changing the table itself.

### Temperature Filter
- **Min CLT**: Ignore cold engine data (160°F / 70°C+)
- Ensures consistent fuel behavior

### Throttle Filter
- **Min TPS**: Ignore closed throttle (1%+)
- Helps avoid decel enleanment data

### Rate Filters
- **Max TPS Rate**: Ignore rapid throttle (10%/sec)
- Filters out transient conditions
- Prevents accel enrichment interference

### Custom Expression Filter
An optional `evalexpr` expression that must evaluate to true for a sample to be
accepted. Available variables:

`rpm`, `map`, `maf`, `load`, `afr`, `ve`, `clt`, `tps`, `tps_rate`, `accel_enrich`

Example: `rpm > 1200 && tps_rate < 5 && clt > 80`

## Authority Limits

### Maximum Change Per Cell
- **Max Increase**: Largest positive correction (e.g., 15%)
- **Max Decrease**: Largest negative correction (e.g., 15%)

### Cumulative Limit
- **Absolute Max**: Total change from baseline
- Prevents runaway corrections

### Starting Values

LibreTune enforces two authority limits together:

| Experience | Max % Change | Max VE Change |
|------------|--------------|---------------|
| Beginner | 5% | 5 VE |
| Intermediate | 10% | 10 VE |
| Expert | 15% | 15 VE |

The effective change is the more restrictive of the two limits.

## Lambda Delay Compensation

Engine exhaust takes time to reach the O2 sensor. AutoTune compensates:

- **At idle**: ~200ms delay (long runner path)
- **At redline**: ~50ms delay (fast exhaust flow)
- LibreTune interpolates between these values
- If your ECU provides a per-cell lambda delay table, LibreTune uses it
  automatically; otherwise the RPM-based curve is used.

### Strict Lambda Match

By default, samples with no historical match inside the delay window are
dropped. This prevents mis-attributing an AFR reading to the wrong VE cell.

Disable strict match only if you are willing to accept some inaccuracy during
transients to get more samples.

## Best Practices

1. **Start conservative** with low authority limits
2. **Cover all cells** before applying changes
3. **Let data accumulate** - more hits = better accuracy
4. **Check heat map** for coverage gaps
5. **Validate changes** with a dyno or controlled test
