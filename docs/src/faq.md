# Frequently Asked Questions

## General

### What ECUs does LibreTune support?

LibreTune works with any ECU that uses the standard INI definition format:
- Speeduino (all versions)
- rusEFI (all boards)
- EpicEFI (all versions)
- MegaSquirt MS2/MS3 (compatibility mode)

### Is LibreTune free?

Yes! LibreTune is open-source software licensed under GPL-2.0. You can use it for free, modify it, and distribute your modifications.

### Can I use my existing TunerStudio projects?

Yes. Use **File → Import TS Project** to import your TunerStudio project folder including tunes, restore points, and settings.

### Will LibreTune work with my existing INI files?

Yes. LibreTune uses the same INI format as TunerStudio. Your existing INI files should work without modification.

## Connection Issues

### My ECU doesn't appear in the port list

1. Check USB cable is connected
2. Verify ECU has power (LEDs on)
3. Install USB drivers (CH340 or FTDI)
4. Click **Refresh** in the connection dialog
5. Try a different USB port

### Connection times out

1. Verify the baud rate matches your ECU (usually 115200)
2. Check ECU is responsive (status LEDs blinking)
3. Try power cycling the ECU
4. Check for loose connections

### "Signature mismatch" error

Your INI file doesn't match your ECU firmware:
1. Download the correct INI for your firmware version
2. Or use LibreTune's online search to find a matching INI
3. Update your ECU firmware to match your INI

### Permission denied on Linux

Add your user to the dialout group:
```bash
sudo usermod -a -G dialout $USER
```
Then log out and back in.

## Table Editing

### How do I undo a change?

Press `Ctrl+Z` to undo. LibreTune maintains a full history of changes.

### What does "Set Equal" do?

Set Equal (`=` key) replaces all selected cells with their average value. Useful for flattening a region.

### What does "Smooth" do?

Smooth (`S` key) applies a Gaussian blur to selected cells, reducing abrupt transitions between neighboring values.

### How do I change the axis values?

Use the **Re-bin** feature:
1. Select the table
2. Click **Re-bin** in the toolbar
3. Enter new axis values
4. Z values are automatically interpolated

## AutoTune

### AutoTune isn't updating any cells

Check these filters:
- Engine temperature above minimum (usually 160°F)
- RPM within range
- TPS above minimum
- No rapid throttle changes

### Recommendations seem too aggressive

Lower the authority limits:
- Start with 5-10% max change
- Increase gradually as base tune improves

### Some cells never get data

Drive to cover those operating conditions:
- Steady state in each cell for several seconds
- Include cruise, light acceleration, and partial throttle

## Data Logging

### Where are log files saved?

Logs are saved in your project folder under `logs/`.

### What format are log files?

LibreTune uses CSV format compatible with MegaLogViewer and other analysis tools.

### How do I play back a log?

1. Go to **Tools → Data Log Viewer**
2. Click **Open Log**
3. Select a CSV file
4. Use playback controls to step through data

## Projects

### Where are projects stored?

By default: `~/Documents/LibreTuneProjects/`

You can change this in Settings.

### How do I back up my project?

Projects are standard folders. You can:
1. Use the built-in **Restore Points** feature
2. Enable **Git version control** for full history
3. Simply copy the project folder

### What's the difference between Save and Burn?

- **Save**: Writes tune to your project file on disk
- **Burn**: Writes tune to ECU's flash memory (permanent)

Always save before burning!

## Performance

### LibreTune is slow on my computer

Try these settings:
1. Disable 3D table visualization
2. Reduce gauge update rate
3. Disable gauge antialiasing
4. Close unused tabs

### Gauges aren't updating smoothly

1. Check ECU connection is stable
2. Reduce polling rate in Settings
3. Close other applications using serial ports
