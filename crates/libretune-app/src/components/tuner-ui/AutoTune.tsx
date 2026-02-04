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
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { valueToHeatmapColor } from '../../utils/heatmapColors';
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

type AutoTuneLoadSource = 'map' | 'maf';

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
}

// =============================================================================
// AutoTune Component
// =============================================================================

export function AutoTune({ tableName: initialTableName = 'veTable1', onClose }: AutoTuneProps) {
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

  const isMafChannelName = useCallback((name?: string | null) => {
    if (!name) return false;
    const lower = name.toLowerCase();
    return lower.includes('maf') || lower.includes('airmass') || lower.includes('airflow');
  }, []);

  const loadAvailableTables = useCallback(async () => {
    try {
      const tables = await invoke<TableInfo[]>('get_tables');
      
      // Sort: prioritize MAF or VE/fuel tables first, then alphabetically
      const priorityKeywords =
        loadSource === 'maf'
          ? ['maf', 'airmass', 'airflow', 'fuel']
          : ['ve', 'fuel', 'afr', 'lambda'];
      const isPriorityTable = (t: TableInfo) =>
        priorityKeywords.some(
          (kw) => t.name.toLowerCase().includes(kw) || t.title.toLowerCase().includes(kw)
        );

      const sorted = [...tables].sort((a, b) => {
        const aIsPriority = isPriorityTable(a);
        const bIsPriority = isPriorityTable(b);
        if (aIsPriority && !bIsPriority) return -1;
        if (!aIsPriority && bIsPriority) return 1;
        return a.title.localeCompare(b.title);
      });
      setAvailableTables(sorted);
      
      // Auto-select VE table if current selection not found
      const currentExists = sorted.some(t => t.name === selectedTable);
      if (!currentExists && sorted.length > 0) {
        const primaryNames =
          loadSource === 'maf'
            ? ['mafTableTbl', 'mafTable1Tbl', 'mafTable1', 'mafTable']
            : ['veTableTbl', 'veTable1Tbl', 'veTable1', 'veTable', 'fuelTableTbl'];
        const primaryTable =
          sorted.find((t) => primaryNames.includes(t.name)) ||
          (loadSource === 'maf'
            ? sorted.find((t) => t.title.toLowerCase().includes('maf'))
            : sorted.find((t) => t.title.toLowerCase().includes('ve table'))) ||
          sorted[0]; // Fallback to first (priority-related due to sorting)
        if (primaryTable) {
          setSelectedTable(primaryTable.name);
        }
      }

      if (!secondaryTable && sorted.length > 1) {
        const fallbackSecondary = sorted.find((t) => t.name !== selectedTable) || sorted[0];
        setSecondaryTable(fallbackSecondary.name);
      }
    } catch (e) {
      console.error('Failed to load available tables:', e);
      setError('Failed to load tables: ' + e);
    }
  }, [loadSource, selectedTable, secondaryTable]);

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

  // Load initial table data
  useEffect(() => {
    loadAvailableTables();
  }, [loadAvailableTables]);

  useEffect(() => {
    loadTableData();
  }, [activeTable]);

  useEffect(() => {
    if (!tableData || isRunning) return;
    if (isMafChannelName(tableData.y_output_channel) && loadSource !== 'maf') {
      setLoadSource('maf');
    }
  }, [isMafChannelName, isRunning, loadSource, tableData]);

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
      });
      setIsRunning(true);
      setError(null);
    } catch (e) {
      setError(`Failed to start AutoTune: ${e}`);
    }
  }, [selectedTable, secondaryTableEnabled, secondaryTable, loadSource, settings, filters, authority]);

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

  // Get cell color based on heatmap mode
  const getCellColor = useCallback(
    (x: number, y: number, value: number) => {
      const key = `${x},${y}`;
      const entry = heatmapLookup[key];

      if (lockedCells.has(key)) {
        return 'var(--cell-locked)';
      }

      if (!entry || showHeatmap === 'none') {
        // Default value-based coloring using centralized heatmap utility
        return valueToHeatmapColor(value, 0, 100, 'tunerstudio');
      }

      if (showHeatmap === 'weighting') {
        // Coverage/weighting heatmap using centralized utility
        const w = Math.min(1, entry.hit_weighting);
        return valueToHeatmapColor(w, 0, 1, 'tunerstudio');
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
    [heatmapLookup, showHeatmap, lockedCells, authority.max_change_per_cell]
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
          <h2>AutoTune</h2>
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
            📂 Load Ref
          </button>
          <button onClick={saveReferenceTable} disabled={!tableData} title="Save current table as reference">
            💾 Save Ref
          </button>
          {isRunning ? (
            <button onClick={stopAutoTune} className="autotune-stop">
              ⏹ Stop
            </button>
          ) : (
            <button onClick={startAutoTune} className="autotune-start">
              ▶ Start
            </button>
          )}
          <button onClick={sendRecommendations} disabled={!isRunning && heatmapData.length === 0}>
            📤 Send to ECU
          </button>
          {onClose && <button onClick={onClose}>✕</button>}
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
              🔒 Lock
            </button>
            <button onClick={unlockSelectedCells} disabled={selectedCells.size === 0}>
              🔓 Unlock
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
                          {isLocked && <span className="cell-lock-icon">🔒</span>}
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
        </div>
      </div>
    </div>
  );
}

export default AutoTune;