# Changelog

All notable changes to LibreTune are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

LibreTune is currently in pre-1.0 development. Until a 1.0 release is cut,
entries here are organized by **session/sprint date** rather than version
number. Standard Keep a Changelog sections (Added / Changed / Fixed /
Removed / Deprecated / Security) appear inside each dated block where
relevant.

## [Unreleased]

### Phase 6 — App.tsx & DialogRenderer decomposition (Apr 26, 2026)

#### Changed
- **Folder reorg**: `src/components/{LoadingContext,ToastContext}.{tsx,css}`
  and `src/utils/useUnitPreferences.tsx` moved into a dedicated
  `src/contexts/` directory. All 10 import sites updated.
- **DialogRenderer.tsx split** (1,548 → 1,158 lines):
  - `dialogs/types.ts` — shared interfaces and helpers.
  - `dialogs/fields/Indicator.tsx` — single-light expression-driven indicator.
  - `dialogs/fields/IndicatorPanelRenderer.tsx` — grid-of-indicators panel.
  - `dialogs/fields/CommandButton.tsx` — controller-command button with
    warning dialog and auto-sync/reconnect.
  The big `DialogField` editor remains inline pending future work.

### Phase 7 — Rust core small cleanups (Apr 26, 2026)

#### Changed
- **Centralize dashboard code**: `crates/libretune-core/src/dashboard.rs`
  moved into `crates/libretune-core/src/dash/layout.rs`. The `dash` module
  now owns both LibreTune's native `DashboardLayout` representation and the
  TunerStudio XML format (`parser`, `writer`, `types`, `validation`).
- **Default dashboards source of truth**: deleted
  `crates/libretune-app/src/components/dashboards/LibreTuneDefaultDashboard.ts`
  and the `__libretune_default__` magic-path fallback in `TsDashboard.tsx`.
  Rust now seeds default dashboards (Basic / Tuning / Racing) on first run
  via `create_default_dashboard_files`; the frontend uses
  `list_available_dashes` / `get_dash_file` only.
- **realtime/evaluator.rs**: added module-level doc clarifying it is a
  derived-channel transform layer, not part of the streaming/transport stack.

#### Removed
- `crates/libretune-app/src/components/tuner-ui/AutoTuneLive.tsx` — single-
  line backwards-compat re-export with zero importers.

#### Fixed
- `smooth_table` weight-array indexing bug — already addressed in tree;
  AGENTS.md "Known Issues" entry trimmed and all 13 `table_ops` tests pass
  with 0 ignored.

### Phase 5 — `lib.rs` god-file split (Apr 2026)

#### Changed
- `crates/libretune-app/src-tauri/src/lib.rs` reduced from 14,475 lines to
  **419 lines** of pure registration glue (state setup +
  `invoke_handler!` manifest). All command bodies, helpers, and types
  extracted into 72 focused modules under `commands/`.
- Imports re-grouped (external → mod decls → `pub(crate)` re-exports →
  alphabetical command imports) with a module-level doc comment.
- `cargo check` clean; 12/12 lib tests pass.

### Phase 4 — UI consistency pass (Apr 2026)

#### Added
- Shared dialog/button/empty-state/form-field primitives under
  `src/components/common/`.
- `Dialog.Footer` convention: secondary buttons left, primary right
  (achieved via `flex-direction: row-reverse`).
- ~40 legacy theme-token aliases in `themes/variables.css`.

#### Changed
- 20+ dialogs migrated to the shared `Dialog` / `Button` / `FormField`
  primitives, dropping bespoke overlay markup and CSS.
- `tuner-ui/Dialogs.tsx` (Save/Load/Burn/NewTune/Settings/About/Connection)
  and `TsDashboard.tsx` (New/Rename/Delete) chrome converted.
- CSS hardcoded-hex sweep: ~600 hex literals reduced to ~100 bespoke ones,
  with the remainder behind theme tokens.
- All `lucide-react` icon migration; emoji literals removed from UI strings.
- `PopOutWindow.css` fully tokenized — pop-out windows now inherit theme.

### Phase 3 — Java plugin removal (Apr 2026)

#### Removed
- All Java/JVM plugin code, Tauri commands, and `PluginPanel.tsx` UI.
  WASM `plugin_system` is the supported plugin path.

### Phase 2 — Layout duplication (Apr 2026)

#### Removed
- The unused `src/components/layout/` directory (Header / Sidebar /
  MenuBar / ConnectionMetrics duplicates). The active layout lives in
  `src/components/tuner-ui/`.

### Phase 1 — Dead-code cleanup (Apr 2026)

#### Removed
- Misplaced Rust files inside the React tree (`mod.rs`, `types.rs`,
  `StatusBar.rsx`, `menubar.rsx`, etc.).
- Unreferenced TS files (`TestApp.tsx`, `libreTuneDefaultDashboardNew.ts`,
  `compatTest.ts`, `DialogWindow.tsx`, `SaveLoadBurnDialogs.tsx`).
- Canvas-based `TableEditor3D.tsx`; the react-three-fiber implementation
  was renamed in its place.
- `scripts/tauri-dev.sh.backup`.

---

## Pre-cleanup history

The entries below pre-date the Phase 1–8 cleanup pass and were preserved
verbatim from `AGENTS.md`. They are organized by date and sprint rather
than by Keep a Changelog section.

## Recent Changes (Session History)

### Realtime Stream Lock Contention Fix & Dashboard Tab Protection - Feb 28, 2026

#### Realtime Stream Fix: get_all_constant_values Connection Lock Starvation
- **Problem**: Dashboard gauges updated for ~2 seconds then froze permanently
- **Root Cause**: `get_all_constant_values()` read every scalar constant from the ECU individually over serial while holding `connection.lock()`. With rusEFI's hundreds of constants, this took many seconds or hung permanently, starving the realtime stream.
- **Evidence**: Stream log showed 80 `conn_lock busy` entries vs only 21 successful emits; connection lock was held permanently after ~2 seconds.
- **Fix** (`lib.rs`):
  - Rewrote `get_all_constant_values()` to **never acquire the connection lock**
  - Now reads exclusively from tune cache and tune file (already populated during sync)
  - Extracted reusable helpers: `read_constant_from_cache_or_tune()` and `read_constant_from_cache()`
  - Removed ~200 lines of duplicated ECU read code, replaced with clean helper calls
- **Diagnostics added** (`lib.rs`):
  - `CONN_LOCK_HOLDER` global atomic tracker records which function currently holds the connection lock
  - `set_conn_lock_holder()` / `get_conn_lock_holder()` helper functions
  - Stream log now reports WHO is holding the lock when `try_lock()` fails (e.g., `conn_lock busy (held by: sync_ecu_data)`)
  - Instrumented: `get_connection_status`, `sync_ecu_data`, `stream_loop`
- **Impact**: Realtime stream no longer starved by constant reads; gauges should update continuously

#### Dashboard Tab Protection
- **Problem**: Dashboard tab could be accidentally closed via middle-click or popout, with no way to reopen it
- **Fix 1** (`App.tsx` - `handleTabClose`):
  - Added guard: tabs with `closable: false` are now protected from closing
  - Checks the `closable` property before removing any tab
- **Fix 2** (`TabBar.tsx` - `handleMiddleClick`):
  - Middle-click close now checks `tab.closable !== false` before closing
  - Changed handler signature from `(e, tabId: string)` to `(e, tab: Tab)` for property access
- **Fix 3** (`App.tsx` - View menu):
  - Added **"Dashboard"** menu item to View menu as first entry
  - Re-creates dashboard tab if missing, or switches to it if present
  - Users can always recover the dashboard via **View → Dashboard**

### Sprint 5 - Advanced Table Editing (Part 2) - Active Feb 9, 2026
- **Feature**: Excel-style Row/Column Selection and Header Editing
- **Goal**: Make table headers interactive selection tools (click to select row/col) instead of static inputs, while preserving ability to edit bins via double-click.
- **Plan**:
  - Refactor `TableGrid.tsx` axis headers to use View/Edit modes.
  - Implement `headerDragStart` state for multi-row/col selection.
  - Add visual feedback for selected headers.
  - Ensure compatibility with existing `selectionRange` logic.

