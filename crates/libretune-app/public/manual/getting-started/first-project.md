# Getting Started with Your First Project

LibreTune uses a **file-centric workflow** — you work with tune files directly, and projects are created automatically to organize your work. Each project contains:

- Your ECU definition (INI file)
- Your current tune (.msq)
- Backup restore points
- Version history (if enabled)

---

## The Welcome Screen

When you launch LibreTune without an open project, you see the **Welcome View** with three main actions and your recent project history.

### Main Actions

| Button | What It Does |
|--------|-------------|
| **Open Tune File** | Browse for an existing `.msq` or `.xml` tune file. A project is created automatically around it. This is the primary way to start working. |
| **Connect to ECU** | Opens the connection dialog to connect directly to your ECU via serial port. Useful when you want to read the tune live from the ECU. |
| **Import TS Project** | Import an existing TunerStudio project folder, including its tune, restore points, and settings. |

Below the main actions, there is a secondary link:

- **Generate Base Map from Engine Specs** — Opens the [Base Map Generator](../features/base-map.md) to create a safe, driveable starting tune from your engine's specifications (cylinder count, displacement, injector size, etc.). Use this when you're starting from scratch and don't have an existing tune file.

### Recent Projects

The bottom section lists your most recent projects (up to 8). Click any project to reopen it instantly.

Each recent project shows:
- **Project name**
- **ECU signature** (the INI definition it uses)
- **Last modified date**
- **Delete button** (✕) on the right side

#### Deleting a Project

To delete a project from the Welcome View:

1. Click the **✕** button on the right side of the project entry
2. The button changes to **✓ Confirm** — click it again to permanently delete
3. The project folder and all its contents (tune, restore points, history) are removed

> **Warning**: Project deletion is permanent. There is no undo. Make sure you have backed up any important tune files before deleting.

If you accidentally click ✕, simply click elsewhere or on a different project to cancel the deletion.

---

## Opening a Tune File

This is the most common way to start working. It works whether you're opening a tune you downloaded, one you got from another tuner, or one you exported previously.

### Step-by-Step

1. Click **Open Tune File** on the Welcome View, or go to **File → Open Tune File...** (Ctrl+N)
2. In the file browser, select your `.msq` or `.xml` tune file
3. LibreTune reads the file and shows a **preview panel**:

| Field | Description |
|-------|-------------|
| **Signature** | The ECU firmware signature embedded in the tune |
| **Constants** | Number of constants/settings in the file |
| **Author** | Who created the tune (if recorded) |
| **Size** | File size in KB |

4. LibreTune **automatically matches** the tune's signature to an ECU definition (INI file) from your local repository:
   - If **one match** is found, it is selected automatically
   - If **multiple matches** are found, pick the correct one from the dropdown
   - If **no match** is found, you can either:
     - Click **Import ECU Definition...** to add the correct INI file
     - Select manually from the full list under "── All definitions ──"

5. A **project name** is auto-generated from the tune's filename. Edit it if you want a different name.
6. Click **Open** to create the project and start tuning.

### What Happens Behind the Scenes

When you click Open:
- A project folder is created in your projects directory (`~/Documents/LibreTuneProjects/`)
- The tune file is copied into the project
- The selected ECU definition is linked
- The project appears in your Recent Projects list

### Supported File Formats

| Format | Extension | Description |
|--------|-----------|-------------|
| MegaSquirt XML | `.msq` | Standard ECU tune format used by Speeduino, rusEFI, MegaSquirt, and others |
| XML Tune | `.xml` | Generic XML tune files |

---

## Connecting Directly to an ECU

If you want to read the current tune directly from a connected ECU:

1. Click **Connect to ECU** on the Welcome View
2. In the Connection dialog:
   - Select your **serial port** (e.g., `/dev/ttyUSB0`, `COM3`)
   - Set the **baud rate** (typically 115200 for modern ECUs, 9600 for Speeduino)
3. Click **Connect**
4. LibreTune reads the ECU's signature and syncs all pages
5. If a matching project already exists, it reopens. Otherwise, you may need to create one using **Open Tune File** first.

For detailed connection instructions, see [Connecting to Your ECU](./connecting.md).

---

## Importing a TunerStudio Project

If you're migrating from TunerStudio:

1. Click **Import TS Project** on the Welcome View, or go to **File → Import TS Project...**
2. Select the TunerStudio project folder (usually in `~/TunerStudioProjects/` or `Documents\TunerStudioProjects\`)
3. A preview shows what will be imported:
   - Project name (from `project.properties`)
   - INI definition
   - Current tune
   - Number of restore points
4. Click **Import**
5. The project is created and opened automatically

For the full import reference, see [Importing Projects](../projects/importing.md).

---

## Generating a Base Map

If you're starting with a brand-new ECU and have no existing tune file, you can generate a safe starting point:

1. Click **Generate Base Map from Engine Specs** on the Welcome View, or go to **Tools → Generate Base Map...**
2. Enter your engine specifications:
   - Cylinder count, displacement, injector size
   - Fuel type, aspiration, injection/ignition mode
   - Idle RPM, redline RPM, boost target (if turbocharged)
3. Click **Generate Base Map**
4. Preview the generated tables (VE, Ignition, AFR, Enrichments)
5. Click **Use as Starting Tune** to create a project with this base map

For the complete guide, see [Base Map Generator](../features/base-map.md).

---

## Managing ECU Definitions (INI Files)

ECU definition files (INI files) tell LibreTune how to interpret your ECU's tune data. You need the correct INI matching your ECU's firmware version.

### Where INI Files Come From

- **Auto-downloaded** when LibreTune detects a signature mismatch and finds a match online
- **Imported manually** via **Settings → ECU Definitions** or during the Open Tune File flow
- **Bundled** with some TunerStudio project imports

### Managing Your INI Library

Open **Settings** (Ctrl+Comma) and switch to the **ECU Definitions** tab:

- **View** all imported INI files with their names and signatures
- **Import** new INI files by clicking the import button and browsing
- **Delete** unused INI files by clicking the ✕ button (click once to select, again to confirm)

For more details, see [Settings — ECU Definitions](./settings.md#ecu-definitions).

---

## Switching Between Projects

To switch to a different project:

1. Go to **File → Close Project**
2. The Welcome View appears, showing your recent projects
3. Click any project to reopen it, or start a new one

---

## Next Steps

With your project created, proceed to:
- [Connecting to Your ECU](./connecting.md) — Establish communication with your ECU
- [Table Editing](../features/table-editing.md) — Learn how to view and edit fuel and ignition tables
- [Base Map Generator](../features/base-map.md) — Create a starting tune from engine specs
- [AutoTune](../features/autotune.md) — Let the software optimize your VE table automatically
