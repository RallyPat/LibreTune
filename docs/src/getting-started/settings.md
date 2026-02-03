# Settings & Preferences

Configure LibreTune behavior, connection settings, and display preferences.

## Opening Settings

**File → Settings** (or **Ctrl+Comma**)

The Settings dialog includes multiple tabs for different options.

---

## Connection Settings

Configure how LibreTune communicates with your ECU.

### Serial Port

**Port Selection:**
- Dropdown list of available serial ports
- COM3, COM4 (Windows)
- /dev/ttyUSB0, /dev/ttyACM0 (Linux)
- /dev/ttyUSB0 (macOS)

**Baud Rate:**
- 9600 - Speeduino, older ECUs
- 115200 - Modern ECUs, rusEFI, FOME
- Custom rates available
- Default: Auto-detect from INI

**Auto-Connect:**
- Enable to connect on startup
- Requires project selection first
- Useful for dev/debugging

### Connection Behavior

**Timeout (seconds):**
- How long to wait for response
- Default: 5 seconds
- Longer on slower connections
- Shorter for responsiveness

**Reconnect on Error:**
- Automatic reconnection attempts
- Number of retries: 3-10
- Retry delay: 100-1000ms

**Keep Alive:**
- Periodic status checks
- Prevents connection dropout
- Default: Every 10 seconds

---

## Display Preferences

### Theme

**Dark Mode** (default)
- Professional dark UI
- Reduces eye strain
- Better for garage lighting

**Light Mode**
- Standard light UI
- Some users prefer for outdoor
- Less common for tuners

### Font Size

**Small**: 12px (more information density)
**Medium**: 14px (default, readable)
**Large**: 16px (accessibility option)

### Table Editor Grid

**Row Height:**
- Compact: 20px (fit more rows)
- Normal: 28px (default)
- Spacious: 36px (easier clicking)

**Column Width:**
- Auto: Fit content
- Fixed: 60px columns
- Custom: User-definable

**Highlight Active Cell:**
- Enable: Shows which cell selected
- Colors: Customize selection color
- Opacity: 50-100%

**Keyboard Navigation Sound:**
- Enable/disable click sound when moving cells
- Volume: Mute, quiet, normal, loud

### Status Bar

**Show Status Bar:**
- Display connection and channel info
- Bottom of window
- Can be toggled on/off

**Status Channels:**
- Selectable display channels
- Maximum 8 channels shown
- Drag to reorder

---

## Unit Preferences

### Temperature

- **Celsius** (default for most ECUs)
- **Fahrenheit** (US standard)
- **Kelvin** (physics/engineering)

All temperature displays convert automatically.

### Pressure

- **kPa** (SI unit, default)
- **PSI** (US/British)
- **bar** (metric atmosphere)
- **inHg** (inches mercury, aviation)

### Speed

- **km/h** (metric, default)
- **mph** (US/British)
- **m/s** (physics)
- **kt** (knots, aviation)

### Air-Fuel Ratio (AFR)

**Display Mode:**
- **AFR** (0.5 - 20.0, default)
- **Lambda** (0.03 - 1.35, physics)

**Fuel Type** (affects conversion):
- **Gasoline** (default AFR ~14.7)
- **E85** (default AFR ~9.6)
- **Methanol** (default AFR ~6.4)
- **Diesel** (default AFR ~14.5)
- **Custom**: User-defined ratio

Auto-conversion between AFR/Lambda based on selection.

---

## Version Control

Configure Git integration for tune versioning.

### Enable Version Control

**Auto-commit Settings:**
- **Never**: Manual commits only
- **Always**: Auto-commit after every save
- **Ask**: Dialog appears before saving

**Commit Message Template:**
```
{date} {time} - Tuned {table}
```

Available placeholders:
- `{date}` - YYYY-MM-DD
- `{time}` - HH:MM:SS
- `{table}` - Table name
- `{user}` - Username
- `{message}` - Custom message

### Branch Management

**Default Branch:**
- Usually `main` or `master`
- Can create new branches for experiments

**Auto-prune:**
- Remove old branches after X days
- Keep repository clean
- Default: 30 days

---

## AutoTune Settings

Default values for AutoTune parameters.

### Authority Limits

**Default Max Increase:** 10% (range: 1-50%)
**Default Max Decrease:** 10% (range: 1-50%)
**Default Absolute Max:** 25% (range: 5-100%)

