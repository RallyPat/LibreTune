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

import { useState, useEffect, useCallback, useMemo } from 'react';
import { FolderOpen, Save, Square, Play, Upload, X, Lock, LockOpen } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { valueToHeatmapColor } from '../../utils/heatmapColors';
import { TuneHealthCard } from './TuneHealth';
import { useToast } from '../../contexts/ToastContext';
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
  /** Minimum throttle position percentage */
  min_tps: number;
  /** Maximum throttle position percentage */
  max_tps: number;
  /** Minimum coolant temperature (reject cold engine data) */
  min_clt: number;
  /** Custom filter expression (e.g., "rpm > 2000 && tps < 50") */
  custom_filter: string;
  /** Maximum TPS change rate (%/sec) before filtering */
  max_tps_rate: number;
  /** Exclude data when accel enrichment is active */
  exclude_accel_enrich: boolean;
  /** Require steady-state operation for valid data */
  require_steady_state: boolean;
  /** Maximum RPM change for steady-state detection */
  steady_state_rpm_delta: number;
  /** Minimum time at steady-state in milliseconds */
  steady_state_time_ms: number;
}

/**
 * Limits on how much AutoTune can modify cell values.
 */
interface AutoTuneAuthorityLimits {
  /** Maximum change per update per cell (percentage) */
  max_change_per_cell: number;
  /** Maximum total change from original value (percentage) */
  max_total_change: number;
  /** Absolute minimum allowed cell value */
  min_value: number;
  /** Absolute maximum allowed cell value */
  max_value: number;
}

type AutoTuneLoadSource = 'map' | 'maf' | 'tps';

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
  const [veAnalyzeConfig, setVeAnalyzeConfig] = useState<VeAnalyzeConfig | null>(null);
  const [lockedCells, setLockedCells] = useState<Set<string>>(new Set());
  const [selectedCells, _setSelectedCells] = useState<Set<string>>(new Set());
  const [currentCell, _setCurrentCell] = useState<{ x: number; y: number } | null>(null);
  const [showHeatmap, setShowHeatmap] = useState<'weighting' | 'change' | 'none'>('weighting');
  const [error, setError] = useState<string | null>(null);
  const [loadSource, setLoadSource] = useState<AutoTuneLoadSource>('map');
  const [loadSourceHint, setLoadSourceHint] = useState<string | null>(null);

  // Settings state
  const [settings, setSettings] = useState<AutoTuneSettings>({
    target_afr: 14.7,
    algorithm: 'simple',
    update_rate_ms: 100,
    lambda_delay_ms: 0,
    lambda_delay_flow_scaled: false,
    lambda_delay_floor_ms: 120,
  });

  const [filters, setFilters] = useState<AutoTuneFilters>({
    min_rpm: 800,
    max_rpm: 7000,
    min_tps: 0,
    max_tps: 100,
    min_clt: 60,
    custom_filter: '',
    max_tps_rate: 10,
    exclude_accel_enrich: true,
    require_steady_state: true,
    steady_state_rpm_delta: 50,
    steady_state_time_ms: 500,
  });

  const [authority, setAuthority] = useState<AutoTuneAuthorityLimits>({
    max_change_per_cell: 15,
    max_total_change: 30,
    min_value: 0,
    max_value: 200,
  });

  // Reference-table / lambda-match configuration (bug #2, #14).
  // Leaving the AFR table blank uses auto-discovery from the INI, falling back
  // to settings.target_afr. Strict lambda matching (default on) drops samples
  // with no delayed-buffer match rather than mis-attributing them.
  const [targetAfrTable, setTargetAfrTable] = useState<string>('');
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
      const tables = await invoke<TableInfo[]>('get_tables');
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

      if (!secondaryTable && tables.length > 1) {
        const preferredSecondary = veAnalyzeConfig?.lambda_target_tables
          ?.map((name) => tables.find((t) => t.name === name))
          .find((t) => t && t.name !== selectedTable);
        const fallbackSecondary = preferredSecondary || tables.find((t) => t.name !== selectedTable) || tables[0];
        setSecondaryTable(fallbackSecondary.name);
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
    if (!tableData || isRunning) return;
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

  const startAutoTune = useCallback(async () => {
    if (!isConnected) {
      showToast('Connect to the ECU to start AutoTune — it needs live data to generate recommendations.', 'warning');
      return;
    }
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
  }, [isConnected, showToast, selectedTable, secondaryTableEnabled, secondaryTable, loadSource, settings, filters, authority, targetAfrTable, lambdaDelayTable, strictLambdaMatch]);

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

  return (
    <div className="autotune">
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
                {secondaryOptions.map((t) => (
                  <option key={t.name} value={t.name}>{t.title || t.name}</option>
                ))}
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
                  checked={filters.require_steady_state}
                  onChange={(e) => setFilters({ ...filters, require_steady_state: e.target.checked })}
                />
                Require Steady State
              </label>
            </div>
          </div>

          <div className="autotune-settings-section">
            <h3>Authority Limits</h3>
            <div className="setting-row">
              <label>Max Change/Cell (%):</label>
              <input
                type="number"
                value={authority.max_change_per_cell}
                onChange={(e) => setAuthority({ ...authority, max_change_per_cell: parseFloat(e.target.value) })}
              />
            </div>
            <div className="setting-row">
              <label>Max Total Change (%):</label>
              <input
                type="number"
                value={authority.max_total_change}
                onChange={(e) => setAuthority({ ...authority, max_total_change: parseFloat(e.target.value) })}
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