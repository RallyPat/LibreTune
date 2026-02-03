# Diagnostic Loggers

LibreTune includes specialized logging tools for advanced ECU diagnostics and engine analysis.

## Tooth Logger

The Tooth Logger captures individual engine crank teeth timing for ignition and trigger analysis.

### Overview

Tooth logging records:
- Tooth angle timing
- Tooth-to-tooth interval
- Crank sensor signal quality
- RPM calculation accuracy
- Trigger signal integrity

### When to Use

- **Trigger setup**: Verify correct crank/cam sensor configuration
- **Ignition diagnostics**: Check for timing errors or drift
- **Signal quality**: Detect noisy or intermittent sensor signals
- **RPM accuracy**: Verify ECU RPM calculations against actual
- **Misfire diagnosis**: Identify timing issues related to misfires

### How to Use

1. **Open Tooth Logger**
   ```
   Tuning → Tooth Logger
   ```

2. **Configure Capture**
   - Select trigger type (360°, 720°, or custom)
   - Set capture duration (1-10 seconds)
   - Choose teeth to log (all or specific range)

3. **Start Capture**
   - Click **Start**
   - Engine should be running at steady RPM
   - Wait for data collection to complete

4. **Analyze Results**
   - View tooth timing graph
   - Check for regular intervals
   - Look for signal noise or dropouts
   - Verify RPM calculation

### Interpreting Results

#### Good Data
```
✓ Teeth evenly spaced
✓ Consistent tooth-to-tooth intervals
✓ No signal dropouts
✓ Clean rising/falling edges
✓ Calculated RPM matches actual
```

#### Bad Data
```
✗ Irregular tooth spacing
✗ Variable intervals
✗ Teeth missing or noise
✗ Noisy signal edges
✗ RPM jumps or fluctuates
```

### Common Issues

**Problem**: Teeth not detected
- Check crank sensor wiring
- Verify sensor air gap (usually 0.5-1.5mm)
- Test sensor resistance/voltage
- Check ECU trigger settings in INI

**Problem**: Noisy tooth signal
- Rotate crank sensor wires away from ignition wires
- Check for shielding breaks or cracks
- Test sensor power supply voltage
- Verify ECU signal input settings

**Problem**: Missing teeth
- Physical damage to crank tone ring
- ECU might be filtering noise
- Wiring connection loose or corroded
- Sensor air gap too large

---

## Composite Logger

The Composite Logger captures multiple engine signals simultaneously for complete engine behavior analysis.

### Overview

Composite logging records:
- Primary and secondary trigger signals
- Synchronization status
- Voltage levels
- Signal timing relationships
- Duty cycle information

### When to Use

- **Sync diagnosis**: Verify primary/secondary signal relationship
- **Cam timing**: Check cam sensor vs crank alignment
- **Multi-channel**: Analyze multiple trigger sources together
- **Dual-inject**: Verify alternating injector timing
- **Turbo control**: Monitor boost and wastegate signals

### How to Use

1. **Open Composite Logger**
   ```
   Tuning → Composite Logger
   ```

2. **Select Channels**
   - Primary trigger
   - Secondary trigger (cam)
   - Voltage input (optional)
   - Duty cycle signal (optional)

3. **Configure Recording**
   - Duration (5-30 seconds recommended)
   - Sample rate (500 Hz - 10 kHz)
   - Trigger on: Start of data, specific event

4. **Capture Data**
   - Click **Start**
   - Let data record
   - Vary engine conditions if desired
   - Stop recording

5. **Analyze**
   - View waveforms overlaid
   - Check signal synchronization
   - Measure timing relationships
   - Export for further analysis

### Understanding Signals

#### Trigger Signals
- **Primary (Crank)**: Base timing reference, fires for every tooth
- **Secondary (Cam)**: Synchronization signal, fires once per engine cycle
- **Relationship**: Secondary should occur at specific crank angle

#### Voltage Signals
- **0V baseline**: Logical low (ground)
- **5V nominal**: Logical high (signal present)
- **Intermediate**: Analog voltage (pressure sensors, AFR, etc.)

#### Duty Cycle
- **0%**: Always off
- **50%**: Half on, half off (injector pulse width example)
- **100%**: Always on

### Interpreting Results

#### Good Synchronization
```
✓ Primary signal regular teeth
✓ Secondary signal at expected location
✓ Voltage stable at logic levels
✓ Duty cycle proportional to engine load
```

#### Sync Issues
```
✗ Secondary not synchronized
✗ Cam signal leads/lags crank too much
✗ No secondary signal detected
✗ Intermittent loss of sync
```

### Common Problems

**No sync (red icon)**
- Cam sensor not installed
- Cam sensor wiring loose
- Cam sensor failed
- ECU settings don't match setup
- Cam timing physically wrong

**Sync drifts**
- Cam belt/chain worn (timing slip)
- Cam sensor signal weak
- EMI interference on wiring
- ECU sync window too narrow

**Noisy signal**
- Check shielding on cables
- Verify sensor power supply
- Test sensor air gap
- Check ground connections

---

## Data Logger

While primarily accessed from **View → Data Logger**, you can also access diagnostic logs here.

See [Data Logging](./datalog.md) for full documentation.

---

## Exporting Logger Data

All logger data can be exported for external analysis:

**Formats supported:**
- CSV (Excel/LibreOffice compatible)
- TXT (raw text)
- PNG (chart screenshots)

**Steps:**
1. Complete capture
2. Click **Export**
3. Choose format and location
4. File saved with timestamp

---

## Advanced Logging Tips

### Best Practices

1. **Warm up engine**: Let ECU reach operating temperature
2. **Steady state**: Keep RPM constant during capture
3. **Multiple captures**: Do several runs to verify consistency
4. **Document baseline**: Save good data before changes
5. **Compare**: Use before/after captures to verify tuning

### Troubleshooting Workflow

1. **Start simple**: Log just primary trigger
2. **Add complexity**: Enable secondary signal
3. **Vary conditions**: Capture at idle, 3000 RPM, 5000 RPM
4. **Document problems**: Note any anomalies seen
5. **Test fixes**: Re-capture after making changes

### Integration with Other Tools

- **Tooth Logger** + **AutoTune**: Verify timing stability during tuning
- **Composite Logger** + **Dashboard**: Monitor signals while watching gauges
- **Data Logger** + **Performance**: Correlate sensor signals with calculated performance

---

## See Also

- [Data Logging](./datalog.md) - General data capture and analysis
- [Troubleshooting](../reference/troubleshooting.md) - Common sensor issues
- [Supporting ECUs](../reference/supported-ecus.md) - ECU-specific logging capabilities

