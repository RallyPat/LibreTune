# Creating Your First Project

LibreTune organizes your work into **projects**. Each project contains:

- Your ECU definition (INI file)
- Your current tune
- Backup restore points
- Version history (if enabled)

## Starting a New Project

1. Launch LibreTune
2. Click **New Project** on the welcome screen, or go to **File → New Project**
3. Choose a project template or start from scratch

### Using a Template

LibreTune includes built-in templates for common ECU configurations:

| Template | Description |
|----------|-------------|
| **Speeduino 4-cyl NA** | Naturally aspirated 4-cylinder gasoline engine |
| **rusEFI Proteus F4** | Proteus F4 development board |
| **epicEFI Standard** | Standard epicEFI configuration |

Select a template to pre-configure your project with appropriate defaults.

### Starting from Scratch

If your ECU isn't in the templates:

1. Select **Start from scratch**
2. Browse to your ECU's INI definition file
3. Enter a project name

## Project Settings

After creating a project, you can configure:

- **Connection settings**: Serial port and baud rate
- **Auto-save options**: How often to save your work
- **Version control**: Enable Git-based tune history

## Loading an Existing Tune

If you have an existing MSQ tune file:

1. Go to **File → Load Tune**
2. Browse to your `.msq` file
3. The tune values will be loaded into the project

## Importing from TunerStudio

Have an existing TunerStudio project? You can import it:

1. Go to **File → Import TS Project**
2. Select your TunerStudio project folder
3. LibreTune will import:
   - Your INI definition
   - Current tune (CurrentTune.msq)
   - Restore points
   - Project settings

## Next Steps

With your project created, proceed to [Connecting to Your ECU](./connecting.md) to establish communication.
