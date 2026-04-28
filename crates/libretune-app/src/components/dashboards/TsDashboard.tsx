import {
  DashFile,
  DashFileInfo,
  TsGaugeConfig,
  isGauge,
  isIndicator,
  buildEmbeddedImageMap,
  tsColorToRgba,
} from './dashTypes';

import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { Plus, Copy, Pencil, Trash2, Save, RotateCw, AlertTriangle, Compass, X, FolderOpen } from 'lucide-react';
import { useRealtimeStore } from '../../stores/realtimeStore';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { save } from '@tauri-apps/plugin-dialog';
import TsGauge from '../gauges/TsGauge';
import GaugeContextMenu, { ContextMenuState } from './GaugeContextMenu';
import ImportDashboardDialog from '../dialogs/ImportDashboardDialog';
import DashboardDesigner from './DashboardDesigner';
import { Dialog, Button } from '../common';
import LiveTsIndicator from './components/LiveTsIndicator';
import { buildDefaultGauge } from './utils/defaultGauge';
import {
  formatValidationIssue,
} from './utils/validation';
import {
  computeCompatibilityReport,
  hasCompatibilityIssues as hasCompatIssues,
} from './utils/compatibility';
import { computeDashboardBounds } from './utils/dashboardBounds';
import { useGaugeSweep } from './hooks/useGaugeSweep';
import { useGaugeDemo } from './hooks/useGaugeDemo';
import { useDashboardScale } from './hooks/useDashboardScale';
import { useDashboardValidation } from './hooks/useDashboardValidation';
import './TsDashboard.css';

/**
 * Props for the TsDashboard component.
 */
interface TsDashboardProps {
  /** Path to initially load (optional, uses last dashboard or default) */
  initialDashPath?: string;
  /** Whether ECU is connected (enables data display) */
  isConnected?: boolean;
}

interface ChannelInfo {
  name: string;
  label?: string | null;
  units: string;
  scale: number;
  translate: number;
}

interface GaugeInfo {
  name: string;
  channel: string;
  title: string;
  units: string;
  lo: number;
  hi: number;
  low_warning: number;
  high_warning: number;
  low_danger: number;
  high_danger: number;
  digits: number;
}