### rusEFI Console Support - Completed Feb 1-2, 2026
- **Overall Feature**: Text-based console interface for rusEFI/FOME/epicEFI ECUs with intelligent command/response handling and FOME optimization support
- **Implementation Timeline**:
  - Step 1: Research official rusEFI console architecture (completed in previous session)
  - Step 2: Implement ECU type detection (completed Feb 1)
  - Step 3: Add console protocol layer (completed Feb 1)
  - Step 4: Create Tauri backend commands (completed Feb 1)
  - Step 5: FOME fast comms support (completed Feb 1)
  - Step 6: Console UI component (completed Feb 2)
  - Step 7: App navigation integration (completed Feb 2)

### Step 3: Console Protocol Implementation - Feb 1, 2026
- **Feature**: Text-based command/response protocol in libretune-core
- **Added** (`protocol/commands.rs`):
  - `ConsoleCommand` struct wrapping text commands for ECU transmission
  - `to_bytes()` helper that appends newline for ECU processing
  - `get_timeout_ms()` for command-specific timeouts
  - 4 unit tests for ConsoleCommand creation and serialization
- **Implemented** (`protocol/connection.rs`):
  - `send_console_command(&mut self, cmd: &ConsoleCommand) -> Result<String, ProtocolError>`
  - Uses same inter-character timeout detection as binary protocol
  - Sends command bytes (with newline) and reads response until timeout
  - Trims whitespace from response
  - Records metrics (tx_bytes, rx_bytes, packets)
- **Updated** (`protocol/mod.rs`):
  - Exported `ConsoleCommand` from module
- **Status**: 133 tests pass (all passing)

### Step 4: Tauri Backend Commands - Feb 1, 2026
- **Feature**: Backend API for console communication and ECU type discovery
- **Modified** (`AppState` struct):
  - Added `console_history: Mutex<Vec<String>>` field for command/response history
- **Implemented Tauri commands** (`lib.rs`):
  - `get_ecu_type() -> Result<String, String>` - Returns ECU type as debug string
  - `send_console_command(command: String) -> Result<String, String>` - Sends command, maintains history
  - `get_console_history() -> Result<Vec<String>, String>` - Retrieves full history
  - `clear_console_history() -> Result<(), String>` - Clears history
- **Command integration**:
  - All 4 commands added to Tauri `invoke_handler`
  - Error handling with user-friendly messages
  - History automatically capped at 1000 entries (LRU)
- **Status**: Release build successful

### Step 5: FOME Fast Comms Support - Feb 1, 2026
- **Feature**: Automatic protocol optimization for FOME ECUs with transparent fallback
- **Added** (`Settings` struct):
  - `fome_fast_comms_enabled: bool` setting (default true) - User-toggleable
- **Enhanced** (`send_console_command` function):
  - Detects FOME ECU type and setting
  - Attempts faster protocol path first (if conditions met)
  - Falls back to standard console protocol on ANY error (transparent to user)
  - No error propagation during fallback - works silently
  - Debug logging for troubleshooting (`[DEBUG]` and `[WARN]` messages)
- **User Experience**:
  - For FOME users: Faster console commands (when fast path available)
  - For non-FOME: Standard protocol always used
  - Toggle available in settings for advanced users
  - Fallback ensures reliability over speed
- **Status**: Compile verified (release build successful)

### Step 6: Console UI Component - Feb 2, 2026
- **Feature**: Professional terminal-style UI for ECU console
- **Created** (`components/console/EcuConsole.tsx`):
  - TypeScript React component with full console interaction
  - Features:
    - Text input field with Enter key submission
    - Scrollable output log showing command/response history
    - Command history navigation (Arrow Up/Down keys)
    - FOME fast comms toggle (shows only for FOME ECUs)
    - Auto-scroll to bottom on new output
    - Usage hints placeholder when empty
    - Connected/disconnected state indicators
    - Loading state during command execution
  - Tauri API integration:
    - Calls `send_console_command()` for execution
    - Calls `get_console_history()` on component mount
    - Calls `clear_console_history()` on clear button
  - Error handling with user-friendly messages
  - Disabled when disconnected or ECU doesn't support console
