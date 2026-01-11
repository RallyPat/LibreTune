import { useState, useEffect, useCallback } from 'react';
import { Play, Square, Activity, Save, Gauge, Lock, Unlock, Zap, Flame } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { valueToHeatmapColor } from '../../utils/heatmapColors';
import './AutoTuneLive.css';

export default function AutoTuneLive({ onClose }: { onClose: () => void }) {
  const [isRunning, setIsRunning] = useState(false);
  const [updateController, setUpdateController] = useState(false);
  const [activeTab, setActiveTab] = useState<'recommended' | 'weighting' | 'change'>('recommended');
  const [selectedCell, setSelectedCell] = useState<{x: number; y: number } | null>(null);
  const [lockedCells, setLockedCells] = useState<Set<string>>(new Set());
  const [showAdvanced, setShowAdvanced] = useState(false);

  const [settings, setSettings] = useState({
    maxCellValueChange: 10.0,
    maxCellPercentageChange: 20.0,
    minRpm: 1000,
    maxRpm: 7000,
    minClt: 160.0,
    customFilter: '',
  });

  const [recommendations, setRecommendations] = useState<Map<string, {
    beginningValue: number;
    recommendedValue: number;
    hitCount: number;
    hitWeighting: number;
    targetAfr: number;
    hitPercentage: number;
  }>>(new Map());

  const [heatMapData, _setHeatMapData] = useState<{
    weighting: Map<string, number>;
    change: Map<string, number>;
  }>({
    weighting: new Map(),
    change: new Map(),
  });

  useEffect(() => {
    const loadRecommendations = async () => {
      try {
        const recs = await invoke('get_autotune_recommendations') as any[];
        const recMap = new Map(recs.map((r: any) => [
          `${r.cell_x},${r.cell_y}`,
          {
            beginningValue: r.beginning_value,
            recommendedValue: r.recommended_value,
            hitCount: r.hit_count,
            hitWeighting: r.hit_weighting,
            targetAfr: r.target_afr,
            hitPercentage: r.hit_percentage,
          },
        ]));
        setRecommendations(recMap);
      } catch (e) {
        console.error('Failed to load recommendations:', e);
      }
    };

    const loadHeatmap = async () => {
      try {
        const heat = await invoke('get_autotune_heatmap') as any[];
        const weighting = new Map<string, number>();
        const change = new Map<string, number>();
        for (const h of heat) {
          const key = `${h.cell_x},${h.cell_y}`;
          weighting.set(key, h.hit_weighting || 0);
          change.set(key, h.change_magnitude || 0);
        }
        _setHeatMapData({ weighting, change });
      } catch (e) {
        console.error('Failed to load heatmap:', e);
      }
    };

    const interval = setInterval(() => {
      if (isRunning) {
        loadRecommendations();
        loadHeatmap();
      }
    }, 2000);

    return () => clearInterval(interval);
  }, [isRunning]);

  const handleStartStop = async () => {
    if (!isRunning) {
      await invoke('start_autotune', {
        table_name: 'veTable1',
        settings: { target_table: 'veTable1', updateController, auto_send_updates: true, send_interval_ms: 15000 },
        filters: settings,
        authority_limits: { max_cell_value_change: settings.maxCellValueChange, max_cell_percentage_change: settings.maxCellPercentageChange },
      });
      setIsRunning(true);
    } else {
      await invoke('stop_autotune');
      setIsRunning(false);
    }
  };

  const handleSend = async () => {
    try {
      await invoke('send_autotune_recommendations', { table_name: 'veTable1' });
      // Refresh table in UI
      try {
        await invoke('get_table_data', { tableName: 'veTable1' });
      } catch (_) {}
      alert('VE recommendations sent to ECU!');
    } catch (e) {
      alert('Failed to send recommendations: ' + e);
    }
  };

  const handleBurn = async () => {
    try {
      await invoke('burn_autotune_recommendations', { table_name: 'veTable1' });
      alert('VE recommendations burned to ECU Flash!');
    } catch (e) {
      alert('Failed to burn recommendations: ' + e);
    }
  };

  const handleCellClick = (x: number, y: number) => {
    const key = `${x},${y}`;
    const isLocked = lockedCells.has(key);
    if (isLocked) return;
    setSelectedCell({ x, y });
  };

  const handleCellLockToggle = async (x: number, y: number) => {
    const key = `${x},${y}`;
    const newLocked = new Set(lockedCells);
    if (lockedCells.has(key)) {
      newLocked.delete(key);
      await invoke('unlock_autotune_cells', { cells: [[x, y] as [number, number]] });
    } else {
      newLocked.add(key);
      await invoke('lock_autotune_cells', { cells: [[x, y] as [number, number]] });
    }
    setLockedCells(newLocked);
  };

  const getCellColor = useCallback((value: number, recommended: number) => {
    // Uses 'change' context for AFR correction visualization
    // Positive diff = needs to be richer (blue), negative = needs to be leaner (red)
    const diff = recommended - value;
    const maxDiff = 10; // Normalize to ±10 range
    const normalizedDiff = Math.max(-maxDiff, Math.min(maxDiff, diff));
    // Map diff to 0-1 range where 0 = leaner (red), 1 = richer (blue)
    const normalizedValue = (normalizedDiff + maxDiff) / (2 * maxDiff);
    return valueToHeatmapColor(normalizedValue, 0, 1, 'tunerstudio');
  }, []);

  const getHeatMapValue = useCallback((key: string, type: 'weighting' | 'change') => {
    const value = heatMapData[type].get(key) || 0;
    const max = Math.max(...Array.from(heatMapData[type].values()), 1);
    // Use 'coverage' context for weighting, 'change' context for magnitude
    const scheme = type === 'weighting' ? 'tunerstudio' : 'tunerstudio';
    return valueToHeatmapColor(value, 0, max, scheme);
  }, [heatMapData]);

  return (
    <div className="autotune-live">
      <div className="autotune-header">
        <h2>AutoTune Live</h2>
        <button className="close-btn" onClick={onClose}>
          <Zap size={20} />
        </button>
      </div>

      <div className="autotune-controls">
        <div className="control-group primary">
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={updateController}
              onChange={async e => {
                const checked = e.target.checked;
                setUpdateController(checked);
                try {
                  if (checked) {
                    await invoke('start_autotune_autosend', { table_name: 'veTable1', interval_ms: 15000 });
                  } else {
                    await invoke('stop_autotune_autosend');
                  }
                } catch (err) {
                  console.error('Failed to toggle auto-send:', err);
                }
              }}
            />
            <span>Update Controller every 15s</span>
          </label>
          <button 
            className={`control-btn ${isRunning ? 'start-active' : ''}`}
            onClick={handleStartStop}
          >
            {isRunning ? (
              <>
                <Square size={18} />
                <span>Stop</span>
              </>
            ) : (
              <>
                <Play size={18} />
                <span>Start</span>
              </>
            )}
          </button>
          <button className="control-btn" onClick={handleSend}>
            <Save size={18} />
            <span>Send</span>
          </button>
          <button className="control-btn" onClick={handleBurn}>
            <Flame size={18} />
            <span>Burn</span>
          </button>
        </div>
      </div>

      <div className="ve-tabs">
        <button 
          className={`tab-btn ${activeTab === 'recommended' ? 'active' : ''}`}
          onClick={() => setActiveTab('recommended')}
        >
          <Activity size={18} />
          <span>Recommended Table</span>
        </button>
        <button 
          className={`tab-btn ${activeTab === 'weighting' ? 'active' : ''}`}
          onClick={() => setActiveTab('weighting')}
        >
          <Gauge size={18} />
          <span>Cell Weighting</span>
        </button>
        <button 
          className={`tab-btn ${activeTab === 'change' ? 'active' : ''}`}
          onClick={() => setActiveTab('change')}
        >
          <Flame size={18} />
          <span>Cell Change</span>
        </button>
      </div>

      <div className="ve-content">
        <div className="recommended-table">
          <div className="table-header">
            <h3>VE Table 1</h3>
            <div className="legend">
              <div className="legend-item">
                <span className="legend-color richer"></span>
                <span>Richer (Blue)</span>
              </div>
              <div className="legend-item">
                <span className="legend-color leaner"></span>
                <span>Leaner (Red)</span>
              </div>
              <div className="legend-item">
                <span className="legend-icon">🔒</span>
                <span>Locked</span>
              </div>
            </div>
          </div>
          
          {Array.from(recommendations.entries()).map(([key, data]) => {
            const [x, y] = key.split(',').map(Number);
            return (
              <div
                key={key}
                className={`table-cell ${lockedCells.has(key) ? 'locked' : ''} ${selectedCell?.x === x && selectedCell?.y === y ? 'selected' : ''}`}
                onClick={() => handleCellClick(x, y)}
                style={{
                  backgroundColor: getCellColor(data.beginningValue, data.recommendedValue)
                }}
              >
                <span className="cell-value">
                  {data.recommendedValue.toFixed(1)}
                </span>
                <div className="lock-indicator" />
              </div>
            );
          })}
          
          {selectedCell && (
            <div className="cell-tooltip" style={{
              left: (selectedCell.x / 16) * 100 + '%',
              top: (selectedCell.y / 12) * 100 + '%'
            }}>
              <div className="tooltip-content">
                <div className="tooltip-row">
                  <span className="tooltip-label">Beginning Value:</span>
                  <span className="tooltip-value">{recommendations.get(`${selectedCell.x},${selectedCell.y}`)?.beginningValue?.toFixed(1) || '0.0'}</span>
                </div>
                <div className="tooltip-row">
                  <span className="tooltip-label">Hit Count:</span>
                  <span className="tooltip-value">{recommendations.get(`${selectedCell.x},${selectedCell.y}`)?.hitCount || 0}</span>
                </div>
                <div className="tooltip-row">
                  <span className="tooltip-label">Hit Weighting:</span>
                  <span className="tooltip-value">{recommendations.get(`${selectedCell.x},${selectedCell.y}`)?.hitWeighting?.toFixed(2) || '0.00'}</span>
                </div>
                <div className="tooltip-row">
                  <span className="tooltip-label">Target AFR:</span>
                  <span className="tooltip-value">{recommendations.get(`${selectedCell.x},${selectedCell.y}`)?.targetAfr?.toFixed(2) || '14.70'}</span>
                </div>
                <div className="tooltip-row">
                  <span className="tooltip-label">Hit %:</span>
                  <span className="tooltip-value">{recommendations.get(`${selectedCell.x},${selectedCell.y}`)?.hitPercentage?.toFixed(1) || '0'}%</span>
                </div>
                <div className="tooltip-actions">
                  <button
                    className={`tooltip-lock-btn ${lockedCells.has(`${selectedCell.x},${selectedCell.y}`) ? 'locked' : ''}`}
                    onClick={() => handleCellLockToggle(selectedCell.x, selectedCell.y)}
                  >
                    {lockedCells.has(`${selectedCell.x},${selectedCell.y}`) ? <Unlock size={14} /> : <Lock size={14} />}
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>

        {activeTab === 'weighting' && (
          <div className="heatmap weighting">
            <h3>Cell Weighting (Data Coverage)</h3>
            <div className="heatmap-grid">
              {Array.from(recommendations.entries()).map(([key]) => {
                return (
                  <div
                    key={key}
                    className="heatmap-cell"
                    style={{ backgroundColor: getHeatMapValue(key, 'weighting') }}
                    title={`Weighting: ${heatMapData.weighting.get(key) || 0}`}
                  />
                );
              })}
            </div>
          </div>
        )}

        {activeTab === 'change' && (
          <div className="heatmap change">
            <h3>Cell Change (Magnitude)</h3>
            <div className="heatmap-grid">
              {Array.from(recommendations.entries()).map(([key]) => {
                return (
                  <div
                    key={key}
                    className="heatmap-cell"
                    style={{ backgroundColor: getHeatMapValue(key, 'change') }}
                    title={`Change: ${Math.abs((recommendations.get(key)?.recommendedValue || 0) - (recommendations.get(key)?.beginningValue || 0)).toFixed(2)}`}
                  />
                );
              })}
            </div>
          </div>
        )}

        <div className="advanced-toggle">
          <button className="advanced-btn" onClick={() => setShowAdvanced(!showAdvanced)}>
            {showAdvanced ? 'Hide Advanced Settings' : 'Show Advanced Settings'}
          </button>
        </div>

        {showAdvanced && (
          <div className="advanced-settings">
            <h3>Advanced Settings</h3>
            
            <div className="settings-section">
              <h4>Authority Limits</h4>
              <div className="setting-row">
                <label>Max Cell Value Change:</label>
                <input
                  type="number"
                  step="0.1"
                  value={settings.maxCellValueChange}
                  onChange={e => setSettings({ ...settings, maxCellValueChange: parseFloat(e.target.value) })}
                />
              </div>
              <div className="setting-row">
                <label>Max Cell Percentage Change (%):</label>
                <input
                  type="number"
                  step="1"
                  min="0"
                  max="100"
                  value={settings.maxCellPercentageChange}
                  onChange={e => setSettings({ ...settings, maxCellPercentageChange: parseFloat(e.target.value) })}
                />
              </div>
            </div>

            <div className="settings-section">
              <h4>Filters</h4>
              <div className="setting-row">
                <label>Min RPM:</label>
                <input
                  type="number"
                  value={settings.minRpm}
                  onChange={e => setSettings({ ...settings, minRpm: parseFloat(e.target.value) })}
                />
              </div>
              <div className="setting-row">
                <label>Max RPM:</label>
                <input
                  type="number"
                  value={settings.maxRpm}
                  onChange={e => setSettings({ ...settings, maxRpm: parseFloat(e.target.value) })}
                />
              </div>
              <div className="setting-row">
                <label>Min CLT (°C):</label>
                <input
                  type="number"
                  value={settings.minClt}
                  onChange={e => setSettings({ ...settings, minClt: parseFloat(e.target.value) })}
                />
              </div>
              <div className="setting-row">
                <label>Custom Filter:</label>
                <input
                  type="text"
                  placeholder="e.g., status3 & !status2"
                  value={settings.customFilter}
                  onChange={e => setSettings({ ...settings, customFilter: e.target.value })}
                />
              </div>
            </div>

            <div className="settings-section">
              <h4>Reference Tables</h4>
              <div className="reference-links">
                <button className="reference-btn">
                  <Activity size={16} />
                  <span>Open Lambda Delay Table</span>
                </button>
                <button className="reference-btn">
                  <Activity size={16} />
                  <span>Open AFR Target Table</span>
                </button>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
