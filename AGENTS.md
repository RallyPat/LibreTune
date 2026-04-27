# LibreTune - Implementation Guide for AI Agents

## Project Overview
LibreTune is a modern, open-source ECU tuning software for Speeduino, EpicEFI, and compatible aftermarket ECUs.
It's built with Rust core + Tauri desktop app + React frontend.

## Supported ECU Platforms
LibreTune supports multiple ECU platforms, each treated as a distinct project:

| Platform | Description | INI Pattern | Test Coverage |
|----------|-------------|-------------|---------------|
| **Speeduino** | Open-source Arduino-based ECU | `speeduino*.ini` | Full platform test |
| **rusEFI** | Open-source STM32-based ECU | `rusEFI*.ini` (not FOME/epicEFI) | Full platform test |
| **FOME** | Fork of rusEFI with enhanced features | `*FOME*.ini` | All files tested |
| **epicEFI** | rusEFI variant for epicECU boards | `*epicECU*.ini` | Sampled testing (10 files) |
| **MegaSquirt** | MS2/MS3 ECU systems | `MS2*.ini`, `MS3*.ini` | Basic parsing |

**Note**: FOME, epicEFI, and rusEFI are separate projects and should not be conflated in code or documentation.

## Architecture
```
crates/
├── libretune-core/      # Rust library (ECU communication, INI parsing, AutoTune)
└── libretune-app/       # Tauri desktop app
    ├── src/              # React frontend
    ├── src-tauri/        # Tauri backend glue
```

## Tuning Goals
The project aims to provide professional ECU tuning workflow and functionality while:
- Using modern UI patterns (glass-card styling, smooth animations)
- Being open-source and legally distinct
- Improving UX (better keyboard navigation, tooltips, responsive design)

## Key Features Implemented

### 1. Table Editing (2D/3D)
- Location: `crates/libretune-app/src/components/tables/`
- Files:
  - `TableEditor2D.tsx` - Main 2D table editor with toolbar
  - `TableEditor3D.tsx` - 3D visualization (canvas-based)
- Backend: `crates/libretune-core/src/table_ops.rs`
- Features: Set Equal, Increase/Decrease, Scale, Interpolate, Smooth, Re-bin, Copy/Paste, History Trail, Follow Mode

### 2. AutoTune
- Location: `crates/libretune-app/src/components/tuner-ui/AutoTune.tsx`
- Backend: `crates/libretune-core/src/autotune.rs`
- Features: Auto-tuning with recommendations, heat maps, cell locking, filters, authority limits
- **Documentation** (Feb 3, 2026):
  - Created comprehensive usage guide: `docs/src/features/autotune/usage-guide.md`
  - Added step-by-step workflow (Setup → Driving → Review → Apply)
  - Included real-world scenarios (NA, turbo, E85 engines)
  - Troubleshooting section with common issues and solutions
  - Keyboard shortcuts and best practices
  - Multi-location documentation: docs/src and public/manual (kept in sync)

### 3. Dashboard System (TunerStudio-Compatible)
- **NEW**: `crates/libretune-app/src/components/dashboards/TsDashboard.tsx` - Main dashboard component
- **NEW**: `crates/libretune-app/src/components/dashboards/dashTypes.ts` - TypeScript types for TunerStudio format
- **NEW**: `crates/libretune-app/src/components/dashboards/GaugeContextMenu.tsx` - Right-click context menu
- **NEW**: `crates/libretune-app/src/components/gauges/TsGauge.tsx` - Canvas-based gauge renderer
- **NEW**: `crates/libretune-app/src/components/gauges/TsIndicator.tsx` - Boolean indicator renderer
- Backend: `crates/libretune-core/src/dash/{mod.rs,types.rs,parser.rs,writer.rs}`
- Features: Native .ltdash.xml format, TunerStudio .dash import, right-click context menu, designer mode, gauge demo, dashboard selector
- Dashboard Storage: `<app_data>/dashboards/` with auto-generated defaults (Basic, Tuning, Racing)

