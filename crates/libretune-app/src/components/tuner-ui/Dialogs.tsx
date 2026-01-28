import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { HeatmapScheme, getAvailableSchemes } from '../../utils/heatmapColors';
import { useUnitPreferences } from '../../utils/useUnitPreferences';
import { TemperatureUnit, PressureUnit, AfrUnit, SpeedUnit, FuelType, STOICH_AFR } from '../../utils/unitConversions';
import './Dialogs.css';
import ConnectionMetrics from '../layout/ConnectionMetrics';

// =============================================================================
// Dialog Types
// =============================================================================

interface DialogProps {
  isOpen: boolean;
  onClose: () => void;
}

interface TuneInfo {
  path: string | null;
  signature: string;
  modified: boolean;
  has_tune: boolean;
}

// =============================================================================
// Save Dialog
// =============================================================================

interface SaveDialogProps extends DialogProps {
  onSaved?: (path: string) => void;
  autoBurnOnClose?: boolean;
}

export function SaveDialog({ isOpen, onClose, onSaved, autoBurnOnClose }: SaveDialogProps) {
  const [tuneInfo, setTuneInfo] = useState<TuneInfo | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [showBurnConfirm, setShowBurnConfirm] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      invoke<TuneInfo>('get_tune_info')
        .then(setTuneInfo)
        .catch((e) => setError(String(e)));
    }
  }, [isOpen]);

  const handleSave = useCallback(async () => {
    setIsSaving(true);
    setError(null);
    try {
      const path = await invoke<string>('save_tune', { path: null });
      onSaved?.(path);

      // Auto-burn on close with confirmation
      if (autoBurnOnClose) {
        setShowBurnConfirm(true);
      } else {
        onClose();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setIsSaving(false);
    }
  }, [onClose, onSaved, autoBurnOnClose]);

  // Handle burn after save with confirmation
  const handleBurnConfirm = useCallback(async () => {
    setShowBurnConfirm(false);
    try {
      await invoke('burn_to_ecu');
      onClose();
    } catch (e) {
      setError(String(e));
    }
  }, [onClose]);

  const handleBurnCancel = useCallback(() => {
    setShowBurnConfirm(false);
    onClose();
  }, [onClose]);

  const handleSaveAs = useCallback(async () => {
    setIsSaving(true);
    setError(null);
    try {
      const selected = await save({
        title: 'Save Tune As',
        filters: [
          { name: 'MSQ Tune File', extensions: ['msq'] },
          { name: 'JSON Tune File', extensions: ['json'] },
        ],
        defaultPath: tuneInfo?.path || undefined,
      });
      
      if (selected) {
        const path = await invoke<string>('save_tune_as', { path: selected });
        onSaved?.(path);
        onClose();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setIsSaving(false);
    }
  }, [onClose, onSaved, tuneInfo]);

  if (!isOpen) return null;

  return (
    <>
      <div className="dialog-overlay" onClick={onClose}>
        <div className="dialog" onClick={(e) => e.stopPropagation()}>
          <div className="dialog-header">
            <h2>Save Tune</h2>
            <button className="dialog-close" onClick={onClose}>×</button>
          </div>

          <div className="dialog-content">
            {error && <div className="dialog-error">{error}</div>}

            <div className="dialog-info">
              <p><strong>ECU:</strong> {tuneInfo?.signature || 'Unknown'}</p>
              {tuneInfo?.path && (
                <p><strong>Current File:</strong> {tuneInfo.path.split('/').pop()}</p>
              )}
              {tuneInfo?.modified && (
                <p className="dialog-warning">⚠ Tune has unsaved changes</p>
              )}
            </div>

            <div className="dialog-help">
              <p>Save your tune to a file for backup or transfer.</p>
              <p><strong>MSQ format</strong> is compatible with other ECU tuning software.</p>
            </div>
          </div>

          <div className="dialog-footer">
            <button onClick={onClose} disabled={isSaving}>Cancel</button>
            <button
              onClick={handleSaveAs}
              disabled={isSaving}
            >
              Save As...
            </button>
            <button
              onClick={handleSave}
              disabled={isSaving || !tuneInfo?.path}
              className="dialog-primary"
            >
              {isSaving ? 'Saving...' : 'Save'}
            </button>
          </div>
        </div>
      </div>

      {/* Auto-burn confirmation dialog */}
      {showBurnConfirm && (
        <div className="dialog-overlay" onClick={(e) => e.stopPropagation()}>
          <div className="dialog" onClick={(e) => e.stopPropagation()}>
            <div className="dialog-header">
              <h2>Burn Tune to ECU?</h2>
            </div>
            <div className="dialog-content">
              <p>Tune saved successfully. Would you like to burn it to the ECU now?</p>
              <p className="dialog-warning">⚠ This will write to ECU memory and may take several seconds.</p>
            </div>
            <div className="dialog-footer">
              <button onClick={handleBurnCancel}>Cancel</button>
              <button onClick={handleBurnConfirm} className="dialog-primary">Burn to ECU</button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

// =============================================================================
// Load Dialog
// =============================================================================

interface LoadDialogProps extends DialogProps {
  onLoaded?: (tuneInfo: TuneInfo) => void;
}

export function LoadDialog({ isOpen, onClose, onLoaded }: LoadDialogProps) {
  const [tuneFiles, setTuneFiles] = useState<string[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      invoke<string[]>('list_tune_files')
        .then(setTuneFiles)
        .catch((e) => setError(String(e)));
    }
  }, [isOpen]);

  const handleLoad = useCallback(async (path: string) => {
    setIsLoading(true);
    setError(null);
    try {
      const info = await invoke<TuneInfo>('load_tune', { path });
      onLoaded?.(info);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  }, [onClose, onLoaded]);

  const handleBrowse = useCallback(async () => {
    try {
      const selected = await open({
        title: 'Open Tune File',
        multiple: false,
        filters: [
          { name: 'Tune Files', extensions: ['msq', 'json'] },
          { name: 'MSQ Tune File', extensions: ['msq'] },
          { name: 'JSON Tune File', extensions: ['json'] },
          { name: 'All Files', extensions: ['*'] },
        ],
      });
      
      if (selected && typeof selected === 'string') {
        await handleLoad(selected);
      }
    } catch (e) {
      setError(String(e));
    }
  }, [handleLoad]);

  if (!isOpen) return null;

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog dialog-wide" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>Load Tune</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>
        
        <div className="dialog-content">
          {error && <div className="dialog-error">{error}</div>}
          
          <div className="dialog-file-list">
            <div className="dialog-file-header">
              <span>Recent Tune Files</span>
              <button onClick={handleBrowse}>Browse...</button>
            </div>
            
            {tuneFiles.length === 0 ? (
              <div className="dialog-empty">No tune files found in projects folder</div>
            ) : (
              <div className="dialog-files">
                {tuneFiles.map((file) => (
                  <div 
                    key={file}
                    className={`dialog-file-item ${selectedFile === file ? 'selected' : ''}`}
                    onClick={() => setSelectedFile(file)}
                    onDoubleClick={() => handleLoad(file)}
                  >
                    <span className="dialog-file-icon">📄</span>
                    <div className="dialog-file-info">
                      <span className="dialog-file-name">{file.split('/').pop()}</span>
                      <span className="dialog-file-path">{file}</span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
        
        <div className="dialog-footer">
          <button onClick={onClose} disabled={isLoading}>Cancel</button>
          <button 
            onClick={() => selectedFile && handleLoad(selectedFile)}
            disabled={isLoading || !selectedFile}
            className="dialog-primary"
          >
            {isLoading ? 'Loading...' : 'Load'}
          </button>
        </div>
      </div>
    </div>
  );
}

// =============================================================================
// Burn Dialog
// =============================================================================

interface BurnDialogProps extends DialogProps {
  connected: boolean;
  onBurned?: () => void;
}

export function BurnDialog({ isOpen, onClose, connected, onBurned }: BurnDialogProps) {
  const [isBurning, setIsBurning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const handleBurn = useCallback(async () => {
    setIsBurning(true);
    setError(null);
    setSuccess(false);
    
    try {
      await invoke('burn_to_ecu');
      setSuccess(true);
      onBurned?.();
      // Auto-close after success
      setTimeout(onClose, 1500);
    } catch (e) {
      setError(String(e));
    } finally {
      setIsBurning(false);
    }
  }, [onClose, onBurned]);

  if (!isOpen) return null;

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>Burn to ECU</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>
        
        <div className="dialog-content">
          {error && <div className="dialog-error">{error}</div>}
          {success && <div className="dialog-success">✓ Burn completed successfully!</div>}
          
          {!connected ? (
            <div className="dialog-warning">
              ⚠ Not connected to ECU. Please connect first.
            </div>
          ) : (
            <div className="dialog-info">
              <p>This will write all changes from ECU RAM to flash memory.</p>
              <p><strong>Warning:</strong> This operation cannot be undone.</p>
              <p>Make sure your tune is tested before burning.</p>
            </div>
          )}
        </div>
        
        <div className="dialog-footer">
          <button onClick={onClose} disabled={isBurning}>Cancel</button>
          <button 
            onClick={handleBurn}
            disabled={isBurning || !connected || success}
            className="dialog-primary dialog-burn"
          >
            {isBurning ? 'Burning...' : success ? 'Done!' : '🔥 Burn to ECU'}
          </button>
        </div>
      </div>
    </div>
  );
}

// =============================================================================
// New Tune Dialog
// =============================================================================

interface NewTuneDialogProps extends DialogProps {
  onCreated?: () => void;
}

export function NewTuneDialog({ isOpen, onClose, onCreated }: NewTuneDialogProps) {
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleCreate = useCallback(async () => {
    setIsCreating(true);
    setError(null);
    try {
      await invoke('new_tune');
      onCreated?.();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsCreating(false);
    }
  }, [onClose, onCreated]);

  if (!isOpen) return null;

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>New Tune</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>
        
        <div className="dialog-content">
          {error && <div className="dialog-error">{error}</div>}
          
          <div className="dialog-info">
            <p>Create a new tune file for the currently loaded ECU definition.</p>
            <p>Any unsaved changes to the current tune will be lost.</p>
          </div>
        </div>
        
        <div className="dialog-footer">
          <button onClick={onClose} disabled={isCreating}>Cancel</button>
          <button 
            onClick={handleCreate}
            disabled={isCreating}
            className="dialog-primary"
          >
            {isCreating ? 'Creating...' : 'Create New Tune'}
          </button>
        </div>
      </div>
    </div>
  );
}

// =============================================================================
// Settings Dialog
// =============================================================================

interface CurrentProject {
  name: string;
  path: string;
  signature: string;
  has_tune: boolean;
  tune_modified: boolean;
  connection: {
    port: string | null;
    baud_rate: number;
    auto_connect: boolean;
  };
}

interface SettingsDialogProps extends DialogProps {
  theme: string;
  onThemeChange: (theme: string) => void;
  onSettingsChange?: (settings: { units?: string; autoBurnOnClose?: boolean; demoMode?: boolean; indicatorColumnCount?: string; indicatorFillEmpty?: boolean; indicatorTextFit?: string; statusBarChannels?: string[]; runtimePacketMode?: string }) => void;
  currentProject?: CurrentProject | null;
}

export function SettingsDialog({ isOpen, onClose, theme, onThemeChange, onSettingsChange, currentProject }: SettingsDialogProps) {
  const [localTheme, setLocalTheme] = useState(theme);
  const [localUnits, setLocalUnits] = useState('metric');
  const [autoBurnOnClose, setAutoBurnOnClose] = useState(false);
  const [demoMode, setDemoMode] = useState(false);
  const [demoLoading, setDemoLoading] = useState(false);
  const [indicatorColumnCount, setIndicatorColumnCount] = useState('auto');
  const [indicatorFillEmpty, setIndicatorFillEmpty] = useState(false);
  const [indicatorTextFit, setIndicatorTextFit] = useState('scale');
  const [currentIniPath, setCurrentIniPath] = useState<string | null>(null);
  const [switchingIni, setSwitchingIni] = useState(false);
  
  // Status bar channel configuration
  const [statusBarChannels, setStatusBarChannels] = useState<string[]>([]);
  const [availableChannels, setAvailableChannels] = useState<string[]>([]);
  
  // Heatmap settings
  const [heatmapValueScheme, setHeatmapValueScheme] = useState<HeatmapScheme>('tunerstudio');
  const [heatmapChangeScheme, setHeatmapChangeScheme] = useState<HeatmapScheme>('tunerstudio');
  const [heatmapCoverageScheme, setHeatmapCoverageScheme] = useState<HeatmapScheme>('tunerstudio');
  
  // Gauge/Dashboard settings
  const [gaugeSnapToGrid, setGaugeSnapToGrid] = useState(true);
  const [gaugeFreeMove, setGaugeFreeMove] = useState(false);
  const [gaugeLock, setGaugeLock] = useState(false);
  
  // Version control settings
  const [autoCommitOnSave, setAutoCommitOnSave] = useState('never');
  const [commitMessageFormat, setCommitMessageFormat] = useState('Tune saved on {date} at {time}');
  const [runtimePacketMode, setRuntimePacketMode] = useState<'Auto'|'ForceBurst'|'ForceOCH'|'Disabled'>('Auto');
  // Auto-reconnect setting: whether to automatically sync & reconnect after controller commands
  const [autoReconnectAfterControllerCommand, setAutoReconnectAfterControllerCommand] = useState<boolean>(false);
  
  // Project-specific settings
  const [autoConnect, setAutoConnect] = useState(false);
  
  // Unit preferences from context
  const unitPrefs = useUnitPreferences();
  
  // Available heatmap schemes
  const availableSchemes = getAvailableSchemes();

  useEffect(() => {
    setLocalTheme(theme);
    // Load settings from backend
    if (isOpen) {
      invoke('get_settings').then((settings: any) => {
        if (settings.units_system !== undefined) setLocalUnits(settings.units_system);
        if (settings.auto_burn_on_close !== undefined) setAutoBurnOnClose(!!settings.auto_burn_on_close);
        if (settings.indicator_column_count !== undefined) setIndicatorColumnCount(settings.indicator_column_count);
        if (settings.indicator_fill_empty !== undefined) setIndicatorFillEmpty(!!settings.indicator_fill_empty);
        if (settings.indicator_text_fit !== undefined) setIndicatorTextFit(settings.indicator_text_fit);
        if (settings.last_ini_path !== undefined) setCurrentIniPath(settings.last_ini_path);
        // Status bar channels
        if (settings.status_bar_channels !== undefined) setStatusBarChannels(settings.status_bar_channels);
        // Heatmap settings
        if (settings.heatmap_value_scheme !== undefined) setHeatmapValueScheme(settings.heatmap_value_scheme);
        if (settings.heatmap_change_scheme !== undefined) setHeatmapChangeScheme(settings.heatmap_change_scheme);
        if (settings.heatmap_coverage_scheme !== undefined) setHeatmapCoverageScheme(settings.heatmap_coverage_scheme);
        // Gauge settings
        if (settings.gauge_snap_to_grid !== undefined) setGaugeSnapToGrid(!!settings.gauge_snap_to_grid);
        if (settings.gauge_free_move !== undefined) setGaugeFreeMove(!!settings.gauge_free_move);
        if (settings.gauge_lock !== undefined) setGaugeLock(!!settings.gauge_lock);
        // Version control settings
        if (settings.auto_commit_on_save !== undefined) setAutoCommitOnSave(settings.auto_commit_on_save);
        if (settings.commit_message_format !== undefined) setCommitMessageFormat(settings.commit_message_format);
        if (settings.runtime_packet_mode !== undefined) setRuntimePacketMode(settings.runtime_packet_mode);
        if (settings.auto_reconnect_after_controller_command !== undefined) setAutoReconnectAfterControllerCommand(!!settings.auto_reconnect_after_controller_command);
      }).catch(console.error);

      // Load project-specific settings
      if (currentProject) {
        setAutoConnect(currentProject.connection.auto_connect);
      }

      // Load available output channels from ECU definition
      // Backend returns ChannelInfo[]; normalize to string[] (channel names) to avoid render errors
      invoke<any[]>('get_available_channels').then((channels) => {
        try {
          const names = (channels || []).map((c) => (typeof c === 'string' ? c : c?.name ?? String(c)));
          setAvailableChannels(names);
        } catch (e) {
          console.error('[SettingsDialog] Failed to normalize channels:', e);
          setAvailableChannels([]);
        }
      }).catch((e) => {
        console.error('[SettingsDialog] get_available_channels failed:', e);
        setAvailableChannels([]);
      });

      // Load demo mode state (runtime flag)
      invoke<boolean>('get_demo_mode')
        .then((v) => setDemoMode(!!v))
        .catch(console.error);
    }
  }, [theme, isOpen, currentProject]);

  const handleDemoToggle = useCallback(async (enabled: boolean) => {
    setDemoLoading(true);
    try {
      await invoke('set_demo_mode', { enabled });
      setDemoMode(enabled);
      onSettingsChange?.({ demoMode: enabled });
    } catch (e) {
      console.error('Failed to toggle demo mode:', e);
      alert(`Failed to toggle demo mode: ${e}`);
    } finally {
      setDemoLoading(false);
    }
  }, [onSettingsChange]);

  const handleSwitchIni = useCallback(async () => {
    if (!currentProject) {
      alert('No project is currently open');
      return;
    }

    setSwitchingIni(true);
    try {
      const selected = await open({
        title: 'Select ECU Definition File',
        multiple: false,
        filters: [
          { name: 'INI Files', extensions: ['ini'] },
          { name: 'All Files', extensions: ['*'] },
        ],
      });

      if (selected && typeof selected === 'string') {
        // Update the project's INI file
        await invoke('update_project_ini', { 
          iniPath: selected, 
          forceResync: false 
        });
        
        setCurrentIniPath(selected);
        
        // Show success message with helpful info
        const message = 'ECU definition updated successfully!\n\n' +
          'The project tune has been re-applied with the new definition. ' +
          'If tables appear empty, you may need to load a matching MSQ file ' +
          'that was created with this INI definition.';
        alert(message);
      }
    } catch (e) {
      console.error('Failed to switch INI:', e);
      alert(`Failed to switch INI file: ${e}`);
    } finally {
      setSwitchingIni(false);
    }
  }, [currentProject]);

  const handleApply = useCallback(async () => {
    onThemeChange(localTheme);
    // Update units setting
    if (localUnits !== 'metric' && localUnits !== 'imperial') {
      setLocalUnits('metric');
      await invoke('update_setting', { key: 'units_system', value: 'metric' });
    } else {
      await invoke('update_setting', { key: 'units_system', value: localUnits });
    }
    // Update auto-burn setting
    await invoke('update_setting', { key: 'auto_burn_on_close', value: autoBurnOnClose.toString() });
    // Update status bar channels
    await invoke('update_setting', { key: 'status_bar_channels', value: JSON.stringify(statusBarChannels) });
    // Update indicator panel settings
    await invoke('update_setting', { key: 'indicator_column_count', value: indicatorColumnCount });
    await invoke('update_setting', { key: 'indicator_fill_empty', value: indicatorFillEmpty.toString() });
    await invoke('update_setting', { key: 'indicator_text_fit', value: indicatorTextFit });
    // Update heatmap settings
    await invoke('update_setting', { key: 'heatmap_value_scheme', value: heatmapValueScheme });
    await invoke('update_setting', { key: 'heatmap_change_scheme', value: heatmapChangeScheme });
    await invoke('update_setting', { key: 'heatmap_coverage_scheme', value: heatmapCoverageScheme });
    // Update gauge settings
    await invoke('update_setting', { key: 'gauge_snap_to_grid', value: gaugeSnapToGrid.toString() });
    await invoke('update_setting', { key: 'gauge_free_move', value: gaugeFreeMove.toString() });
    await invoke('update_setting', { key: 'gauge_lock', value: gaugeLock.toString() });
    // Update version control settings
    await invoke('update_setting', { key: 'auto_commit_on_save', value: autoCommitOnSave });
    await invoke('update_setting', { key: 'commit_message_format', value: commitMessageFormat });
    // Update runtime packet mode
    await invoke('update_setting', { key: 'runtime_packet_mode', value: runtimePacketMode });
    await invoke('update_setting', { key: 'auto_reconnect_after_controller_command', value: autoReconnectAfterControllerCommand.toString() });
    
    // Update project-specific settings
    if (currentProject) {
      try {
        await invoke('update_project_auto_connect', { autoConnect });
      } catch (e) {
        console.error('Failed to update auto-connect setting:', e);
      }
    }
    
    onSettingsChange?.({ units: localUnits, autoBurnOnClose, indicatorColumnCount, indicatorFillEmpty, indicatorTextFit, statusBarChannels, runtimePacketMode });
    onClose();
  }, [localTheme, localUnits, autoBurnOnClose, statusBarChannels, indicatorColumnCount, indicatorFillEmpty, indicatorTextFit, heatmapValueScheme, heatmapChangeScheme, heatmapCoverageScheme, gaugeSnapToGrid, gaugeFreeMove, gaugeLock, autoCommitOnSave, commitMessageFormat, runtimePacketMode, autoReconnectAfterControllerCommand, autoConnect, currentProject, onThemeChange, onSettingsChange, onClose]);

  if (!isOpen) return null;

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>Settings</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>
        
        <div className="dialog-content">
          <div className="dialog-form-group">
            <label>Theme</label>
            <select 
              value={localTheme} 
              onChange={(e) => setLocalTheme(e.target.value)}
            >
              <option value="dark">Dark</option>
              <option value="light">Light</option>
              <option value="midnight">Midnight</option>
              <option value="carbon">Carbon</option>
            </select>
          </div>
          
          <div className="dialog-form-group">
            <label>Units Preset</label>
            <select
              value={localUnits}
              onChange={(e) => {
                setLocalUnits(e.target.value);
                if (e.target.value === 'metric') {
                  unitPrefs.useMetricUnits();
                } else if (e.target.value === 'imperial') {
                  unitPrefs.useUSUnits();
                }
              }}
            >
              <option value="metric">Metric (°C, kPa)</option>
              <option value="imperial">Imperial (°F, PSI)</option>
              <option value="custom">Custom</option>
            </select>
          </div>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Unit Preferences</h3>

          <div className="dialog-form-group">
            <label>Temperature</label>
            <select
              value={unitPrefs.preferences.temperature}
              onChange={(e) => {
                unitPrefs.updatePreference('temperature', e.target.value as TemperatureUnit);
                setLocalUnits('custom');
              }}
            >
              <option value="C">Celsius (°C)</option>
              <option value="F">Fahrenheit (°F)</option>
              <option value="K">Kelvin (K)</option>
            </select>
          </div>

          <div className="dialog-form-group">
            <label>Pressure</label>
            <select
              value={unitPrefs.preferences.pressure}
              onChange={(e) => {
                unitPrefs.updatePreference('pressure', e.target.value as PressureUnit);
                setLocalUnits('custom');
              }}
            >
              <option value="kPa">Kilopascals (kPa)</option>
              <option value="PSI">PSI</option>
              <option value="bar">Bar</option>
              <option value="inHg">Inches of Mercury (inHg)</option>
            </select>
          </div>

          <div className="dialog-form-group">
            <label>Air-Fuel Ratio</label>
            <select
              value={unitPrefs.preferences.afr}
              onChange={(e) => {
                unitPrefs.updatePreference('afr', e.target.value as AfrUnit);
                setLocalUnits('custom');
              }}
            >
              <option value="AFR">AFR (Air-Fuel Ratio)</option>
              <option value="Lambda">Lambda (λ)</option>
            </select>
          </div>

          <div className="dialog-form-group">
            <label>Speed</label>
            <select
              value={unitPrefs.preferences.speed}
              onChange={(e) => {
                unitPrefs.updatePreference('speed', e.target.value as SpeedUnit);
                setLocalUnits('custom');
              }}
            >
              <option value="km/h">km/h</option>
              <option value="mph">mph</option>
            </select>
          </div>

          <div className="dialog-form-group">
            <label>Fuel Type (for Lambda ↔ AFR)</label>
            <select
              value={unitPrefs.preferences.fuelType}
              onChange={(e) => unitPrefs.updatePreference('fuelType', e.target.value as FuelType)}
            >
              <option value="gasoline">Gasoline (λ=1 @ {STOICH_AFR.gasoline}:1)</option>
              <option value="e85">E85 (λ=1 @ {STOICH_AFR.e85}:1)</option>
              <option value="ethanol">Ethanol (λ=1 @ {STOICH_AFR.ethanol}:1)</option>
              <option value="methanol">Methanol (λ=1 @ {STOICH_AFR.methanol}:1)</option>
              <option value="diesel">Diesel (λ=1 @ {STOICH_AFR.diesel}:1)</option>
            </select>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={autoBurnOnClose}
                onChange={(e) => setAutoBurnOnClose(e.target.checked)}
              />
              Auto-burn on close
            </label>
            <span className="dialog-form-note">Shows confirmation before burning</span>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={demoMode}
                disabled={demoLoading}
                onChange={(e) => handleDemoToggle(e.target.checked)}
              />
              Demo Mode (simulate ECU)
            </label>
            <span className="dialog-form-note">Simulate ECU data for testing (runtime-only)</span>
          </div>

          <div className="dialog-form-group">
            <label>Default Runtime Packet Mode</label>
            <select
              value={runtimePacketMode}
              onChange={(e) => setRuntimePacketMode(e.target.value as any)}
            >
              <option value={'Auto'}>Auto (recommended)</option>
              <option value={'ForceBurst'}>Force Burst</option>
              <option value={'ForceOCH'}>Force OCH</option>
              <option value={'Disabled'}>Disabled (use Burst)</option>
            </select>
            <span className="dialog-form-note">Default runtime packet mode for new connections</span>
            <span className="dialog-form-note">OCH (On-Controller Block Read): use INI-defined block reads when supported by the ECU (configured via <code>ochGetCommand</code> / <code>ochBlockSize</code>).</span>

            {/* Auto-reconnect after controller commands */}
            <div className="dialog-form-group" style={{ marginTop: '0.5rem' }}>
              <label>
                <input
                  type="checkbox"
                  checked={autoReconnectAfterControllerCommand}
                  onChange={(e) => setAutoReconnectAfterControllerCommand(e.target.checked)}
                />
                Auto-sync & reconnect after controller commands
              </label>
              <span className="dialog-form-note">When enabled, the app will automatically sync and reconnect to the ECU after executing controller commands that modify ECU settings (e.g., applying base maps).</span>
            </div>

            {/* Show small live metrics in connection dialog too */}
            <div style={{ marginTop: '0.6rem' }}>
              <ConnectionMetrics compact />
            </div>
          </div>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Status Bar</h3>
          
          <div className="dialog-form-group">
            <label>Channels to Display (max 8)</label>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
              {/* Selected channels with remove buttons */}
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.25rem', minHeight: '2rem', padding: '0.5rem', backgroundColor: 'var(--bg-secondary)', borderRadius: '4px', border: '1px solid var(--border-color)' }}>
                {statusBarChannels.length === 0 ? (
                  <span style={{ color: 'var(--text-muted)', fontSize: '0.85rem' }}>Using default channels from ECU definition</span>
                ) : (
                  statusBarChannels.map((channel, idx) => (
                    <span key={channel} style={{ 
                      display: 'inline-flex', 
                      alignItems: 'center', 
                      gap: '0.25rem',
                      padding: '0.25rem 0.5rem',
                      backgroundColor: 'var(--accent-color)',
                      color: '#fff',
                      borderRadius: '3px',
                      fontSize: '0.85rem'
                    }}>
                      {channel}
                      <button 
                        onClick={() => setStatusBarChannels(prev => prev.filter((_, i) => i !== idx))}
                        style={{ 
                          background: 'none', 
                          border: 'none', 
                          color: '#fff', 
                          cursor: 'pointer',
                          padding: '0 0.25rem',
                          fontSize: '1rem',
                          lineHeight: 1
                        }}
                      >×</button>
                    </span>
                  ))
                )}
              </div>
              {/* Add channel dropdown */}
              <select
                value=""
                onChange={(e) => {
                  if (e.target.value && statusBarChannels.length < 8 && !statusBarChannels.includes(e.target.value)) {
                    setStatusBarChannels(prev => [...prev, e.target.value]);
                  } else if (statusBarChannels.length >= 8) {
                    alert('Maximum 8 channels allowed in status bar');
                  }
                }}
                style={{ padding: '0.5rem' }}
              >
                <option value="">+ Add channel...</option>
                {availableChannels
                  .filter(ch => !statusBarChannels.includes(ch))
                  .sort()
                  .map(ch => (
                    <option key={ch} value={ch}>{ch}</option>
                  ))
                }
              </select>
              {statusBarChannels.length > 0 && (
                <button 
                  onClick={() => setStatusBarChannels([])}
                  style={{ alignSelf: 'flex-start', padding: '0.25rem 0.5rem', fontSize: '0.85rem' }}
                >
                  Reset to Defaults
                </button>
              )}
            </div>
            <span className="dialog-form-note">Select which realtime channels appear in the status bar. Leave empty for auto-detection from ECU definition.</span>
          </div>

          {currentProject && (
            <>
              <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Project Settings</h3>
              
              <div className="dialog-form-group">
                <label>
                  <input
                    type="checkbox"
                    checked={autoConnect}
                    onChange={(e) => setAutoConnect(e.target.checked)}
                  />
                  {' '}Auto-connect to ECU on project open
                </label>
                <span className="dialog-form-note">
                  When enabled, LibreTune will automatically attempt to connect to the last used COM port when opening this project.
                </span>
              </div>
              
              <div className="dialog-form-group">
                <label>ECU Definition (INI File)</label>
                <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
                  <button type="button"
                    title={currentIniPath || 'Not set'}
                    className="ini-select-btn"
                    onClick={handleSwitchIni}
                    style={{ flex: 1, padding: '0.5rem', fontSize: '0.9rem', backgroundColor: 'var(--bg-secondary)', border: '1px solid var(--border-color)', borderRadius: '4px' }}
                  >
                    {currentIniPath ? currentIniPath.split(/[\\\/]/).pop() || currentIniPath : 'Not set'}
                    <span style={{ float: 'right', opacity: 0.85 }}>{switchingIni ? 'Switching...' : 'Change'}</span>
                  </button>

                </div>
                <span className="dialog-form-note">
                  Switch to a different ECU definition file. The project tune will be re-applied automatically.
                </span>
              </div>
            </>
          )}

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Heatmap Colors</h3>
          
          <div className="dialog-form-group">
            <label>Value Tables (VE, Timing)</label>
            <select
              value={heatmapValueScheme}
              onChange={(e) => setHeatmapValueScheme(e.target.value as HeatmapScheme)}
            >
              {availableSchemes.filter(s => s.id !== 'custom').map(scheme => (
                <option key={scheme.id} value={scheme.id}>
                  {scheme.name} {scheme.colorblindSafe && '(colorblind-safe)'}
                </option>
              ))}
            </select>
          </div>

          <div className="dialog-form-group">
            <label>Change Display (AFR Correction)</label>
            <select
              value={heatmapChangeScheme}
              onChange={(e) => setHeatmapChangeScheme(e.target.value as HeatmapScheme)}
            >
              {availableSchemes.filter(s => s.id !== 'custom').map(scheme => (
                <option key={scheme.id} value={scheme.id}>
                  {scheme.name} {scheme.colorblindSafe && '(colorblind-safe)'}
                </option>
              ))}
            </select>
          </div>

          <div className="dialog-form-group">
            <label>Coverage Display (Hit Weighting)</label>
            <select
              value={heatmapCoverageScheme}
              onChange={(e) => setHeatmapCoverageScheme(e.target.value as HeatmapScheme)}
            >
              {availableSchemes.filter(s => s.id !== 'custom').map(scheme => (
                <option key={scheme.id} value={scheme.id}>
                  {scheme.name} {scheme.colorblindSafe && '(colorblind-safe)'}
                </option>
              ))}
            </select>
          </div>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Dashboard</h3>
          
          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={gaugeSnapToGrid}
                onChange={(e) => setGaugeSnapToGrid(e.target.checked)}
              />
              Snap gauges to grid
            </label>
            <span className="dialog-form-note">Align gauges when dragging in designer mode</span>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={gaugeFreeMove}
                onChange={(e) => setGaugeFreeMove(e.target.checked)}
              />
              Free move (ignore snap)
            </label>
            <span className="dialog-form-note">Allow gauges to be placed anywhere</span>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={gaugeLock}
                onChange={(e) => setGaugeLock(e.target.checked)}
              />
              Lock gauge positions
            </label>
            <span className="dialog-form-note">Prevent accidental gauge movement</span>
          </div>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Version Control</h3>
          
          <div className="dialog-form-group">
            <label>Auto-Commit on Save</label>
            <select
              value={autoCommitOnSave}
              onChange={(e) => setAutoCommitOnSave(e.target.value)}
            >
              <option value="never">Never</option>
              <option value="always">Always</option>
              <option value="ask">Ask each time</option>
            </select>
            <span className="dialog-form-note">Automatically create a Git commit when saving the tune</span>
          </div>

          <div className="dialog-form-group">
            <label>Commit Message Format</label>
            <input
              type="text"
              value={commitMessageFormat}
              onChange={(e) => setCommitMessageFormat(e.target.value)}
              style={{ width: '100%', padding: '0.5rem', fontFamily: 'monospace' }}
            />
            <span className="dialog-form-note">Available placeholders: {'{date}'}, {'{time}'}, {'{table}'}</span>
          </div>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>Indicator Panel</h3>
          
          <div className="dialog-form-group">
            <label>Column Count</label>
            <select
              value={indicatorColumnCount}
              onChange={(e) => setIndicatorColumnCount(e.target.value)}
            >
              <option value="auto">Auto (fill width)</option>
              <option value="8">8 columns</option>
              <option value="10">10 columns</option>
              <option value="12">12 columns</option>
              <option value="14">14 columns</option>
              <option value="16">16 columns</option>
            </select>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={indicatorFillEmpty}
                onChange={(e) => setIndicatorFillEmpty(e.target.checked)}
              />
              Fill empty cells in last row
            </label>
            <span className="dialog-form-note">Add blank cells to complete the grid</span>
          </div>

          <div className="dialog-form-group">
            <label>Text Fit Mode</label>
            <select
              value={indicatorTextFit}
              onChange={(e) => setIndicatorTextFit(e.target.value)}
            >
              <option value="scale">Scale to fit</option>
              <option value="wrap">Wrap text (2 lines)</option>
            </select>
          </div>
        </div>
        
        <div className="dialog-footer">
          <button onClick={onClose}>Cancel</button>
          <button onClick={handleApply} className="dialog-primary">Apply</button>
        </div>
      </div>
    </div>
  );
}

// =============================================================================
// About Dialog
// =============================================================================

export function AboutDialog({ isOpen, onClose }: DialogProps) {
  if (!isOpen) return null;

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>About LibreTune</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>
        
        <div className="dialog-content dialog-about">
          <div className="dialog-about-logo">🔧</div>
          <h3>LibreTune</h3>
          <p className="dialog-version">Version 0.1.0</p>
          
          <p>Open-source ECU tuning software compatible with standard INI definition files.</p>
          
          <div className="dialog-about-links">
            <a 
              href="#" 
              onClick={(e) => { e.preventDefault(); openUrl('https://github.com/RallyPat/LibreTune'); }}
            >
              GitHub
            </a>
            <a 
              href="#" 
              onClick={(e) => { e.preventDefault(); openUrl('https://github.com/RallyPat/LibreTune/tree/main/docs'); }}
            >
              Documentation
            </a>
          </div>
          
          <p className="dialog-license">
            Licensed under GPL-2.0
          </p>
        </div>
        
        <div className="dialog-footer">
          <button onClick={onClose} className="dialog-primary">Close</button>
        </div>
      </div>
    </div>
  );
}

// =============================================================================
// Connection Dialog
// =============================================================================

interface ConnectionDialogProps extends DialogProps {
  ports: string[];
  selectedPort: string;
  baudRate: number;
  timeoutMs: number;
  connected: boolean;
  connecting: boolean;
  onPortChange: (port: string) => void;
  onBaudChange: (baud: number) => void;
  onTimeoutChange: (timeoutMs: number) => void;
  onConnect: () => void;
  onDisconnect: () => void;
  onRefreshPorts: () => void;
  statusMessage?: string;
  iniDefaults?: {
    default_baud_rate: number;
    timeout_ms: number;
    inter_write_delay: number;
    delay_after_port_open: number;
    message_envelope_format?: string | null;
    page_activation_delay: number;
  };
  onApplyIniDefaults?: () => void;
  runtimePacketMode?: 'Auto'|'ForceBurst'|'ForceOCH'|'Disabled';
  onRuntimePacketModeChange?: (mode: 'Auto'|'ForceBurst'|'ForceOCH'|'Disabled') => void;
}

export function ConnectionDialog({ 
  isOpen, 
  onClose,
  ports,
  selectedPort,
  baudRate,
  timeoutMs,
  connected,
  connecting,
  onPortChange,
  onBaudChange,
  onTimeoutChange,
  onConnect,
  onDisconnect,
  onRefreshPorts,
  statusMessage,
  iniDefaults,
  onApplyIniDefaults,
  runtimePacketMode,
  onRuntimePacketModeChange,
}: ConnectionDialogProps) {
  if (!isOpen) return null;

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>ECU Connection</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>
        
        <div className="dialog-content">
          <div className="dialog-form-group">
            <label>Serial Port</label>
            <div className="dialog-port-row">
              <select 
                value={selectedPort} 
                onChange={(e) => onPortChange(e.target.value)}
                disabled={connected}
              >
                {ports.length === 0 ? (
                  <option value="">No ports found</option>
                ) : (
                  ports.map((port) => (
                    <option key={port} value={port}>{port}</option>
                  ))
                )}
              </select>
              <button onClick={onRefreshPorts} disabled={connected}>
                🔄 Refresh
              </button>
            </div>
          </div>
          
          <div className="dialog-form-group">
            <label>Baud Rate</label>
            <select 
              value={baudRate} 
              onChange={(e) => onBaudChange(Number(e.target.value))}
              disabled={connected}
            >
              <option value={115200}>115200</option>
              <option value={57600}>57600</option>
              <option value={38400}>38400</option>
              <option value={19200}>19200</option>
              <option value={9600}>9600</option>
            </select>
          </div>

          <div className="dialog-form-group">
            <label>Timeout</label>
            <select
              value={timeoutMs}
              onChange={(e) => onTimeoutChange(Number(e.target.value))}
              disabled={connected}
            >
              <option value={1000}>1000 ms</option>
              <option value={2000}>2000 ms</option>
              <option value={3000}>3000 ms</option>
              <option value={5000}>5000 ms</option>
            </select>
          </div>

          <div className="dialog-form-group">
            <label>Runtime Packet Mode</label>
            <select
              value={runtimePacketMode}
              onChange={(e) => onRuntimePacketModeChange && onRuntimePacketModeChange(e.target.value as any)}
              disabled={connected}
            >
              <option value={'Auto'}>Auto (recommended)</option>
              <option value={'ForceBurst'}>Force Burst</option>
              <option value={'ForceOCH'}>Force OCH</option>
              <option value={'Disabled'}>Disabled (use Burst)</option>
            </select>
            <div className="field-help">Per-connection override for runtime packet selection</div>
            <div className="field-help">OCH (On-Controller Block Read): use INI-defined block reads when supported by the ECU (configured via <code>ochGetCommand</code> / <code>ochBlockSize</code>).</div>
          </div>



          {/* INI defaults display */}
          {iniDefaults && (
            <div className="dialog-form-group ini-defaults">
              <label>INI Defaults</label>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                <div><strong>Baud:</strong> {iniDefaults.default_baud_rate}</div>
                <div><strong>Timeout:</strong> {iniDefaults.timeout_ms} ms</div>
                <div><strong>interWriteDelay:</strong> {iniDefaults.inter_write_delay} ms</div>
                <div><strong>Delay after port open:</strong> {iniDefaults.delay_after_port_open} ms</div>
                <button className="primary-btn" style={{ marginTop: '8px' }} onClick={() => onApplyIniDefaults && onApplyIniDefaults()} disabled={connected === true ? false : false}>
                  Apply INI defaults
                </button>
              </div>
            </div>
          )} 
          
          <div className="dialog-status">
            <span className={`status-indicator ${connected ? 'connected' : 'disconnected'}`} />
            {statusMessage ? statusMessage : (connected ? 'Connected' : connecting ? 'Connecting...' : 'Disconnected')}
          </div>
        </div>
        
        <div className="dialog-footer">
          <button onClick={onClose}>Close</button>
          {connected ? (
            <button onClick={onDisconnect} className="dialog-danger">
              Disconnect
            </button>
          ) : (
            <button 
              onClick={onConnect} 
              disabled={connecting || !selectedPort}
              className="dialog-primary"
            >
              {connecting ? 'Connecting...' : 'Connect'}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
