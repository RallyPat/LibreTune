import { useState, useEffect, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { valueToHeatmapColor } from '../../utils/heatmapColors';
import './AutoTuneLive.css';

// =============================================================================
// Types
// =============================================================================

interface AutoTuneSettings {
  target_afr: number;
  algorithm: string;
  update_rate_ms: number;
}

interface AutoTuneFilters {
  min_rpm: number;
  max_rpm: number;
  min_tps: number;
  max_tps: number;
  min_clt: number;
  require_steady_state: boolean;
  steady_state_rpm_delta: number;
  steady_state_time_ms: number;
}

interface AutoTuneAuthorityLimits {
  max_change_per_cell: number;
  max_total_change: number;
  min_value: number;
  max_value: number;
}

interface HeatmapEntry {
  cell_x: number;
  cell_y: number;
  hit_weighting: number;
  change_magnitude: number;
  beginning_value: number;
  recommended_value: number;
  hit_count: number;
}

interface TableData {
  name: string;
  title: string;
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
}

interface TableInfo {
  name: string;
  title: string;
}

interface AutoTuneLiveProps {
  tableName?: string;
  onClose?: () => void;
}

// =============================================================================
// AutoTune Live Component
// =============================================================================

export function AutoTuneLive({ tableName: initialTableName = 'veTable1', onClose }: AutoTuneLiveProps) {
  // State
  const [isRunning, setIsRunning] = useState(false);
  const [selectedTable, setSelectedTable] = useState(initialTableName);
  const [availableTables, setAvailableTables] = useState<TableInfo[]>([]);
  const [tableData, setTableData] = useState<TableData | null>(null);
  const [_referenceData, setReferenceData] = useState<TableData | null>(null);
  const [heatmapData, setHeatmapData] = useState<HeatmapEntry[]>([]);
  const [lockedCells, setLockedCells] = useState<Set<string>>(new Set());
  const [selectedCells, _setSelectedCells] = useState<Set<string>>(new Set());
  const [currentCell, _setCurrentCell] = useState<{ x: number; y: number } | null>(null);
  const [showHeatmap, setShowHeatmap] = useState<'weighting' | 'change' | 'none'>('weighting');
  const [error, setError] = useState<string | null>(null);

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

  // Load initial table data
  useEffect(() => {
    loadAvailableTables();
  }, []);

  useEffect(() => {
    loadTableData();
  }, [selectedTable]);

  // Poll heatmap data when running
  useEffect(() => {
    if (!isRunning) return;

    const interval = setInterval(async () => {
      try {
        const data = await invoke<HeatmapEntry[]>('get_autotune_heatmap');
        setHeatmapData(data);
      } catch (e) {
        console.error('Failed to fetch heatmap:', e);
      }
    }, 500);

    return () => clearInterval(interval);
  }, [isRunning]);

  const loadAvailableTables = useCallback(async () => {
    try {
      const tables = await invoke<TableInfo[]>('get_available_tables');
      setAvailableTables(tables);
    } catch (e) {
      console.error('Failed to load available tables:', e);
    }
  }, []);

  const loadTableData = useCallback(async () => {
    try {
      const data = await invoke<TableData>('get_table_data', { tableName: selectedTable });
      setTableData(data);
    } catch (e) {
      setError(`Failed to load table: ${e}`);
    }
  }, [selectedTable]);

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
        settings,
        filters,
        authorityLimits: authority,
      });
      setIsRunning(true);
      setError(null);
    } catch (e) {
      setError(`Failed to start AutoTune: ${e}`);
    }
  }, [selectedTable, settings, filters, authority]);

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
      await invoke('send_autotune_recommendations');
      // Refresh table data after sending
      await loadTableData();
    } catch (e) {
      setError(`Failed to send recommendations: ${e}`);
    }
  }, [loadTableData]);

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
      await invoke('lock_autotune_cells', { cells });
      setLockedCells((prev) => new Set([...prev, ...selectedCells]));
    } catch (e) {
      console.error('Failed to lock cells:', e);
    }
  }, [selectedCells]);

  const unlockSelectedCells = useCallback(async () => {
    const cells = Array.from(selectedCells).map((key) => {
      const [x, y] = key.split(',').map(Number);
      return [x, y] as [number, number];
    });
    
    try {
      await invoke('unlock_autotune_cells', { cells });
      setLockedCells((prev) => {
        const next = new Set(prev);
        selectedCells.forEach((key) => next.delete(key));
        return next;
      });
    } catch (e) {
      console.error('Failed to unlock cells:', e);
    }
  }, [selectedCells]);

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
    <div className="autotune-live">
      {/* Header */}
      <div className="autotune-header">
        <div className="autotune-title-row">
          <h2>AutoTune Live</h2>
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

export default AutoTuneLive;