### 4. Gauge Rendering (TunerStudio-Compatible)
- Location: `crates/libretune-app/src/components/gauges/TsGauge.tsx`
- **Enhanced in Jan 2026**: All gauge types now feature metallic bezels, shadows, gradients, and 3D effects
- Gauge Types Implemented (13 of 13 - ALL COMPLETE):
  - BasicReadout - LCD-style digital numeric display with metallic frame and inset shadows
  - HorizontalBarGauge - Horizontal progress bar with rounded corners and gradient fills
  - VerticalBarGauge - Vertical progress bar with tick marks and 3D gradient effects
  - AnalogGauge - Classic circular dial with metallic bezel, minor ticks, gradient needle, center cap
  - AsymmetricSweepGauge - Curved sweep gauge with glowing tip, warning zones, gradient fills
  - HorizontalLineGauge - Horizontal line indicator with position dot and glow effect
  - VerticalDashedBar - Segmented vertical bar with per-segment zone coloring
  - Histogram - Bar chart distribution visualization centered on current value
  - LineGraph - Time-series line chart with filled gradient area and current value dot
  - RoundGauge - Circular gauge with 270° arc and tick marks
  - RoundDashedGauge - Circular gauge with segmented arc
  - FuelMeter - Specialized fuel level gauge
  - Tachometer - RPM-specific gauge with redline zone

### 5. Professional Default Dashboards
- Location: `crates/libretune-app/src-tauri/src/lib.rs` (create_*_dashboard functions)
- Three professionally designed dashboards:
  - **Basic**: Large analog RPM + digital AFR + vertical CLT/IAT bars + horizontal MAP bar + battery/advance/VE/PW readouts
  - **Racing**: Giant center RPM analog + oil pressure/water temp vertical bars + speed/AFR/boost/fuel digital readouts
  - **Tuning**: Mixed layout with sweep gauge, analog gauge, vertical bars, horizontal bars, lambda line graph, EGT/duty dashed bars, correction factor readouts
- All dashboards use consistent dark color scheme with accent colors matching gauge purposes

### 6. Dialog System
- Location: `crates/libretune-app/src/components/dialogs/`
- Files:
  - `SaveLoadBurnDialogs.tsx` - Save/Load/Burn tunes
  - `PerformanceFieldsDialog.tsx` - Vehicle specs for HP/Torque calculations
  - `NewProjectDialog.tsx` - Project creation wizard
  - `BrowseProjectsDialog.tsx` - Project selection
  - `RebinDialog.tsx` - Table re-binning with interpolation
  - `CellEditDialog.tsx` - Cell value editing dialog

### 7. Menu & Navigation
- Location: `crates/libretune-app/src/components/MenuManager.ts`
- Parses INI [Menu] sections, builds hierarchical menu tree
- Location: `crates/libretune-app/src/components/HotkeyManager.ts`
- Global keyboard shortcuts (see HotkeyManager for complete list)

### 8. Action Management
- Location: `crates/libretune-app/src/components/ActionManagement.tsx`
- Features: Action list, queue system, recording/playback

### 9. Pop-out Windows (Multi-Monitor)
- Location: `crates/libretune-app/src/PopOutWindow.tsx` - Standalone pop-out window renderer
- Location: `crates/libretune-app/src/PopOutWindow.css` - Pop-out window styles
- Features:
  - Pop any tab to its own window (External Link button in tab bar)
  - Dock-back button to return tab to main window
  - Bidirectional sync for realtime data and table edits
  - Window state persistence via `tauri-plugin-window-state`
- Implementation:
  - Hash-based routing: `#/popout?tabId=...&type=...&title=...`
  - localStorage for initial data transfer (`popout-{tabId}` key)
  - Tauri events for sync: `tab:dock`, `table:updated`, `realtime:update`
  - WebviewWindow API for creating new windows

### 10. Git-Based Tune Versioning
- Location: `crates/libretune-core/src/project/version_control.rs`
- Features:
  - Initialize Git repo for project (`git_init_project`)
  - Manual and auto-commit on save (user preference: "always"/"never"/"ask")
  - View commit history with timeline (`git_history`)
  - Diff between commits showing changed files (`git_diff`)
  - Checkout specific commits to restore tune state (`git_checkout`)
  - Branch management (create, switch, list branches)
  - Commit message format with placeholders: `{date}`, `{time}`, `{table}`
- Frontend: `TuneHistoryPanel.tsx` - Timeline view with diff modal, branch selector
- Settings: Version Control section in SettingsDialog

### 11. Project Templates
- Location: `crates/libretune-core/src/project/templates.rs`
- Built-in templates:
  - **Speeduino 4-cyl NA**: Basic naturally aspirated 4-cylinder gasoline engine
  - **rusEFI Proteus F4**: Advanced tuning for Proteus F4 board
  - **epicEFI Standard**: Standard configuration for epicEFI boards
- Template structure: name, description, ECU type, INI pattern, default connection settings
- Frontend: Template picker in NewProjectDialog with 3-mode flow (select template → configure or scratch)

## Development Commands

### Backend (Rust)
```bash
cd /home/pat/.gemini/antigravity/scratch/libretune
cargo build -p libretune-core
cargo test -p libretune-core
cargo clippy -p libretune-core
```

