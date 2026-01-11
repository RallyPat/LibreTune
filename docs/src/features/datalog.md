# Data Logging

LibreTune can record real-time engine data for later analysis.

## Overview

Data logging captures engine parameters over time, allowing you to:
- Analyze runs after the fact
- Compare before/after tune changes
- Diagnose problems
- Share data with others

## Starting a Log

1. Connect to your ECU
2. Go to **Tools → Start Logging** or press `Ctrl+L`
3. Choose log settings:
   - **Log rate**: Samples per second (10-100 Hz)
   - **Channels**: Which parameters to log
4. Click **Start**

## Stopping a Log

Click **Stop Logging** or press `Ctrl+L` again.

Logs are automatically saved to your project's `logs/` folder.

## Log File Format

LibreTune uses CSV format:
```csv
Time,RPM,MAP,AFR,CLT,TPS,...
0.000,850,35.2,14.7,185,1.2,...
0.100,860,35.5,14.6,185,1.3,...
```

This format is compatible with:
- MegaLogViewer
- Excel/Google Sheets
- Custom analysis scripts

## Data Log Viewer

### Opening a Log

1. Go to **Tools → Data Log Viewer**
2. Click **Open Log**
3. Select a CSV file

### Playback Controls

| Control | Description |
|---------|-------------|
| ▶️ Play | Advance through log at real-time speed |
| ⏸️ Pause | Stop playback |
| ⏪⏩ Skip | Jump forward/backward |
| Slider | Scrub to any position |
| Speed | 0.5x, 1x, 2x playback speed |

### Synchronized Views

When playing back a log:
- Dashboards update with logged values
- Tables highlight the operating cell
- Graphs show the current position

## Automatic Logging

Configure automatic logging in Settings:
- **Auto-start on connect**: Begin logging when ECU connects
- **Max log duration**: Limit log file size
- **Auto-stop on disconnect**: End log when disconnected

## Log Analysis Tips

### Finding Problems
1. Open log in viewer
2. Look for AFR excursions
3. Check for sensor dropouts
4. Compare expected vs actual values

### Tune Validation
1. Log a baseline run
2. Make tune changes
3. Log the same run again
4. Compare the two logs

### Sharing Logs

Log CSV files can be:
- Emailed to tuners for review
- Opened in MegaLogViewer for detailed analysis
- Imported into spreadsheets for custom analysis

## Exporting Tune Data

Export your entire tune as CSV:
1. Go to **File → Export Tune as CSV**
2. Choose a filename
3. All constants are exported with metadata

## Importing Tune Data

Import tune values from CSV:
1. Go to **File → Import Tune from CSV**
2. Select a previously exported CSV
3. Values are validated and applied
