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
