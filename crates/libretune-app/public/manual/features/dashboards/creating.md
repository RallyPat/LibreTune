# Creating Dashboards

Build custom dashboard layouts from templates or imported files.

## Starting a New Dashboard

1. Click **New** in the dashboard header
2. Enter a name
3. Pick a template:
   - **Basic Dashboard** — widescreen driver cluster
   - **Tuning Dashboard** — calibration-focused layout
   - **Telemetry Live** — dense Grafana-style live view
4. Click **Create** — the new dashboard opens immediately

New dashboard files are created in the app dashboards folder as `.ltdash.xml`. If the name is already taken, a numeric suffix is added automatically.

## Duplicating a Dashboard

Click **Duplicate** in the header to clone the current dashboard (including any unsaved edits) into a new "… (Copy)" file — a good starting point for variations.

## Importing TunerStudio Dashboards

1. Open the dashboard selector (**Change ▼**) and click **Import TS Dashboard Files...**
2. Select one or more `.dash` or `.gauge` files
3. Imported files are copied into the dashboards folder and listed under the *Legacy (TunerStudio)* category

Imports are byte-preserving file copies — LibreTune validates they parse but does not rewrite them.

## Resetting to Defaults

The dashboard selector's **Reset to Defaults...** action deletes **every** dashboard in the folder (including your custom ones) and recreates the three built-in defaults. A confirmation dialog spells this out before anything is deleted.

To restore a single default without touching anything else, delete just that dashboard's file — defaults are re-created automatically whenever missing.

## Renaming and Deleting

- **Rename** — header button, edits the current dashboard's file name (extension is preserved)
- **Delete** — header button, removes the current dashboard's file after confirmation; the selector falls back to the first remaining dashboard

## Exporting Dashboards

Click **Export** in the header to save the current dashboard (including unsaved edits) to any location — useful for sharing or backing up. The export dialog offers `.ltdash.xml`, `.dash`, and `.gauge` extensions.

## Managing Files

| Action | Where |
|--------|-------|
| New from template | Header → **New** |
| Duplicate | Header → **Duplicate** |
| Rename | Header → **Rename** |
| Delete | Header → **Delete** |
| Export | Header → **Export** |
| Import TunerStudio files | Selector → **Import TS Dashboard Files...** |
| Reset all to defaults | Selector → **Reset to Defaults...** |