### Frontend (React/Tauri)
```bash
cd crates/libretune-app
npm install
npm run dev        # Development mode
npm run tauri dev  # Full Tauri app
npm run build       # Production build
npm run lint       # ESLint
npm run typecheck  # TypeScript checking
```

## INI File Format
LibreTune uses standard ECU INI definition files. Structure:
- `[MegaTune]` - Version info, signature, query command
- `[Menu]` - Menu structure with sub-menus
- `[TableEditor]` - Table definitions
- Dialog sections - Define settings dialogs

## Backend Command Pattern
All Tauri commands follow:
```rust
#[tauri::command]
async fn command_name(param: Type) -> Result<ReturnType, String> {
    // Implementation
}
```

## Component State Management
- Use React hooks (useState, useEffect, useMemo)
- Realtime data via `get_realtime_data()` command (100ms polling)
- Table data fetched on-demand via `get_table_data()`

## Tuning Best Practices (for AI agents)
1. **Backend First**: Always implement Rust backend commands before UI
2. **Type Safety**: All TypeScript interfaces should match Rust structs
3. **Error Handling**: All commands return Result<T, String>
4. **Performance**: Use useMemo for expensive computations, debounce user input
5. **Keyboard Navigation**: Follow standard ECU tuning hotkey patterns
6. **Legal Distinction**: All code must be original, never copied from proprietary software
7. **Git Workflow**: Commit changes but **NEVER push to remote** unless explicitly instructed by the user

## ECU Tuning Software Reference Analysis
Based on analysis of common ECU tuning software patterns:

### Menu Structure
- Main menu: Basic/Load, Fuel, Ignition, Tools, Diagnostics
- Sub-menus can be conditional based on ECU settings
- Special item: `std_realtime` opens dashboard

### Table Editor (2D)
- Toolbar: Set Equal (=), Increase (>), Decrease (<), Scale (*), Interpolate (/), Smooth (s)
- Right-click: Set Equal, Scale, Interpolate, Smooth, Lock/Unlock cells
- Re-binning: Change X/Y bins with automatic Z interpolation

### AutoTune
- Primary controls: Update Controller (checkbox), Send button, Burn button, Start/Stop
- Recommended table: Color coding (blue=richer, red=leaner)
- Tooltips: Beginning Value, Hit Count, Hit Weighting, Target AFR, Hit %
- Heat maps: Cell Weighting (data coverage), Cell Change (magnitude)
- Advanced: Authority limits, Filters, Reference tables

### Dashboard
- Gauge types: Analog dial, Digital readout, Bar gauge, Sweep gauge, LED
- Gauge properties: Channel, Min/Max, Units, Colors, Show history, Positioning
- Full-screen mode: Double-click gauge or background

### Dialogs
- Multi-panel layout: North, Center, East panels
- Field types: scalar (number), bits (dropdown/checkbox), array (table reference)

