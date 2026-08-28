/**
 * AutoTune - Real-time VE table auto-tuning component.
 * 
 * Provides automatic VE table correction recommendations based on wideband O2
 * sensor feedback. Monitors engine operation and suggests cell adjustments
 * to achieve target AFR values.
 * 
 * Features:
 * - Real-time AFR monitoring and correction calculation
 * - Heat map visualization (data coverage, change magnitude)
 * - Cell locking to exclude specific cells from tuning
 * - Configurable filters (RPM, TPS, CLT, steady-state)
 * - Authority limits to prevent over-correction
 * - Lambda delay compensation for accurate cell attribution
 * - Transient filtering to ignore acceleration enrichment
 * - Import/export recommendations as CSV
 * 
 * @example
 * ```tsx
 * <AutoTune
 *   tableName="veTable1Tbl"
 *   onClose={() => closeTab()}
 * />
 * ```
 * 
 * @see {@link AutoTuneSettings} for tuning configuration
 * @see {@link AutoTuneFilters} for data filtering options
 * @see {@link AutoTuneAuthorityLimits} for correction limits
 */

import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { FolderOpen, Save, Square, Play, Upload, X, Lock, LockOpen } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { valueToHeatmapColor } from '../../utils/heatmapColors';
import { TuneHealthCard } from './TuneHealth';
import { useToast } from '../../contexts/ToastContext';
import { useChannels } from '../../stores/realtimeStore';
import './AutoTune.css';

// =============================================================================
// Types
// =============================================================================

/**
 * AutoTune session settings.
 */
interface AutoTuneSettings {
  /** Target AFR for corrections (e.g., 14.7 for stoich) */
  target_afr: number;
  /** How much a sample counts toward the cell it lands in. */
  hit_weighting: 'uniform' | 'cell_proximity' | 'cell_proximity_squared' | 'cell_centre_only';
  /** Accumulated weight at which a cell proposes its full change (TS: baseWeight, 20). */
  base_weight: number;
  /** Smallest change worth making, in table units (TS: minChangeThreshold, 1). */
  min_change: number;
  /** Algorithm name (e.g., 'proportional', 'integral') */
  algorithm: string;
  /** How often to process data in milliseconds */
  update_rate_ms: number;
  /**
   * Fixed lambda/AFR transport delay in ms (0 = auto: per-cell table if set,
   * else the RPM-based curve). Set a measured value here — the RPM curve tops
   * out at ~200 ms, far short of a real exhaust's dead time. When flow-scaled
   * (below) is on, this is the delay at the low-flow (idle/cruise) anchor.
   */
  lambda_delay_ms: number;
  /**
   * Build a per-cell delay table scaled by exhaust flow (rpm·load·VE) instead
   * of one fixed delay: long at idle, short at high load. lambda_delay_ms
   * anchors the low-flow end; lambda_delay_floor_ms is the high-flow floor.
   */
  lambda_delay_flow_scaled: boolean;
  /** High-flow floor (ms) for the flow-scaled table (sensor response). */
  lambda_delay_floor_ms: number;
}

/**
 * Data filtering configuration for AutoTune.
 * Samples outside these ranges are ignored.
 */
interface AutoTuneFilters {
  /** Minimum RPM to accept data */
  min_rpm: number;
  /** Maximum RPM to accept data */
  max_rpm: number;
  /**
   * Throttle-position bounds, placeholder for a planned TPS window filter.
   *
   * No control and no backend field yet - the Rust AutoTuneFilters struct
   * drops them. Until then, `custom_filter` expresses the same thing
   * ("tps > 5 && tps < 95"). Keep: the pair is the shape the filter will take.
   */
  min_tps: number;
  max_tps: number;
  /** Minimum coolant temperature (reject cold engine data) */
  min_clt: number;
  /** Custom filter expression (e.g., "rpm > 2000 && tps < 50") */
  custom_filter: string;
  /** Maximum TPS change rate (%/sec) before filtering */
  max_tps_rate: number;
  /** Exclude data when accel enrichment is active */
  exclude_accel_enrich: boolean;
  /**
   * How long rpm and load must have held steady before a sample counts, in ms.
   * `0` disables the check, which is the backend default.
   *
   * This is the field `AutoTuneFilters` actually reads. The panel used to send
   * `require_steady_state` / `steady_state_time_ms`, which no longer exist on
   * the Rust side, so the checkbox has never done anything.
   */
  min_steady_ms: number;
  /**
   * Placeholder: per-session steadiness tolerance.
   *
   * The backend currently judges steadiness against fixed constants
   * (STEADY_RPM_TOLERANCE 100 rpm, STEADY_LOAD_TOLERANCE 3). Exposing them is
   * the natural next step; this is the control that would carry the rpm half.
   */
  steady_state_rpm_delta: number;
}

/**
 * Limits on how much AutoTune can modify cell values.
 */
interface AutoTuneAuthorityLimits {
  /**
   * Maximum change per update per cell, in ABSOLUTE table units (VE points).
   * Not a percentage - the backend clamps the raw delta against this value.
   * It was labelled "(%)" here and in the panel for as long as it has existed,
   * so anyone who typed 15 meaning 15% was authorising +/-15 VE.
   */
  max_change_per_cell: number;
  /** Maximum change per update as a percentage of the cell's session-start value. */
  max_total_change: number;
  /**
   * Absolute floor for any cell value. The two limits above are both measured
   * from the cell's value at session start, so they reset every session and
   * cannot bound drift across several; these rails can.
   */
  min_value: number;
  /** Absolute ceiling for any cell value. See `min_value`. */
  max_value: number;
}

type AutoTuneLoadSource = 'map' | 'maf' | 'tps';

/** One thing worth telling the user before a session starts. */
interface PreflightFinding {
  severity: 'blocker' | 'warning' | 'info';
  code: string;
  title: string;
  detail: string;
  current: string | null;
  suggested: string | null;
}

interface FlowDelayFit {
  floorMs: number;
  k: number;
  anchorMs: number;
  rmsMs: number;
  samples: number;
}

/** A filter the INI declares for VE Analyze, and what this session does about it. */
interface DeclaredFilter {
  name: string;
  displayName: string;
  channel: string;
  operator: string;
  iniValue: number;
  userAdjustable: boolean;
  sessionValue: number | null;
  differs: boolean;
}

interface PreflightReport {
  findings: PreflightFinding[];
  delayFit: FlowDelayFit | null;
  hasBlocker: boolean;
  candidateTargetTables: string[];
  resolvedTargetTable: string | null;
}

/**
 * Heat map data for a single table cell.
 */
interface HeatmapEntry {
  /** X-axis cell index */
  cell_x: number;
  /** Y-axis cell index */
  cell_y: number;
  /** Data coverage weighting (0-1, higher = more data) */
  hit_weighting: number;
  /** Magnitude of recommended change */
  change_magnitude: number;
  /** Original cell value before tuning */
  beginning_value: number;
  /** Recommended new value */
  recommended_value: number;
  /** Number of data samples for this cell */
  hit_count: number;
}

/**
 * Table data structure from backend.
 */
interface TableData {
  name: string;
  title: string;
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  x_output_channel?: string | null;
  y_output_channel?: string | null;
}

interface ChannelInfo {
  name: string;
  label?: string | null;
}

/**
 * Live sample tallies for the rejection indicator (issue #132).
 *
 * A session that accepts nothing looks exactly like a broken one — the
 * indicator surfaces how many samples passed the filters and, when data is
 * being rejected, which filter is eating it (most frequent reason first).
 */
interface AutotuneSampleStats {
  accepted: number;
  rejections: { reason: string; count: number }[];
}

/**
 * Minimal table info for selection dropdown.
 */
interface TableInfo {
  name: string;
  title: string;
}

/**
 * Props for AutoTune component.
 */
interface AutoTuneProps {
  /** Initial table to tune (defaults to VE table detection) */
  tableName?: string;
  /** Callback when component is closed */
  onClose?: () => void;
  /** Whether the app currently has a live ECU connection */
  isConnected: boolean;
}

