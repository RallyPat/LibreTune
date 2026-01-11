# Restore Points

Quick backups for tune safety.

## What Are Restore Points?

Restore points are timestamped snapshots of your tune:
- Created manually or automatically
- Quick to create and restore
- Simpler than full version control

## Creating Restore Points

### Manual Creation
1. **File → Create Restore Point**
2. Point created instantly
3. Confirmation shown

### Automatic Creation
Restore points are auto-created:
- Before loading a different tune
- Before major operations
- Based on time interval (configurable)

## Viewing Restore Points

1. **File → Restore Points...**
2. List shows all restore points
3. Sorted by date (newest first)

### Restore Point Info
- **Filename**: Timestamped name
- **Date**: When created
- **Size**: File size

## Restoring a Point

1. Open Restore Points dialog
2. Select the desired point
3. Click **Load**
4. Confirm if unsaved changes exist
5. Tune restored

**Warning**: Current tune is replaced. Create a new restore point first if needed.

## Deleting Restore Points

1. Select restore point
2. Click **Delete**
3. Confirm deletion

### Bulk Delete
- Right-click → **Delete older than...**
- Choose timeframe
- Old points removed

## Automatic Cleanup

Configure max restore points in Settings:
- Default: 20 restore points
- Oldest deleted when limit reached
- Keeps most recent backups

## Restore Points vs Version Control

| Feature | Restore Points | Version Control |
|---------|---------------|-----------------|
| Setup | None needed | Initialize repo |
| History | Linear list | Full commit tree |
| Branching | No | Yes |
| Messages | Timestamps only | Custom messages |
| Comparison | No | Yes (diff) |
| Complexity | Simple | More powerful |

### When to Use Each

**Restore Points**: Quick backups, simple needs, no Git knowledge required.

**Version Control**: Detailed history, experiments, collaboration, professional workflow.

## File Location

Restore points are stored in:
```
[Project Folder]/restore-points/
├── tune_2026-01-11_14-30-00.msq
├── tune_2026-01-11_12-15-00.msq
└── ...
```

You can manually copy these files for external backup.
