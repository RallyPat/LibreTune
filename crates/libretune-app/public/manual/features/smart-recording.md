# Smart Recording Guide

Smart Recording is an automatic data logging feature that starts and stops recording based on ECU key-on/off events, eliminating the need for manual logging control during extended testing sessions.

## Overview

Smart Recording monitors ECU RPM and automatically:
- **Starts logging** when the key turns on (RPM exceeds threshold)
- **Stops logging** when the key turns off (RPM below threshold for configured timeout)
- **Prevents false triggers** with configurable debouncing timeouts

This is ideal for:
- Long diagnostic sessions where manual start/stop is impractical
- Capturing complete test cycles automatically
- Reducing operator workload during extensive tuning
- Ensuring consistent logging for AFR analysis and AutoTune input

## Enabling Auto-Record

### Via Settings Dialog

1. **Open Settings** → File menu → Settings (or press Ctrl+,)
2. **Navigate to Data Logging section** at the bottom of the settings dialog
3. **Enable auto-record**:
   - Check the "Enable auto-record" checkbox
   - Configure threshold and timeout values (see Configuration section below)
4. **Apply** → Click the Apply button to save

### Quick Toggle in Data Logger

When you open the Data Logger view:
- **Live Mode**: Auto-record toggle appears in the recording controls
- Click the checkbox to enable/disable without opening full settings
- Current key state displays as `[on]` or `[off]` when enabled

## Configuration

### Key-On Threshold (50-500 RPM)

**What it does**: Sets the RPM level that triggers "key-on" detection

**Default**: 100 RPM

**How to set it**:
- **Idle RPM < 500**: Use 75-100 RPM (ensures quick detection of cranking)
- **Idle RPM 500-1000**: Use 100-200 RPM (allows stable idle before logging)
- **Idle RPM > 1000**: Use 150-300 RPM (prevents logging during cranking)

**Real-world examples**:
- **Stock vehicle (750 RPM idle)**: Set to 150 RPM
- **High-idle turbo (1200 RPM)**: Set to 200 RPM
- **Race motor with drop-rev limiter**: Set to 500 RPM (let it stabilize first)

### Key-Off Timeout (1-10 seconds)

**What it does**: Waiting period after RPM drops below threshold before stopping recording

**Default**: 2 seconds

**Why it matters**: Prevents repeated stop/start if RPM dips briefly (downshifting, throttle off)

**How to set it**:
- **Street driving**: 1-2 seconds (responsive, minimal false stops)
- **Highway cruising**: 3-5 seconds (ignores brief dips)
- **Track work with engine braking**: 5-10 seconds (capture entire braking event)

**Real-world examples**:
- **Normal commute**: 1.5 seconds (catches most stops)
- **Spirited driving**: 3 seconds (overlooks throttle blips)
- **Drag racing**: 10 seconds (entire run from launch to coast)

## Workflow: Typical Auto-Record Session

### Setup (One-time)
1. Open Settings and enable auto-record
2. Set key-on threshold to your vehicle's idle RPM ± 50
3. Set key-off timeout based on your driving style
4. Click Apply and close Settings

### Recording Session
1. **Open Data Logger** (View → Data Logger)
2. **Enable auto-record toggle** if not already enabled
3. **Set sample rate** to desired frequency (10 Hz = 100ms intervals recommended)
4. **Insert key** (or simulate in demo mode):
   - Status shows `[on]` in the toggle
   - Logging **automatically starts**
5. **Drive your test cycle** normally
6. **Remove key** (or stop simulation):
   - RPM drops and stays below threshold for configured timeout
   - Status shows `[off]` in the toggle
   - Logging **automatically stops**
7. **Review data** in playback mode

### Analysis
- Toggle view mode to "Playback" to review recorded data
- Open Statistics panel (click Stats button) to analyze:
  - Mean/median AFR and other channels
  - Standard deviation (data consistency)
  - Percentiles (identify outliers)
- Use the multi-channel statistics table to compare channels across your test cycle

## Real-World Scenarios

### Scenario 1: NA Carburetor Tuning

**Goal**: Capture complete acceleration run for AFR analysis

**Setup**:
- Key-On Threshold: **150 RPM** (carbs idle at ~1000, start logging at 1150)
- Key-Off Timeout: **2 seconds** (stop logging 2 sec after decel)
- Sample Rate: **10 Hz** (plenty of resolution for carburetor changes)

**Execution**:
1. Start car, let it idle
2. Perform wide-open throttle run from idle to redline
3. Coast back to idle
4. Turn off key
5. Auto-record captures entire run → stops automatically

**Benefit**: No manual start/stop, consistent data from 100% of the test cycle

### Scenario 2: Turbo EFI Development

**Goal**: Monitor boost target accuracy and AFR stability during boost ramp

**Setup**:
- Key-On Threshold: **200 RPM** (prevents spooling false triggers)
- Key-Off Timeout: **5 seconds** (boost spool might cause brief RPM dip)
- Sample Rate: **20 Hz** (captures transient boost response)

**Execution**:
1. Start engine, reach operating temperature
2. Gradually spool turbo with increasing throttle
3. Hold steady boost for 10 seconds
4. Back off throttle slowly to cool down
5. Turn off key
6. Auto-record captures boost ramp with AFR response

**Benefit**: Automated logging of precise boost transients; easy to repeat tests

### Scenario 3: Highway Cruise & Load Point Testing

**Goal**: Capture steady-state data at multiple load points for VE table refinement