interface VeAnalyzeConfig {
  ve_table_name: string;
  target_table_name: string;
  lambda_channel: string;
  ego_correction_channel: string;
  lambda_target_tables: string[];
}

// =============================================================================
// AutoTune Component
// =============================================================================

/// AutoTune's tuning parameters, kept across restarts.
///
/// These are per-engine facts the operator measures once - a transport delay of
/// ~470 ms at idle, a coolant threshold in the INI's own units - not view state
/// worth re-deriving each launch. They used to reset on every start, and the
/// default `lambda_delay_ms: 0` does not mean "no delay": it means "fall back
/// to the built-in RPM curve", which tops out at 200 ms. On a car whose real
/// idle delay is more than twice that, samples land in the wrong cell and the
/// low-load corrections come back inflated - which is what happened on a
/// 59-minute drive where the delay had simply never been set.
///
/// Versioned so a field change discards old state rather than merging a stale
/// shape into a new one.

/**
 * Steady-state window applied when the box is ticked, in ms.
 *
 * Longer than the longest transport delay in use. Across two logged drives
 * 800 ms lifted the held-out AFR improvement from 34.1% to 39.5% and roughly
 * halved the largest proposed change, at the cost of covering 52 cells rather
 * than 131 - a sample has to earn its place.
 */
const STEADY_MS_DEFAULT = 800;

/// Bumped to v2 with the steady-state rewiring: a
/// persisted v1 blob carries require_steady_state / steady_state_time_ms,
/// which no longer mean anything, and merging it would leave min_steady_ms at
/// its default while the tuner believes their old setting survived.
const SETTINGS_KEY = 'libretune.autotune.settings.v2';

function loadPersisted<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return fallback;
    // Merge over the fallback so a field added since the value was written
    // takes its default instead of arriving undefined.
    return { ...fallback, ...(JSON.parse(raw) as object) } as T;
  } catch {
    return fallback;
  }
}

function persist(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // localStorage unavailable - settings just won't survive the restart
  }
}

/** Stable empty list so the axis-channel subscription below can skip
 * subscribing entirely when the table declares no channels. */
const NO_CHANNELS: string[] = [];

/**
 * Index of the bin nearest to `value` — the cell a live reading falls into.
 * Matches the nearest-bin logic the table editors use for their cursors.
 */
function nearestBinIndex(value: number, bins: number[]): number {
  let best = 0;
  let diff = Infinity;
  bins.forEach((bin, i) => {
    const d = Math.abs(bin - value);
    if (d < diff) {
      diff = d;
      best = i;
    }
  });
  return best;
}

