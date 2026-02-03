# LibreTune Implementation TODO

## Phase 1: Backend (Rust Core) - COMPLETED

### AutoTune Module
- [x] Create `crates/libretune-core/src/autotune.rs`
  - [x] Implement AutoTuneRecommendation struct
  - [x] Implement AutoTuneSettings struct
  - [x] Implement AutoTuneFilters struct
  - [x] Implement AutoTuneAuthorityLimits struct
  - [x] Implement AutoTuneState struct
  - [x] Add `start_autotune` command
  - [x] Add `stop_autotune` command
  - [x] Add `get_autotune_recommendations` command
  - [x] Add `send_autotune_recommendations` command
  - [x] Add `burn_autotune_recommendations` command
  - [x] Add `lock_autotune_cells` command
  - [x] Add `unlock_autotune_cells` command
  - [x] Add `get_autotune_reference_tables` command
  - [x] Implement simplified auto-tune algorithm (basic version)

### Table Operations Module
- [x] Extend `crates/libretune-core/src/table_ops.rs`
  - [x] Add `rebin_table` command (change X/Y bins with interpolation)
  - [x] Add `smooth_table` command
  - [x] Add `interpolate_cells` command
  - [x] Add `scale_cells` command
  - [x] Add `set_cells_equal` command
  - [x] Add `get_table_history_trail` command

### Realtime Module
- [ ] Extend `crates/libretune-core/src/realtime.rs`
  - [ ] Add RealtimeUpdate event emission for dashboard streaming
  - [ ] Implement streaming support for efficient dashboard updates

### Dashboard Module
- [ ] Create `crates/libretune-core/src/dashboard.rs`
  - [ ] Implement DashboardLayout struct
  - [ ] Implement GaugeConfig struct
  - [ ] Implement GaugeType enum
  - [ ] Add `save_dashboard_layout` command
  - [ ] Add `load_dashboard_layout` command
  - [ ] Add `list_dashboard_layouts` command
  - [ ] Implement JSON serialization/deserialization

### Tauri Integration
- [x] Update `crates/libretune-app/src-tauri/src/lib.rs`
  - [x] Register all AutoTune commands
  - [x] Register all table operation commands
  - [x] Register all dashboard commands
  - [x] Test command registration with `cargo build -p libretune-app`

## Phase 2: UI Components (React)

### Shared Components
- [x] Create `crates/libretune-app/src/components/tables/TableToolbar.tsx`
  - [x] Set Equal (=) button
  - [x] Increase (>, +) buttons
  - [x] Decrease (<, -) buttons
  - [x] Scale (*) button
  - [x] Interpolate (/) button
  - [x] Smooth (s) button
  - [x] Re-bin button
  - [x] Copy/Paste buttons
  - [x] Tooltips showing hotkeys

- [x] Create `crates/libretune-app/src/components/tables/TableGrid.tsx`
  - [x] Editable X/Y axis bins
  - [x] Color-coded Z cells with value intensity
  - [x] History trail overlay (blue trace)
  - [x] Cell locking visual indicators
  - [x] Keyboard navigation support
  - [x] Cell selection (click + drag)

- [x] Create `crates/libretune-app/src/components/tables/TableContextMenu.tsx`
  - [x] Right-click context menu
  - [x] Menu items: Set Equal, Scale, Interpolate, Smooth, Lock/Unlock

- [x] Add CSS for table components
  - [ ] Table grid styling
  - [ ] Cell color coding
  - [ ] History trail animation
  - [ ] Toolbar styling
  - [ ] Context menu styling

### Table Editors
- [x] Complete `crates/libretune-app/src/components/tables/TableEditor2D.tsx`
  - [x] Integrate TableToolbar component
  - [x] Integrate TableGrid component
  - [x] Add TableContext integration
  - [x] Implement toolbar button handlers (call backend commands)
  - [x] Add keyboard navigation (arrow keys, Enter, Escape)
  - [x] Add re-binning dialog integration
  - [x] Add undo functionality
  - [x] Add clipboard support

- [x] Complete `crates/libretune-app/src/components/tables/TableEditor3D.tsx`
  - [x] Canvas-based 3D surface rendering
  - [x] Wireframe mesh rendering
  - [x] Z-value coloring on mesh
  - [x] Rotation controls (yaw M/K, roll N/J, pitch up/down)
  - [x] Z-axis scale slider
  - [x] Follow mode toggle
  - [x] Active cell highlight
  - [x] Selected XY coordinate display
  - [x] Add CSS for 3D editor

### AutoTune
- [ ] Complete `crates/libretune-app/src/components/tuner-ui/AutoTune.tsx`
  - [ ] Primary controls panel:
    - [ ] Update Controller checkbox
    - [ ] Send button
    - [ ] Burn button
    - [ ] Start/Stop button with status
  - [ ] Recommended table view:
        - [ ] Cell color coding (blue=richer, red=leaner)
        - [ ] Tooltips: Beginning Value, Hit Count, Hit Weighting, Target AFR, Hit %
    - [ ] Cell locking (right-click to lock/unlock)
    - [ ] Heat maps:
        - [ ] Cell Weighting (data coverage - green scale)
        - [ ] Cell Change (magnitude - blue to red scale)
    - [ ] Advanced Settings panel:
        - [ ] Authority Limits inputs (max value, max %)
        - [ ] Filter inputs (min/max RPM, Y-axis ranges, min CLT, custom filter)
        - [ ] Reference Tables links (Lambda Delay, AFR Target)

