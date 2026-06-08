import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { HeatmapScheme, getAvailableSchemes } from '../../utils/heatmapColors';
import { useUnitPreferences } from '../../utils/useUnitPreferences';
import { TemperatureUnit, PressureUnit, AfrUnit, SpeedUnit, FuelType, STOICH_AFR } from '../../utils/unitConversions';
import './Dialogs.css';

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
        title: '另存为调校文件',
        filters: [
          { name: 'MSQ 调校文件', extensions: ['msq'] },
          { name: 'JSON 调校文件', extensions: ['json'] },
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
            <h2>保存调校</h2>
            <button className="dialog-close" onClick={onClose}>×</button>
          </div>

          <div className="dialog-content">
            {error && <div className="dialog-error">{error}</div>}

            <div className="dialog-info">
              <p><strong>ECU:</strong> {tuneInfo?.signature || '未知'}</p>
              {tuneInfo?.path && (
                <p><strong>当前文件：</strong> {tuneInfo.path.split('/').pop()}</p>
              )}
              {tuneInfo?.modified && (
                <p className="dialog-warning">⚠ 调校有未保存的更改</p>
              )}
            </div>

            <div className="dialog-help">
              <p>将调校保存到文件中，用于备份或传输。</p>
              <p><strong>MSQ 格式</strong>兼容其他 ECU 调校软件。</p>
            </div>
          </div>

          <div className="dialog-footer">
            <button onClick={onClose} disabled={isSaving}>取消</button>
            <button
              onClick={handleSaveAs}
              disabled={isSaving}
            >
              另存为...
            </button>
            <button
              onClick={handleSave}
              disabled={isSaving || !tuneInfo?.path}
              className="dialog-primary"
            >
              {isSaving ? '保存中...' : '保存'}
            </button>
          </div>
        </div>
      </div>

      {/* Auto-burn confirmation dialog */}
      {showBurnConfirm && (
        <div className="dialog-overlay" onClick={(e) => e.stopPropagation()}>
          <div className="dialog" onClick={(e) => e.stopPropagation()}>
            <div className="dialog-header">
              <h2>将调校烧录到 ECU？</h2>
            </div>
            <div className="dialog-content">
              <p>调校保存成功。是否要将其烧录到 ECU？</p>
              <p className="dialog-warning">⚠ 这将会写入 ECU 内存，可能需要几秒钟。</p>
            </div>
            <div className="dialog-footer">
              <button onClick={handleBurnCancel}>取消</button>
              <button onClick={handleBurnConfirm} className="dialog-primary">烧录到 ECU</button>
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
        title: '打开调校文件',
        multiple: false,
        filters: [
          { name: '调校文件', extensions: ['msq', 'json'] },
          { name: 'MSQ 调校文件', extensions: ['msq'] },
          { name: 'JSON 调校文件', extensions: ['json'] },
          { name: '所有文件', extensions: ['*'] },
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
          <h2>加载调校</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>
        
        <div className="dialog-content">
          {error && <div className="dialog-error">{error}</div>}
          
          <div className="dialog-file-list">
            <div className="dialog-file-header">
              <span>最近调校文件</span>
              <button onClick={handleBrowse}>浏览...</button>
            </div>
            
            {tuneFiles.length === 0 ? (
              <div className="dialog-empty">项目文件夹中未找到调校文件</div>
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
          <button onClick={onClose} disabled={isLoading}>取消</button>
          <button 
            onClick={() => selectedFile && handleLoad(selectedFile)}
            disabled={isLoading || !selectedFile}
            className="dialog-primary"
          >
            {isLoading ? '加载中...' : '加载'}
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
          <h2>烧录到 ECU</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>
        
        <div className="dialog-content">
          {error && <div className="dialog-error">{error}</div>}
          {success && <div className="dialog-success">✓ 烧录完成！</div>}
          
          {!connected ? (
            <div className="dialog-warning">
              ⚠ 未连接到 ECU，请先连接。
            </div>
          ) : (
            <div className="dialog-info">
              <p>这将把 ECU RAM 中的所有更改写入闪存。</p>
              <p><strong>警告：</strong>此操作不可撤销。</p>
              <p>请确保调校已在烧录前经过测试。</p>
            </div>
          )}
        </div>
        
        <div className="dialog-footer">
          <button onClick={onClose} disabled={isBurning}>取消</button>
          <button 
            onClick={handleBurn}
            disabled={isBurning || !connected || success}
            className="dialog-primary dialog-burn"
          >
            {isBurning ? '烧录中...' : success ? '完成！' : '🔥 烧录到 ECU'}
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
          <h2>新建调校</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>
        
        <div className="dialog-content">
          {error && <div className="dialog-error">{error}</div>}
          
          <div className="dialog-info">
            <p>为当前加载的 ECU 定义创建新的调校文件。</p>
            <p>当前调校中任何未保存的更改都将丢失。</p>
          </div>
        </div>
        
        <div className="dialog-footer">
          <button onClick={onClose} disabled={isCreating}>取消</button>
          <button 
            onClick={handleCreate}
            disabled={isCreating}
            className="dialog-primary"
          >
            {isCreating ? '创建中...' : '新建调校'}
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
  };
}

interface SettingsDialogProps extends DialogProps {
  theme: string;
  onThemeChange: (theme: string) => void;
  onSettingsChange?: (settings: { units?: string; autoBurnOnClose?: boolean; demoMode?: boolean; indicatorColumnCount?: string; indicatorFillEmpty?: boolean; indicatorTextFit?: string; statusBarChannels?: string[] }) => void;
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
  const [commitMessageFormat, setCommitMessageFormat] = useState('调校保存于 {date} {time}');
  
  // Unit preferences from context
  const unitPrefs = useUnitPreferences();
  
  // Available heatmap schemes
  const availableSchemes = getAvailableSchemes();

  useEffect(() => {
    setLocalTheme(theme);
    // Load settings from backend
    if (isOpen) {
      invoke('get_settings').then((settings: any) => {
        setLocalUnits(settings.units_system || 'metric');
        setAutoBurnOnClose(settings.auto_burn_on_close || false);
        setIndicatorColumnCount(settings.indicator_column_count || 'auto');
        setIndicatorFillEmpty(settings.indicator_fill_empty || false);
        setIndicatorTextFit(settings.indicator_text_fit || 'scale');
        setCurrentIniPath(settings.last_ini_path || null);
        // Status bar channels
        setStatusBarChannels(settings.status_bar_channels || []);
        // Heatmap settings
        setHeatmapValueScheme(settings.heatmap_value_scheme || 'tunerstudio');
        setHeatmapChangeScheme(settings.heatmap_change_scheme || 'tunerstudio');
        setHeatmapCoverageScheme(settings.heatmap_coverage_scheme || 'tunerstudio');
        // Gauge settings
        setGaugeSnapToGrid(settings.gauge_snap_to_grid ?? true);
        setGaugeFreeMove(settings.gauge_free_move ?? false);
        setGaugeLock(settings.gauge_lock ?? false);
        // Version control settings
        setAutoCommitOnSave(settings.auto_commit_on_save ?? 'never');
        setCommitMessageFormat(settings.commit_message_format ?? '调校保存于 {date} {time}');
      }).catch(console.error);

      // Load available output channels from ECU definition
      invoke<string[]>('get_available_channels').then((channels) => {
        setAvailableChannels(channels || []);
      }).catch(() => setAvailableChannels([]));

      // Load demo mode state (runtime flag)
      invoke<boolean>('get_demo_mode').then(setDemoMode).catch(console.error);
    }
  }, [theme, isOpen]);

  const handleDemoToggle = useCallback(async (enabled: boolean) => {
    setDemoLoading(true);
    try {
      await invoke('set_demo_mode', { enabled });
      setDemoMode(enabled);
      onSettingsChange?.({ demoMode: enabled });
    } catch (e) {
      console.error('Failed to toggle demo mode:', e);
      alert('切换演示模式失败：' + String(e));
    } finally {
      setDemoLoading(false);
    }
  }, [onSettingsChange]);

  const handleSwitchIni = useCallback(async () => {
    if (!currentProject) {
      alert('当前没有打开的项目');
      return;
    }

    setSwitchingIni(true);
    try {
      const selected = await open({
        title: '选择 ECU 定义文件',
        multiple: false,
        filters: [
          { name: 'INI 文件', extensions: ['ini'] },
          { name: '所有文件', extensions: ['*'] },
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
        const message = 'ECU 定义更新成功！\n\n' +
          '项目调校已使用新定义重新应用。' +
          '如果表格显示为空，您可能需要加载与此 INI 定义' +
          '匹配的 MSQ 文件。';
        alert(message);
      }
    } catch (e) {
      console.error('Failed to switch INI:', e);
      alert('切换 INI 文件失败：' + String(e));
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
    onSettingsChange?.({ units: localUnits, autoBurnOnClose, indicatorColumnCount, indicatorFillEmpty, indicatorTextFit, statusBarChannels });
    onClose();
  }, [localTheme, localUnits, autoBurnOnClose, statusBarChannels, indicatorColumnCount, indicatorFillEmpty, indicatorTextFit, heatmapValueScheme, heatmapChangeScheme, heatmapCoverageScheme, gaugeSnapToGrid, gaugeFreeMove, gaugeLock, autoCommitOnSave, commitMessageFormat, onThemeChange, onSettingsChange, onClose]);

  if (!isOpen) return null;

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>设置</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>
        
        <div className="dialog-content">
          <div className="dialog-form-group">
            <label>主题</label>
            <select 
              value={localTheme} 
              onChange={(e) => setLocalTheme(e.target.value)}
            >
              <option value="dark">深色</option>
              <option value="light">浅色</option>
              <option value="midnight">午夜</option>
              <option value="carbon">碳黑</option>
            </select>
          </div>
          
          <div className="dialog-form-group">
            <label>单位预设</label>
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
              <option value="metric">公制 (°C, kPa)</option>
              <option value="imperial">英制 (°F, PSI)</option>
              <option value="custom">自定义</option>
            </select>
          </div>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>单位偏好</h3>

          <div className="dialog-form-group">
            <label>温度</label>
            <select
              value={unitPrefs.preferences.temperature}
              onChange={(e) => {
                unitPrefs.updatePreference('temperature', e.target.value as TemperatureUnit);
                setLocalUnits('custom');
              }}
            >
              <option value="C">摄氏度 (°C)</option>
              <option value="F">华氏度 (°F)</option>
              <option value="K">开尔文 (K)</option>
            </select>
          </div>

          <div className="dialog-form-group">
            <label>压力</label>
            <select
              value={unitPrefs.preferences.pressure}
              onChange={(e) => {
                unitPrefs.updatePreference('pressure', e.target.value as PressureUnit);
                setLocalUnits('custom');
              }}
            >
              <option value="kPa">千帕 (kPa)</option>
              <option value="PSI">PSI</option>
              <option value="bar">巴</option>
              <option value="inHg">英寸汞柱 (inHg)</option>
            </select>
          </div>

          <div className="dialog-form-group">
            <label>空燃比</label>
            <select
              value={unitPrefs.preferences.afr}
              onChange={(e) => {
                unitPrefs.updatePreference('afr', e.target.value as AfrUnit);
                setLocalUnits('custom');
              }}
            >
              <option value="AFR">AFR (空燃比)</option>
              <option value="Lambda">Lambda (λ)</option>
            </select>
          </div>

          <div className="dialog-form-group">
            <label>速度</label>
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
            <label>燃油类型 (Lambda ↔ AFR 转换)</label>
            <select
              value={unitPrefs.preferences.fuelType}
              onChange={(e) => unitPrefs.updatePreference('fuelType', e.target.value as FuelType)}
            >
              <option value="gasoline">汽油 (λ=1 @ {STOICH_AFR.gasoline}:1)</option>
              <option value="e85">E85 (λ=1 @ {STOICH_AFR.e85}:1)</option>
              <option value="ethanol">乙醇 (λ=1 @ {STOICH_AFR.ethanol}:1)</option>
              <option value="methanol">甲醇 (λ=1 @ {STOICH_AFR.methanol}:1)</option>
              <option value="diesel">柴油 (λ=1 @ {STOICH_AFR.diesel}:1)</option>
            </select>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={autoBurnOnClose}
                onChange={(e) => setAutoBurnOnClose(e.target.checked)}
              />
              关闭时自动烧录
            </label>
            <span className="dialog-form-note">烧录前显示确认提示</span>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={demoMode}
                disabled={demoLoading}
                onChange={(e) => handleDemoToggle(e.target.checked)}
              />
              演示模式 (模拟 ECU)
            </label>
            <span className="dialog-form-note">模拟 ECU 数据用于测试 (仅运行时)</span>
          </div>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>状态栏</h3>
          
          <div className="dialog-form-group">
            <label>显示通道 (最多 8 个)</label>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
              {/* Selected channels with remove buttons */}
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.25rem', minHeight: '2rem', padding: '0.5rem', backgroundColor: 'var(--bg-secondary)', borderRadius: '4px', border: '1px solid var(--border-color)' }}>
                {statusBarChannels.length === 0 ? (
                  <span style={{ color: 'var(--text-muted)', fontSize: '0.85rem' }}>使用 ECU 定义的默认通道</span>
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
                    alert('状态栏最多允许 8 个通道');
                  }
                }}
                style={{ padding: '0.5rem' }}
              >
                <option value="">+ 添加通道...</option>
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
                  重置为默认值
                </button>
              )}
            </div>
            <span className="dialog-form-note">选择要在状态栏中显示的实时通道。留空则自动从 ECU 定义中检测。</span>
          </div>

          {currentProject && (
            <>
              <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>项目设置</h3>
              
              <div className="dialog-form-group">
                <label>ECU 定义 (INI 文件)</label>
                <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
                  <input
                    type="text"
                    value={currentIniPath ? currentIniPath.split(/[\\/]/).pop() || currentIniPath : '未设置'}
                    readOnly
                    style={{ flex: 1, padding: '0.5rem', fontSize: '0.9rem', backgroundColor: 'var(--bg-secondary)', border: '1px solid var(--border-color)', borderRadius: '4px' }}
                  />
                  <button
                    onClick={handleSwitchIni}
                    disabled={switchingIni}
                    style={{ padding: '0.5rem 1rem', fontSize: '0.9rem', whiteSpace: 'nowrap' }}
                  >
                    {switchingIni ? '切换中...' : '切换 INI...'}
                  </button>
                </div>
                <span className="dialog-form-note">
                  切换到不同的 ECU 定义文件。项目调校将自动重新应用。
                </span>
              </div>
            </>
          )}

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>热图颜色</h3>
          
          <div className="dialog-form-group">
            <label>数值表 (VE, 点火正时)</label>
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
            <label>变化显示 (AFR 修正)</label>
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
            <label>覆盖率显示 (命中加权)</label>
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

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>仪表盘</h3>
          
          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={gaugeSnapToGrid}
                onChange={(e) => setGaugeSnapToGrid(e.target.checked)}
              />
              仪表吸附到网格
            </label>
            <span className="dialog-form-note">在设计模式下拖动仪表时对齐</span>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={gaugeFreeMove}
                onChange={(e) => setGaugeFreeMove(e.target.checked)}
              />
              自由移动 (忽略吸附)
            </label>
            <span className="dialog-form-note">允许仪表放置在任意位置</span>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={gaugeLock}
                onChange={(e) => setGaugeLock(e.target.checked)}
              />
              锁定仪表位置
            </label>
            <span className="dialog-form-note">防止意外移动仪表</span>
          </div>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>版本控制</h3>
          
          <div className="dialog-form-group">
            <label>保存时自动提交</label>
            <select
              value={autoCommitOnSave}
              onChange={(e) => setAutoCommitOnSave(e.target.value)}
            >
              <option value="never">从不</option>
              <option value="always">总是</option>
              <option value="ask">每次询问</option>
            </select>
            <span className="dialog-form-note">保存调校时自动创建 Git 提交</span>
          </div>

          <div className="dialog-form-group">
            <label>提交消息格式</label>
            <input
              type="text"
              value={commitMessageFormat}
              onChange={(e) => setCommitMessageFormat(e.target.value)}
              style={{ width: '100%', padding: '0.5rem', fontFamily: 'monospace' }}
            />
            <span className="dialog-form-note">可用占位符：{' {date}'}、{' {time}'}、{' {table}'}</span>
          </div>

          <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>指示器面板</h3>
          
          <div className="dialog-form-group">
            <label>列数</label>
            <select
              value={indicatorColumnCount}
              onChange={(e) => setIndicatorColumnCount(e.target.value)}
            >
              <option value="auto">自动 (填充宽度)</option>
              <option value="8">8 列</option>
              <option value="10">10 列</option>
              <option value="12">12 列</option>
              <option value="14">14 列</option>
              <option value="16">16 列</option>
            </select>
          </div>

          <div className="dialog-form-group">
            <label>
              <input
                type="checkbox"
                checked={indicatorFillEmpty}
                onChange={(e) => setIndicatorFillEmpty(e.target.checked)}
              />
              填充最后一行的空白单元格
            </label>
            <span className="dialog-form-note">添加空白单元格以完成网格</span>
          </div>

          <div className="dialog-form-group">
            <label>文本适配模式</label>
            <select
              value={indicatorTextFit}
              onChange={(e) => setIndicatorTextFit(e.target.value)}
            >
              <option value="scale">缩放适配</option>
              <option value="wrap">换行 (2 行)</option>
            </select>
          </div>
        </div>
        
        <div className="dialog-footer">
          <button onClick={onClose}>取消</button>
          <button onClick={handleApply} className="dialog-primary">应用</button>
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
          <h2>关于 FCoreTuner</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>
        
        <div className="dialog-content dialog-about">
          <div className="dialog-about-logo">🔧</div>
          <h3>FCoreTuner</h3>
          <p className="dialog-version">版本 0.1.0</p>
          
          <p>开源的 ECU 调校软件，兼容标准 INI 定义文件。</p>
          
          <div className="dialog-about-links">
            <a 
              href="#" 
              onClick={(e) => { e.preventDefault(); openUrl('https://github.com/RallyPat/FCoreTuner'); }}
            >
              GitHub
            </a>
            <a 
              href="#" 
              onClick={(e) => { e.preventDefault(); openUrl('https://github.com/RallyPat/FCoreTuner/tree/main/docs'); }}
            >
              文档
            </a>
          </div>
          
          <p className="dialog-license">
            基于 GPL-2.0 许可证
          </p>
        </div>
        
        <div className="dialog-footer">
          <button onClick={onClose} className="dialog-primary">关闭</button>
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
}: ConnectionDialogProps) {
  if (!isOpen) return null;

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>ECU 连接</h2>
          <button className="dialog-close" onClick={onClose}>×</button>
        </div>
        
        <div className="dialog-content">
          <div className="dialog-form-group">
            <label>串口</label>
            <div className="dialog-port-row">
              <select 
                value={selectedPort} 
                onChange={(e) => onPortChange(e.target.value)}
                disabled={connected}
              >
                {ports.length === 0 ? (
                  <option value="">未找到端口</option>
                ) : (
                  ports.map((port) => (
                    <option key={port} value={port}>{port}</option>
                  ))
                )}
              </select>
              <button onClick={onRefreshPorts} disabled={connected}>
                🔄 刷新
              </button>
            </div>
          </div>
          
          <div className="dialog-form-group">
            <label>波特率</label>
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
            <label>超时</label>
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

          {/* INI defaults display */}
          {iniDefaults && (
            <div className="dialog-form-group ini-defaults">
              <label>INI 默认值</label>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                <div><strong>波特率：</strong> {iniDefaults.default_baud_rate}</div>
                <div><strong>超时：</strong> {iniDefaults.timeout_ms} ms</div>
                <div><strong>interWriteDelay：</strong> {iniDefaults.inter_write_delay} ms</div>
                <div><strong>端口打开后延迟：</strong> {iniDefaults.delay_after_port_open} ms</div>
                <button className="primary-btn" style={{ marginTop: '8px' }} onClick={() => onApplyIniDefaults && onApplyIniDefaults()} disabled={connected === true ? false : false}>
                  应用 INI 默认值
                </button>
              </div>
            </div>
          )} 
          
          <div className="dialog-status">
            <span className={`status-indicator ${connected ? 'connected' : 'disconnected'}`} />
            {statusMessage ? statusMessage : (connected ? '已连接' : connecting ? '连接中...' : '未连接')}
          </div>
        </div>
        
        <div className="dialog-footer">
          <button onClick={onClose}>关闭</button>
          {connected ? (
            <button onClick={onDisconnect} className="dialog-danger">
              断开连接
            </button>
          ) : (
            <button 
              onClick={onConnect} 
              disabled={connecting || !selectedPort}
              className="dialog-primary"
            >
              {connecting ? '连接中...' : '连接'}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