export function AutoTune({ tableName: initialTableName = '', onClose, isConnected }: AutoTuneProps) {
  const { showToast } = useToast();

  // State
  const [isRunning, setIsRunning] = useState(false);
  const [selectedTable, setSelectedTable] = useState(initialTableName);
  const [secondaryTableEnabled, setSecondaryTableEnabled] = useState(false);
  const [secondaryTable, setSecondaryTable] = useState('');
  const [activeView, setActiveView] = useState<'primary' | 'secondary'>('primary');
  const [availableTables, setAvailableTables] = useState<TableInfo[]>([]);
  const [tableData, setTableData] = useState<TableData | null>(null);
  const [_referenceData, setReferenceData] = useState<TableData | null>(null);
  const [heatmapData, setHeatmapData] = useState<HeatmapEntry[]>([]);
  const [sampleStats, setSampleStats] = useState<AutotuneSampleStats | null>(null);
  const [veAnalyzeConfig, setVeAnalyzeConfig] = useState<VeAnalyzeConfig | null>(null);
  const [lockedCells, setLockedCells] = useState<Set<string>>(new Set());
  const [selectedCells, _setSelectedCells] = useState<Set<string>>(new Set());
  const [showHeatmap, setShowHeatmap] = useState<'weighting' | 'change' | 'none'>('weighting');
  const [error, setError] = useState<string | null>(null);
  const [loadSource, setLoadSource] = useState<AutoTuneLoadSource>('map');
  const [loadSourceHint, setLoadSourceHint] = useState<string | null>(null);
  // Set the moment the user picks a load source themselves. Auto-detection
  // (by Y-axis channel name or by the `algorithm` constant) must never fight
  // an explicit choice — without this, a stale-closure check on `loadSource`
  // lets async detection re-fire after e.g. the MAF-verify demotion and flip
  // the dropdown back (issue #132).
  const manualLoadSourceRef = useRef(false);

  // Current operating cell for the grid's "current" highlight, derived from
  // the table's own axis channels (issue #132: "no indication of the current
  // position"). A `currentCell` state used to sit here but its setter was
  // never called, so the highlight never fired. x indexes columns (x_bins),
  // y indexes rows (y_bins) — matching how the grid iterates them. A missing
  // Y channel (1-D curves) pins the row at 0, same as the table editors.
  const currentXChannel = tableData?.x_output_channel ?? undefined;
  const currentYChannel = tableData?.y_output_channel ?? undefined;
  const liveAxisValues = useChannels(
    currentXChannel
      ? currentYChannel
        ? [currentXChannel, currentYChannel]
        : [currentXChannel]
      : NO_CHANNELS
  );
  const currentCell = useMemo<{ x: number; y: number } | null>(() => {
    if (!tableData || !currentXChannel) return null;
    const xv = liveAxisValues[currentXChannel];
    if (xv === undefined) return null;
    const x = nearestBinIndex(xv, tableData.x_bins);
    if (!currentYChannel || tableData.y_bins.length <= 1) {
      return { x, y: 0 };
    }
    const yv = liveAxisValues[currentYChannel];
    if (yv === undefined) return { x, y: 0 };
    return { x, y: nearestBinIndex(yv, tableData.y_bins) };
  }, [tableData, currentXChannel, currentYChannel, liveAxisValues]);

  // Settings state
  const [settings, setSettings] = useState<AutoTuneSettings>(() =>
    loadPersisted<AutoTuneSettings>(`${SETTINGS_KEY}.settings`, {
    target_afr: 14.7,
    hit_weighting: 'uniform',
    base_weight: 20,
    min_change: 1,
    algorithm: 'simple',
    update_rate_ms: 100,
    lambda_delay_ms: 0,
    lambda_delay_flow_scaled: false,
    lambda_delay_floor_ms: 120,
  }));

  const [filters, setFilters] = useState<AutoTuneFilters>(() =>
    loadPersisted<AutoTuneFilters>(`${SETTINGS_KEY}.filters`, {
    min_rpm: 800,
    max_rpm: 7000,
    min_tps: 0,
    max_tps: 100,
    min_clt: 60,
    custom_filter: '',
    // 50 %/s — ITB/Alpha-N throttles move far faster than the old 10 %/s
    // default allowed, which made AutoTune reject nearly every sample and
    // look dead (issue #132). Accel transients are still filtered by
    // exclude_accel_enrich.
    max_tps_rate: 50,
    exclude_accel_enrich: true,
    // Off, matching the backend default. Turning it on changes which samples
    // an existing session accepts, and that is the tuner's call - the panel
    // defaulting it *on* while the backend ignored it is how this came to be
    // reported as working for so long.
    min_steady_ms: 0,
    steady_state_rpm_delta: 100,
  }));

  const [authority, setAuthority] = useState<AutoTuneAuthorityLimits>(() =>
    loadPersisted<AutoTuneAuthorityLimits>(`${SETTINGS_KEY}.authority`, {
    max_change_per_cell: 15,
    max_total_change: 30,
    min_value: 0,
    max_value: 200,
  }));

  // Write back on change, so the next launch starts where the operator left
  // off rather than at a default that silently disables the measured delay.
  useEffect(() => persist(`${SETTINGS_KEY}.settings`, settings), [settings]);
  useEffect(() => persist(`${SETTINGS_KEY}.filters`, filters), [filters]);
  useEffect(() => persist(`${SETTINGS_KEY}.authority`, authority), [authority]);

  // Reference-table / lambda-match configuration (bug #2, #14).
  // Leaving the AFR table blank uses auto-discovery from the INI, falling back
  // to settings.target_afr. Strict lambda matching (default on) drops samples
  // with no delayed-buffer match rather than mis-attributing them.
  const [targetAfrTable, setTargetAfrTable] = useState<string>('');
  // Preflight: AutoTune fails quietly, so a session gets checked before it is
  // allowed to eat a drive. `null` means nothing pending.
  const [preflight, setPreflight] = useState<PreflightReport | null>(null);
  const [preflightBusy, setPreflightBusy] = useState(false);
  // What the INI itself declares for VE Analyze. These carry the project's unit
  // system already resolved (coolant < 71 on a Celsius project, < 160 on a
  // Fahrenheit one), which is the number that should be used rather than a
  // constant compiled into the app.
  const [declaredFilters, setDeclaredFilters] = useState<DeclaredFilter[]>([]);
  const [lambdaDelayTable, setLambdaDelayTable] = useState<string>('');
  const [strictLambdaMatch, setStrictLambdaMatch] = useState(true);

  const isMafChannelName = useCallback((name?: string | null) => {
    if (!name) return false;
    const lower = name.toLowerCase();
    return lower.includes('maf') || lower.includes('airmass') || lower.includes('airflow');
  }, []);

  // Detect a throttle-position (Alpha-N / ITB) load channel from an INI
  // channel name or label. Mirrors isMafChannelName. A TPS-based VE table has
  // its load (Y) axis indexed by throttle opening, so live data must be
  // attributed by TPS instead of MAP/MAF (issue #132).
  const isTpsChannelName = useCallback((name?: string | null) => {
    if (!name) return false;
    const lower = name.toLowerCase();
    return lower === 'tps' || lower === 'tp' || lower === 'throttle' || lower.includes('tps') || lower.includes('throttle');
  }, []);

  useEffect(() => {
    let cancelled = false;

    const loadVeAnalyze = async () => {
      try {
        const config = await invoke<VeAnalyzeConfig | null>('get_ve_analyze_config');
        if (!cancelled) {
          setVeAnalyzeConfig(config);
          if (config?.ve_table_name && !initialTableName) {
            setSelectedTable(config.ve_table_name);
          }
        }
      } catch (e) {
        if (!cancelled) {
          console.warn('get_ve_analyze_config failed:', e);
          setVeAnalyzeConfig(null);
        }
      }
    };

    loadVeAnalyze();
    return () => {
      cancelled = true;
    };
  }, [initialTableName]);

  const loadAvailableTables = useCallback(async () => {
    try {
      // Only tables a fuel tune may legitimately be applied to. `get_tables`
      // returns every table in the INI, which put the spark table two clicks
      // from being scaled by measured/target AFR — a lean cell multiplies by
      // more than one, so that *adds advance*. The backend refuses it either
      // way; this keeps it out of the picker so nobody is offered the choice.
      // Falls back to the unfiltered list if that call fails, rather than
      // leaving the picker empty: `start_autotune` refuses a non-fuel table on
      // its own, so the worst case is an error on Start instead of a dead UI.
      const allowed = await invoke<string[]>('list_tunable_tables').catch(() => null);
      const all = await invoke<TableInfo[]>('get_tables');
      const tables = Array.isArray(allowed) ? all.filter((t) => allowed.includes(t.name)) : all;
      setAvailableTables(tables);

      // Auto-select table: prefer INI config, then common VE table names, then first table
      const currentExists = selectedTable && tables.some((t) => t.name === selectedTable);
      if (!currentExists && tables.length > 0) {
        // 1. Try INI-defined VeAnalyze config
        const fromConfig = veAnalyzeConfig?.ve_table_name
          ? tables.find((t) => t.name === veAnalyzeConfig.ve_table_name)
          : null;
        if (fromConfig) {
          setSelectedTable(fromConfig.name);
        } else {
          // 2. Try common VE/fuel table name patterns
          const vePatterns = [/^ve/i, /fuel/i, /lambda/i, /afr/i];
          const veTable = vePatterns.reduce<TableInfo | undefined>(
            (found, pat) => found || tables.find((t) => pat.test(t.name) || pat.test(t.title)),
            undefined
          );
          setSelectedTable((veTable || tables[0]).name);
        }
      }

      // Only a *different* fuel table is a valid secondary pick. The old
      // `|| tables[0]` fallback made the secondary selector echo the primary
      // when only one table is tunable — and an empty picker looks exactly
      // like a broken one (issue #132). '' selects the placeholder below.
      if (!secondaryTable) {
        const preferredSecondary = veAnalyzeConfig?.lambda_target_tables
          ?.map((name) => tables.find((t) => t.name === name))
          .find((t) => t && t.name !== selectedTable);
        const fallbackSecondary = preferredSecondary || tables.find((t) => t.name !== selectedTable);
        setSecondaryTable(fallbackSecondary ? fallbackSecondary.name : '');
      }
    } catch (e) {
      console.error('Failed to load available tables:', e);
      setError('Failed to load tables: ' + e);
    }
  }, [selectedTable, secondaryTable, veAnalyzeConfig]);

  const activeTable = useMemo(() => {
    if (activeView === 'secondary' && secondaryTableEnabled && secondaryTable) {
      return secondaryTable;
    }
    return selectedTable;
  }, [activeView, secondaryTableEnabled, secondaryTable, selectedTable]);

  const secondaryOptions = useMemo(
    () => availableTables.filter((t) => t.name !== selectedTable),
    [availableTables, selectedTable]
  );

  useEffect(() => {
    if (!secondaryTableEnabled && activeView !== 'primary') {
      setActiveView('primary');
    }
  }, [secondaryTableEnabled, activeView]);

  useEffect(() => {
    if (!secondaryTableEnabled) {
      return;
    }

    if (!secondaryTable || secondaryTable === selectedTable) {
      setSecondaryTable(secondaryOptions[0]?.name ?? '');
    }
  }, [secondaryTableEnabled, secondaryTable, selectedTable, secondaryOptions]);

  // Re-attach to a live session. The backend session survives this view
  // unmounting (switching to the dashboard and back), but isRunning is
  // component state and resets to false on remount — so the view showed a
  // running session as stopped and never resumed polling.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const status = await invoke<{
          running: boolean;
          tableName: string | null;
          secondaryTableName: string | null;
        }>('get_autotune_status');
        if (cancelled || !status?.running) return;
        setIsRunning(true);
        if (status.tableName) {
          setSelectedTable(status.tableName);
        }
        if (status.secondaryTableName) {
          setSecondaryTableEnabled(true);
          setSecondaryTable(status.secondaryTableName);
        }
      } catch {
        // No session (or status unavailable) — keep the stopped default.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Load initial table data
  useEffect(() => {
    loadAvailableTables();
  }, [loadAvailableTables]);

  useEffect(() => {
    loadTableData();
  }, [activeTable]);

  useEffect(() => {
    if (!tableData || isRunning || manualLoadSourceRef.current) return;
    // Auto-detect the load source from the selected table's Y-axis output
    // channel when the user hasn't already picked one. TPS (Alpha-N / ITB) is
    // checked first because a TPS channel name like "tps" would not otherwise
    // match MAF and would silently stay on the wrong MAP source (issue #132).
    const yChan = tableData.y_output_channel;
    if (isTpsChannelName(yChan) && loadSource !== 'tps') {
      setLoadSource('tps');
      setLoadSourceHint('Throttle (TPS/Alpha-N) load axis detected.');
    } else if (isMafChannelName(yChan) && loadSource !== 'maf') {
      setLoadSource('maf');
    }
  }, [isMafChannelName, isTpsChannelName, isRunning, loadSource, tableData]);

  // Speeduino names its VE load-axis output channel `fuelLoad` regardless of
  // the fuel algorithm, so channel-name detection (above) cannot fire there.
  // The `algorithm` constant is authoritative instead: 1 = TPS / Alpha-N on
  // Speeduino and MS2/MS3 alike. Only corrects the untouched MAP default so a
  // deliberate manual choice is respected (issue #132).
  useEffect(() => {
    if (!tableData || isRunning || loadSource !== 'map' || manualLoadSourceRef.current) {
      return;
    }
    const yChan = tableData.y_output_channel;
    if (isTpsChannelName(yChan) || isMafChannelName(yChan)) {
      return; // channel-name detection already decided
    }
    let cancelled = false;
    (async () => {
      try {
        const v = await invoke<number>('get_constant_value', { name: 'algorithm' });
        if (!cancelled && v === 1 && loadSource === 'map') {
          setLoadSource('tps');
          setLoadSourceHint('Fuel algorithm is TPS (Alpha-N) — throttle load selected.');
        }
      } catch {
        // No `algorithm` constant in this INI — nothing to detect.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [isMafChannelName, isTpsChannelName, isRunning, loadSource, tableData]);

  useEffect(() => {
    if (loadSource !== 'maf') {
      return;
    }

    let cancelled = false;

    const checkMafChannels = async () => {
      try {
        const channels = await invoke<ChannelInfo[]>('get_available_channels');
        const hasMafChannel = channels.some(
          (channel) => isMafChannelName(channel.name) || isMafChannelName(channel.label)
        );

        if (!hasMafChannel && !cancelled) {
          setLoadSource('map');
          setLoadSourceHint('MAF channel not detected. Switched to MAP load.');
        } else if (!cancelled) {
          setLoadSourceHint(null);
        }
      } catch (e) {
        if (!cancelled) {
          setLoadSourceHint('Unable to verify MAF channels. Using MAP load.');
          setLoadSource('map');
        }
      }
    };

    checkMafChannels();
    return () => {
      cancelled = true;
    };
  }, [isMafChannelName, loadSource]);

  useEffect(() => {
    setLockedCells(new Set());
    _setSelectedCells(new Set());
    setHeatmapData([]);
  }, [activeTable]);

  // Poll heatmap data when running
  useEffect(() => {
    if (!isRunning) return;

    const interval = setInterval(async () => {
      try {
        const data = await invoke<HeatmapEntry[]>('get_autotune_heatmap', {
          tableName: activeTable,
        });
        setHeatmapData(data);
      } catch (e) {
        console.error('Failed to fetch heatmap:', e);
      }
      // Sample tallies ride the same poll: the rejection indicator must say
      // *why* nothing accumulates while the user watches (issue #132).
      try {
        const status = await invoke<{
          acceptedSamples: number;
          rejections: { reason: string; count: number }[];
        }>('get_autotune_status');
        setSampleStats({
          accepted: status.acceptedSamples ?? 0,
          rejections: status.rejections ?? [],
        });
      } catch {
        // Status is diagnostic; keep the last known tallies.
      }
    }, 500);

    return () => clearInterval(interval);
  }, [isRunning, activeTable]);

  const loadTableData = useCallback(async () => {
    try {
      if (!activeTable) {
        return;
      }
      const data = await invoke<TableData>('get_table_data', { tableName: activeTable });
      setTableData(data);
    } catch (e) {
      setError(`Failed to load table: ${e}`);
    }
  }, [activeTable]);

  const loadReferenceTable = useCallback(async () => {
    try {
      const filePath = await open({
        title: 'Load Reference Table (CSV)',
        filters: [{ name: 'CSV Files', extensions: ['csv'] }],
        multiple: false,
      });
      
      if (filePath && typeof filePath === 'string') {
        // Parse CSV reference table
        const content = await invoke<string>('read_file_contents', { path: filePath });
        const lines = content.trim().split('\n');
        const zValues: number[][] = [];
        
        for (const line of lines) {
          const row = line.split(',').map((v) => parseFloat(v.trim()) || 0);
          zValues.push(row);
        }
        
        if (tableData) {
          setReferenceData({
            ...tableData,
            z_values: zValues,
          });
        }
      }
    } catch (e) {
      setError(`Failed to load reference: ${e}`);
    }
  }, [tableData]);

  const saveReferenceTable = useCallback(async () => {
    if (!tableData) return;
    
    try {
      const filePath = await save({
        title: 'Save Reference Table (CSV)',
        filters: [{ name: 'CSV Files', extensions: ['csv'] }],
        defaultPath: `${tableData.name}_reference.csv`,
      });
      
      if (filePath) {
        // Convert table to CSV
        const csvContent = tableData.z_values
          .map((row) => row.map((v) => v.toFixed(2)).join(','))
          .join('\n');
        
        await invoke('write_file_contents', { path: filePath, content: csvContent });
      }
    } catch (e) {
      setError(`Failed to save reference: ${e}`);
    }
  }, [tableData]);

  const reallyStartAutoTune = useCallback(async () => {
    setPreflight(null);
    try {
      await invoke('start_autotune', {
        tableName: selectedTable,
        secondaryTableName:
          secondaryTableEnabled && secondaryTable && secondaryTable !== selectedTable
            ? secondaryTable
            : null,
        loadSource,
        settings,
        filters,
        authorityLimits: authority,
        targetAfrTableName: targetAfrTable.trim() || null,
        lambdaDelayTableName: lambdaDelayTable.trim() || null,
        strictLambdaMatch,
      });
      setIsRunning(true);
      setError(null);
    } catch (e) {
      setError(`Failed to start AutoTune: ${e}`);
    }
  }, [selectedTable, secondaryTableEnabled, secondaryTable, loadSource, settings, filters, authority, targetAfrTable, lambdaDelayTable, strictLambdaMatch]);

  /**
   * Check before starting. AutoTune will happily run against a missing target
   * table or a filter that rejects every sample and report nothing wrong, so
   * the whole drive is wasted before anyone finds out.
   */
  const startAutoTune = useCallback(async () => {
    if (!isConnected) {
      showToast('Connect to the ECU to start AutoTune — it needs live data to generate recommendations.', 'warning');
      return;
    }
    setPreflightBusy(true);
    try {
      const report = await invoke<PreflightReport>('preflight_autotune', {
        tableName: selectedTable,
        settings,
        filters,
        authorityLimits: authority,
        targetAfrTableName: targetAfrTable.trim() || null,
        lambdaDelayTableName: lambdaDelayTable.trim() || null,
        willWriteToEcu: false,
      });
      // Always show it: the write-mode line is an Info finding, and "nothing is
      // wrong" is worth seeing once rather than inferred from silence.
      setPreflight(report);
      try {
        setDeclaredFilters(
          await invoke<DeclaredFilter[]>('get_declared_analyze_filters', { filters })
        );
      } catch {
        // An INI with no [VeAnalyze] section simply has none to show.
        setDeclaredFilters([]);
      }
    } catch (e) {
      // A preflight that cannot run must not block the session - it is a
      // safety net, not a gate.
      setError(`Preflight check failed (starting anyway is your call): ${e}`);
      setPreflight({ findings: [], hasBlocker: false, candidateTargetTables: [], resolvedTargetTable: null, delayFit: null });
    } finally {
      setPreflightBusy(false);
    }
  }, [isConnected, showToast, selectedTable, settings, filters, authority, targetAfrTable, lambdaDelayTable]);

  const stopAutoTune = useCallback(async () => {
    try {
      await invoke('stop_autotune');
      setIsRunning(false);
    } catch (e) {
      setError(`Failed to stop AutoTune: ${e}`);
    }
  }, []);

  const sendRecommendations = useCallback(async () => {
    try {
      await invoke('send_autotune_recommendations', {
        tableName: activeTable,
      });
      // Refresh table data after sending
      await loadTableData();
    } catch (e) {
      setError(`Failed to send recommendations: ${e}`);
    }
  }, [activeTable, loadTableData]);

  const toggleCellLock = useCallback((x: number, y: number) => {
    const key = `${x},${y}`;
    setLockedCells((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }, []);

  const lockSelectedCells = useCallback(async () => {
    const cells = Array.from(selectedCells).map((key) => {
      const [x, y] = key.split(',').map(Number);
      return [x, y] as [number, number];
    });
    
    try {
      await invoke('lock_autotune_cells', { cells, tableName: activeTable });
      setLockedCells((prev) => new Set([...prev, ...selectedCells]));
    } catch (e) {
      console.error('Failed to lock cells:', e);
    }
  }, [activeTable, selectedCells]);

  const unlockSelectedCells = useCallback(async () => {
    const cells = Array.from(selectedCells).map((key) => {
      const [x, y] = key.split(',').map(Number);
      return [x, y] as [number, number];
    });
    
    try {
      await invoke('unlock_autotune_cells', { cells, tableName: activeTable });
      setLockedCells((prev) => {
        const next = new Set(prev);
        selectedCells.forEach((key) => next.delete(key));
        return next;
      });
    } catch (e) {
      console.error('Failed to unlock cells:', e);
    }
  }, [activeTable, selectedCells]);

  // Build heatmap lookup
  const heatmapLookup = useMemo(() => {
    const lookup: Record<string, HeatmapEntry> = {};
    for (const entry of heatmapData) {
      lookup[`${entry.cell_x},${entry.cell_y}`] = entry;
    }
    return lookup;
  }, [heatmapData]);

  // Highest hit count on the board, for normalizing the weighting heatmap.
  const maxHits = useMemo(
    () => heatmapData.reduce((m, e) => Math.max(m, e.hit_count), 0),
    [heatmapData]
  );

  // Get cell color based on heatmap mode
  const getCellColor = useCallback(
    (x: number, y: number, value: number) => {
      const key = `${x},${y}`;
      const entry = heatmapLookup[key];

      if (lockedCells.has(key)) {
        return 'var(--cell-locked)';
      }

      if (showHeatmap === 'weighting') {
        // hit_weighting accumulates 1.0 per accepted sample, so the previous
        // min(1, hit_weighting) saturated after a single hit and every visited
        // cell rendered the same colour regardless of count. Colour by hit
        // count on a log scale against the busiest cell, interpolating the
        // same yellow->blue ramp the legend shows (hsl(60,80%,30%) ->
        // hsl(240,80%,50%) in RGB, matching the CSS gradient). Unhit cells go
        // neutral — the VE-value gradient here read as hit intensity.
        const hits = entry?.hit_count ?? 0;
        if (hits <= 0 || maxHits <= 0) {
          return 'var(--cell-neutral)';
        }
        const t = Math.log1p(hits) / Math.log1p(maxHits);
        const lerp = (a: number, b: number) => Math.round(a + (b - a) * t);
        return `rgb(${lerp(138, 26)}, ${lerp(138, 26)}, ${lerp(15, 230)})`;
      }

      if (!entry || showHeatmap === 'none') {
        // Default value-based coloring using centralized heatmap utility
        return valueToHeatmapColor(value, 0, 100, 'tunerstudio');
      }

      if (showHeatmap === 'change') {
        // Change magnitude: uses centralized utility
        // Positive change = leaner (towards red), negative = richer (towards blue)
        const change = entry.recommended_value - entry.beginning_value;
        if (Math.abs(change) < 0.5) {
          return 'var(--cell-neutral)';
        }
        // Normalize change to 0-1 range, where 0.5 = no change
        const maxChange = authority.max_change_per_cell || 10;
        const normalizedChange = (change / maxChange + 1) / 2; // Maps -max..+max to 0..1
        const clampedChange = Math.max(0, Math.min(1, normalizedChange));
        return valueToHeatmapColor(clampedChange, 0, 1, 'tunerstudio');
      }

      return 'var(--cell-default)';
    },
    [heatmapLookup, showHeatmap, lockedCells, authority.max_change_per_cell, maxHits]
  );

  // Stats
  const stats = useMemo(() => {
    if (heatmapData.length === 0) return null;
    
    const totalHits = heatmapData.reduce((sum, e) => sum + e.hit_count, 0);
    const avgChange = heatmapData.reduce((sum, e) => sum + Math.abs(e.change_magnitude), 0) / heatmapData.length;
    const cellsWithData = heatmapData.filter((e) => e.hit_count > 0).length;
    
    return { totalHits, avgChange, cellsWithData };
  }, [heatmapData]);

  if (!tableData) {
    return (
      <div className="autotune-loading">
        {error ? <div className="autotune-error">{error}</div> : 'Loading table data...'}
      </div>
    );
  }

  /**
   * How to repair each finding, in place. Keyed by the backend's stable code so
   * a new check arrives with its fix attached rather than as another thing to
   * read and act on somewhere else in the panel.
   */
  const applyFix = (finding: PreflightFinding) => {
    // The backend already worked out the right value and put it in `suggested`
    // ("60 C", "10 %/s", "10 / 20%"). Reading it back beats re-deriving the
    // rule here, where it would drift out of step with the check that raised it.
    const num = parseFloat((finding.suggested ?? '').replace(/[^0-9.\-]/g, ''));
    switch (finding.code) {
      case 'min_clt_units':
        if (Number.isFinite(num)) setFilters((f) => ({ ...f, min_clt: num }));
        break;
      case 'rpm_window_empty':
        setFilters((f) => ({ ...f, min_rpm: 1000, max_rpm: 7000 }));
        break;
      case 'tps_rate_inert':
        setFilters((f) => ({ ...f, max_tps_rate: 10 }));
        break;
      case 'authority_zero':
        setAuthority((a) => ({ ...a, max_change_per_cell: 10, max_total_change: 20 }));
        break;
      case 'rails_reversed':
        setAuthority((a) => ({ ...a, min_value: Math.min(a.min_value, a.max_value), max_value: Math.max(a.min_value, a.max_value) }));
        break;
      case 'delay_default_curve':
        if (preflight?.delayFit) {
          setSettings((s2) => ({
            ...s2,
            lambda_delay_flow_scaled: true,
            lambda_delay_floor_ms: Math.round(preflight.delayFit!.floorMs),
            lambda_delay_ms: Math.round(preflight.delayFit!.anchorMs),
          }));
        }
        break;
      default:
        break;
    }
  };

  /** Codes this dialog knows how to repair without leaving it. */
  const fixable = (code: string) =>
    ['min_clt_units', 'rpm_window_empty', 'tps_rate_inert', 'authority_zero', 'rails_reversed'].includes(code) ||
    (code === 'delay_default_curve' && !!preflight?.delayFit);

  const severityLabel: Record<PreflightFinding['severity'], string> = {
    blocker: 'Will not work',
    warning: 'Check this',
    info: 'For information',
  };

  return (
    <div className="autotune">
      {preflight && (
        <div className="preflight-backdrop" role="dialog" aria-modal="true" aria-label="AutoTune pre-start check">
          <div className="preflight-dialog">
            <h2>
              {preflight.hasBlocker
                ? 'AutoTune will not produce a usable result'
                : preflight.findings.some((f) => f.severity === 'warning')
                  ? 'Worth checking before you start'
                  : 'Ready to start'}
            </h2>
            <p className="preflight-target">
              AFR target:{' '}
              {preflight.resolvedTargetTable
                ? <strong>{preflight.resolvedTargetTable}</strong>
                : <strong className="preflight-none">none — a flat {settings.target_afr} will be used for every cell</strong>}
            </p>

            {/* Everything the session depends on, always visible and always
                changeable - not only surfaced once something has gone wrong.
                Getting the target table right matters more than any other
                setting here, so it does not hide when it happens to resolve. */}
            <div className="preflight-settings">
              <div className="preflight-setting">
                <label htmlFor="pf-target">AFR target table</label>
                <select
                  id="pf-target"
                  value={targetAfrTable || preflight.resolvedTargetTable || ''}
                  onChange={(e) => setTargetAfrTable(e.target.value)}
                >
                  <option value="">Auto-discover</option>
                  {preflight.candidateTargetTables.map((t) => (
                    <option key={t} value={t}>{t}</option>
                  ))}
                </select>
                <span className="pf-hint">
                  {preflight.resolvedTargetTable
                    ? `resolved: ${preflight.resolvedTargetTable}`
                    : `none — every cell would use a flat ${settings.target_afr}`}
                </span>
              </div>

              <div className="preflight-setting">
                <label htmlFor="pf-weight">Hit weighting</label>
                <select
                  id="pf-weight"
                  value={settings.hit_weighting}
                  onChange={(e) => setSettings({ ...settings, hit_weighting: e.target.value as AutoTuneSettings['hit_weighting'] })}
                >
                  {/* Soft / Medium / Hard are the names tuners coming from other
                      popular tuning software will already know. The description
                      after each says what it actually does, so the familiar label
                      does not have to carry the meaning on its own. */}
                  <option value="uniform">
                    None — every sample counts fully for its nearest cell
                  </option>
                  <option value="cell_proximity">
                    Soft — a sample is shared with the cell it sits nearest to
                  </option>
                  <option value="cell_proximity_squared">
                    Medium — sharing falls away faster, so cells stay distinct
                  </option>
                  <option value="cell_centre_only">
                    Hard — only samples near a cell centre count at all
                  </option>
                </select>
                <span className="pf-hint">
                  {settings.hit_weighting === 'uniform'
                    ? 'no sharing: a sample on a cell boundary is credited entirely to one side'
                    : settings.hit_weighting === 'cell_centre_only'
                      ? 'cleanest per-cell answer, and the slowest to fill a map'
                      : 'full authority at ' + settings.base_weight + ' accumulated weight'}
                </span>
              </div>

              <div className="preflight-setting">
                <label htmlFor="pf-base">Base weight</label>
                <input id="pf-base" type="number" value={settings.base_weight}
                  onChange={(e) => setSettings({ ...settings, base_weight: parseFloat(e.target.value) || 0 })} />
                <label htmlFor="pf-minch">Min change</label>
                <input id="pf-minch" type="number" step="0.1" value={settings.min_change}
                  onChange={(e) => setSettings({ ...settings, min_change: parseFloat(e.target.value) || 0 })} />
                <span className="pf-hint">common defaults: 20 / 1.0</span>
              </div>

              {declaredFilters.length > 0 && (
                <div className="preflight-filters">
                  <div className="pf-filters-head">
                    Filters this INI declares
                    <span className="pf-hint">
                      values come from the INI and already match its unit system
                    </span>
                  </div>
                  {declaredFilters.filter((f) => f.channel).map((f) => (
                    <div key={f.name} className={`pf-filter${f.differs ? ' pf-filter-differs' : ''}`}>
                      <span className="pf-filter-name">{f.displayName}</span>
                      <code>{f.channel} {f.operator} {f.iniValue}</code>
                      {f.sessionValue !== null ? (
                        <>
                          <input
                            type="number"
                            value={f.sessionValue}
                            onChange={(e) => {
                              const v = parseFloat(e.target.value);
                              if (!Number.isFinite(v)) return;
                              if (f.name === 'minCltFilter') setFilters({ ...filters, min_clt: v });
                              if (f.name === 'minRPMFilter') setFilters({ ...filters, min_rpm: v });
                              setDeclaredFilters((prev) =>
                                prev.map((x) => (x.name === f.name
                                  ? { ...x, sessionValue: v, differs: Math.abs(v - x.iniValue) > 0.5 }
                                  : x)));
                            }}
                          />
                          {f.differs && (
                            <button type="button" onClick={() => {
                              if (f.name === 'minCltFilter') setFilters({ ...filters, min_clt: f.iniValue });
                              if (f.name === 'minRPMFilter') setFilters({ ...filters, min_rpm: f.iniValue });
                              setDeclaredFilters((prev) =>
                                prev.map((x) => (x.name === f.name
                                  ? { ...x, sessionValue: x.iniValue, differs: false } : x)));
                            }}>Use INI value</button>
                          )}
                        </>
                      ) : (
                        <span className="pf-hint">not applied by this session yet</span>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>

            <ul className="preflight-findings">
              {preflight.findings
                // The delay has its own block below with the numbers and the
                // controls; repeating the whole explanation here said the same
                // thing twice on one screen.
                .filter((f) => f.code !== 'delay_default_curve')
                .map((f) => (
                <li key={f.code} className={`preflight-${f.severity}`}>
                  <div className="preflight-head">
                    <span className="preflight-sev">{severityLabel[f.severity]}</span>
                    <span className="preflight-title">{f.title}</span>
                  </div>
                  <div className="preflight-detail">{f.detail}</div>
                  {f.current && (
                    <div className="preflight-values">
                      now <code>{f.current}</code>
                      {f.suggested && <> → suggested <code>{f.suggested}</code></>}
                      {fixable(f.code) && (
                        <button
                          type="button"
                          className="preflight-fixbtn"
                          onClick={() => { applyFix(f); void startAutoTune(); }}
                          disabled={preflightBusy}
                        >
                          Apply &amp; re-check
                        </button>
                      )}
                    </div>
                  )}
                </li>
              ))}
              {preflight.findings.length === 0 && <li className="preflight-info">Nothing to flag.</li>}
            </ul>

            {/* The delay is the one setting where a measurement beats any
                default, so the fitted model is offered - but left editable,
                because a fit over few samples is a suggestion, not a fact. */}
            <div className="preflight-delay">
              <div className="preflight-delay-head">
                <strong>Transport delay</strong>
                {preflight.delayFit ? (
                  <span className="preflight-delay-fit">
                    fitted to {preflight.delayFit.samples} of your own measurements
                    {' '}(±{Math.round(preflight.delayFit.rmsMs)} ms)
                  </span>
                ) : (
                  <span className="preflight-delay-fit">
                    no measurements this session — run the AFR Delay tool for a fitted model
                  </span>
                )}
              </div>
              <label>
                <input
                  type="checkbox"
                  checked={settings.lambda_delay_flow_scaled}
                  onChange={(e) => setSettings({ ...settings, lambda_delay_flow_scaled: e.target.checked })}
                />
                Scale with flow
              </label>
              <label>
                Idle anchor (ms)
                <input
                  type="number"
                  value={settings.lambda_delay_ms}
                  onChange={(e) => setSettings({ ...settings, lambda_delay_ms: parseFloat(e.target.value) || 0 })}
                />
              </label>
              <label>
                Floor (ms)
                <input
                  type="number"
                  value={settings.lambda_delay_floor_ms}
                  onChange={(e) => setSettings({ ...settings, lambda_delay_floor_ms: parseFloat(e.target.value) || 0 })}
                />
              </label>
              {preflight.delayFit && (
                <button
                  type="button"
                  onClick={() => {
                    setSettings({
                      ...settings,
                      lambda_delay_flow_scaled: true,
                      lambda_delay_floor_ms: Math.round(preflight.delayFit!.floorMs),
                      lambda_delay_ms: Math.round(preflight.delayFit!.anchorMs),
                    });
                  }}
                >
                  Use fitted ({Math.round(preflight.delayFit.anchorMs)} / {Math.round(preflight.delayFit.floorMs)} ms)
                </button>
              )}
            </div>

            <div className="preflight-actions">
              <button type="button" onClick={() => setPreflight(null)}>Cancel</button>
              <button
                type="button"
                className="preflight-go"
                onClick={reallyStartAutoTune}
                disabled={preflight.hasBlocker}
                title={preflight.hasBlocker ? 'Fix the blocking problems first' : undefined}
              >
                {preflight.findings.some((f) => f.severity === 'warning') ? 'Start anyway' : 'Start AutoTune'}
              </button>
            </div>
          </div>
        </div>
      )}
      {/* Header */}
      <div className="autotune-header">
        <div className="autotune-title-row">
          <h2>
            AutoTune
            {!isConnected && <span className="autotune-disconnected-badge">DISCONNECTED</span>}
          </h2>
          <div className="autotune-table-selectors">
            <div className="autotune-table-group">
              <label>Primary:</label>
              <select
                className="autotune-table-selector"
                value={selectedTable}
                onChange={(e) => setSelectedTable(e.target.value)}
                disabled={isRunning}
              >
                {availableTables.map((t) => (
                  <option key={t.name} value={t.name}>{t.title || t.name}</option>
                ))}
              </select>
            </div>
            <div className="autotune-table-group">
              <label className="autotune-secondary-toggle">
                <input
                  type="checkbox"
                  checked={secondaryTableEnabled}
                  onChange={(e) => setSecondaryTableEnabled(e.target.checked)}
                  disabled={isRunning}
                />
                Secondary:
              </label>
              <select
                className="autotune-table-selector"
                value={secondaryTable}
                onChange={(e) => setSecondaryTable(e.target.value)}
                disabled={!secondaryTableEnabled || isRunning}
              >
                {secondaryOptions.length === 0 ? (
                  <option value="" disabled>
                    No other fuel tables
                  </option>
                ) : (
                  secondaryOptions.map((t) => (
                    <option key={t.name} value={t.name}>{t.title || t.name}</option>
                  ))
                )}
              </select>
            </div>
            <div className="autotune-table-group">
              <label>View:</label>
              <select
                className="autotune-table-selector"
                value={activeView}
                onChange={(e) => setActiveView(e.target.value as 'primary' | 'secondary')}
                disabled={!secondaryTableEnabled}
              >
                <option value="primary">Primary</option>
                <option value="secondary">Secondary</option>
              </select>
            </div>
          </div>
        </div>
        {/* Rejection indicator (issue #132): a session that accepts nothing
            looks exactly like a broken one. Show what the filters are doing
            while the session runs; highlight when nothing gets through. */}
        {isRunning && sampleStats && (
          <div
            className={`autotune-sample-stats${
              sampleStats.accepted === 0 && sampleStats.rejections.length > 0
                ? ' autotune-sample-stats-warning'
                : ''
            }`}
            title={
              sampleStats.rejections.length > 0
                ? `Rejected samples by reason:\n${sampleStats.rejections
                    .map((r) => `${r.reason}: ${r.count}`)
                    .join('\n')}`
                : 'All samples passed the filters.'
            }
          >
            <span className="autotune-sample-accepted">
              {sampleStats.accepted} sample{sampleStats.accepted === 1 ? '' : 's'} accepted
            </span>
            {sampleStats.rejections.length > 0 && (
              <span className="autotune-sample-rejected">
                {sampleStats.rejections
                  .slice(0, 2)
                  .map((r) => `${r.count}× ${r.reason}`)
                  .join(' · ')}
                {sampleStats.rejections.length > 2
                  ? ` · +${sampleStats.rejections.length - 2} more`
                  : ''}
              </span>
            )}
          </div>
        )}
        <div className="autotune-controls">
          <button onClick={loadReferenceTable} title="Load reference table from CSV">
            <FolderOpen size={14} /> Load Ref
          </button>
          <button onClick={saveReferenceTable} disabled={!tableData} title="Save current table as reference">
            <Save size={14} /> Save Ref
          </button>
          {isRunning ? (
            <button onClick={stopAutoTune} className="autotune-stop">
              <Square size={14} fill="currentColor" /> Stop
            </button>
          ) : (
            <button
              onClick={startAutoTune}
              className="autotune-start"
              disabled={!isConnected}
              title={isConnected ? undefined : 'Connect to the ECU to start AutoTune'}
            >
              <Play size={14} fill="currentColor" /> Start
            </button>
          )}
          <button onClick={sendRecommendations} disabled={!isRunning && heatmapData.length === 0}>
            <Upload size={14} /> Send to ECU
          </button>
          {onClose && <button onClick={onClose} aria-label="Close"><X size={14} /></button>}
        </div>
      </div>

      {error && <div className="autotune-error">{error}</div>}

      {/* Main content */}
      <div className="autotune-content">
        {/* Left panel - Table view */}
        <div className="autotune-table-panel">
          <div className="autotune-table-toolbar">
            <span>Heatmap:</span>
            <select 
              value={showHeatmap} 
              onChange={(e) => setShowHeatmap(e.target.value as 'weighting' | 'change' | 'none')}
            >
              <option value="weighting">Hit Weighting</option>
              <option value="change">Change Magnitude</option>
              <option value="none">Value Only</option>
            </select>
            <button onClick={lockSelectedCells} disabled={selectedCells.size === 0}>
              <Lock size={14} /> Lock
            </button>
            <button onClick={unlockSelectedCells} disabled={selectedCells.size === 0}>
              <LockOpen size={14} /> Unlock
            </button>
          </div>

          <div className="autotune-table-container">
            <table className="autotune-table">
              <thead>
                <tr>
                  <th className="autotune-corner"></th>
                  {tableData.x_bins.map((bin, i) => (
                    <th key={i}>{bin.toFixed(0)}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {tableData.y_bins.map((yBin, y) => (
                  <tr key={y}>
                    <th>{yBin.toFixed(0)}</th>
                    {tableData.x_bins.map((_, x) => {
                      const value = tableData.z_values[y]?.[x] ?? 0;
                      const key = `${x},${y}`;
                      const isLocked = lockedCells.has(key);
                      const isSelected = selectedCells.has(key);
                      const isCurrent = currentCell?.x === x && currentCell?.y === y;
                      const entry = heatmapLookup[key];

                      return (
                        <td
                          key={x}
                          className={`autotune-cell ${isLocked ? 'locked' : ''} ${isSelected ? 'selected' : ''} ${isCurrent ? 'current' : ''} ${entry && entry.hit_count > 0 ? 'has-hits' : ''}`}
                          style={{ backgroundColor: getCellColor(x, y, value) }}
                          onClick={() => toggleCellLock(x, y)}
                          title={
                            entry
                              ? `Beginning: ${entry.beginning_value.toFixed(1)}\nRecommended: ${entry.recommended_value.toFixed(1)}\nHits: ${entry.hit_count}`
                              : `Value: ${value.toFixed(1)}`
                          }
                        >
                          {entry && showHeatmap === 'change' ? (
                            <span className="cell-change">
                              {entry.recommended_value.toFixed(1)}
                            </span>
                          ) : (
                            value.toFixed(1)
                          )}
                          {isLocked && <span className="cell-lock-icon"><Lock size={10} /></span>}
                          {entry && entry.hit_count > 0 && (
                            <span className="cell-hit-badge" title={`${entry.hit_count} hits`}>
                              {entry.hit_count > 99 ? '99+' : entry.hit_count}
                            </span>
                          )}
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Legend */}
          <div className="autotune-legend">
            {showHeatmap === 'weighting' && (
              <>
                <span className="legend-label">Low hits</span>
                <div className="legend-gradient weighting"></div>
                <span className="legend-label">High hits</span>
              </>
            )}
            {showHeatmap === 'change' && (
              <>
                <span className="legend-label">Richer</span>
                <div className="legend-gradient change"></div>
                <span className="legend-label">Leaner</span>
              </>
            )}
          </div>
        </div>

        {/* Right panel - Settings */}
        <div className="autotune-settings-panel">
          {/* Stats */}
          {stats && (
            <div className="autotune-stats">
              <h3>Statistics</h3>
              <div className="stat-row">
                <span>Total Hits:</span>
                <span>{stats.totalHits}</span>
              </div>
              <div className="stat-row">
                <span>Cells with Data:</span>
                <span>{stats.cellsWithData}</span>
              </div>
              <div className="stat-row">
                <span>Avg Change:</span>
                <span>{stats.avgChange.toFixed(2)}%</span>
              </div>
              <div className="stat-row">
                <span>Locked Cells:</span>
                <span>{lockedCells.size}</span>
              </div>
            </div>
          )}

          {/* AI Tune Health */}
          {selectedTable && <TuneHealthCard tableName={selectedTable} />}

          {/* Settings */}
          <div className="autotune-settings-section">
            <h3>Target</h3>
            <div className="setting-row">
              <label>Target AFR:</label>
              <input
                type="number"
                value={settings.target_afr}
                onChange={(e) => setSettings({ ...settings, target_afr: parseFloat(e.target.value) })}
                step="0.1"
                min="10"
                max="20"
              />
            </div>
            <div className="setting-row">
              <label>Algorithm:</label>
              <select
                value={settings.algorithm}
                onChange={(e) => setSettings({ ...settings, algorithm: e.target.value })}
              >
                <option value="simple">Simple</option>
                <option value="weighted">Weighted Average</option>
                <option value="pid">PID</option>
              </select>
            </div>
            <div className="setting-row">
              <label>Load Source:</label>
              <select
                value={loadSource}
                onChange={(e) => {
                  manualLoadSourceRef.current = true;
                  setLoadSource(e.target.value as AutoTuneLoadSource);
                  setLoadSourceHint(null);
                }}
                disabled={isRunning}
              >
                <option value="map">MAP (Speed Density)</option>
                <option value="maf">MAF</option>
                <option value="tps">TPS (Alpha-N / ITB)</option>
              </select>
            </div>
            {loadSourceHint && <div className="autotune-hint">{loadSourceHint}</div>}
          </div>

          <div className="autotune-settings-section">
            <h3>Filters</h3>
            <div className="setting-row">
              <label>Min RPM:</label>
              <input
                type="number"
                value={filters.min_rpm}
                onChange={(e) => setFilters({ ...filters, min_rpm: parseInt(e.target.value) })}
              />
            </div>
            <div className="setting-row">
              <label>Max RPM:</label>
              <input
                type="number"
                value={filters.max_rpm}
                onChange={(e) => setFilters({ ...filters, max_rpm: parseInt(e.target.value) })}
              />
            </div>
            <div className="setting-row">
              <label>Min Coolant (°C):</label>
              <input
                type="number"
                value={filters.min_clt}
                onChange={(e) => setFilters({ ...filters, min_clt: parseInt(e.target.value) })}
              />
            </div>
            <div className="setting-row">
              <label>Custom Filter:</label>
              <input
                type="text"
                value={filters.custom_filter}
                onChange={(e) => setFilters({ ...filters, custom_filter: e.target.value })}
                placeholder="rpm > 2000 && tps < 50 && clt > 70"
              />
            </div>
            <div className="setting-row">
              <label>Max TPS Rate (%/sec):</label>
              <input
                type="number"
                value={filters.max_tps_rate}
                onChange={(e) => setFilters({ ...filters, max_tps_rate: parseFloat(e.target.value) })}
              />
            </div>
            <div className="setting-row">
              <label>
                <input
                  type="checkbox"
                  checked={filters.exclude_accel_enrich}
                  onChange={(e) => setFilters({ ...filters, exclude_accel_enrich: e.target.checked })}
                />
                Exclude Accel Enrich
              </label>
            </div>
            <div className="setting-row">
              <label>
                <input
                  type="checkbox"
                  checked={filters.min_steady_ms > 0}
                  onChange={(e) =>
                    setFilters({
                      ...filters,
                      // 800 ms is the figure that held up across two logged
                      // drives: held-out AFR improvement 34.1% -> 39.5%, and
                      // the largest proposed change roughly halved.
                      min_steady_ms: e.target.checked ? STEADY_MS_DEFAULT : 0,
                    })
                  }
                />
                Require Steady State
              </label>
            </div>
            <div className="setting-row">
              <label>Steady For (ms):</label>
              <input
                type="number"
                min={0}
                step={100}
                disabled={filters.min_steady_ms === 0}
                value={filters.min_steady_ms}
                onChange={(e) =>
                  setFilters({
                    ...filters,
                    // u64 on the Rust side - serde rejects a float for it and
                    // the whole start_autotune call fails, so round here.
                    min_steady_ms: Math.max(0, Math.round(parseFloat(e.target.value) || 0)),
                  })
                }
              />
            </div>
          </div>

          <div className="autotune-settings-section">
            <h3>Authority Limits</h3>
            <div className="setting-row">
              <label>Max Change/Cell (VE):</label>
              <input
                type="number"
                value={authority.max_change_per_cell}
                onChange={(e) => setAuthority({ ...authority, max_change_per_cell: parseFloat(e.target.value) })}
              />
            </div>
            <div className="setting-row">
              <label>Max Change/Cell (%):</label>
              <input
                type="number"
                value={authority.max_total_change}
                onChange={(e) => setAuthority({ ...authority, max_total_change: parseFloat(e.target.value) })}
              />
            </div>
            <div className="setting-row">
              <label>Min Cell Value (VE):</label>
              <input
                type="number"
                value={authority.min_value}
                onChange={(e) => setAuthority({ ...authority, min_value: parseFloat(e.target.value) })}
              />
            </div>
            <div className="setting-row">
              <label>Max Cell Value (VE):</label>
              <input
                type="number"
                value={authority.max_value}
                onChange={(e) => setAuthority({ ...authority, max_value: parseFloat(e.target.value) })}
              />
            </div>
          </div>

          <div className="autotune-settings-section">
            <h3>Reference Tables &amp; Lambda Delay</h3>
            <div className="setting-row">
              <label>Target AFR Table:</label>
              <input
                type="text"
                placeholder="Auto-discover (blank = use Target AFR setting)"
                value={targetAfrTable}
                onChange={(e) => setTargetAfrTable(e.target.value)}
              />
            </div>
            <div className="setting-row">
              <label>Lambda Delay Table:</label>
              <input
                type="text"
                placeholder="Optional (blank = RPM-based curve)"
                value={lambdaDelayTable}
                onChange={(e) => setLambdaDelayTable(e.target.value)}
              />
            </div>
            <div className="setting-row">
              <label>{settings.lambda_delay_flow_scaled ? 'Idle Delay (ms):' : 'Lambda Delay (ms):'}</label>
              <input
                type="number"
                value={settings.lambda_delay_ms}
                onChange={(e) =>
                  setSettings({
                    ...settings,
                    lambda_delay_ms: Math.max(0, parseFloat(e.target.value) || 0),
                  })
                }
                step="10"
                min="0"
                placeholder="0 = auto (RPM curve)"
                title="AFR transport delay. 0 = auto. Measure it for your car — the RPM curve tops out at ~200 ms. When flow-scaled is on, this is the delay at the low-flow (idle/cruise) anchor."
              />
            </div>
            <label className="autotune-checkbox-row">
              <input
                type="checkbox"
                checked={settings.lambda_delay_flow_scaled}
                onChange={(e) =>
                  setSettings({ ...settings, lambda_delay_flow_scaled: e.target.checked })
                }
              />
              Flow-scale the delay across the table (long at idle, short at high load)
            </label>
            {settings.lambda_delay_flow_scaled && (
              <div className="setting-row">
                <label>High-flow Floor (ms):</label>
                <input
                  type="number"
                  value={settings.lambda_delay_floor_ms}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      lambda_delay_floor_ms: Math.max(0, parseFloat(e.target.value) || 0),
                    })
                  }
                  step="10"
                  min="0"
                  title="High-flow asymptote — roughly the sensor's own response time, approached as exhaust flow rises."
                />
              </div>
            )}
            <label className="autotune-checkbox-row">
              <input
                type="checkbox"
                checked={strictLambdaMatch}
                onChange={(e) => setStrictLambdaMatch(e.target.checked)}
              />
              Strict lambda-delay match (drop unmatched samples — recommended)
            </label>
            {!strictLambdaMatch && (
              <div className="autotune-warning-banner" role="alert">
                ⚠️ Strict matching is OFF. Samples with no delayed-buffer match
                will be attributed to the current cell, which can inject AFR
                readings into the wrong load cell during throttle transients.
                Leave ON for safest tuning.
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export default AutoTune;