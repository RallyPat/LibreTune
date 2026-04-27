# LibreTune Architecture

This document describes module boundaries and the high-level shape of the
LibreTune codebase, current as of the Phase 1–8 cleanup pass (April 2026).

## Workspace layout

LibreTune is a Cargo + npm hybrid laid out as a Cargo workspace plus a
front-end Tauri app:

```
crates/
  libretune-core/       # Pure-Rust domain library (no Tauri, no Tokio runtime)
  libretune-app/        # React/Vite frontend
    src/                # TypeScript/TSX
    src-tauri/          # Tauri host (Rust); depends on libretune-core
docs/                   # mdBook documentation
reference/              # External reference material (TunerStudio INIs, ECU
                        # software for format study) — not shipped
scripts/                # Dev / release / packaging helpers
```

## Crate boundaries

### `libretune-core`

Pure domain logic, no UI, no Tauri, no async runtime in the public API.
Public modules (post-Phase 7):

- `action_scripting` — Lua-driven controller-command scripts.
- `autotune` — VE / AFR / dwell adaptation algorithms; recommendations and
  authority limits live in `autotune/anomaly.rs` and friends.
- `basemap` — Built-in base-map generation logic.
- `dash` — Dashboard data structures:
  - `dash::layout` (LibreTune-native simple format: `DashboardLayout`,
    `GaugeConfig`, `GaugeType`, file-path helpers).
  - `dash::{parser, writer, types, validation, templates}` — TunerStudio
    `.dash` XML format (full schema with `DashFile`, `GaugeCluster`,
    `DashComponent`, `Bibliography`, etc.).
- `datalog` — Streaming log writer.
- `demo` — Synthetic ECU for offline / demo mode.
- `ecu` — `EcuMemory`, `Value`, page model.
- `ini` — INI parser + `EcuDefinition` (`Constant`, `OutputChannel`,
  `TableDefinition`, etc.).
- `plugin_api`, `plugin_system` — WASM plugin host + plugin-facing API.
  (The legacy Java plugin host has been removed; see Phase 3 in the
  changelog.)
- `project` — Project model, repository, online-INI repository.
- `protocol` — `Connection`, `ConnectionState`, transport abstractions
  (Serial, TCP, in-process simulator).
- `realtime` — `Evaluator` derived-channel transform (raw output channels +
  `EcuDefinition` → computed channels). Pure transform; not part of the
  streaming/transport stack.
- `table_ops` — Table re-binning, smoothing, interpolation, scaling,
  cell-equalize. Pure value-in / value-out helpers.
- `tune` — `TuneFile`, `TuneCache`, `PageState`, migration.
- `unit_conversion` — Unit conversions used by both UI and analysis.

The `prelude` module re-exports the most commonly used types so callers
can `use libretune_core::prelude::*;`.

### `libretune-app/src-tauri`

Tauri host. The crate root (`src-tauri/src/lib.rs`, ~419 lines) is pure
glue:

- `AppState` construction (Mutex-wrapped EcuDefinition, project, tune
  cache, connection, etc.).
- A few shared helpers retained at crate root.
- `tauri::generate_handler![…]` listing every command.

All command bodies live under `src-tauri/src/commands/` (72 files). Each
file owns a coherent slice of functionality, e.g.:

- `connection.rs`, `metrics.rs`, `realtime_get.rs` — connection lifecycle
  and metrics tasks.
- `dash_files.rs`, `dash_layout.rs`, `dash_convert.rs` — dashboard file
  IO + layout and `DashboardLayout` ↔ `DashFile` conversion.
- `tune_io.rs`, `tune_info.rs`, `tune_misc.rs`, `tune_health.rs`,
  `tune_migration.rs` — tune persistence, diffing, migration.
- `table_ops.rs`, `table_compare.rs`, `csv_io.rs` — table editing
  Tauri wrappers.
- `autotune_*.rs`, `base_map.rs`, `adaptive_timing.rs` — tuning
  primitives.
- `settings.rs`, `hotkeys.rs`, `restore_points.rs` — user-facing settings
  state.
- `ini_meta.rs`, `ini_dialogs.rs`, `ini_metadata.rs`, `load_ini.rs`,
  `channels.rs`, `constants_read.rs` — INI surface area exposed to the
  frontend.
- `menu.rs`, `project_*.rs`, `ts_import.rs` — project / menu plumbing.