### Dashboard
- [ ] Complete `crates/libretune-app/src/components/dashboards/TabbedDashboard.tsx`
  - [ ] Dashboard tab management (add/rename/delete)
  - [ ] Tab selector (top of screen)
  - [ ] Gauge grid with drag & drop
  - [ ] Full-screen mode (double-click toggle)
  - [ ] Designer mode toggle
  - [ ] Add gauge button (opens gauge type selector)
  - [ ] Gauge properties panel (edit selected gauge)
  - [ ] Save/Load dashboard layouts

### Gauge Renderer
- [ ] Complete `crates/libretune-app/src/components/gauges/GaugeRenderer.tsx`
  - [ ] AnalogDial gauge:
    - [ ] Circular gauge with needle
    - [ ] Sweep angle (default 300° from TunerStudio)
    - [ ] Tick marks (major/minor)
    - [ ] Value display at 180°
  - [ ] DigitalReadout:
    - [ ] Text-only value display
    - [ ] Configurable decimals
  - [ ] BarGauge:
    - [ ] Horizontal/vertical progress bar
    - [ ] Configurable orientation
  - [ ] SweepGauge:
    - [ ] Semi-circular 180° gauge
    - [ ] Needle-style indicator
    - [ ] LEDIndicator:
    - [ ] On/off state with color change
    - [ ] Configurable on/off colors
  - [ ] Common features:
        - [ ] Min/Max values
        - [ ] Warning/Critical zones with colors
        - [ ] Configurable font, needle, trim colors
        - [ ] Show history toggle
        - [ ] Show min/max display

### Dialogs
- [ ] Complete `crates/libretune-app/src/components/dialogs/SaveLoadBurnDialogs.tsx`
  - [ ] Save tune (.msq) dialog
  - [ ] Load tune dialog
  - [ ] Burn to ECU button
  - [ ] Auto-burn settings integration

- [ ] Complete `crates/libretune-app/src/components/dialogs/PerformanceFieldsDialog.tsx`
  - [ ] Vehicle specs form:
    - [ ] Primary injector size (cc)
    - [ ] Fuel flow preferences
    - [ ] Input source (GPS/VSS selection)
    - [ ] Vehicle weight
    - [ ] Vehicle weight units
    - [ ] Tire pressure
    - [ ] Frontal area
    - [ ] Frontal area units
    - [ ] Aerodynamic drag coefficient
    - [ ] Log power fields selection

- [ ] Complete `crates/libretune-app/src/components/dialogs/NewProjectDialog.tsx`
  - [ ] Project creation wizard
  - [ ] Project name input
  - [ ] ECU type selection
  - [ ] INI file selection

- [ ] Complete `crates/libretune-app/src/components/dialogs/BrowseProjectsDialog.tsx`
  - [ ] Project list view
  - [ ] Create new project button
  - [ ] Delete project button
  - [ ] Open project button

- [ ] Complete `crates/libretune-app/src/components/dialogs/RebinDialog.tsx`
  - [ ] X axis bins editor
  - [ ] Y axis bins editor
  - [ ] Interpolate Z values checkbox
  - [ ] Apply button
  - [ ] Cancel button

- [ ] Complete `crates/libretune-app/src/components/dialogs/CellEditDialog.tsx`
  - [ ] Cell coordinates display
  - [ ] Value input
  - [ ] Apply button
  - [ ] Cancel button

### Menu & Navigation
- [ ] Complete `crates/libretune-app/src/components/MenuManager.tsx`
  - [ ] INI [Menu] section parsing
  - [ ] Hierarchical menu tree rendering
  - [ ] Conditional item display based on ECU settings
  - [ ] Handle std_realtime special item (opens dashboard)

- [ ] Complete `crates/libretune-app/src/components/HotkeyManager.tsx`
  - [ ] Global keyboard event listener
  - [ ] Context-sensitive hotkeys (different in tables vs dialogs)
  - [ ] Hotkey hint display in UI

### Action Management
- [ ] Complete `crates/libretune-app/src/components/ActionManagement.tsx`
  - [ ] Action list with CRUD operations
  - [ ] Action queue system
  - [ ] Execute actions in sequence
  - [ ] Record/playback actions
  - [ ] Save/load action sets

## Phase 3: Integration (App.tsx)

- [x] Add missing imports (Play, MoreHorizontal from lucide-react)
- [ ] Fix `showAutoTune` render logic
- [ ] Replace sidebar placeholders:
    - [ ] AutoTune → Opens AutoTune component
    - [ ] Performance → Opens PerformanceFieldsDialog
    - [ ] Actions → Opens ActionManagement component
- [ ] Integrate TabbedDashboard into main content area
- [ ] Add global state for:
    - [ ] showAutoTune boolean
    - [ ] showPerformanceDialog boolean
    - [ ] showActionsPanel boolean
    - [ ] dashboardLayouts array
- [ ] Connect TableEditor2D/3D to backend commands
- [ ] Connect AutoTune to backend commands
- [ ] Connect TabbedDashboard to backend commands
- [ ] Connect all dialogs to backend commands

## Phase 4: Testing

### Backend Testing
- [ ] Write Rust unit tests for AutoTune
- [ ] Write Rust unit tests for table operations
- [ ] Write Rust unit tests for dashboard persistence
- [ ] Run `cargo test -p libretune-core`
- [ ] Run `cargo clippy -p libretune-core`

### Frontend Testing
- [ ] Write React component tests
- [ ] Run `npm test` for each component
- [ ] Integration testing with ECU connection

## Phase 5: Documentation

- [x] Create AGENTS.md (done)
- [ ] Update README.md with new features
- [ ] Add user guide for AutoTune
- [ ] Add user guide for dashboard designer
- [ ] Document all keyboard shortcuts
- [ ] Create screenshots for feature showcase

## Notes

- Backend AutoTune and table operations implemented but may need algorithm refinement
- App.tsx integration incomplete - requires more work
- All Tauri commands registered but not yet tested
