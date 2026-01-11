# Version Control

Track tune history with Git-based versioning.

## Overview

LibreTune can use Git to track changes to your tune:
- Full history of all changes
- Restore any previous version
- Branch for experiments
- Compare versions

## Enabling Version Control

### New Projects
Check **Enable version control** when creating a project.

### Existing Projects
1. Open **File → Tune History**
2. Click **Initialize Repository**
3. Git tracking begins

## How It Works

When enabled, LibreTune:
1. Creates a Git repository in your project folder
2. Commits changes when you save (based on settings)
3. Tracks all tune file modifications

## Auto-Commit Settings

Configure in **Settings → Version Control**:

| Setting | Behavior |
|---------|----------|
| **Never** | Manual commits only |
| **Always** | Commit on every save |
| **Ask** | Prompt each time |

### Commit Message Format

Customize the automatic commit message:
- `{date}` - Current date
- `{time}` - Current time
- `{table}` - Last edited table name

Example: `Tune saved on {date} at {time}`

## Viewing History

1. **File → Tune History** or click history icon
2. Timeline shows all commits
3. Click a commit to see details

### Commit Details
- Date and time
- Commit message
- Changed files

## Restoring Previous Versions

1. Open Tune History
2. Find the desired commit
3. Click **Checkout**
4. Confirm the restore
5. Your tune is now at that version

**Note**: Uncommitted changes will be lost. Save first!

## Comparing Versions

1. Select a commit in history
2. Click **View Diff**
3. See what changed between versions

### Diff View
- Shows file-by-file changes
- Highlights modified values
- Easy to identify what changed

## Branching

Create branches for experimental tunes:

### Create Branch
1. Open Tune History
2. Click **New Branch**
3. Enter branch name
4. Branch created from current point

### Switch Branch
1. Open branch selector dropdown
2. Choose branch
3. Tune switches to that branch's state

### Use Cases
- **Main**: Stable working tune
- **E85 experiment**: Testing E85 fuel
- **Track day**: Aggressive timing settings

## Manual Commits

Make a commit with custom message:
1. Open Tune History
2. Click **Commit**
3. Enter descriptive message
4. Commit created

### Good Commit Messages
- "Increased VE by 5% across the board"
- "Fixed rich spot at 3000 RPM / 60 kPa"
- "Pre-track day backup"

## Tips

### Commit Often
- Before major changes
- After successful runs
- When you have a "known good" state

### Use Branches
- Keep main branch stable
- Experiment on feature branches
- Merge when proven

### Write Good Messages
- Future you will thank you
- Describe what and why
- Reference issues or goals