Adding a new command typically means: pick or create a `commands/<topic>.rs`
file, write the `pub async fn` and mark it `#[tauri::command]`, then
register it in the `invoke_handler!` manifest in `lib.rs`.

### `libretune-app/src` (frontend)

React + Vite + TypeScript. The current shape:

```
src/
  App.tsx               # Top-level orchestrator (~1.4k lines, heavily
                        # decomposed: most logic lives in hooks/ + components/)
  main.tsx              # Provider tree (Theme, Loading, Toast, UnitPrefs)

  contexts/             # Cross-cutting React providers
    LoadingContext.tsx
    ToastContext.tsx
    useUnitPreferences.tsx

  hooks/                # Custom hooks (most "side-effect" code lives here)
    useBackendEventListeners.ts
    useEcuEventListeners.ts
    useGlobalShortcuts.ts
    useIniDefaultsLoader.ts
    useRealtimeStream.ts
    useTabPopout.ts
    useTableCurveRefresh.ts

  menus/                # Menu + toolbar definitions (data with callbacks)
    buildMenuItems.ts
    buildToolbarItems.tsx

  stores/               # Zustand stores
    realtimeStore.ts    # Per-channel realtime subscriptions

  services/             # Singletons / non-React glue
    hotkeyService.ts
    openTarget.ts
    …

  components/
    common/             # Shared primitives (Dialog, Button, FormField,
                        # EmptyState, ErrorBoundary)
    dialogs/
      DialogRenderer.tsx        # Generic INI-driven dialog renderer
      types.ts                  # DialogComponent / DialogDefinition / etc.
      fields/                   # Per-component-kind renderers
        Indicator.tsx
        IndicatorPanelRenderer.tsx
        CommandButton.tsx
      <DialogName>.tsx          # Concrete dialogs (BaseMap, Connection, …)
    dashboards/                 # TsDashboard + gauge editor + import
    tables/                     # 2D / 3D table editors
    curves/                     # Curve editor
    tuner-ui/                   # The active layout chrome
                                # (TunerLayout, MenuBar, Toolbar, StatusBar,
                                #  Sidebar, AutoTune, Console, …)
    hardware/                   # Port editor
    SettingsView.tsx            # Inline project settings surface
    DialogOverlays.tsx          # Mounts every modal overlay
    TabContentRouter.tsx        # Maps active-tab id → component
    PluginPanel.tsx             # WASM plugin host UI

  themes/                       # Theme provider + CSS variables
  styles/                       # Global stylesheets
  types/                        # App-wide TS types (mirroring Rust shapes)
  utils/                        # Pure helpers (formatError, buildSidebarItems, …)
  i18n/                         # Translation strings
```

### Frontend ↔ backend contract

- The frontend never talks to hardware directly. Every effect that touches
  the ECU, the filesystem, or settings goes through a Tauri command in
  `src-tauri/src/commands/`.
- Realtime data flows via Tauri events: the backend emits `realtime:update`
  packets, and `useRealtimeStream` hydrates the Zustand store; components
  subscribe per-channel using `useChannelValue` / `useChannels` to avoid
  re-rendering parents at the realtime cadence.
- Default dashboards (Basic / Tuning / Racing) are seeded by Rust at first
  launch via `create_default_dashboard_files`; the frontend treats these
  like any other dashboard via `list_available_dashes` / `get_dash_file`.
- The TunerStudio `.dash` XML format is parsed by `libretune_core::dash`
  (parser/writer); LibreTune's simpler runtime layout
  (`DashboardLayout`) is converted at the Tauri-command boundary by
  `commands::dash_convert`.

## State management

- **Server / domain state** lives in Rust (`AppState` mutexes around
  `EcuDefinition`, `TuneCache`, `Connection`, project, settings).
- **Realtime channel data** lives in the Zustand store
  (`stores/realtimeStore.ts`) and is subscribed to per-channel.
- **UI state** lives in React (`useState` / `useReducer`).
- **Cross-cutting UI services** (loading overlay, toast queue, unit
  preferences) live in dedicated providers under `src/contexts/`.

## Build / test entrypoints

```sh
# Workspace builds + tests
cargo build --workspace
cargo test  --workspace

# Frontend
cd crates/libretune-app
npm install
npm run dev          # Vite only
./scripts/tauri-dev.sh  # Full Tauri dev (preferred)
npx tsc --noEmit     # Typecheck
npm test -- --run    # Vitest
npm run build        # Production bundle
```

See `scripts/` for the release / packaging helpers.
