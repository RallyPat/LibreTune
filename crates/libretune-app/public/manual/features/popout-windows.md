# Pop-out Windows (Multi-Monitor Support)

LibreTune lets you pop any tab out into its own standalone window, making it easy to spread your workspace across multiple monitors.

## Overview

When tuning, you often want to see your dashboard on one screen while editing tables on another. Pop-out windows give you this flexibility — any tab can become its own independent window that stays synchronized with the main application.

## Popping Out a Tab

1. Open the tab you want to pop out (e.g., a table editor, data log, dashboard)
2. Click the **External Link** button (↗) in the tab bar
3. The tab moves to a new standalone window

The original tab is removed from the main window and appears in the new window with full functionality.

## Docking a Window Back

To return a popped-out tab to the main window:

1. Click the **Dock Back** button in the pop-out window's toolbar
2. The window closes and the tab reappears in the main window's tab bar

## What Stays in Sync

Pop-out windows maintain **bidirectional synchronization** with the main application:

| Data | Sync Behavior |
|------|---------------|
| **Realtime ECU data** | Gauges and live values update in all windows simultaneously |
| **Table edits** | Changes made in a popped-out table editor are reflected everywhere |
| **Connection status** | All windows show the same connected/disconnected state |

## Protected Tabs

The **Dashboard** tab is protected and cannot be popped out or accidentally closed. If you ever lose the Dashboard tab, recover it via **View → Dashboard**.

## Tips

- **Arrange freely**: Each pop-out window can be resized and positioned independently on any monitor
- **Window state**: Window positions and sizes are remembered between sessions via the window state plugin
- **Multiple pop-outs**: You can have several tabs popped out at once — for example, a dashboard on monitor 2 and a data log on monitor 3
- **Close = dock**: Closing a pop-out window (✕) docks the tab back to the main window rather than discarding it

## Use Cases

### Dual-Monitor Tuning Setup
- **Monitor 1**: Main LibreTune window with table editor and sidebar
- **Monitor 2**: Dashboard with live gauges popped out

### Triple-Monitor Racing Setup
- **Monitor 1**: Table editor
- **Monitor 2**: Dashboard with large RPM and AFR gauges
- **Monitor 3**: Data log playback or AutoTune view

### Laptop + External Monitor
- **Laptop screen**: Sidebar navigation and dialogs
- **External monitor**: Full-screen table editor or dashboard

## See Also

- [Dashboards](./dashboards.md) — Customize what appears on your dashboard
- [Table Editing](./table-editing.md) — Full table editor documentation
- [Data Logging](./datalog.md) — Log playback and analysis
