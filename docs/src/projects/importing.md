# Importing Projects

Bring in projects from TunerStudio and other sources.

## TunerStudio Project Import

### What Gets Imported
- **INI definition file**
- **Current tune** (CurrentTune.msq)
- **Restore points** (if present)
- **PC variables** (pcVariableValues.msq)
- **Connection settings** (port, baud rate)

### Import Process

1. **File → Import TS Project...**
2. Select your TunerStudio project folder
3. Preview shows what will be imported
4. Click **Import**
5. Project created in LibreTune

### Project Folder Location

TunerStudio projects are typically in:
- **Windows**: `Documents\TunerStudioProjects\`
- **macOS**: `Documents/TunerStudioProjects/`
- **Linux**: `~/TunerStudioProjects/`

## After Import

### Verify INI Match
LibreTune checks if the INI matches:
- ✅ **Match**: Ready to use
- ⚠️ **Mismatch**: May need different INI

### Check Connection
Default settings imported but verify:
1. Go to connection settings
2. Confirm port and baud rate
3. Test connection

## Importing Just a Tune

Import an MSQ file into existing project:
1. **File → Load Tune**
2. Select any MSQ file
3. Values loaded (if INI compatible)

## Importing an INI File

Add an INI to your local repository:
1. Open **Settings** (Ctrl+Comma) → **ECU Definitions** tab
2. Click **Import ECU Definition**
3. Select your INI file
4. The file is copied to the repository and available for project creation

You can also import INI files inline during the [Open Tune File](../getting-started/first-project.md#opening-a-tune-file) flow — if no matching INI is found, an "Import ECU Definition..." button appears directly in the dialog.

### Managing Your INI Library

To view or delete imported INI files, go to **Settings → ECU Definitions**. See [Settings — ECU Definitions](../getting-started/settings.md#ecu-definitions) for details.

## Importing Dashboards

Import TunerStudio dashboard layouts:
1. **File → Import Dashboard...**
2. Select `.dash` file
3. Dashboard converted to LibreTune format
4. Available in dashboard selector

## Troubleshooting Import

### "INI not found"
The project references an INI not in the folder:
1. Locate the correct INI file
2. Place in project folder
3. Or select different INI during import

### "Tune format mismatch"
MSQ doesn't match INI structure:
1. Get the correct INI for your firmware
2. Or the MSQ from the matching firmware
3. Signature must match

### "Missing restore points"
TunerStudio didn't create restore points:
1. This is normal for new projects
2. Create restore points in LibreTune going forward