## Remaining Tasks for Future Agents
[x] Fix smooth_table bug (weight array indexing issue in table_ops.rs) - FIXED, all 10 tests pass
[x] Add more AutoTune algorithms (lambda compensation, transient filtering) - COMPLETED Jan 11, 2026
[x] Implement 3D table with react-three-fiber for better visualization - Enhanced with live cursor, trail line, cell grid overlay
[x] Add data logging/playback features - Implemented DataLogView with CSV import, playback controls
[x] All gauge types implemented (13/13) - RoundGauge, RoundDashedGauge, FuelMeter, Tachometer added
[x] Implement action scripting engine - COMPLETED Sprint 3 (Feb 4, 2026)
[x] Add plugin system for extensibility - COMPLETED Sprint 3 (WASM plugins with wasmtime)
[ ] **DEPRECATED**: Java/JVM plugin system (disabled Feb 4, 2026, see DEPRECATION_NOTICE.md)
[x] Add user manual/help system - COMPLETED: mdBook user manual, UserManualViewer component
[x] Implement project templates - COMPLETED Jan 12, 2026 (3 built-in templates: Speeduino, rusEFI, epicEFI)
[x] Add tune comparison/diff view - Implemented compare_tables command
[x] Implement Git integration for tune versioning - COMPLETED Jan 12, 2026 (local git, auto-commit settings, history panel)
[x] Add unit conversion layer (°C↔°F, kPa↔PSI, AFR↔Lambda) with user preferences - UnitPreferencesProvider implemented
[x] Add user-configurable status bar channel selection - COMPLETED Jan 11, 2026
[x] Create comprehensive test suite (CI + 46 unit tests added)
[x] Fix table map_name lookup (veTable1Map → veTable1Tbl)
[x] Remove hardcoded ECU channel names from status bar
[x] Remove hardcoded gauge configurations from dashboard
[x] Handle lastOffset keyword in constant parsing (afrTable, lambdaTable, etc.)
[x] Fix std_separator duplicate key warning in MenuBar
[x] Implement TunerStudio dashboard format (.dash/.gauge XML files)
[x] INI signature mismatch detection with user notification dialog
[x] Online INI repository search and download from GitHub (Speeduino, rusEFI)
[x] Resilient ECU sync with partial failure handling and status bar indicator
[x] TunerStudio project import (project.properties, restore points, pcVariables)
[x] Java properties file parser for TunerStudio compatibility
[x] PC variables persistence (pcVariableValues.msq)
[x] Restore points system (create, list, load, delete, prune)
[x] INI version tracking in tune files - COMPLETED Jan 11, 2026
[x] User-driven tune migration between INI versions - COMPLETED Jan 11, 2026
[x] Fix table operations integration (scale, smooth, interpolate, set equal, rebin) - All Tauri commands now wired to core library
[x] Frontend dialogs for restore points and project import
[x] Pop-out windows for multi-monitor support (dock-back, bidirectional sync)
[x] CSV export/import for tune data - Implemented with file dialogs
[x] Reset tune to defaults - Implemented reset_tune_to_defaults command
[x] Tooth logger (Speeduino/rusEFI/MS2) - Backend + ToothLoggerView.tsx
[x] Composite logger (Speeduino/rusEFI/MS2) - Backend + CompositeLoggerView.tsx
[x] Action Engine Enforcement (validation against INI) - COMPLETED Feb 8, 2026
[x] Math Channels / Expression Engine - Backend COMPLETED Feb 8, 2026
[ ] rusEFI console support (text-based command interface) - COMPLETED Feb 1-2, 2026
  - [x] ECU type detection (Speeduino, RusEFI, FOME, EpicEFI, MS2, MS3)
  - [x] Console command pass-through protocol (Step 3 - connection.rs)
  - [x] FOME fast comms with intelligent fallback (Step 5 - settings)
  - [x] Console UI component (Step 6 - EcuConsole.tsx + CSS)
  - [x] Tauri commands: send_console_command, get_ecu_type (Step 4 - lib.rs)
  - [x] App integration and menu items (Step 7 - App.tsx)

## Recent Changes

Detailed session-by-session history has moved to [CHANGELOG.md](CHANGELOG.md).
What follows is a high-level pointer to the most recent cleanup pass.

### Phase 1–8 cleanup pass (Apr 2026)

The codebase has just completed an eight-phase cleanup pass focused on
removing vestigial code, deduplicating architecture, and unifying the UI
language. See `CHANGELOG.md` for full per-phase details.

- **Phase 1–3**: Dead-code deletions, layout-duplication removal, full Java
  plugin removal (WASM is now the only plugin system).
- **Phase 4**: UI consistency pass — shared `Dialog` / `Button` /
  `FormField` / `EmptyState` primitives in `components/common/`; 20+
  dialogs migrated; CSS hex sweep tokenized; lucide-react adopted; pop-out
  window theme parity.
- **Phase 5**: `lib.rs` reduced from 14,475 → 419 lines; 72 command modules
  under `crates/libretune-app/src-tauri/src/commands/`.
- **Phase 6**: Folder reorg (`src/contexts/` for cross-cutting providers);
  `DialogRenderer.tsx` split into `dialogs/types.ts` and `dialogs/fields/`.
- **Phase 7**: `dashboard.rs` centralized under `dash/layout.rs`; default
  dashboards become Rust source of truth; deleted vestigial
  `LibreTuneDefaultDashboard.ts` and `AutoTuneLive.tsx`; `realtime/evaluator.rs`
  documented; `smooth_table` confirmed fixed (13/13 tests, 0 ignored).
- **Phase 8**: Documentation hygiene — `CHANGELOG.md` created in Keep a
  Changelog format; this section trimmed; `docs/architecture.md` added.

## Notes
- The project is a git repository — commit incrementally on a feature branch.
- **Cross-platform directories** (resolved at runtime):
  - Projects: `~/Documents/LibreTuneProjects/` (or platform equivalent)
  - App Data: `~/.local/share/LibreTune/` (Linux), `~/Library/Application Support/LibreTune/` (macOS), `%APPDATA%\LibreTune\` (Windows)
  - Definitions: `<app_data_dir>/definitions/`
- Reference ECU software files are in `TunerStudioMS/` (for understanding INI format patterns only)
- **IMPORTANT**: Always use "AutoTune" not "VE Analyze", and avoid TunerStudio trademark terminology
