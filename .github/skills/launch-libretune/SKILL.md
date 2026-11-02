---
name: launch-libretune
description: 'Launch the LibreTune desktop app for manual testing. Use for: running the app, starting tauri dev, testing UI changes, testing with demo mode / simulated ECU without hardware, enabling backend debug logs (RUST_LOG), or resetting app state between test runs.'
argument-hint: '[demo|vite-only|logs|reset]'
user-invocable: true
---

# Launch LibreTune for Testing

## When to Use

- Verify frontend/backend changes in the running app (tables, AutoTune, dashboards, dialogs)
- Test live-data features without an ECU attached (trail lines, current-cell highlight, gauges, AutoTune)
- Reproduce a bug the user reported in the running app
- Capture backend logs for a failing feature

## Choose the Launch Mode

| Mode | Command | Use when |
|------|---------|----------|
| **Full app + demo ECU** (default) | `npm run tauri dev`, then enable Demo Mode | Testing anything involving live data, the ECU protocol, or backend commands |
| **Full app, real ECU** | `npm run tauri dev`, connect via the Connect dialog | Testing against physical hardware (only when the user has an ECU connected) |
| **Frontend only** | `npm run dev` | Pure UI/CSS/layout work — no Tauri backend, so every `invoke()` fails; dashboards and tables render empty |

All commands run from `crates/libretune-app/`, NOT the repo root.

## Procedure

### 0. Fast pre-checks (only if code changed since the last green run)

```
cd crates/libretune-app
npx tsc --noEmit                     # ~5 s
npx vitest run src/<changed-area>    # targeted, not the whole suite
```

Rust changes also need `cargo check -p libretune-app` (~10 s warm). Skip the
full suites here — the point is to avoid launching a 30 s build that dies on
an obvious error, not to re-run CI.

### 1. Launch the app

Run in a terminal (background/async mode — it blocks until the window closes):

```
cd crates/libretune-app
npm run tauri dev
```

- First build after a clone or `cargo clean` takes minutes; warm starts are
  seconds. Watch the terminal: Rust compile errors appear there, not in the
  app window.
- The window opens automatically when the build finishes. Vite hot-reloads
  frontend edits instantly; Rust edits trigger a rebuild + app restart.
- Windows note: `scripts/tauri-dev.sh` exists for Linux (Snap environment
  sanitization) — on Windows just use `npm run tauri dev` directly.

### 2. Enable demo mode (hardware-free live data)

In the app: open the **Settings** tab → **Demo Mode (Simulated ECU)** →
**Enable**. This loads the bundled demo INI (epicEFI) and starts a simulated
engine: ~850 RPM idle with occasional throttle blips, plus a realtime stream
at 50 ms.

What this gives you to test, without any hardware:

- **Dashboards/gauges** — live values (RPM sweeps with the throttle blips)
- **Table editors + AutoTune** — open a fuel table: the current-cell
  highlight and trace line follow the simulated operating point; in AutoTune,
  Start a session and samples accumulate against the simulated stream
- **Realtime charts/status bar** — any channel-driven UI

Demo mode replaces any open project/connection with the demo state — it does
NOT touch a physically connected ECU, but it does clear the current project,
so don't enable it while the user has unsaved work open in the app.

### 3. Backend logs

`npm run tauri dev` already prints backend `tracing` output to the terminal.
Filter with `RUST_LOG` (PowerShell):

```powershell
$env:RUST_LOG = "libretune_core::protocol=debug"; npm run tauri dev     # serial/ECU traffic
$env:RUST_LOG = "error"; npm run tauri dev                              # quiet
```

Frontend `console.log` from the webview also surfaces in that terminal.
Remove the env var afterwards (`Remove-Item Env:RUST_LOG`) or it persists in
that shell.

### 4. Resetting state between runs

When a test depends on clean settings/tunes (e.g. verifying a settings
default), the app reads/writes:

| What | Windows path |
|------|--------------|
| App data (settings.json, definitions/, dashboards/) | `%APPDATA%\LibreTune\` |
| Projects / tunes | `%USERPROFILE%\Documents\LibreTuneProjects\` |

Close the app before deleting anything there. Rename rather than delete when
the user's own projects might be present — never delete
`Documents\LibreTuneProjects` wholesale.

## Testing the Current Branch (AutoTune follow-ups, issue #132)

The branch `feat/issue132-autotune-yflip-trail-live-switching` changes
AutoTune. After launching with demo mode on, verify:

1. **AutoTune tab** (Tuning menu or tab bar) — grid shows the trace line +
   current-cell halo moving with the simulated engine; **MapPin** toolbar
   button toggles it off/on (persists across app restarts)
2. **Y-axis flip** — **ArrowUpDown** toolbar button reverses rows; the X-axis
   header moves below the grid; the setting matches the table editors'
   (toggle one, the other follows — both read `table_y_axis_bottom`)
3. **Mid-session switching** — Start a session, then change the **Primary**
   or **Secondary** table while running: no stop needed, the session
   re-configures; switching a table OUT with unsent recommendations raises
   the Send & Switch / Discard & Switch dialog
4. **Regression sanity** — the rejection indicator still ticks up samples;
   Send to ECU works against the demo ECU (writes go to the simulator)

## Troubleshooting

- **Window never opens** — check the terminal; Rust errors abort the launch
- **`invoke` failures everywhere** — the app is running Vite-only
  (`npm run dev`); relaunch with `npm run tauri dev`
- **Demo toggle stuck on "Loading..."** — the demo INI is missing from
  `src-tauri/resources/demo.ini`; the error names both searched paths
- **Stale backend behavior after a Rust edit** — the dev runner rebuilds on
  edit, but a hung rebuild can serve the old binary; stop the task, rerun
