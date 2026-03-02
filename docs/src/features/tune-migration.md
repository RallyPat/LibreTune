# Tune Migration & INI Version Tracking

LibreTune automatically tracks which ECU definition (INI file) was used when a tune was saved, and detects when the INI version has changed. This helps you safely migrate tunes between firmware versions.

## How It Works

Every time you save a tune, LibreTune records:
- The **INI signature** (firmware version identifier)
- A **structural hash** of all constant definitions (names, types, offsets, scales)
- A **constant manifest** listing every setting in the INI

When you later open that tune with a different INI file (e.g., after a firmware update), LibreTune compares the saved manifest against the current INI and generates a **migration report** showing exactly what changed.

## Migration Report Dialog

When a version mismatch is detected, the Migration Report Dialog appears automatically. It shows:

### Severity Levels

| Severity | Color | Meaning |
|----------|-------|---------|
| **None** | — | No changes detected, tune is fully compatible |
| **Low** | Blue | New constants added in the new INI version (your existing values are untouched) |
| **Medium** | Orange | Scale or offset changes — values may need adjustment to produce the same real-world result |
| **High** | Red | Type changes or removed constants — some settings may be lost or misinterpreted |

### Change Categories

The dialog groups changes into collapsible sections:

- **Type Changes** (Critical) — Constants whose data type changed (e.g., U08 → S16). These require careful review as values may be interpreted differently.
- **Scale Changes** (Warning) — Constants whose scale or offset factor changed. The raw value may produce a different real-world value.
- **Removed Constants** — Settings that existed in the old INI but are no longer present. These values will be lost.
- **New Constants** — Settings added in the new INI version. These will use default values.

Each section shows up to 20 items, with a "...and N more" indicator for larger lists.

### Actions

- **Dismiss**: Close the dialog and continue with the tune as-is
- **Continue with Tune**: Acknowledge the changes and proceed with editing

## When Migration Is Triggered

Migration detection occurs when:
1. You **open a tune** that was saved with a different INI version
2. You **switch the INI file** for an open project (via Settings or the Signature Mismatch dialog)
3. You **update your ECU firmware** and reconnect with a new INI

## Best Practices

### Before Updating Firmware

1. **Create a restore point** (File → Create Restore Point) to save your current tune
2. **Export as CSV** (File → Export Tune as CSV) for a human-readable backup
3. Update the firmware on your ECU
4. Download or import the new INI file matching the firmware version
5. Open your tune — the migration report will show what changed

### Reviewing Changes

- **Low severity**: Generally safe to proceed. New constants will use sensible defaults.
- **Medium severity**: Review scale changes carefully. A constant with a different scale factor means the same raw value now represents a different real-world value (e.g., a fuel enrichment multiplier that changed from 0.1 to 0.01 resolution).
- **High severity**: Review each type change individually. Consider whether the raw values are still valid. You may need to re-enter some settings.

### After Migration

1. Check critical tables (VE, ignition, AFR targets) to verify values look correct
2. Use **Table Comparison** (Tools → Table Comparison) to diff the migrated tune against a known-good baseline
3. Save the tune to update the INI metadata for future migrations

## Technical Details

- INI metadata is stored in the `.msq` tune file within `<iniMetadata>` and `<constantManifest>` XML sections
- The structural hash uses SHA-256 over all non-PC constant definitions
- Migration reports are generated in memory and not persisted — they are shown once each time a mismatch is detected
- The tune file format version was bumped from `1.0` to `1.1` to support INI metadata

## See Also

- [Managing Tunes](../projects/tunes.md) — Saving and loading tune files
- [Restore Points](../projects/restore-points.md) — Creating backups before changes
- [Settings & Preferences](../getting-started/settings.md) — Version control settings
- [Troubleshooting](../reference/troubleshooting.md) — Resolving signature mismatches
