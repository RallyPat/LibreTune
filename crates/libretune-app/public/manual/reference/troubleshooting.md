# Troubleshooting

Common problems and solutions.

## Connection Issues

### ECU Not Detected

**Symptoms**: Port not in list, or "failed to open port"

**Solutions**:
1. Check USB cable connection
2. Verify ECU has power (LEDs on)
3. Install USB drivers (CH340/FTDI)
4. Try different USB port
5. On Linux: add user to dialout group
6. On Mac: check Security preferences

### Connection Timeout

**Symptoms**: "Connection timed out" or "No response"

**Solutions**:
1. Verify baud rate (usually 115200)
2. Check ECU is running (not in bootloader)
3. Power cycle the ECU
4. Try lower baud rate
5. Check for other apps using the port

### Signature Mismatch

**Symptoms**: "ECU signature doesn't match INI"

**Solutions**:
1. Download correct INI for firmware version
2. Use LibreTune's online INI search
3. Update ECU firmware to match INI
4. Continue anyway (advanced users)

### Communication Errors

**Symptoms**: Random disconnects, corrupted data

**Solutions**:
1. Check USB cable quality
2. Reduce cable length
3. Add ferrite cores
4. Check for electrical noise sources
5. Try different USB port

## Table Editing Issues

### Values Not Saving

**Symptoms**: Changes lost after restart

**Solutions**:
1. Press Ctrl+S to save tune file
2. Use Burn to ECU for permanent storage
3. Check project folder permissions
4. Verify disk space available

### Wrong Values Displayed

**Symptoms**: Numbers don't match expected

**Solutions**:
1. Check unit settings (metric/imperial)
2. Verify correct INI loaded
3. Sync with ECU (read from ECU)
4. Check for INI version mismatch

### Can't Edit Cells

**Symptoms**: Cells appear locked

**Solutions**:
1. Check if cells are locked in AutoTune
2. Verify table is editable (not read-only)
3. Check INI defines table as writable

## AutoTune Issues

### No Recommendations

**Symptoms**: All cells gray, no corrections shown

**Solutions**:
1. Check engine is at operating temp
2. Verify RPM is within filter range
3. Check TPS is above minimum
4. Confirm wideband is working
5. Review filter settings

### Erratic Recommendations

**Symptoms**: Values jumping around

**Solutions**:
1. Tighten TPS rate filter
2. Enable accel enrichment exclusion
3. Check wideband sensor health
4. Look for vacuum leaks

### Not Reaching Cells

**Symptoms**: Some cells never get data

**Solutions**:
1. Drive in those RPM/load conditions
2. Steady state required (no throttle changes)
3. Expand filter ranges slightly
4. May need dyno for some cells

## Dashboard Issues

### Gauges Not Updating

**Symptoms**: Values frozen or "--"

**Solutions**:
1. Check ECU connection
2. Verify channel names in INI
3. Restart real-time streaming
4. Check for JavaScript console errors

### Gauges Missing

**Symptoms**: Dashboard appears empty

**Solutions**:
1. Reload default dashboard
2. Check dashboard file exists
3. Create new dashboard
4. Import backup dashboard

## Performance Issues

### App Running Slowly

**Symptoms**: Lag, unresponsive UI

**Solutions**:
1. Disable 3D visualization
2. Reduce gauge update rate
3. Close unused tabs
4. Disable antialiasing
5. Check system resources

### High CPU Usage

**Symptoms**: Fan running, system hot

**Solutions**:
1. Reduce polling rate
2. Disable unused features
3. Check for runaway processes
4. Update graphics drivers

## Getting Help

If these solutions don't work:

1. Check [GitHub Issues](https://github.com/RallyPat/LibreTune/issues)
2. Search existing issues first
3. Create new issue with:
   - LibreTune version
   - Build ID (About → Build)
   - Operating system
   - ECU type and firmware
   - Steps to reproduce
   - Error messages/logs

**Build ID format**: `YYYY.MM.DD+g<short-sha>` (nightly build date plus git commit hash).