**Setup**:
- Key-On Threshold: **100 RPM** (responsive)
- Key-Off Timeout: **3 seconds** (smooth highway driving, occasional blips)
- Sample Rate: **5 Hz** (steady-state doesn't need high frequency)

**Execution**:
1. Drive to highway
2. Cruise at 40 mph → auto-record starts
3. Hold 5 minutes at constant throttle (captures load point #1)
4. Accelerate to 60 mph → continue recording
5. Hold 5 minutes (captures load point #2)
6. Accelerate to 80 mph → continue recording
7. Hold 5 minutes (captures load point #3)
8. Exit highway, coast down, turn off key → auto-record stops

**Result**: Single drive captures 3 complete load points with minimal operator intervention

**Benefit**: Easy to build VE table data; one long session replaces multiple manual recordings

### Scenario 4: Diagnostic Logging with Tooth Logger

**Goal**: Capture ignition timing issues during specific driving condition

**Setup**:
- Key-On Threshold: **100 RPM**
- Key-Off Timeout: **2 seconds**
- Sample Rate: **10 Hz** for AFR, enable Tooth Logger simultaneously

**Execution**:
1. Enable Tooth Logger (Tuning → Tooth Logger)
2. Start Tooth Logger capture
3. Open Data Logger with auto-record enabled
4. Start engine (both loggers auto-start)
5. Perform the driving condition that exhibits the issue
6. Turn off key (both loggers auto-stop)

**Result**: Synchronized AFR + timing data shows exact ignition vs fuel relationship

**Benefit**: Detailed diagnosis without manual logger management

## Troubleshooting

### Auto-Record Not Starting

**Problem**: Key is on but logging doesn't start

**Check**:
1. Is auto-record toggle **checked** in Data Logger? (must be enabled)
2. Is view mode set to **"Live"**? (auto-record only works in live mode)
3. Is ECU connected and streaming RPM data? (check status bar)
4. Is sample rate set? (defaults to 10 Hz if blank)

**Fix**:
- Verify ECU connection with live RPM on status bar
- Check Data Logger is in Live mode (not Playback)
- Toggle auto-record checkbox off and back on
- Increase RPM above threshold manually (press throttle in demo mode)

### Auto-Record Stops Unexpectedly

**Problem**: Recording stops before key is actually off

**Possible causes**:
1. Key-off timeout is too short
2. RPM dips below threshold during driving (downshifts, coasting)
3. ECU data timeout (network disconnect)

**Fix**:
- Increase Key-Off Timeout to 3-5 seconds (more forgiving)
- Increase Key-On Threshold slightly if dips are frequent
- Check ECU connection stability (wireless interference?)
- If developing, use longer timeout during testing (5-10 sec)

### Too Much Unwanted Data at Start/End

**Problem**: Recording includes cranking noise or idle before throttle application

**Cause**: Key-on threshold is too low, logging starts too early

**Fix**:
- Increase Key-On Threshold to your idle RPM + 50 (e.g., idle 1000 → threshold 1050)
- Or manually clear start/end data after recording (slice in playback mode if available)
- For production use, key-on threshold should match idle + ~10%

### Settings Not Persisting

**Problem**: Changed auto-record settings but they reset

**Cause**: Settings dialog not properly saved

**Fix**:
1. Open Settings (File → Settings)
2. Verify Data Logging section shows your values
3. Click **Apply** button (not just close dialog)
4. Restart app to verify persistence

## Best Practices

### General Tips
- **Test threshold values** with your vehicle before critical logging sessions
- **Set conservative timeout** (3-5 sec) if you frequently shift or brake hard
- **Use live preview** in Data Logger to verify auto-start/stop is working before real testing
- **Monitor status indicator** during testing to confirm logging is active

### For Tuning
- **Create separate logs** for different conditions (cruise, acceleration, deceleration)
- **Run multiple cycles** of the same test to ensure consistency
- **Save logs regularly** (File → Export) as backup before AutoTune analysis
- **Label logs by condition** (e.g., "WOT_3000-6000_boost" or "Idle_closed_loop_5min")

### Performance Optimization
- **Use 5-10 Hz sample rate** for steady-state logging (cruise, idle)
- **Use 20 Hz** for transient analysis (acceleration, boost ramp)
- **Disable channels** you don't need (reduces file size, improves responsiveness)
- **Rotate old logs** to external storage (keep app data folder clean)

### Safety Considerations
- **Always enable auto-record during unmanned testing** (dyno, vehicle bench)
- **Test auto-record function** on safe roads before relying on it for critical work
- **Keep eyes on road** - auto-record enables more attentive driving
- **Verify key-off detection** with test ignition cycles before important sessions

## Integration with Other Features

### With AutoTune
- Run auto-record during baseline run to capture raw AFR data
- Export logs as CSV (File → Export Log as CSV)
- Use logs as input to AutoTune analysis → apply recommendations
- Re-record after changes to verify improvements

### With Statistics Panel
- Auto-record longer test cycle
- Open Statistics panel (click Stats button in Data Logger)
- Review mean/median AFR, std dev, percentiles across test
- Identify data outliers and driving anomalies

### With Tooth Logger
- Enable Tooth Logger first (Tuning → Tooth Logger)
- Start Tooth Logger capture
- Open Data Logger with auto-record
- Both start automatically when key turns on
- Synchronized AFR + timing data shows tuning effectiveness

## See Also
- [Data Logging](datalog.md) - Data logging overview
- [Data Statistics](data-statistics.md) - Statistical analysis workflows
- [Diagnostic Loggers](diagnostic-loggers.md) - Tooth and Composite logging
- [AutoTune Usage](autotune/usage-guide.md) - Using logs for AutoTune input
