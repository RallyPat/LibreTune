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

### Gauges Stall / Invalid Values / Slow Data

**Symptoms**: Gauges freeze or show no changes; Rx rate is very low in Auto mode
(e.g. ~13 B/sec where Force Burst gives ~1 KB/sec); the Disconnect button does
nothing; opening a table stays on "Loading…"; the line graph stops scrolling
until the value changes.

**Solutions**:
1. Set **Runtime Packet Mode** to **Force Burst** (Settings dialog). For
   Speeduino / MegaSquirt (MS2/MS3), Burst is the high-throughput realtime path;
   Auto mode now selects it automatically, but forcing it rules out any
   mis-detection.
2. If gauges still stall, click **Disconnect** and reconnect. Disconnect now
   aborts in-flight serial reads instead of hanging.
3. Verify the loaded INI matches your firmware version (a signature mismatch can
   mis-size the realtime block).
4. Check the baud rate matches your ECU (Speeduino / rusEFI / epicEFI / MS2 / MS3
   all default to 115200).
5. For Bluetooth or low-baud links, **Force OCH** may be more efficient on
   rusEFI-family ECUs — but avoid it on Speeduino/MS unless you know your firmware
   supports it.

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

### VE Table Flickers / Twitches

**Symptoms**: The VE table grid visibly jitters a few times per second;
horizontal/vertical scrollbars flash on and off around the table; resizing
the window or the panel makes it stop or get worse.

**Cause**: The VE table is the only table rendered in "fit viewport" mode
(it sizes its cells to fill the available panel space). A measure → overflow →
scrollbar → re-measure feedback loop caused the grid to overshoot its host
by ~2px (the grid container's border), which toggled the scrollbars on and
off every animation frame.

**Solutions**:
1. This is fixed in current builds — the fit math now accounts for the grid
   border, and the host reserves a stable scrollbar gutter so `clientWidth`/
   `clientHeight` can't change when scrollbars toggle.
2. If you still see it on an older build, update to the latest nightly.
3. If it recurs on a *non*-VE table (which use fixed cell sizing), it is a
   different issue — please report it with the table name.

## AutoTune Issues

### No Recommendations

**Symptoms**: All cells gray, no corrections shown

**Solutions**:
1. Check engine is at operating temp
2. Verify RPM is within filter range
3. Check TPS is above minimum
4. Confirm wideband is working
5. Review filter settings

### Recommendations Never Accumulate

**Symptoms**: AutoTune runs and data flows, but hit counts stay at zero or
climb far slower than the session length suggests.

**Solutions**:
1. This was a fixed bug — on real hardware the delayed-sample match window
   was too tight for the stream's actual cadence and silently dropped a
   third to a half of all samples. Update to a current nightly.
2. Samples taken during **overrun fuel cut** (lift-off) and **railed wideband
   readings** (below ~10 or above ~19.5 AFR — sensor at a stop, unpowered
   heater, or a cut still washing out) are excluded by design; each rejection
   is named in the diagnostic log, so check which filter is dropping your
   samples.
3. If the wideband reads railed while the engine runs, fix the sensor/heater
   first — those readings carry no mixture information.

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

## AppImage Issues (Linux)

AppImages bundle the application with necessary libraries for maximum compatibility. However, on some modern Linux systems (especially Arch-based distributions like CachyOS), bundled graphics libraries may conflict with system drivers.

### AppImage Crashes or Freezes on Wayland

**Symptoms**: AppImage window appears but is completely blank, or app crashes immediately with graphics errors

**Environment**: Arch-based systems (CachyOS, Manjaro) running Wayland display server with Intel/AMD integrated graphics (Mesa drivers)

**Root Causes**:
1. Bundled Wayland/EGL libraries (`libwayland-*.so`, `libepoxy.so`) conflict with system Mesa drivers
2. WebKit subprocess library paths don't match packaged file structure
3. ICU libraries bundled in AppImage but not on library search path

**Automatic Fix**:
The LibreTune AppImage includes an automatic runtime fix that:
- Detects Wayland display server
- Removes conflicting graphics libraries to use system versions
- Creates symlinks for WebKit subprocess library discovery
- Configures library search paths for bundled ICU

This should resolve the issue automatically on most systems.

**Manual Workaround** (if automatic fix fails):

1. Extract the AppImage:
```bash
./libretune-*.AppImage --appimage-extract
cd squashfs-root
```

2. Remove conflicting graphics libraries:
```bash
rm -f usr/lib/libwayland-egl.so.1
rm -f usr/lib/libwayland-client.so.0
rm -f usr/lib/libwayland-server.so.0
rm -f usr/lib/libwayland-cursor.so.0
rm -f usr/lib/libepoxy.so.0
```

3. Create library path symlink for WebKit:
```bash
mkdir -p lib
ln -s ../usr/lib/x86_64-linux-gnu lib/x86_64-linux-gnu
```

4. Launch with library path configured:
```bash
LD_LIBRARY_PATH=./usr/lib:$LD_LIBRARY_PATH ./usr/bin/libretune-app
```

**Non-Critical Warnings**:
The following warnings may appear on launch but do not affect functionality:
- `Fontconfig warning: using without calling FcInit()`
- `Failed to load module "colorreload-gtk-module"`
- `Failed to load module "window-decorations-gtk-module"`

These are harmless and can be safely ignored.

**Prevention**:
- Keep Mesa drivers updated: `sudo pacman -S mesa` (Arch)
- Ensure Wayland session is properly configured
- Use the automatic bundled fix (no manual steps needed)

**Getting Help**:
If the AppImage still fails after these steps:
1. Check your display server: `echo $WAYLAND_DISPLAY` (should be non-empty for Wayland)
2. Verify GPU drivers: `glxinfo | grep "OpenGL version"`
3. Report issue with system information and error messages

## Debug Logging

LibreTune prints backend log messages to the terminal where it was started. By
default the log level is `info`, so verbose protocol debug output is hidden.

To see detailed ECU communication logs while troubleshooting a connection:

```bash
RUST_LOG=libretune_core::protocol=debug ./libretune
```

To see plugin debug logs:

```bash
RUST_LOG=libretune_core::plugin_system=debug ./libretune
```

To quiet all non-error output:

```bash
RUST_LOG=error ./libretune
```

Multiple filters can be combined:

```bash
RUST_LOG=libretune_core::protocol=debug,libretune_core::plugin_system=warn ./libretune
```

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