- **Created** (`components/console/EcuConsole.css`):
  - Professional dark terminal theme with cyan accents (#00d9ff)
  - Glass-card style header and footer with backdrop effects
  - Responsive scrollbar styling (thin, custom colors)
  - Loading state animation (blinking cursor)
  - Color-coded output lines:
    - Cyan for commands (> prefix)
    - Gray for responses (<- prefix)
    - Red for errors (✗ prefix)
    - Orange for loading (… prefix)
  - Mobile-friendly (16px font size prevents iOS zoom on input)
  - Smooth transitions and hover effects
- **TypeScript Compilation**: Passes (no console component errors)

### Step 7: App Navigation Integration - Feb 2, 2026
- **Feature**: Console tab accessible from menu with context-aware visibility
- **Updated** (`App.tsx`):
  - Imported `EcuConsole` component from `components/console/EcuConsole`
  - Updated `TabContent` interface type to include `"console"`
  - Added `ecuType` state variable to track current ECU type
  - Enhanced `checkStatus()` function:
    - Calls `get_ecu_type()` after connection established
    - Sets `ecuType` to "Unknown" when disconnected
    - Fetches type asynchronously in background
  - Added console case to `renderTabContent()`:
    - Renders `<EcuConsole ecuType={ecuType} isConnected={status.state === "Connected"} />`
  - Added ECU Console menu item to Tools menu:
    - Label: "&ECU Console"
    - Target: Opens new tab "console" with title "Console - [EcuType]"
    - Disabled when: no project, not connected, or ECU doesn't support console
    - Only visible for RusEFI/FOME/EpicEFI ECUs
    - Keyboard shortcut available: Alt+E (on Windows/Linux)
- **Status**: TypeScript compilation passes (no console-related errors)

### ECU Type Detection Infrastructure - Completed Feb 1, 2026
- **Feature**: rusEFI console support foundation and ECU type detection
- **Added** (`ini/types.rs`):
  - `EcuType` enum with variants: Speeduino, RusEFI, FOME, EpicEFI, MS2, MS3, Unknown
  - `EcuType::detect()` method identifies ECU from INI signature and filename patterns
  - `supports_console()` method returns true for RusEFI/FOME/epicEFI
  - `is_fome()` method for FOME-specific optimizations
- **Added** (`ini/mod.rs`):
  - `ecu_type` field in `EcuDefinition` struct stores detected type
  - Default value set to `EcuType::Unknown`
- **Updated** (`ini/parser.rs`):
  - Auto-detect ECU type during INI parsing before returning definition
  - Import `EcuType` in module dependencies
- **Benefit**: Foundation for conditional console UI, FOME fast comms, and ECU-specific features
- **Status**: All tests pass (84+ unit tests), no breaking changes

### Build Number & Nightly Version Management - Completed Feb 1, 2026
- **Status** (from previous session): Build ID (YYYY.MM.DD+g<sha>) displayed in About dialog
- **Added** (`src-tauri/tests/build_info.rs`):
  - New test file to verify build ID format matches `YYYY.MM.DD+g<sha>` pattern
  - Validates date components (year 4-digit, month 01-12, day 01-31)
  - Validates git SHA contains only hex characters
  - 2 tests: `test_build_id_format` and `test_build_id_not_empty`
- **Updated** (`.github/workflows/ci.yml`):
  - Added "Verify build info format" CI step to check build ID format after compilation
- **Updated** (`src-tauri/tauri.conf.json`):
  - Changed version from "0.1.0" to "0.1.0-nightly" for consistency
- **Updated** (`CONTRIBUTING.md`):
  - Added "Version Management & Nightly Builds" section with clear guidelines
  - Documented build ID format and display location
  - Explained nightly vs. release versioning strategy
- **Status**: All CI checks pass, build metadata verified

### Build & Drag-Drop Features - Completed Jan 31, 2026
- **Issue #27**: Build number feature (see above for details)
- **Issue #28**: Drag-drop gauge creation from sidebar to dashboard
  - [crates/libretune-app/src/components/dashboards/DashboardDesigner.tsx](crates/libretune-app/src/components/dashboards/DashboardDesigner.tsx)
    - Added `ChannelInfo` interface matching TsDashboard structure
    - Added `channelInfoMap` optional prop to `DashboardDesignerProps`
    - Implemented `handleDragOver`, `handleDragLeave`, `handleDrop` handlers
    - Calculate drop position, apply grid snap, create gauge with INI metadata
    - Added to undo/redo history via `pushHistory()`
  - [crates/libretune-app/src/components/dashboards/DashboardDesigner.css](crates/libretune-app/src/components/dashboards/DashboardDesigner.css)
    - Added `.drag-over-dropzone` class with dashed border and semi-transparent blue background
  - [crates/libretune-app/src/components/dashboards/TsDashboard.tsx](crates/libretune-app/src/components/dashboards/TsDashboard.tsx)
    - Pass `channelInfoMap` prop to DashboardDesigner component
  - **Features**: INI data population (min/max/units), grid snap, visual feedback, history tracking
  - **Status**: Tested and working, pushed to GitHub (commit d6f06f5)

### AutoTune Table Lookup Fix - Completed Jan 11, 2026
- **Problem**: AutoTune failed with "Table veTable1 not found" for rusEFI/epicEFI INIs
- **Root Cause**: Frontend called non-existent `get_available_tables` command and hardcoded `veTable1`
- **Fix** (`tuner-ui/AutoTune.tsx`):
  - Changed `get_available_tables` → `get_tables` (correct backend command)
  - Added VE table auto-detection: tries `veTableTbl`, `veTable1Tbl`, `veTable1`, etc.
  - Sorted table list: VE/fuel tables first, then alphabetically
  - Fallback to first VE-related table or first table overall
- **Fix** (`TableComparisonDialog.tsx`): Changed `get_available_tables` → `get_tables`
- **Fix** (`App.tsx`): Changed hardcoded `"veTable1"` to `""` for auto-detection
- **Cleanup**: Deleted unused `realtime/AutoTune.tsx` (tuner-ui version is active)

### INI Version Tracking & Tune Migration - Completed Jan 11, 2026
- **TuneFile format update** (`file.rs`):
  - Added `IniMetadata` struct: `signature`, `name`, `hash`, `spec_version`, `saved_at`
  - Added `ConstantManifestEntry` struct: `name`, `data_type`, `page`, `offset`, `scale`, `translate`
  - Added `ini_metadata: Option<IniMetadata>` and `constant_manifest: Option<Vec<ConstantManifestEntry>>` to TuneFile
  - Bumped TuneFile version from "1.0" to "1.1"

- **MSQ parser/writer** (`file.rs`):
  - Parses new `<iniMetadata>` XML section with signature, name, hash, specVersion, savedAt attributes
  - Parses new `<constantManifest>` section containing `<entry>` elements
  - `save_msq()` writes both new sections after bibliography

- **INI fingerprinting** (`ini/mod.rs`):
  - `compute_structural_hash()` - SHA-256 hash of all non-PC constants (name, type, page, offset, scale)
  - `generate_constant_manifest()` - Creates manifest entries from current EcuDefinition
  - `generate_ini_metadata()` - Combines hash + manifest + timestamp into IniMetadata

- **Migration detection** (`tune/migration.rs` - NEW):
  - `MigrationReport` struct with: `missing_in_tune`, `missing_in_ini`, `type_changed`, `scale_changed`, `can_auto_migrate`, `requires_user_review`, `severity`
  - `ConstantChange` struct for detailed change info (old/new type, scale, offset, translate)
  - `compare_manifests()` function compares saved manifest against current INI definition
  - Severity levels: "none", "low" (new constants), "medium" (scale changes), "high" (type changes/removals)
  - Unit tests for empty manifest and summary generation

- **Backend integration** (`lib.rs`):
  - `save_tune()` now populates `ini_metadata` and `constant_manifest` before saving
  - `load_tune()` generates migration report when tune has manifest and INI is loaded
  - Emits `tune:migration_needed` event when severity != "none"
  - Added `migration_report: Mutex<Option<MigrationReport>>` to AppState
  - New Tauri commands: `get_migration_report`, `clear_migration_report`, `get_tune_ini_metadata`, `get_tune_constant_manifest`

- **MigrationReportDialog** (`dialogs/MigrationReportDialog.tsx` - NEW):
  - Shows severity badge (color-coded: red=high, orange=medium, blue=low)
  - Collapsible sections for: type changes (critical), scale changes (warning), removed constants, new constants
  - Lists first 20 items in each section with "...and N more" for larger lists
  - "Dismiss" and "Continue with Tune" buttons
  - Auto-opens when `tune:migration_needed` event is received

### User-Configurable Status Bar - Completed Jan 11, 2026
- **Settings persistence** (`lib.rs`):
  - Added `status_bar_channels: Vec<String>` to Settings struct with `#[serde(default)]`
  - Updated `get_status_bar_defaults` to check user settings first before INI FrontPage/common defaults
  - Added JSON array parsing in `update_setting` for status_bar_channels

- **Status Bar Channel Selector UI** (`SettingsDialog.tsx`):
  - Tag-style chip display showing currently selected channels
  - Remove button (×) on each channel tag
  - Dropdown to add available channels (filtered to exclude already-selected)
  - Maximum 8 channels enforced
  - "Reset to Defaults" button to clear custom selection
  - Live updates reflected immediately in status bar

- **App.tsx integration**:
  - `handleSettingsChange` callback now handles `statusBarChannels` updates
  - Refreshes status bar display when settings are changed

### AutoTune Enhancements - Completed Jan 11, 2026
- **Transient Filtering** (`autotune.rs`):
  - Added `tps: f64` field to `VEDataPoint` for current TPS value
  - Added `tps_rate: f64` field for TPS change rate (%/sec)
  - Added `accel_enrich_active: Option<bool>` for ECU accel enrichment flag
  - Added `timestamp_ms: u64` for lambda delay correlation
  - Added `max_tps_rate: f64` to `AutoTuneFilters` (default 10.0 %/sec)
  - Added `exclude_accel_enrich: bool` to `AutoTuneFilters` (default true)
  - Updated `passes_filters()` to reject data during fast TPS changes or accel enrichment

- **Lambda Delay Compensation** (`autotune.rs`):
  - Added `data_buffer: VecDeque<VEDataPoint>` to `AutoTuneState` for buffering
  - Added `buffer_max_age_ms: u64` (default 500ms) for buffer pruning
  - Implemented `get_lambda_delay_ms(rpm)` with default curve:
    - 200ms at idle (800 RPM)
    - 50ms at redline (6000 RPM)
    - Linear interpolation between
  - `find_delayed_data_point()` finds historical data point matching delay
  - `add_data_point()` now correlates current AFR with historical VE cell

- **Authority Limits Enforcement** (`autotune.rs`):
  - Added `apply_authority_limits()` static function
  - Clamps recommendations by absolute value change AND percentage change
  - Applied in `add_data_point()` before storing recommendation

- **Realtime Stream Integration** (`lib.rs`):
  - Added `AutoTuneConfig` struct to store table name, settings, filters, authority limits, and table bins
  - Added `autotune_config: Mutex<Option<AutoTuneConfig>>` to AppState
  - `start_autotune()` now extracts table bin values from INI and stores config
  - `stop_autotune()` clears the config
  - Added `feed_autotune_data()` helper function
  - Realtime stream loop now feeds data to AutoTune when running
  - Automatically calculates TPS rate from consecutive samples
  - Looks up common channel names (rpm, RPM, map, MAP, afr, AFR, etc.)
  - Converts lambda readings to AFR if needed

- **Axis Bin Reading** (`lib.rs`):
  - Added `read_axis_bins()` helper to read table axis values from tune cache
  - Falls back to generated linear bins if data not available
  - Handles both RPM-like (wide range) and MAP-like (narrow range) axes

### Table Operations Integration - Completed Jan 11, 2026
- **Fixed table editing toolbar operations** (`lib.rs`):
  - All 5 Tauri commands were stubs returning "requires ECU connection" error
  - Now properly wired to `libretune_core::table_ops` functions
  - Added `get_table_data_internal()` helper for code reuse
  - Added `update_table_z_values_internal()` helper for saving changes
  - Operations work in offline mode (edit tune file) and write to ECU if connected

- **Implemented commands**:
  - `smooth_table` - 2D Gaussian weighted averaging of selected cells
  - `interpolate_cells` - Bilinear interpolation between corner cells
  - `scale_cells` - Multiply selected cells by factor
  - `set_cells_equal` - Set selected cells to average value
  - `rebin_table` - Change axis bins with Z-value interpolation

- **Frontend performance improvements** (`TableEditor2D.tsx`):
  - `handleScale` now calls single backend command instead of N individual `update_table_data` calls
  - `handleSetEqual` now calls single backend command for atomic operation
  - All handlers use async/await with try/catch error handling
  - Fixed coordinate order: frontend sends `(row, col)` not `(x, y)` to match backend

- **Test verification**: All 10 table_ops unit tests pass

### Data Tools & Diagnostic Loggers - Completed Jan 11, 2026
- **CSV Export/Import** (`lib.rs`):
  - `export_tune_as_csv` - Exports all scalar constants to CSV with metadata
  - `import_tune_from_csv` - Parses CSV, validates bounds, applies to tune
  - `parse_csv_line()` - Helper for quoted field handling
  - `encode_constant_value()` - Converts display values to raw bytes
  - Frontend file dialogs using `@tauri-apps/plugin-dialog`

- **Reset to Defaults** (`lib.rs`):
  - `reset_tune_to_defaults` - Resets all constants to INI default values
  - Reads from `def.default_values` or uses min value as fallback
  - Updates both TuneCache and TuneFile

- **Table Comparison** (`lib.rs`):
  - `compare_tables` - Compares two tables cell-by-cell
  - Returns `TableComparisonResult` with differences, max_diff, avg_diff
  - `read_table_values()` - Helper to extract table data from cache
  - `read_raw_value()` - Generic byte-to-value conversion

- **Tooth Logger** (`lib.rs` + `ToothLoggerView.tsx`):
  - Backend supports Speeduino (`H` command), rusEFI (`l\x01-03`), MS2/MS3
  - `ToothLogEntry` struct: tooth_number, tooth_time_us, crank_angle
  - `ToothLogResult` with detected_rpm and teeth_per_rev calculation
  - Frontend: Canvas-based bar chart, statistics panel, CSV export
  - CSS: Dark theme, capture button animation, stat cards

- **Composite Logger** (`lib.rs` + `CompositeLoggerView.tsx`):
  - Backend supports Speeduino (`J`/`O`/`X`), rusEFI (`l\x04-06`), MS2/MS3
  - `CompositeLogEntry` struct: time_us, primary, secondary, sync, voltage
  - Multi-channel waveform display with zoom/scroll controls
  - Sync status detection with lost-sync counting
  - Legend with color-coded channels

- **Frontend menu updates** (`App.tsx`):
  - Reset to Defaults now works with success toast
  - Tooth/Composite logger show capture status toasts
  - CSV export uses save dialog, import uses open dialog
  - Proper error handling for each command

### Dashboard System Enhancement - Completed Jan 9, 2026
- **TsGauge.tsx major upgrade**:
  - All gauge types now feature metallic bezels, shadows, and gradient fills
  - Added `createMetallicGradient()` helper for realistic bezel effects
  - Added `lightenColor()` and `darkenColor()` utility functions
  - Added `roundRect()` helper for rounded corner shapes
  - Added embedded font loading with FontFace API
  
- **Gauge type improvements**:
  - BasicReadout: LCD-style display with metallic frame, inset shadows, gradient background
  - HorizontalBarGauge: Rounded corners, gradient fill, highlight stripe
  - VerticalBarGauge: Tick marks on side, gradient fill, segment highlighting
  - AnalogGauge: Multi-layer metallic bezel, minor tick marks, gradient needle, metallic center cap
  - AsymmetricSweepGauge: Glowing tip, gradient arc fill, track background with inset
  - HorizontalLineGauge: Gradient track, glowing position indicator
  - VerticalDashedBar: Per-segment zone coloring, gradient fills, glow on top segment

- **New gauge types implemented**:
  - Histogram: Bar chart distribution centered on current value, colored zones
  - LineGraph: Time-series chart with gradient fill area and current value dot

- **Default dashboard redesign** (`lib.rs`):
  - Tuning dashboard now uses mixed gauge types (sweep, analog, bars, line graph, dashed bars)
  - Added lambda history line graph, EGT/duty dashed bars, correction factor readouts
  - Consistent dark color scheme with purpose-matched accent colors

- **Dashboard browser categories**:
  - `list_available_dashes()` now scans reference/TunerStudioMS/Dash directory
  - Added category field (User, Reference) for grouping in UI
  - TsDashboard.tsx groups dashboards by category with collapsible headers

### Frontend Dialogs for Restore Points & Project Import - Completed Jan 7, 2026
- **RestorePointsDialog.tsx** (new component):
  - Lists all restore points with filename, date, and size
  - Load button with unsaved changes warning confirmation
  - Delete button with confirmation dialog
  - Create restore point button
  - Error handling and loading states

- **RestorePointsDialog.css** (new styles):
  - Glass-card overlay with backdrop blur
  - Animated dialog appearance
  - List items with hover effects
  - Confirmation dialogs with warning icons

- **ImportProjectWizard.tsx** (new component):
  - Two-step wizard: Select folder → Confirm import
  - Uses `@tauri-apps/plugin-dialog` folder picker
  - Preview panel showing project name, INI, tune status, restore points
  - Auto-opens imported project after completion

- **ImportProjectWizard.css** (new styles):
  - Step indicators with completion states
  - Drag-and-drop style folder selector
  - Preview card with details grid

- **Backend additions** (`lib.rs`):
  - `preview_tunerstudio_import` command - previews TS project before import
  - `TunerStudioImportPreview` struct with project metadata
  - Auto-prune in `create_restore_point` using `max_restore_points` setting

- **ProjectSettings enhancement** (`project.rs`):
  - Added `max_restore_points: u32` with default value 20
  - Serde default function for backward compatibility

- **App.tsx integration**:
  - Added `restorePointsOpen` and `importProjectOpen` state
  - File menu additions: "Import TunerStudio Project...", "Create Restore Point", "Restore Points..."
  - `handleCreateRestorePoint()` function with toast notification
  - Dialog rendering with refresh callbacks

### TunerStudio Project Compatibility - Completed Jan 7, 2026
- **Java Properties Parser** (`properties.rs`):
  - Full Java properties file format support
  - Backslash continuation lines, Unicode escapes (\uXXXX)
  - Comment handling (# and !), escaped special characters
  - TunerStudio-specific key escaping (spaces in keys like `Gauge\ Settings`)

- **Restore Points System** (`project.rs`):
  - `create_restore_point()` - Creates timestamped MSQ backup
  - `list_restore_points()` - Lists all restore points with metadata
  - `load_restore_point()` - Restores tune from backup
  - `delete_restore_point()` - Removes specific restore point
  - `prune_restore_points()` - Keeps only N most recent backups

- **PC Variables Persistence** (`file.rs`, `project.rs`):
  - Added `pc_variables: HashMap<String, TuneValue>` to TuneFile
  - Separate parsing for `<pcVariable>` elements
  - `save_pc_variables()` / `load_pc_variables()` for pcVariableValues.msq
  - Page -1 convention for PC variable storage

- **TunerStudio Project Import** (`project.rs`):
  - `import_tunerstudio()` reads project.properties and converts format
  - Copies CurrentTune.msq, pcVariableValues.msq, restore points
  - Extracts connection settings (port, baud rate)
  - Preserves INI file and signature

- **New Tauri Commands** (`lib.rs`):
  - `create_restore_point` - Create backup from current tune
  - `list_restore_points` - List all backups for project
  - `load_restore_point` - Load a backup as current tune
  - `delete_restore_point` - Remove a backup
  - `import_tunerstudio_project` - Import TS project folder

- **Bug Fix**: Table editor blue highlighting caused by CSS `::selection`
  - Added `user-select: none` to `.table-editor` in TableEditor.css

### Resilient ECU Sync & Mismatch Handling - Completed Jan 5, 2026
- **Problem**: ECU protocol error (status 132) shown as scary dialog when INI doesn't match ECU
- **Root Cause**: Sync started immediately after connect before user could respond to mismatch dialog
- **Solution**: Return mismatch info directly from connect, skip auto-sync on mismatch

- **Backend changes** (`lib.rs`):
  - `ConnectResult` struct returns signature + optional mismatch_info directly
  - `SyncResult` struct tracks pages_synced, pages_failed, total_pages, errors
  - `sync_ecu_data` now continues on page failures instead of aborting
  - `SyncProgress` includes `failed_page` field for per-page failure tracking

- **Frontend changes** (`App.tsx`):
  - `ConnectResult` and `SyncResult` TypeScript interfaces added
  - `SyncStatus` state tracks partial sync for status bar indicator
  - `doSync()` helper function with resilient error handling
  - `connect()` now handles mismatch from return value, skips auto-sync
  - Dialog callbacks trigger sync after user decision
  - Status bar shows "⚠ Partial sync (X/Y)" when pages_failed > 0
  - INI change listener uses resilient `doSync()` function

### INI Signature Mismatch Detection & Online Repository - Completed Jan 4, 2026
- **Backend signature comparison system**:
  - `SignatureMatchType` enum: `Exact`, `Partial`, `Mismatch`
  - `SignatureMismatchInfo` struct with ECU signature, INI signature, match type
  - `compare_signatures()` helper function for signature comparison
  - `find_matching_inis_internal()` searches local repository for matches
  - Emits `signature:mismatch` event when ECU/INI signatures don't match

- **New Tauri commands**:
  - `find_matching_inis(ecu_signature)` - Find INIs matching ECU signature
  - `update_project_ini(ini_path, force_resync)` - Switch INI with optional re-sync
  - `check_internet_connectivity()` - Check if GitHub is reachable
  - `search_online_inis(signature)` - Search Speeduino/rusEFI GitHub repos
  - `download_ini(download_url, name, source)` - Download INI from GitHub

- **Online INI repository module** (`online_repository.rs`):
  - `OnlineIniRepository` client with reqwest HTTP
  - `IniSource` enum: `Speeduino`, `RusEFI`, `Custom`
  - GitHub API integration for listing INI files
  - Download and import to local repository

- **SignatureMismatchDialog component**:
  - Shows ECU vs INI signature comparison
  - Lists matching INIs from local repository
  - "Search Online" button opens GitHub search
  - Connectivity check with "No Internet" message
  - Download buttons for online INIs
  - "Continue Anyway" option for advanced users

- **App.tsx integration**:
  - Listens for `signature:mismatch` events
  - Listens for `ini:changed` events (triggers re-sync)
  - Shows SignatureMismatchDialog when mismatch detected

- **Files created/modified**:
  - `crates/libretune-core/src/project/online_repository.rs` - New online repo module
  - `crates/libretune-app/src/components/dialogs/SignatureMismatchDialog.tsx` - New dialog
  - `crates/libretune-app/src/components/dialogs/SignatureMismatchDialog.css` - Styling
  - `crates/libretune-app/src-tauri/src/lib.rs` - New commands and AppState field

### Dashboard Visual Fixes & Context Menu - Completed Jan 4, 2026
- **Fixed visual glitches**:
  - Canvas transform accumulation: Added `ctx.setTransform(1,0,0,1,0,0)` reset before scaling
  - Added null/undefined guards to `tsColorToRgba()` and `tsColorToHex()` functions
  - Added bounds checking for gauge position values (clamp to 0-1 range)
  - Added default values for analog gauge angles (225° start, 270° sweep)

- **Removed hardcoded reference paths**:
  - `list_available_dashes()` now uses `get_dashboards_dir()` helper
  - Dashboards stored in `<app_data>/dashboards/` (cross-platform)
  - Supports `.ltdash.xml` (native) and `.dash` (TunerStudio import)

- **Auto-generated default dashboards**:
  - Creates Basic, Tuning, Racing dashboards on first run
  - `create_default_dashboard_files()` function in lib.rs
  - Files saved as `.ltdash.xml` format

- **Right-click context menu** (`GaugeContextMenu.tsx`):
  - Reload Default Gauges
  - LibreTune Gauges → (categories from INI)
  - Reset Value
  - Background → (color, dither, image, position)
  - Antialiasing Enabled (toggle)
  - Designer Mode (toggle)
  - Gauge Demo (animates gauges with fake data)

- **Gauge interactivity enabled**:
  - Removed `pointer-events: none` from gauges
  - Added hover glow effect on gauges
  - Added designer mode styles (dashed borders, selection highlight)
  - Right-click opens context menu on gauge or background

### TunerStudio Dashboard Rewrite - Completed Jan 4, 2026
- **Complete rewrite of dashboard system to use TunerStudio format natively**:
  - Replaced TabbedDashboard with new TsDashboard component
  - New files created:
    - `crates/libretune-app/src/components/dashboards/TsDashboard.tsx` - Main dashboard with selector
    - `crates/libretune-app/src/components/dashboards/TsDashboard.css` - Styling
    - `crates/libretune-app/src/components/dashboards/dashTypes.ts` - TypeScript types matching Rust
    - `crates/libretune-app/src/components/gauges/TsGauge.tsx` - Canvas gauge renderer (7 types)
    - `crates/libretune-app/src/components/gauges/TsIndicator.tsx` - Boolean indicator renderer
  
- **Backend commands added** (`lib.rs`):
  - `get_dash_file(path: String) -> DashFile` - Load full DashFile structure
  - `list_available_dashes() -> Vec<DashFileInfo>` - List available .dash files

- **TsGauge implementation** (7 of 13 GaugePainter types):
  - BasicReadout: Digital numeric display with units
  - HorizontalBarGauge: Horizontal progress bar with gradient
  - VerticalBarGauge: Vertical progress bar with gradient  
  - AnalogGauge: Classic circular dial with needle, ticks, warning arcs
  - AsymmetricSweepGauge: Curved sweep gauge with arc
  - HorizontalLineGauge: Horizontal line indicator
  - VerticalDashedBar: Vertical dashed bar gauge

- **App.tsx integration**:
  - Replaced `TabbedDashboard` import with `TsDashboard`
  - Removed legacy indicator settings (indicatorColumnCount, indicatorFillEmpty, indicatorTextFit)
  - Dashboard now loads TunerStudio .dash files from `reference/TunerStudioMS/Dash/`

### TunerStudio Dashboard Format Implementation - Completed Jan 4, 2026
- **Implemented full TunerStudio dashboard XML format support**:
  - Created new `dash` module in libretune-core with parser and writer
  - Files: `crates/libretune-core/src/dash/{mod.rs,types.rs,parser.rs,writer.rs}`
  - XML namespace: `http://www.EFIAnalytics.com/:dsh` and `:gauge`
  - Supports file format version 3.0

- **Data structures implemented** (`dash/types.rs`):
  - `TsColor`: ARGB color with CSS hex and Java-style integer conversion
  - `GaugePainter` enum: 13 gauge types (AnalogGauge, BasicReadout, HorizontalBarGauge, etc.)
  - `IndicatorPainter` enum: LED and image-based indicators
  - `GaugeConfig`: 40+ properties matching TunerStudio exactly
  - `IndicatorConfig`: Boolean indicator with on/off states
  - `DashComponent` enum: Gauge | Indicator
  - `GaugeCluster`: Container with background, embedded images, components
  - `DashFile`/`GaugeFile`: Top-level file structures with bibliography and version info

- **XML parsing** (`dash/parser.rs`):
  - Parses TunerStudio .dash and .gauge files
  - Handles color elements with ARGB attributes
  - Supports embedded base64 images/fonts
  - Unit tests for parsing and color conversion

- **XML writing** (`dash/writer.rs`):
  - Writes TunerStudio v3.0 format
  - Round-trip test validates parse → write → parse produces same data

- **Backend commands updated** (`lib.rs`):
  - `save_dashboard_layout`: Now writes TunerStudio XML format
  - `load_dashboard_layout`: Reads XML (with JSON fallback for backward compatibility)
  - `create_default_dashboard`: Creates dashboard from template (basic, racing, tuning)
  - `get_dashboard_templates`: Returns available template info

- **Dashboard templates created** (3 legally-distinct layouts):
  - **Basic**: Essential gauges - RPM, AFR, Coolant, IAT, TPS, MAP, Battery, Advance, VE, PW
  - **Racing**: Large center RPM with oil pressure, water temp, speed, AFR, boost, fuel
  - **Tuning**: 3x3 grid of tuning-relevant readouts (RPM, AFR, MAP, TPS, VE, ADV, EGT, PW, DUTY)

- **Frontend updates**:
  - `TabbedDashboard.tsx`: Added template selector dialog and "New from Template" button
  - Added `DashboardTemplateInfo` interface
  - Added template loading useEffect and `handleCreateFromTemplate` function
  - `TabbedDashboard.css`: Added template selector styles

- **Dependencies added**:
  - `quick-xml = { version = "0.37", features = ["serialize"] }` - XML parsing
  - `base64 = "0.22"` - Embedded resource encoding
  - `chrono = { workspace = true }` - Date formatting for bibliography

### Realtime Streaming & AutoTune Heatmaps - Completed Dec 26, 2025
- **Implemented event-based realtime streaming**:
  - Added `start_realtime_stream` and `stop_realtime_stream` Tauri commands
  - Backend spawns tokio task emitting `realtime:update` events every 100ms
  - Frontend listens to events instead of polling (fallback to polling if events fail)
  - Files: `lib.rs` (backend), `App.tsx` (frontend)

- **Implemented AutoTune heatmap calculations**:
  - Added `get_autotune_heatmap` command returning weighting and change magnitude per cell
  - Frontend fetches heatmap data and renders overlays in AutoTune component
  - Added unit test for heatmap recommendation accumulation
  - Files: `lib.rs` (AutoTuneHeatEntry struct + command), `AutoTune.tsx`, `tests/autotune_heatmap.rs`

- **Fixed dashboard gauge layout**:
  - Adjusted gauge positions and sizes (x: 0.05-0.65, width: 0.25-0.30) to prevent overlap
  - Increased container min-height to 500px and reduced padding for better space usage
  - Files: `TabbedDashboard.tsx`, `TabbedDashboard.css`

- **Fixed WebKit launch crash**:
  - Root cause: Snap environment vars caused WebKit to load incompatible libpthread
  - Created `scripts/tauri-dev.sh` wrapper to launch with sanitized environment
  - Documented fix in AGENTS.md

### Trademark Cleanup - Completed Dec 26, 2025
- **Renamed "VE Analyze" to "AutoTune"** throughout entire codebase to avoid TunerStudio trademark
  - Frontend: `VEAnalyze.tsx` → `AutoTune.tsx`
  - Backend: `ve_analyze.rs` → `autotune.rs`
  - All structs/functions renamed (VEAnalyzeState → AutoTuneState, etc.)
  
- **Replaced all "TunerStudio" references** in code comments with generic terminology:
  - "TunerStudio-compatible" → "INI definition compatible"
  - "TunerStudio patterns" → "standard ECU tuning patterns"
  - "TunerStudio's features" → "ECU tuning features"
  
- **Files updated (28 total)**:
  - Documentation: README.md, AGENTS.md, IMPLEMENTATION_TODO.md
  - Frontend: All dialog components, HotkeyManager, ActionManagement, mod.rs files
  - Backend: ini/parser.rs, ini/mod.rs, ini/expression.rs, lib.rs

### Gauge Rendering Integration - Completed Dec 26, 2025
- **Fixed gauge display issues**:
  - Removed triple-nested wrappers causing rendering problems
  - Fixed CSS pointer-events blocking interaction
  - Established proper component hierarchy with absolute positioning
  
- **Implemented Analog Dial Gauge**:
  - Canvas-based drawing with 270° arc
  - Major/minor tick marks with value labels
  - Warning zones (orange/red arcs)
  - Animated needle with drop shadow
  - Value display with units
  
- **Wired realtime data to gauges**:
  - Data flow: App.tsx (100ms polling) → TabbedDashboard → GaugeRenderer
  - Default gauges configured: RPM (Analog), AFR (Digital), Coolant (Bar)

### App.tsx Refactoring & Component Extraction - Completed Dec 27, 2025
- **Extracted reusable layout components**:
  - `Header.tsx` - Top bar with ECU status and action buttons
  - `Sidebar.tsx` - Navigation sidebar with menu tree
  - `Overlays.tsx` - All modal overlay dialogs consolidated
  - `DialogRenderer.tsx` - Dialog rendering utilities
  - Files: `crates/libretune-app/src/components/layout/`

- **Created standalone dialog components**:
  - `RebinDialog.tsx` - Table re-binning with add/remove bins, linear spacing, interpolation
  - `CellEditDialog.tsx` - Cell value editor with +/- buttons, validation, min/max limits
  - Files: `crates/libretune-app/src/components/dialogs/`

- **Reduced App.tsx complexity**:
  - Reduced from 1,157 lines to 626 lines (46% reduction)
  - Consolidated 10 boolean dialog states into OverlayState object
  - Added openOverlay/closeOverlay helper functions

- **Integrated new dialogs into TableEditor2D**:
  - Added handleRebin(newXBins, newYBins, interpolateZ) callback
  - Added handleCellEditApply(value) for cell editing
  - Added onCellDoubleClick prop to TableGrid component

### CI/CD & Unit Tests - Completed Dec 27, 2025
- **Created GitHub Actions CI workflow**:
  - File: `.github/workflows/ci.yml`
  - 4 jobs: test, format, frontend, build
  - Multi-platform builds (ubuntu, macos, windows)
  - Runs on push to main and pull requests

- **Added table_ops unit tests**:
  - File: `crates/libretune-core/tests/table_ops.rs`
  - Tests for: rebin_table, scale_cells, set_cells_equal, interpolate_cells
  - 5 passing tests (smooth_table tests added once bug was fixed)

- **Added platform-specific corpus tests** (Jan 2026):
  - File: `crates/libretune-core/tests/corpus.rs`
  - `test_parse_all_corpus_inis()` - All 687 INI files in reference/ecuDef must parse (100% pass)
  - `test_speeduino_ini_fields()` - Speeduino-specific validation
  - `test_rusefi_ini_fields()` - rusEFI validation (excludes FOME/epicEFI)
  - `test_fome_ini_fields()` - Tests ALL FOME files (currently 2)
  - `test_epicefi_ini_fields()` - Samples 10 epicEFI files for efficiency

- **Test results**: 84+ passed, 2 ignored across all test files

### Known Issues
- (none currently tracked at this level — see CHANGELOG / issue tracker)

### lastOffset Keyword Support - Completed Dec 31, 2025
- **Root Cause**: Constants like `afrTable` use `lastOffset` keyword instead of numeric offset
  - INI line: `afrTable = array, U08, lastOffset, [16x16], "AFR", 0.1, 0.0, 7, 25.5, 1`
  - Parser was returning None when offset couldn't parse as u16, skipping the constant entirely
  
- **Implementation**:
  - Added `last_offset: u16` field to `ParserState` struct (parser.rs)
  - `last_offset` resets to 0 when page changes
  - After parsing each constant, `last_offset` is updated to `offset + size_in_bytes`
  - `parse_constant_line()` now accepts `last_offset` parameter
  - When offset field equals "lastOffset" (case-insensitive), uses the running counter value
  
- **Files modified**:
  - [parser.rs](crates/libretune-core/src/ini/parser.rs) - ParserState, parse_constants_entry
  - [constants.rs](crates/libretune-core/src/ini/constants.rs) - parse_constant_line signature and logic
  
- **Test added**: `test_parse_constant_line_lastoffset` verifies keyword handling

### MenuBar Duplicate Key Fix - Completed Dec 31, 2025
- **Issue**: React warning about duplicate keys for menu separators
- **Fix**: Updated `renderMenuItem()` to use index-based unique keys
- **File**: [MenuBar.tsx](crates/libretune-app/src/components/layout/MenuBar.tsx)

### PcVariables & std_separator Fix - Completed Dec 31, 2025
- **Issue 1 - std_separator showing as menu item**:
  - Root Cause: `std_separator` targets created `MenuItem::Std` instead of `MenuItem::Separator`
  - Fix: Added check for `target == "std_separator"` before the `target.starts_with("std_")` check in [parser.rs](crates/libretune-core/src/ini/parser.rs#L1242)

- **Issue 2 - PcVariables not available to dialogs** ("Loading..." shown):
  - Root Cause: `[PcVariables]` like `rpmwarn`, `rpmdang` were only stored as byte values, not full `Constant` structs
  - Fix: Added `parse_pc_variable_line()` function to create proper constants from PcVariables
  
- **Implementation**:
  - Added `is_pc_variable: bool` field to `Constant` struct (default `false`)
  - Added `parse_pc_variable_line()` in [constants.rs](crates/libretune-core/src/ini/constants.rs) - parses PcVariable format (no offset field)
  - Updated `parse_pc_variable_entry()` to use new function and store in `def.constants`
  - Added `default_values: HashMap<String, f64>` to `EcuDefinition` for INI defaults
  - Added `[Defaults]` section parsing (`defaultValue = name, value` entries)
  - Added `local_values: HashMap<String, f64>` to `TuneCache` for PC variable storage
  - Updated `get_constant_value` to check `is_pc_variable` and return from local cache or defaults
  - Updated `update_constant` to store PC variables locally instead of writing to ECU

- **Value Resolution Order** for PcVariables:
  1. User-set value in `cache.local_values`
  2. INI default in `def.default_values`
  3. Constant min value (last resort)

- **Tests added**: `test_parse_pc_variable_line_scalar`, `test_parse_pc_variable_line_bits`

### Current Status
- **Build Status**: All builds passing (Rust + TypeScript)
- **Test Status**: 84+ tests passing, 2 ignored
- **CI Status**: GitHub Actions workflow ready
- **Trademark Status**: Clean - all proprietary terminology removed
- **UI Status**: Gauges rendering correctly with live data; Dashboard tab protected from accidental close
- **Backend Status**: AutoTune module functional with heatmap support
- **Realtime Streaming**: Event-based streaming (50ms intervals) with `try_lock()` contention handling and lock-holder diagnostics
- **Connection Lock**: `get_all_constant_values` no longer holds connection lock; reads from cache/tune only
- **Cross-Platform**: Fully cross-platform path handling (Windows/macOS/Linux)
- **Dashboard Format**: TunerStudio XML format support with 3 default templates

### Cross-Platform Path Implementation - Completed
- **Replaced hardcoded Unix paths** with Tauri's cross-platform APIs:
  - `get_app_data_dir()` - Uses `tauri::AppHandle::path().app_data_dir()` with `dirs` crate fallback
  - `get_projects_dir()` - Cross-platform projects directory
  - `get_definitions_dir()` - Cross-platform ECU definitions directory
  - `get_settings_path()` - Cross-platform settings.json location

- **Updated lib.rs commands**:
  - `get_available_inis()` - Now accepts `app: tauri::AppHandle`
  - `load_ini()` - Now uses `Path::new(&path).is_absolute()` instead of Unix-specific check
  - `auto_load_last_ini()` - Now accepts app handle
  - `save_tune()` - Uses `Project::projects_dir()` from libretune-core
  - `list_tune_files()` - Uses cross-platform path resolution
  - `list_dashboard_layouts()` - Uses cross-platform path resolution

- **Updated dashboard.rs**:
  - `get_dashboard_file_path()` - Uses `Project::projects_dir()` from project module

- **Added Linux-only guard** in serial.rs:
  - `/dev` directory fallback scan wrapped in `#[cfg(target_os = "linux")]`

- **Added dirs crate** to libretune-app/Cargo.toml for fallback path resolution

### WebKit / Tauri Launch Fix
**Issue**: Tauri app crashed on startup with WebKit internal error due to Snap environment variable leakage causing libpthread symbol lookup failures.

**Solution**: Launch Tauri with a sanitized environment to prevent Snap library paths from interfering:
```bash
./scripts/tauri-dev.sh
```

Or manually:
```bash
env -i PATH="$PATH" HOME="$HOME" DISPLAY="$DISPLAY" XAUTHORITY="$XAUTHORITY" \
  XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" TERM="$TERM" \
  bash -lc 'cd crates/libretune-app && npm run tauri dev'
```

This preserves only essential environment variables (PATH, HOME, DISPLAY) and removes Snap-related vars that cause WebKit to load incompatible libraries.

### Multi-ECU Support & Dynamic Configuration - Completed Dec 31, 2025

**Table Map Name Lookup Fix** (fixes "Fuel Table" not opening):
- **Root Cause**: INI `[TableEditor]` format: `table = tableName, mapName, "Title", page`
  - Tables indexed by `tableName` (e.g., "veTable1Tbl")
  - Menus reference tables by `mapName` (e.g., "veTable1Map")
  - Lookup was failing because it only checked by tableName

- **Changes made**:
  - Added `map_name: Option<String>` field to `TableDefinition` in [tables.rs](crates/libretune-core/src/ini/tables.rs)
  - Added `table_map_to_name: HashMap<String, String>` to `EcuDefinition` in [mod.rs](crates/libretune-core/src/ini/mod.rs)
  - Updated parser to store map_name and build reverse lookup in [parser.rs](crates/libretune-core/src/ini/parser.rs)
  - Added `get_table_by_name_or_map()` method for fallback lookup
  - Updated all `def.tables.get()` calls in [lib.rs](crates/libretune-app/src-tauri/src/lib.rs) to use new resolver

**Channel Discovery API** (enables dynamic status bar & dashboard):
- Added Tauri commands:
  - `get_available_channels()` - Returns all output channels from INI [OutputChannels]
  - `get_status_bar_defaults()` - Returns suggested channels from FrontPage or common defaults

**Dynamic Status Bar** (App.tsx):
- Removed hardcoded "RPM" and "AFR" status indicators
- Status bar now shows channels from `get_status_bar_defaults()` API
- Falls back to common channel names (RPM, AFR, MAP, TPS, coolant) if FrontPage unavailable
- Added `statusBarChannels` state initialized from backend

**Dynamic Dashboard Gauges** (TabbedDashboard.tsx):
- Removed 5 hardcoded gauge definitions (rpm, afr, clt, tps, map)
- Dashboard now builds gauges from:
  1. FrontPage gauge references (if available)
  2. First 4 gauges from [GaugeConfigurations] (fallback)
  3. Minimal single-gauge fallback (last resort)
- Added `BackendGaugeInfo` interface matching lib.rs `GaugeInfo` struct
- `buildGaugeConfig()` helper creates GaugeConfig from INI gauge definitions
- Min/max/units/warnings now come from INI instead of hardcoded values

**Files modified**:
- Backend: tables.rs, mod.rs, parser.rs, lib.rs
- Frontend: App.tsx, TabbedDashboard.tsx

### Comprehensive User Manual Documentation Audit - Completed Feb 3, 2026
- **Overall Task**: Systematic review of entire project to ensure all implemented features are documented in user manual
- **Methodology**: Audited MenuBar.tsx menu items against existing documentation, identified gaps, created comprehensive new docs

- **Audit Results**: 
  - Total menu items: 30+
  - Documented items: 20 (67%)
  - Undocumented items identified: 10 (33%)
  - All gaps now closed with new documentation

- **Undocumented Features Found**:
  1. Performance Calculator (Tuning menu) - ✅ NOW DOCUMENTED
  2. Diagnostic Loggers (Tuning menu) - ✅ NOW DOCUMENTED  
  3. Table Comparison (Tools menu) - ✅ NOW DOCUMENTED
  4. Action Manager (Tools menu) - ✅ NOW DOCUMENTED
  5. Reset to Defaults (Tools menu) - ✅ NOW DOCUMENTED
  6. Tooth Logger (Tuning menu) - ✅ NOW DOCUMENTED
  7. Composite Logger (Tuning menu) - ✅ NOW DOCUMENTED
  8. Settings & Preferences (File menu) - ✅ NOW DOCUMENTED
  9. Data Logger details (View menu) - ✅ ENHANCED
  10. ECU Console (Tools menu - rusEFI only) - ✅ REFERENCE ADDED

- **New Documentation Created** (~1,900 lines total):
  1. **Performance Calculator Guide** (`docs/src/features/performance-calculator.md`, 400+ lines)
     - Vehicle specifications (weight, tires, gearing, drag coefficient)
     - Engine settings (type, displacement, target AFR, boost)
     - Understanding power/torque curves and acceleration times
     - Factors that affect results (tuning impact, vehicle impact, environmental)
     - Workflow examples (stock NA, turbocharged, tune comparisons)
     - Real-world scenarios with examples
     - Limitations and disclaimers
     - Physics explanations for accuracy

  2. **Diagnostic Loggers Guide** (`docs/src/features/diagnostic-loggers.md`, 350+ lines)
     - Tooth Logger: Individual crank teeth timing analysis, when to use, configuration, troubleshooting
     - Composite Logger: Multi-signal synchronization, primary/secondary timing, voltage monitoring
     - Common issues and solutions (missing teeth, noisy signals, sync problems)
     - Signal interpretation guide (good vs bad data)
     - Integration with AutoTune and dashboards
     - Data export options

  3. **Tools Guide** (`docs/src/features/tools.md`, 350+ lines)
     - Table Comparison: Side-by-side tune diffing, comparison modes (values/percentages/heatmap)
     - Use cases (verifying AutoTune, documenting progression, identifying problems)
     - Action Manager: Record and replay tuning actions, templates, collaboration
     - Reset to Defaults: Emergency tune reset with warnings
     - Action types and export formats
     - Workflow examples

  4. **Settings & Preferences Guide** (`docs/src/getting-started/settings.md`, 450+ lines)
     - Connection settings (port, baud rate, timeouts, reconnection)
     - Display preferences (theme, font size, table grid)
     - Unit preferences (temperature, pressure, speed, AFR/Lambda)
     - Version control (Git integration, branches, auto-commit)
     - AutoTune defaults (authority limits, filters)
     - Dashboard, logging, calculator defaults
     - Advanced settings (validation, keyboard, networking)
     - Reset options and backup information
     - First-time setup and tuning prep workflows

- **Documentation System Updates**:
  1. **toc.json** (`crates/libretune-app/public/manual/`):
     - Added 4 new entries to navigation structure
     - Core Features: +3 (Performance Calculator, Diagnostic Loggers, Tools)
     - Getting Started: +1 (Settings & Preferences)

  2. **docs/src/SUMMARY.md**:
     - Added 4 new markdown links to match toc.json structure
     - Maintains consistency between in-app and static documentation

  3. **File Sync**:
     - All 4 new .md files created in BOTH locations:
       - `crates/libretune-app/public/manual/` (in-app manual)
       - `docs/src/` (static website documentation)
     - Keeps dual documentation systems synchronized

- **Documentation Quality**:
  - Consistent formatting with existing manual pages
  - Cross-links between related documentation pages
  - "See Also" sections pointing to complementary features
  - Real-world examples and use cases
  - Troubleshooting sections for diagnostic tools
  - Warnings and limitations clearly marked
  - Keyboard shortcuts documented where applicable

- **Outcome**:
  - ✅ Zero undocumented menu items - all features now have comprehensive docs
  - ✅ ~1,900 new lines of documentation across 4 files
  - ✅ Proper TOC integration for navigation sidebar
  - ✅ Dual documentation maintained (in-app + static website)
  - ✅ Ready for next release with complete feature coverage

### Java Plugin System Deprecation - Completed Feb 4, 2026
- **Overall Task**: Deprecate Java/JVM plugin system in favor of native WASM plugin system (Sprint 3)
- **Status**: Step 1 Complete - UI disabled, deprecation notices added, grace period begins
- **Timeline**: 2-4 releases (6-12 months) before complete removal

- **Step 1 Implementation (Feb 4, 2026)**:
  1. **Created** (`DEPRECATION_NOTICE.md`):
     - Comprehensive 400-line deprecation announcement
     - Timeline: Feb 4, 2026 → Grace period → Removal (TBD)
     - Migration guide preview (Java → WASM)
     - Community feedback request
     - FAQ section addressing user concerns
     - Technical details on affected components (~5,100 lines)
  
  2. **Updated** (`App.tsx`):
     - Commented out "Plugins..." menu item in Tools menu
     - Added deprecation comment with reference to DEPRECATION_NOTICE.md
     - Users cannot access Java plugin UI in new builds
  
  3. **Updated** (`lib.rs`):
     - Added deprecation warnings to `load_plugin()` command
     - Added deprecation warnings to `unload_plugin()` command
     - Added deprecation warnings to `get_plugin_ui()` command
     - All Java plugin commands now log ⚠️ warnings to stderr
  
  4. **Updated** (`AGENTS.md`):
     - Marked "Add plugin system" as COMPLETED (WASM system)
     - Added "Java/JVM plugin system" as DEPRECATED entry
     - This section documents the deprecation process

- **Affected Components** (~5,100 lines total):
  - **Java Source**: 19 files (~2,000 lines)
    - `plugin-host/src/main/java/com/libretune/pluginhost/` (8 files)
    - `plugin-host/src/main/java/com/tunerstudio/plugin/api/` (11 stubs)
  - **Rust Backend**: 4 files (~1,042 lines)
    - `src/plugin/{mod.rs,manager.rs,bridge.rs,types.rs}`
  - **TypeScript Frontend**: 7 files (~1,878 lines)
    - `src/components/plugin/` (PluginPanel, SwingRenderer, EventBridge, etc.)
  - **Tauri Commands**: 8 commands (~192 lines)
  - **Build System**: Gradle files, bundled JAR resources

- **Rationale**:
  - **Maintenance Burden**: Separate Java codebase, Gradle build, JRE detection
  - **Security Concerns**: Full JVM permissions, no sandboxing, no resource limits
  - **External Dependency**: Requires JRE 11+ on user systems
  - **Architectural Redundancy**: WASM plugin system provides superior isolation
  - **User Confusion**: Two competing plugin systems with no clear guidance

- **WASM Plugin System** (Sprint 3 - Active):
  - Location: `crates/libretune-core/src/{plugin_system.rs,plugin_api.rs}`
  - Features: Permission model, sandboxing, no external dependencies
  - Tests: 37 unit tests (18 plugin_system + 19 plugin_api)
  - Documentation: SPRINT_3_SUMMARY.md (comprehensive implementation guide)
  - UI: PluginPanel.tsx (250+ lines, glass-card design)

- **Next Steps** (Completed Steps 1-3, Feb 4-5, 2026):
  - [x] **Step 1**: Add deprecation notices and disable UI (Feb 4, 2026)
  - [x] **Step 2**: Create migration guide documentation (Feb 5, 2026)
  - [x] **Step 3**: Update documentation indices (Feb 5, 2026)
  - [ ] **Step 4**: Grace period monitoring (ongoing, 2-4 releases)
  - [ ] **Step 5**: Remove all Java plugin code after grace period (~5,100 lines)

- **Community Impact**:
  - **Unknown**: No telemetry on Java plugin usage
  - **Risk**: Users may rely on TunerStudio JAR plugins
  - **Mitigation**: Grace period, migration guide, example WASM plugins (Sprint 4)

- **Documentation Status**:
  - ✅ DEPRECATION_NOTICE.md created (comprehensive, 400+ lines)
  - ✅ README.md updated with deprecation notice
  - ✅ Migration guide created (docs/src/reference/java-to-wasm-migration.md)
  - ✅ Documentation indices updated (SUMMARY.md + toc.json)
  - ✅ Dual documentation system synchronized (docs/src + public/manual)
  - ⏳ User-facing announcement (website/forum TBD)

- **Code Status**:
  - ✅ UI disabled (menu item commented out)
  - ✅ Backend warnings added (stderr logging)
  - ✅ AGENTS.md updated (this section)
  - ✅ All code still functional (but deprecated)
  - ⏳ Removal scheduled for after grace period (Steps 4-5)