These are starting values; adjustable per session.

### Filters

**Default RPM Range:**
- Min: 800 (idle)
- Max: 6500 (below redline)

**Default Temperature Filter:**
- Min CLT: 160°F / 70°C
- Min IAT: 40°F / 5°C

**Default Throttle:**
- Min TPS: 1% (above closed throttle)
- Max TPS Rate: 10%/sec

**Default Update Rate:** 100ms

---

## Dashboard Settings

### Default Dashboard

**On Startup:**
- Last used dashboard (default)
- Specific dashboard (named)
- None (empty)

**Designer Mode:**
- Show edit buttons by default
- Grid snap: 5px, 10px, 20px
- Show background grid
- Show bounding boxes

### Gauge Defaults

**Default Gauge Type:**
- Analog Gauge (classic dial)
- Digital Readout (numbers)
- Bar Gauge (progress bar)

**Font Size:**
- Small: 10px
- Medium: 14px (default)
- Large: 18px

**Value Precision:**
- Decimal places: 0-3 (default: 1)

---

## Logging Settings

### Data Logger

**Default Log Rate:** 10 Hz (10 samples/second)
- Range: 1 Hz to 100 Hz
- Higher rate = more detailed but larger files
- Lower rate = longer session duration

**Default Channels:**
- Select which channels log by default
- Can customize per log

**Log Location:**
- Save in project `/logs/` folder (default)
- Custom location available

**Auto-save:**
- Auto-save log when stopped (default: enabled)
- Filename format: `YYYY-MM-DD_HH-MM-SS.csv`

### File Retention

**Keep logs for:** 30 days (default)
- Auto-delete older files
- Can be disabled
- Affects storage usage

---

## Performance Calculator

### Vehicle Defaults

**Default Weight:** 3200 lbs
**Default Tire Diameter:** 25.5 inches
**Default Drag Coefficient:** 0.32

These appear as defaults when opening calculator.

### Engine Defaults

**Default AFR:** 14.7
**Default Boost:** 0 PSI
**Default Type:** Naturally Aspirated

---

## Advanced Settings

### Data Validation

**Warn on Out-of-Bounds:**
- Enable warnings when entering values outside normal range
- Prevents accidental bad data
- Can be overridden

**Warn on Large Changes:**
- Alert if cell change exceeds threshold
- Default: 20%
- Prevents typos

**Auto-normalize:**
- Clamp values to valid range automatically
- Keep data in safe range
- Can disable for power users

### Keyboard

**Key Repeat Rate:**
- Fast (short delay)
- Normal (medium delay, default)
- Slow (long delay)

**Selection Behavior:**
- Click once to select
- Double-click to edit
- Configurable

### Networking

**Check for Updates:**
- On startup (default)
- Manually
- Never (disable checking)

**Download Offline Docs:**
- Enable to keep manual available offline
- Required for some deployments

---

## Resetting to Defaults

**Reset All Settings:**
- Button at bottom of settings
- Restores all original values
- Warning: Cannot undo

**Reset Specific Section:**
- Reset only one category
- Other settings unchanged
- Use when something breaks

---

## Keyboard Shortcuts

Common settings-related shortcuts:
- `Ctrl+,` - Open Settings
- `Ctrl+Z` - Undo (in table editors)
- `Ctrl+Y` - Redo
- `Tab` - Next field
- `Shift+Tab` - Previous field

---

## Tips for Configuration

### First Time Setup

1. Set connection settings (port, baud rate)
2. Choose theme/font size preference
3. Set unit preferences (°C/°F, PSI/kPa)
4. Configure AutoTune defaults
5. Enable version control (recommended)

### Tuning Session Prep

1. Verify connection settings are correct
2. Set comfortable font/display size
3. Ensure units match vehicle specifications
4. Review AutoTune filter defaults
5. Check keyboard repeat rate

### Backup Settings

Settings are saved automatically in:
- Windows: `%APPDATA%\LibreTune\`
- macOS: `~/Library/Application Support/LibreTune/`
- Linux: `~/.local/share/LibreTune/`

Manual backup can be made by copying this folder.

---

## See Also

- [Getting Started](../getting-started/installation.md) - Initial setup
- [Connecting to Your ECU](../getting-started/connecting.md) - Connection guide
- [AutoTune Setup](./autotune/setup.md) - AutoTune configuration
- [Keyboard Shortcuts](../reference/shortcuts.md) - All hotkeys

