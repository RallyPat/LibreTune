# Supported ECUs

ECUs compatible with LibreTune.

## Officially Supported

These ECUs are tested and fully supported:

### Speeduino
All Speeduino firmware versions are supported:
- Speeduino 202xxx series
- All board variants (UA4C, NO2C, etc.)
- Standard 115200 baud

**Features**: Full table editing, AutoTune, data logging, all diagnostics.

### rusEFI
All rusEFI board variants:
- Proteus F4
- Prometheus
- MRE
- Hellen boards
- Custom boards

**Features**: Full support including advanced features.

### EpicEFI
All EpicEFI firmware versions:
- Standard epicECU boards
- 115200 baud communication

**Features**: Full table editing, AutoTune, diagnostics.

## Compatible ECUs

These ECUs use compatible INI format and should work:

### MegaSquirt
- **MS2**: Basic compatibility
- **MS3**: Basic compatibility
- **MS3 Pro**: Basic compatibility

**Note**: Some advanced MS features may not be fully supported.

### Other INI-Compatible ECUs
Any ECU using standard INI format should work for:
- Basic table editing
- Real-time data viewing
- Tune file management

## ECU Detection

LibreTune detects your ECU by:
1. Connecting to serial port
2. Querying ECU signature
3. Matching to loaded INI file

If signature doesn't match:
- Search for correct INI locally
- Search online repositories
- Continue with warning

## Getting INI Files

### Built-in Repository
LibreTune includes common INI files:
- Speeduino (multiple versions)
- rusEFI (multiple boards)
- epicEFI

### Online Search
Search GitHub repositories:
1. Speeduino official repo
2. rusEFI official repo
3. Auto-download matching INI

### Manual Import
1. **File → Import ECU Definition**
2. Select your INI file
3. Added to local repository

## Adding ECU Support

To add support for a new ECU:
1. Obtain the INI definition file
2. Import into LibreTune
3. Test connection and features
4. Report issues on GitHub

Most INI-compatible ECUs should work without modification.

## Troubleshooting

### "Unknown ECU signature"
1. Check you have the correct INI
2. Try online search for matching INI
3. Verify ECU firmware version

### "Communication error"
1. Check USB connection
2. Verify baud rate
3. Try different USB port
4. Check ECU power

### "Features not working"
Some ECU-specific features may require:
- Updated INI file
- LibreTune updates
- Feature requests on GitHub

## Trigger Patterns

### Understanding Trigger Pattern Support

**Important**: Trigger patterns (like "60-2", "36-1", "Nissan QG18") are implemented in the **ECU firmware**, not in LibreTune.

LibreTune reads available patterns from the INI file and presents them in a dropdown. The actual trigger decoding happens in real-time on the ECU's microcontroller.

### If Your Trigger Pattern Is Missing

If you need a trigger pattern that's not in the list:

1. **Check ECU Firmware**: Verify your firmware version supports the pattern
2. **Update Firmware**: If support was added recently, flash new firmware
3. **Request Feature**: Contact your ECU manufacturer:
   - Speeduino: https://speeduino.com/forum/
   - rusEFI: https://github.com/rusefi/rusefi
   - EpicEFI: Contact through their channels
4. **Import New INI**: Once firmware is updated, import matching INI to LibreTune

**Example**: For detailed information on requesting trigger pattern support, see [NISSAN_QG18_TRIGGER_SETUP.md](../../NISSAN_QG18_TRIGGER_SETUP.md).

### Temporary Solutions

While waiting for firmware support:
- Use "custom toothed wheel" with manual configuration
- Use a similar pattern if timing characteristics match
- Use a trigger converter/adapter board