export default function TsDashboard({ initialDashPath, isConnected = false }: TsDashboardProps) {
  const [dashFile, setDashFile] = useState<DashFile | null>(null);
  const [availableDashes, setAvailableDashes] = useState<DashFileInfo[]>([]);
  const [selectedPath, setSelectedPath] = useState<string>(initialDashPath || '');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showSelector, setShowSelector] = useState(false);
  const [channelInfoMap, setChannelInfoMap] = useState<Record<string, ChannelInfo>>({});
  
  // Gauge sweep animation (sportscar-style min→max→min on load)
  const { sweepActive, sweepValues, startGaugeSweep } = useGaugeSweep();

  // Context menu state
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    targetGaugeId: null,
  });

  // Dashboard settings
  const [designerMode, setDesignerMode] = useState(false);
  const [gaugeDemoActive, setGaugeDemoActive] = useState(false);
  const demoValues = useGaugeDemo(gaugeDemoActive, dashFile);
  
  // Designer mode state
  const [selectedGaugeId, setSelectedGaugeId] = useState<string | null>(null);
  const [gridSnap, setGridSnap] = useState(5); // 5% snap
  const [showGrid, setShowGrid] = useState(true);
  
  // Import dialog state
  const [showImportDialog, setShowImportDialog] = useState(false);
  
  // Dashboard management dialogs
  const [showNewDialog, setShowNewDialog] = useState(false);
  const [showRenameDialog, setShowRenameDialog] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [newDashName, setNewDashName] = useState('');
  const [renameName, setRenameName] = useState('');
  const [legacyMode, setLegacyMode] = useState(false);
  const [compatBarVisible, setCompatBarVisible] = useState(true);
  const [syncToken, setSyncToken] = useState(0);
  const [autoSyncGaugeRanges, setAutoSyncGaugeRanges] = useState(true);
  const [showValidationPanel, setShowValidationPanel] = useState(false);
  const initialSyncDoneRef = useRef(false);

  // Build embedded images map — memoized so TsGauge's React.memo and
  // animation effect don't re-run on every TsDashboard render.
  const embeddedImages = useMemo(
    () => dashFile
      ? buildEmbeddedImageMap(dashFile.gauge_cluster.embedded_images)
      : new Map<string, string>(),
    [dashFile]
  );

  // NOTE: TsDashboard no longer subscribes to realtime channel data.
  // Each TsGauge subscribes to its own channel directly via the Zustand store,
  // and indicators use the LiveTsIndicator wrapper below.
  // This eliminates the 20Hz re-render cascade that was freezing the UI.
  useEffect(() => {
    const loadChannels = async () => {
      try {
        const channels = await invoke<ChannelInfo[]>('get_available_channels');
        const map: Record<string, ChannelInfo> = {};
        channels.forEach((ch) => {
          map[ch.name] = ch;
        });
        setChannelInfoMap(map);
      } catch (e) {
        console.warn('[TsDashboard] Failed to load available channels:', e);
        setChannelInfoMap({});
      }
    };
    loadChannels();
  }, []);

  // Calculate dashboard aspect ratio from gauge bounding box.
  // Must be before any early returns to comply with React Rules of Hooks.
  const dashboardBounds = useMemo(
    () => computeDashboardBounds(dashFile),
    [dashFile],
  );

  const isLegacyPath = useMemo(() => {
    const lower = (selectedPath ?? '').toLowerCase();
    return lower.endsWith('.dash') || lower.endsWith('.gauge');
  }, [selectedPath]);

  const compatibilityReport = useMemo(
    () => (dashFile ? computeCompatibilityReport(dashFile) : null),
    [dashFile],
  );

  const hasCompatibilityIssues = useMemo(
    () => hasCompatIssues(compatibilityReport),
    [compatibilityReport],
  );

  // Dynamic scaling: shrink the dashboard when the viewport is too small.
  const { scale, wrapperRef: dashboardWrapperRef, recompute: computeScale } =
    useDashboardScale(dashboardBounds.aspectRatio);

  // Validation: re-runs whenever the dash file changes.
  const validationReport = useDashboardValidation(dashFile);



  // Handle right-click context menu
  const handleContextMenu = useCallback((e: React.MouseEvent, gaugeId: string | null) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({
      visible: true,
      x: e.clientX,
      y: e.clientY,
      targetGaugeId: gaugeId,
    });
  }, []);

  // Close context menu
  const closeContextMenu = useCallback(() => {
    setContextMenu(prev => ({ ...prev, visible: false }));
  }, []);

  // Reload default gauges
  const handleReloadDefaultGauges = useCallback(async () => {
    // Reload the current dashboard from file
    if (selectedPath) {
      try {
        const file = await invoke<DashFile>('get_dash_file', { path: selectedPath });
        setDashFile(file);
      } catch (e) {
        console.error('Failed to reload dashboard:', e);
      }
    }
  }, [selectedPath]);

  // Save dashboard to file
  const handleSaveDashboard = useCallback(async () => {
    if (!dashFile || !selectedPath) return;
    
    try {
      await invoke('save_dash_file', { 
        path: selectedPath,
        dashFile: dashFile,
      });
      console.log('Dashboard saved successfully');
    } catch (e) {
      console.error('Failed to save dashboard:', e);
    }
  }, [dashFile, selectedPath]);

  // Sync gauge ranges from INI GaugeConfigurations
  const handleSyncGaugeRanges = useCallback(async () => {
    if (!dashFile) return;

    try {
      const gauges = await invoke<GaugeInfo[]>('get_gauge_configs');
      const byChannel = new Map(gauges.map(g => [g.channel.toLowerCase(), g]));
      const byName = new Map(gauges.map(g => [g.name.toLowerCase(), g]));

      const updatedComponents = dashFile.gauge_cluster.components.map((comp) => {
        if (!isGauge(comp)) return comp;

        const gauge = comp.Gauge;
        const channelKey = (gauge.output_channel || '').toLowerCase();
        const nameKey = (gauge.title || '').toLowerCase();
        const info = byChannel.get(channelKey) || byName.get(nameKey);
        if (!info) return comp;

        return {
          Gauge: {
            ...gauge,
            min: info.lo,
            max: info.hi,
            units: info.units,
            low_warning: Number.isFinite(info.low_warning) ? info.low_warning : gauge.low_warning,
            high_warning: Number.isFinite(info.high_warning) ? info.high_warning : gauge.high_warning,
            low_critical: Number.isFinite(info.low_danger) ? info.low_danger : gauge.low_critical,
            high_critical: Number.isFinite(info.high_danger) ? info.high_danger : gauge.high_critical,
            value_digits: Number.isFinite(info.digits) ? info.digits : gauge.value_digits,
          },
        };
      });

      setDashFile({
        ...dashFile,
        gauge_cluster: { ...dashFile.gauge_cluster, components: updatedComponents },
      });
    } catch (e) {
      console.warn('Failed to sync gauge ranges from INI:', e);
    }
  }, [dashFile]);

  // Auto-sync once on initial dashboard load
  useEffect(() => {
    if (!dashFile) return;
    if (!autoSyncGaugeRanges) return;
    if (initialSyncDoneRef.current) return;
    initialSyncDoneRef.current = true;
    handleSyncGaugeRanges();
  }, [dashFile, handleSyncGaugeRanges, autoSyncGaugeRanges]);

  // Auto-sync on INI/definition changes
  useEffect(() => {
    if (!dashFile) return;
    if (!autoSyncGaugeRanges) return;
    if (syncToken === 0) return;
    handleSyncGaugeRanges();
  }, [syncToken, dashFile, handleSyncGaugeRanges, autoSyncGaugeRanges]);

  // Load auto-sync preference
  useEffect(() => {
    invoke<any>('get_settings')
      .then((settings) => {
        if (settings.auto_sync_gauge_ranges !== undefined) {
          setAutoSyncGaugeRanges(!!settings.auto_sync_gauge_ranges);
        }
      })
      .catch((e) => console.warn('[TsDashboard] get_settings failed:', e));
  }, []);

  useEffect(() => {
    let unlistenIni: UnlistenFn | null = null;
    let unlistenDefLoaded: UnlistenFn | null = null;
    let unlistenDefChanged: UnlistenFn | null = null;
    let unlistenSettings: UnlistenFn | null = null;

    (async () => {
      try {
        unlistenIni = await listen('ini:changed', () => {
          setSyncToken((v) => v + 1);
        });
      } catch (e) {
        console.warn('[TsDashboard] Failed to listen for ini:changed:', e);
      }

      try {
        unlistenDefLoaded = await listen('definition:loaded', () => {
          setSyncToken((v) => v + 1);
        });
      } catch (e) {
        console.warn('[TsDashboard] Failed to listen for definition:loaded:', e);
      }

      try {
        unlistenDefChanged = await listen('definition:changed', () => {
          setSyncToken((v) => v + 1);
        });
      } catch (e) {
        console.warn('[TsDashboard] Failed to listen for definition:changed:', e);
      }

      try {
        unlistenSettings = await listen<string>('settings:changed', (event) => {
          if (event.payload === 'auto_sync_gauge_ranges') {
            invoke<any>('get_settings')
              .then((settings) => {
                if (settings.auto_sync_gauge_ranges !== undefined) {
                  setAutoSyncGaugeRanges(!!settings.auto_sync_gauge_ranges);
                }
              })
              .catch((e) => console.warn('[TsDashboard] get_settings failed:', e));
          }
        });
      } catch (e) {
        console.warn('[TsDashboard] Failed to listen for settings:changed:', e);
      }
    })();

    return () => {
      if (unlistenIni) unlistenIni();
      if (unlistenDefLoaded) unlistenDefLoaded();
      if (unlistenDefChanged) unlistenDefChanged();
      if (unlistenSettings) unlistenSettings();
    };
  }, []);

  // Exit designer mode
  const handleExitDesigner = useCallback(() => {
    setDesignerMode(false);
    setSelectedGaugeId(null);
  }, []);

  // Load/refresh available dashboards list
  // Load/refresh available dashboards list. The Rust backend seeds the
  // app data dir with default dashboards (Basic/Tuning/Racing) on first run,
  // so list_available_dashes should always return at least these. If it does
  // come back empty (e.g. seeding failed), we surface an empty list and let
  // the empty-state UI / Reset to Defaults action recover.
  const refreshDashboardList = useCallback(async () => {
    try {
      const dashes = await invoke<DashFileInfo[]>('list_available_dashes');
      setAvailableDashes(dashes ?? []);
      return dashes ?? [];
    } catch (e) {
      console.warn('[TsDashboard] list_available_dashes failed:', e);
      setAvailableDashes([]);
      return [];
    }
  }, []);

  // Handle import completion - refresh list and optionally select first imported
  const handleImportComplete = useCallback(async (imported: DashFileInfo[]) => {
    await refreshDashboardList();
    
    // Select the first imported dashboard if any were imported
    if (imported.length > 0) {
      setSelectedPath(imported[0].path);
    }
    
    setShowImportDialog(false);
  }, [refreshDashboardList]);

  // Create new dashboard from template
  const handleNewDashboard = useCallback(async () => {
    if (!newDashName.trim()) return;
    
    try {
      // Create a new dashboard with basic template
      const newPath = await invoke<string>('create_new_dashboard', { 
        name: newDashName.trim(),
        template: 'basic' 
      });
      await refreshDashboardList();
      setSelectedPath(newPath);
      setShowNewDialog(false);
      setNewDashName('');
    } catch (e) {
      console.error('Failed to create dashboard:', e);
    }
  }, [newDashName, refreshDashboardList]);

  // Rename current dashboard
  const handleRenameDashboard = useCallback(async () => {
    if (!renameName.trim() || !selectedPath) return;
    
    try {
      const newPath = await invoke<string>('rename_dashboard', { 
        path: selectedPath, 
        newName: renameName.trim() 
      });
      await refreshDashboardList();
      setSelectedPath(newPath);
      setShowRenameDialog(false);
      setRenameName('');
    } catch (e) {
      console.error('Failed to rename dashboard:', e);
    }
  }, [renameName, selectedPath, refreshDashboardList]);

  // Delete current dashboard
  const handleDeleteDashboard = useCallback(async () => {
    if (!selectedPath) return;
    
    try {
      await invoke('delete_dashboard', { path: selectedPath });
      const dashes = await refreshDashboardList();
      // Select next available dashboard
      if (dashes.length > 0) {
        setSelectedPath(dashes[0].path);
      } else {
        setSelectedPath('');
        setDashFile(null);
      }
      setShowDeleteConfirm(false);
    } catch (e) {
      console.error('Failed to delete dashboard:', e);
    }
  }, [selectedPath, refreshDashboardList]);

  // Duplicate current dashboard
  const handleDuplicateDashboard = useCallback(async () => {
    if (!dashFile || !selectedPath) return;
    
    try {
      // Generate a name for the copy
      const currentName = selectedPath.split('/').pop()?.replace(/\.(ltdash\.xml|dash)$/i, '') || 'Dashboard';
      const copyName = `${currentName} (Copy)`;
      
      const newPath = await invoke<string>('duplicate_dashboard', { 
        path: selectedPath,
        newName: copyName
      });
      await refreshDashboardList();
      setSelectedPath(newPath);
    } catch (e) {
      console.error('Failed to duplicate dashboard:', e);
    }
  }, [dashFile, selectedPath, refreshDashboardList]);

  // Export dashboard to file
  const handleExportDashboard = useCallback(async () => {
    if (!dashFile) return;
    
    try {
      const currentName = selectedPath.split('/').pop()?.replace(/\.(ltdash\.xml|dash)$/i, '') || 'Dashboard';
      const filePath = await save({
        title: 'Export Dashboard',
        filters: [{ name: 'Dashboard Files', extensions: ['ltdash.xml', 'dash', 'gauge'] }],
        defaultPath: `${currentName}.ltdash.xml`,
      });
      
      if (filePath) {
        await invoke('export_dashboard', { dashFile, path: filePath });
      }
    } catch (e) {
      console.error('Failed to export dashboard:', e);
    }
  }, [dashFile, selectedPath]);

  // Recompute scale when validation panel visibility changes
  useEffect(() => {
    // Small delay to ensure DOM has updated and layout has settled
    const timer = setTimeout(() => computeScale(), 100);
    return () => clearTimeout(timer);
  }, [showValidationPanel, computeScale]);

  // Load available dashboards
  useEffect(() => {
    const loadInitial = async () => {
      const dashes = await refreshDashboardList();
      
      // If no initial path, select first available
      if (!selectedPath && dashes.length > 0) {
        // Prefer Basic.ltdash.xml as the default
        const basicDash = dashes.find(d => d.name === 'Basic.ltdash.xml');
        if (basicDash) {
          setSelectedPath(basicDash.path);
          return;
        }

        const libreTuneDash = dashes.find(d => d.category === 'LibreTune');
        setSelectedPath(libreTuneDash?.path || dashes[0].path);
      }
    };
    loadInitial();
  }, []);

  // Load selected dashboard (only when the selected path changes)
  useEffect(() => {
    const loadDashboard = async () => {
      if (!selectedPath) {
        setLoading(false);
        return;
      }

      setLoading(true);
      setError(null);

      try {
        const file = await invoke<DashFile>('get_dash_file', { path: selectedPath });
        setDashFile(file);
        setLegacyMode(isLegacyPath);
        requestAnimationFrame(() => computeScale());

        // Note: Do not start sweep here based on realtime updates — we will decide sweep in a separate effect using an instantaneous snapshot
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    };

    loadDashboard();
  }, [selectedPath, isLegacyPath, computeScale]);

  // On dashboard file load, decide whether to run the initial sweep using a snapshot of realtime data.
  // Uses a direct store read for RPM instead of the async rpmChannel state (which is null on mount,
  // causing sweep to fire on every tab switch even when the engine is running).
  useEffect(() => {
    if (!dashFile) return;

    // Try common RPM channel names directly from the store (no async dependency)
    const channels = useRealtimeStore.getState().channels;
    const rpm = channels['rpm'] ?? channels['RPM'] ?? channels['RPMValue'] ?? channels['engineSpeed'] ?? undefined;
    const isEngineRunning = typeof rpm === 'number' && rpm > 50;

    if (!isConnected || !isEngineRunning) {
      startGaugeSweep(dashFile);
    }
    // Only trigger on dashFile load (not on every isConnected change)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dashFile]);

  const handleDashSelect = (path: string) => {
    setSelectedPath(path);
    setShowSelector(false);
  };

  if (loading) {
    return (
      <div className="ts-dashboard ts-dashboard-loading">
        <div className="loading-spinner">Loading dashboard...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="ts-dashboard ts-dashboard-error">
        <div className="error-message">
          <h3>Failed to load dashboard</h3>
          <p>{error}</p>
          <button onClick={() => setShowSelector(true)}>Select Dashboard</button>
        </div>
      </div>
    );
  }

  if (!dashFile) {
    return (
      <div className="ts-dashboard ts-dashboard-empty">
        <div className="empty-message">
          <h3>No Dashboard Selected</h3>
          <button onClick={() => setShowSelector(true)}>Select Dashboard</button>
        </div>
      </div>
    );
  }

  const cluster = dashFile.gauge_cluster;
  const bgColor = tsColorToRgba(cluster.cluster_background_color);
  const bgImageUrl = cluster.cluster_background_image_file_name
    ? embeddedImages.get(cluster.cluster_background_image_file_name)
    : null;
  const ditherColor = cluster.background_dither_color
    ? tsColorToRgba(cluster.background_dither_color)
    : null;
  const ditherPattern = ditherColor
    ? `repeating-linear-gradient(45deg, ${ditherColor} 0 1px, transparent 1px 3px)`
    : null;
  const imageSize = cluster.cluster_background_image_style === 'Stretch' ? 'cover'
    : cluster.cluster_background_image_style === 'Fit' ? 'contain'
    : cluster.cluster_background_image_style === 'Center' ? 'auto'
    : undefined;
  const backgroundImageLayers = [ditherPattern, bgImageUrl ? `url(${bgImageUrl})` : null]
    .filter(Boolean)
    .join(', ');
  const backgroundSizeLayers = ditherPattern && bgImageUrl
    ? `4px 4px, ${imageSize ?? 'auto'}`
    : ditherPattern
      ? '4px 4px'
      : imageSize;
  const backgroundRepeatLayers = ditherPattern && bgImageUrl
    ? `repeat, ${cluster.cluster_background_image_style === 'Tile' ? 'repeat' : 'no-repeat'}`
    : ditherPattern
      ? 'repeat'
      : (cluster.cluster_background_image_style === 'Tile' ? 'repeat' : 'no-repeat');



  return (
    <div className="ts-dashboard-container">
      {/* Header with dashboard selector */}
      <div className="ts-dashboard-header">
        <div className="ts-dashboard-header-left">
          <span className="ts-dashboard-title">
            {dashFile.bibliography.author || selectedPath.split('/').pop()?.replace(/\.(ltdash\.xml|dash)$/i, '') || 'Dashboard'}
          </span>
          <button 
            className="ts-dashboard-selector-btn"
            onClick={() => setShowSelector(!showSelector)}
          >
            Change ▼
          </button>

        </div>
        <div className="ts-dashboard-header-right">
          <button 
            className="ts-dashboard-action-btn"
            onClick={() => { setNewDashName(''); setShowNewDialog(true); }}
            title="New Dashboard"
          >
            <Plus size={14} /> New
          </button>
          <button 
            className="ts-dashboard-action-btn"
            onClick={handleDuplicateDashboard}
            title="Duplicate Dashboard"
          >
            <Copy size={14} /> Duplicate
          </button>
          <button 
            className="ts-dashboard-action-btn"
            onClick={() => { 
              const currentName = selectedPath.split('/').pop()?.replace(/\.(ltdash\.xml|dash)$/i, '') || '';
              setRenameName(currentName);
              setShowRenameDialog(true); 
            }}
            title="Rename Dashboard"
          >
            <Pencil size={14} /> Rename
          </button>
          <button 
            className="ts-dashboard-action-btn danger"
            onClick={() => setShowDeleteConfirm(true)}
            title="Delete Dashboard"
          >
            <Trash2 size={14} /> Delete
          </button>
          <button 
            className="ts-dashboard-action-btn"
            onClick={handleExportDashboard}
            title="Export Dashboard"
          >
            <Save size={14} /> Export
          </button>
          <button 
            className="ts-dashboard-action-btn"
            onClick={handleSyncGaugeRanges}
            title="Sync gauge ranges from INI"
          >
            <RotateCw size={14} /> Sync Ranges
          </button>
          {validationReport && (
            <button
              className={`ts-dashboard-action-btn ${
                validationReport.errors.length > 0
                  ? 'danger'
                  : validationReport.warnings.length > 0
                    ? 'warn'
                    : ''
              }`}
              onClick={() => setShowValidationPanel((prev) => !prev)}
              title="Dashboard validation issues"
            >
              <AlertTriangle size={14} /> Validate ({validationReport.errors.length}E/{validationReport.warnings.length}W)
            </button>
          )}
          <button
            className={`ts-dashboard-action-btn ${legacyMode ? 'active' : ''}`}
            onClick={() => setLegacyMode(prev => !prev)}
            title={legacyMode ? 'Legacy TS layout enabled' : 'Enable legacy TS layout'}
          >
            <Compass size={14} /> Legacy: {legacyMode ? 'On' : 'Off'}
          </button>
        </div>
      </div>

      {showValidationPanel && validationReport && (
        <div className="ts-dashboard-validation">
          <div className="ts-dashboard-validation-header">
            <div>
              Validation: {validationReport.errors.length} error(s), {validationReport.warnings.length} warning(s)
            </div>
            <button
              className="ts-dashboard-compat-close"
              onClick={() => setShowValidationPanel(false)}
              title="Dismiss"
              aria-label="Dismiss"
            >
              <X size={14} />
            </button>
          </div>
          {validationReport.errors.length === 0 && validationReport.warnings.length === 0 ? (
            <div className="ts-dashboard-validation-empty">No issues detected.</div>
          ) : (
            <div className="ts-dashboard-validation-body">
              {validationReport.errors.length > 0 && (
                <div className="ts-dashboard-validation-section">
                  <h4>Errors</h4>
                  <ul>
                    {validationReport.errors.map((issue, idx) => (
                      <li key={`err-${idx}`}>{formatValidationIssue(issue)}</li>
                    ))}
                  </ul>
                </div>
              )}
              {validationReport.warnings.length > 0 && (
                <div className="ts-dashboard-validation-section">
                  <h4>Warnings</h4>
                  <ul>
                    {validationReport.warnings.map((issue, idx) => (
                      <li key={`warn-${idx}`}>{formatValidationIssue(issue)}</li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {compatibilityReport && compatBarVisible && hasCompatibilityIssues && (
        <div className={`ts-dashboard-compat warn`}>
          <span>
            Compatibility: some features not yet supported
          </span>
          <button
            className="ts-dashboard-compat-close"
            onClick={() => setCompatBarVisible(false)}
            title="Dismiss"
            aria-label="Dismiss"
          >
            <X size={14} />
          </button>
        </div>
      )}

      {/* Dashboard selector dropdown */}
      {showSelector && (
        <div className="ts-dashboard-selector-overlay" onClick={() => setShowSelector(false)}>
          <div className="ts-dashboard-selector" onClick={e => e.stopPropagation()}>
            <h3>Select Dashboard</h3>
            <div className="ts-dashboard-list">
              {/* Group dashboards by category */}
              {(() => {
                const categories = new Map<string, DashFileInfo[]>();
                availableDashes.forEach(dash => {
                  const cat = dash.category || 'Other';
                  if (!categories.has(cat)) {
                    categories.set(cat, []);
                  }
                  categories.get(cat)!.push(dash);
                });
                
                // Sort categories: User first, then Reference, then others
                const sortedCats = Array.from(categories.keys()).sort((a, b) => {
                  if (a === 'User') return -1;
                  if (b === 'User') return 1;
                  if (a === 'Reference') return -1;
                  if (b === 'Reference') return 1;
                  return a.localeCompare(b);
                });
                
                return sortedCats.map(category => (
                  <div key={category} className="ts-dashboard-category">
                    <div className="ts-dashboard-category-header">
                      {category}
                      <span className="ts-dashboard-category-count">
                        ({categories.get(category)!.length})
                      </span>
                    </div>
                    <div className="ts-dashboard-category-items">
                      {categories.get(category)!.map((dash) => (
                        <button
                          key={dash.path}
                          className={`ts-dashboard-option ${dash.path === selectedPath ? 'selected' : ''}`}
                          onClick={() => handleDashSelect(dash.path)}
                          title={dash.path}
                        >
                          {dash.name.replace(/\.(ltdash\.xml|dash|gauge)$/i, '')}
                        </button>
                      ))}
                    </div>
                  </div>
                ));
              })()}
            </div>
            
            {/* Import button */}
            <div className="ts-dashboard-import-section">
              <button
                className="ts-dashboard-import-btn"
                onClick={() => {
                  setShowSelector(false);
                  setShowImportDialog(true);
                }}
              >
                <FolderOpen size={14} /> Import TS Dashboard Files...
              </button>
            </div>
          </div>
        </div>
      )}


      
      {/* Import dialog */}
      <ImportDashboardDialog
        isOpen={showImportDialog}
        onClose={() => setShowImportDialog(false)}
        onImportComplete={handleImportComplete}
      />

      {/* New Dashboard Dialog */}
      <Dialog
        open={showNewDialog}
        onClose={() => setShowNewDialog(false)}
        title="New Dashboard"
        size="sm"
      >
        <Dialog.Body>
          <label>Dashboard Name:</label>
          <input
            type="text"
            value={newDashName}
            onChange={(e) => setNewDashName(e.target.value)}
            placeholder="My Dashboard"
            autoFocus
            onKeyDown={(e) => e.key === 'Enter' && handleNewDashboard()}
          />
        </Dialog.Body>
        <Dialog.Footer>
          <Button variant="secondary" onClick={() => setShowNewDialog(false)}>Cancel</Button>
          <Button variant="primary" onClick={handleNewDashboard} disabled={!newDashName.trim()}>
            Create
          </Button>
        </Dialog.Footer>
      </Dialog>

      {/* Rename Dashboard Dialog */}
      <Dialog
        open={showRenameDialog}
        onClose={() => setShowRenameDialog(false)}
        title="Rename Dashboard"
        size="sm"
      >
        <Dialog.Body>
          <label>New Name:</label>
          <input
            type="text"
            value={renameName}
            onChange={(e) => setRenameName(e.target.value)}
            placeholder="Dashboard Name"
            autoFocus
            onKeyDown={(e) => e.key === 'Enter' && handleRenameDashboard()}
          />
        </Dialog.Body>
        <Dialog.Footer>
          <Button variant="secondary" onClick={() => setShowRenameDialog(false)}>Cancel</Button>
          <Button variant="primary" onClick={handleRenameDashboard} disabled={!renameName.trim()}>
            Rename
          </Button>
        </Dialog.Footer>
      </Dialog>

      {/* Delete Confirmation Dialog */}
      <Dialog
        open={showDeleteConfirm}
        onClose={() => setShowDeleteConfirm(false)}
        title="Delete Dashboard?"
        size="sm"
      >
        <Dialog.Body>
          <p>Are you sure you want to delete "{selectedPath.split('/').pop()?.replace(/\.(ltdash\.xml|dash)$/i, '')}"?</p>
          <p className="warning">This action cannot be undone.</p>
        </Dialog.Body>
        <Dialog.Footer>
          <Button variant="secondary" onClick={() => setShowDeleteConfirm(false)}>Cancel</Button>
          <Button variant="danger" onClick={handleDeleteDashboard}>Delete</Button>
        </Dialog.Footer>
      </Dialog>

      {/* Designer Mode - full screen editor */}
      {designerMode && dashFile ? (
        <DashboardDesigner
          dashFile={dashFile}
          onDashFileChange={setDashFile}
          selectedGaugeId={selectedGaugeId}
          onSelectGauge={setSelectedGaugeId}
          onContextMenu={handleContextMenu}
          gridSnap={gridSnap}
          onGridSnapChange={setGridSnap}
          showGrid={showGrid}
          onShowGridChange={setShowGrid}
          onSave={handleSaveDashboard}
          onExit={handleExitDesigner}
          channelInfoMap={channelInfoMap}
        />
      ) : (
        <>
      {/* Dashboard scaling wrapper - handles dynamic scaling for small viewports */}
      <div 
        ref={dashboardWrapperRef}
        className="ts-dashboard-wrapper"
      >
        {/* Dashboard canvas with derived aspect ratio */}
        <div 
          className={`ts-dashboard ${designerMode ? 'designer-mode' : ''}`}
          style={{
            backgroundColor: bgColor,
            backgroundImage: backgroundImageLayers || undefined,
            backgroundSize: backgroundSizeLayers,
            backgroundRepeat: backgroundRepeatLayers,
            backgroundPosition: 'center',
            aspectRatio: `${dashboardBounds.aspectRatio}`,
            transform: scale < 1 ? `scale(${scale})` : undefined,
            transformOrigin: 'top center',
        }}
        onContextMenu={(e) => handleContextMenu(e, null)}
        onDragOver={(e) => {
          if (!designerMode) return;
          e.preventDefault();
          e.stopPropagation();
          e.dataTransfer.dropEffect = 'copy';
          e.currentTarget.style.opacity = '0.8';
        }}
        onDragLeave={(e) => {
          if (!designerMode) return;
          e.currentTarget.style.opacity = '1';
        }}
        onDrop={(e) => {
          if (!designerMode) return;
          e.preventDefault();
          e.stopPropagation();
          e.currentTarget.style.opacity = '1';

          try {
            const data = e.dataTransfer.getData('application/json');
            if (!data) return;
            
            const channel = JSON.parse(data);
            if (channel.type !== 'channel' || !dashFile) return;

            // Calculate relative drop position (0.0-1.0)
            const rect = e.currentTarget.getBoundingClientRect();
            const relX = (e.clientX - rect.left) / rect.width;
            const relY = (e.clientY - rect.top) / rect.height;

            // Get channel info (units, label)
            const info = channelInfoMap[channel.id];
            const units = info?.units || '';
            const label = info?.label || channel.label;

            // Create default gauge config
            const defaultGauge: TsGaugeConfig = buildDefaultGauge({
              id: `gauge_${Date.now()}`,
              channel: channel.id,
              title: label || channel.label,
              units,
              relativeX: relX - 0.1,
              relativeY: relY - 0.1,
            });

            // Add new gauge to dashboard
            const updatedComponents = [...dashFile.gauge_cluster.components, { Gauge: defaultGauge }];
            const updatedFile: DashFile = {
              ...dashFile,
              gauge_cluster: {
                ...dashFile.gauge_cluster,
                components: updatedComponents,
              },
            };
            setDashFile(updatedFile);

            // Auto-save
            try {
              invoke('save_dash_file', { 
                path: selectedPath,
                dashFile: updatedFile,
              }).catch(err => console.error('Failed to auto-save dashboard:', err));
            } catch (err) {
              console.error('Failed to save dashboard:', err);
            }
          } catch (err) {
            console.error('Failed to process dropped channel:', err);
          }
        }}
      >
        {cluster.components.map((component, index) => {
          // Convert relative positions to percentages - allow values outside 0-1 range
          // (TunerStudio dashboards can have negative positions or >1.0 for extending beyond bounds)
          const toPercent = (v: number | undefined | null) => ((v ?? 0) * 100);

          if (isGauge(component)) {
            const gauge = component.Gauge;
            // TsGauge handles its own store subscription for live data.
            // We only pass live values via props for sweep/demo mode.
            // In normal mode, pass gauge.value (config default) — the prop is stable,
            // so React.memo blocks re-renders and the internal store subscription
            // drives the animation without causing the dashboard to cascade re-renders.
            const value = sweepActive
              ? (sweepValues[gauge.output_channel] ?? gauge.min)
              : gaugeDemoActive 
                ? (demoValues[gauge.output_channel] ?? gauge.value)
                : gauge.value;
            
            // Build gauge style with shape_locked_to_aspect and shortest_size support
            const gaugeStyle: React.CSSProperties = {
              left: `${toPercent(gauge.relative_x)}%`,
              top: `${toPercent(gauge.relative_y)}%`,
              width: `${toPercent(gauge.relative_width)}%`,
              height: `${toPercent(gauge.relative_height)}%`,
              // Enforce minimum size from shortest_size property
              minWidth: !legacyMode && gauge.shortest_size > 0 ? `${gauge.shortest_size}px` : undefined,
              minHeight: !legacyMode && gauge.shortest_size > 0 ? `${gauge.shortest_size}px` : undefined,
              // Force square aspect ratio when shape is locked
              aspectRatio: gauge.shape_locked_to_aspect ? '1 / 1' : undefined,
            };
            
            return (
              <div
                key={gauge.id || `gauge-${index}`}
                className={`ts-component ts-gauge ${designerMode ? 'editable' : ''}`}
                style={gaugeStyle}
                onContextMenu={(e) => handleContextMenu(e, gauge.id)}
              >
                <TsGauge 
                  config={gauge}
                  value={value}
                  embeddedImages={embeddedImages}
                  legacyMode={legacyMode}
                  overrideStore={sweepActive || gaugeDemoActive}
                />
              </div>
            );
          }

          if (isIndicator(component)) {
            const indicator = component.Indicator;
            
            return (
              <div
                key={indicator.id || `indicator-${index}`}
                className={`ts-component ts-indicator ${designerMode ? 'editable' : ''}`}
                style={{
                  left: `${toPercent(indicator.relative_x)}%`,
                  top: `${toPercent(indicator.relative_y)}%`,
                  width: `${toPercent(indicator.relative_width)}%`,
                  height: `${toPercent(indicator.relative_height)}%`,
                }}
                onContextMenu={(e) => handleContextMenu(e, indicator.id)}
              >
                <LiveTsIndicator
                  config={indicator}
                  embeddedImages={embeddedImages}
                />
              </div>
            );
          }

          return null;
        })}
        </div>
      </div>

      {/* Context Menu */}
      <GaugeContextMenu
        state={contextMenu}
        onClose={closeContextMenu}
        designerMode={designerMode}
        onDesignerModeChange={setDesignerMode}
        antialiasingEnabled={cluster.anti_aliasing}
        onAntialiasingChange={(enabled) => {
          if (dashFile) {
            setDashFile({
              ...dashFile,
              gauge_cluster: { ...dashFile.gauge_cluster, anti_aliasing: enabled }
            });
          }
        }}
        gaugeDemoActive={gaugeDemoActive}
        onGaugeDemoToggle={() => setGaugeDemoActive(!gaugeDemoActive)}
        backgroundColor={cluster.cluster_background_color}
        onBackgroundColorChange={(color) => {
          if (dashFile) {
            setDashFile({
              ...dashFile,
              gauge_cluster: { ...dashFile.gauge_cluster, cluster_background_color: color }
            });
          }
        }}
        backgroundDitherColor={cluster.background_dither_color}
        onBackgroundDitherColorChange={(color) => {
          if (dashFile) {
            setDashFile({
              ...dashFile,
              gauge_cluster: { ...dashFile.gauge_cluster, background_dither_color: color }
            });
          }
        }}
        onReloadDefaultGauges={handleReloadDefaultGauges}
        onResetValue={() => {
          // Reset channel value to minimum
          if (!contextMenu.targetGaugeId || !dashFile) return;
          
          const gauge = dashFile.gauge_cluster.components.find((comp) =>
            isGauge(comp) && comp.Gauge.id === contextMenu.targetGaugeId
          );
          
          if (gauge && isGauge(gauge)) {
            const channel = gauge.Gauge.output_channel;
            const minValue = gauge.Gauge.min || 0;
            // You would need to emit an update to the realtime store or send to ECU
            console.log('Reset channel', channel, 'to', minValue);
          }
          closeContextMenu();
        }}
        onReplaceGauge={(channel, gaugeInfo) => {
          // Replace the targeted gauge with a new one from INI
          if (!dashFile || !contextMenu.targetGaugeId) return;
          
          // Find the gauge to replace
          const updatedComponents = dashFile.gauge_cluster.components.map((comp) => {
            if (!isGauge(comp)) return comp;
            if (comp.Gauge.id !== contextMenu.targetGaugeId) return comp;
            
            // Replace with new gauge info - keep position/size but update channel
            return {
              Gauge: {
                ...comp.Gauge,
                output_channel: channel,
                title: gaugeInfo.title,
                units: gaugeInfo.units,
                min: gaugeInfo.min,
                max: gaugeInfo.max,
              }
            };
          });
          
          const newFile = {
            ...dashFile,
            gauge_cluster: { ...dashFile.gauge_cluster, components: updatedComponents },
          };
          setDashFile(newFile);
          closeContextMenu();
        }}
      />
        </>
      )}
    </div>
  );
}
