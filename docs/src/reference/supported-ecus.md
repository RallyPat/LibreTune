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
