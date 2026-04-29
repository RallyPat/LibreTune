import {
  DashFile,
  DashFileInfo,
  isGauge,
  buildEmbeddedImageMap,
  tsColorToRgba,
} from './dashTypes';

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { useRealtimeStore } from '../../stores/realtimeStore';
import { invoke } from '@tauri-apps/api/core';
import GaugeContextMenu, { ContextMenuState } from './GaugeContextMenu';
import ImportDashboardDialog from '../dialogs/ImportDashboardDialog';
import DashboardDesigner from './DashboardDesigner';
import DashboardHeader from './components/DashboardHeader';
import ValidationPanel from './components/ValidationPanel';
import CompatibilityBar from './components/CompatibilityBar';
import DashboardSelectorOverlay from './components/DashboardSelectorOverlay';
import DashboardManagementDialogs from './components/DashboardManagementDialogs';
import DashboardCanvas from './components/DashboardCanvas';
import {
  computeCompatibilityReport,
  hasCompatibilityIssues as hasCompatIssues,
} from './utils/compatibility';
import { computeDashboardBounds } from './utils/dashboardBounds';
import { useGaugeSweep } from './hooks/useGaugeSweep';
import { useGaugeDemo } from './hooks/useGaugeDemo';
import { useDashboardScale } from './hooks/useDashboardScale';
import { useDashboardValidation } from './hooks/useDashboardValidation';
import { useDashboardCRUD } from './hooks/useDashboardCRUD';
import { useGaugeRangeSync } from './hooks/useGaugeRangeSync';
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

export default function TsDashboard({ initialDashPath, isConnected = false }: TsDashboardProps) {
  const [dashFile, setDashFile] = useState<DashFile | null>(null);
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
  const [showValidationPanel, setShowValidationPanel] = useState(false);

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
  // Dashboard CRUD operations (list, save, new, rename, delete, duplicate, export, import).
  const {
    availableDashes,
    refreshDashboardList,
    reloadCurrentDashboard,
    saveDashboard,
    createDashboard,
    renameDashboard,
    deleteDashboard,
    duplicateDashboard,
    exportDashboard,
    onImportComplete,
  } = useDashboardCRUD({ dashFile, selectedPath, setSelectedPath, setDashFile });

  // Sync gauge ranges from INI GaugeConfigurations (manual trigger).
  const { syncGaugeRanges: handleSyncGaugeRanges } =
    useGaugeRangeSync(dashFile, setDashFile);

  // Exit designer mode
  const handleExitDesigner = useCallback(() => {
    setDesignerMode(false);
    setSelectedGaugeId(null);
  }, []);

  const handleReloadDefaultGauges = useCallback(
    () => reloadCurrentDashboard(),
    [reloadCurrentDashboard],
  );

  const handleSaveDashboard = useCallback(
    () => saveDashboard(),
    [saveDashboard],
  );

  // Handle import completion - close dialog after CRUD has refreshed/selected
  const handleImportComplete = useCallback(async (imported: DashFileInfo[]) => {
    await onImportComplete(imported);
    setShowImportDialog(false);
  }, [onImportComplete]);

  // Create new dashboard from template
  const handleNewDashboard = useCallback(async () => {
    if (!newDashName.trim()) return;
    await createDashboard(newDashName);
    setShowNewDialog(false);
    setNewDashName('');
  }, [newDashName, createDashboard]);

  // Rename current dashboard
  const handleRenameDashboard = useCallback(async () => {
    if (!renameName.trim() || !selectedPath) return;
    await renameDashboard(renameName);
    setShowRenameDialog(false);
    setRenameName('');
  }, [renameName, selectedPath, renameDashboard]);

  // Delete current dashboard
  const handleDeleteDashboard = useCallback(async () => {
    await deleteDashboard();
    setShowDeleteConfirm(false);
  }, [deleteDashboard]);

  const handleDuplicateDashboard = useCallback(
    () => duplicateDashboard(),
    [duplicateDashboard],
  );

  const handleExportDashboard = useCallback(
    () => exportDashboard(),
    [exportDashboard],
  );

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
      <DashboardHeader
        title={dashFile.bibliography.author || selectedPath.split('/').pop()?.replace(/\.(ltdash\.xml|dash)$/i, '') || 'Dashboard'}
        showSelector={showSelector}
        onToggleSelector={() => setShowSelector(!showSelector)}
        onNew={() => { setNewDashName(''); setShowNewDialog(true); }}
        onDuplicate={handleDuplicateDashboard}
        onRename={() => {
          const currentName = selectedPath.split('/').pop()?.replace(/\.(ltdash\.xml|dash)$/i, '') || '';
          setRenameName(currentName);
          setShowRenameDialog(true);
        }}
        onDelete={() => setShowDeleteConfirm(true)}
        onExport={handleExportDashboard}
        onSyncRanges={handleSyncGaugeRanges}
        validationReport={validationReport}
        onToggleValidationPanel={() => setShowValidationPanel((prev) => !prev)}
        legacyMode={legacyMode}
        onToggleLegacyMode={() => setLegacyMode((prev) => !prev)}
      />

      {showValidationPanel && validationReport && (
        <ValidationPanel report={validationReport} onClose={() => setShowValidationPanel(false)} />
      )}

      {compatibilityReport && compatBarVisible && hasCompatibilityIssues && (
        <CompatibilityBar onClose={() => setCompatBarVisible(false)} />
      )}

      {/* Dashboard selector dropdown */}
      {showSelector && (
        <DashboardSelectorOverlay
          availableDashes={availableDashes}
          selectedPath={selectedPath}
          onSelect={handleDashSelect}
          onClose={() => setShowSelector(false)}
          onImportClick={() => {
            setShowSelector(false);
            setShowImportDialog(true);
          }}
        />
      )}

      {/* Import dialog */}
      <ImportDashboardDialog
        isOpen={showImportDialog}
        onClose={() => setShowImportDialog(false)}
        onImportComplete={handleImportComplete}
      />

      <DashboardManagementDialogs
        newOpen={showNewDialog}
        newName={newDashName}
        onNewNameChange={setNewDashName}
        onNewClose={() => setShowNewDialog(false)}
        onNewCreate={handleNewDashboard}
        renameOpen={showRenameDialog}
        renameValue={renameName}
        onRenameValueChange={setRenameName}
        onRenameClose={() => setShowRenameDialog(false)}
        onRenameConfirm={handleRenameDashboard}
        deleteOpen={showDeleteConfirm}
        deleteTargetName={selectedPath.split('/').pop()?.replace(/\.(ltdash\.xml|dash)$/i, '') || ''}
        onDeleteClose={() => setShowDeleteConfirm(false)}
        onDeleteConfirm={handleDeleteDashboard}
      />

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
      <DashboardCanvas
        dashFile={dashFile}
        selectedPath={selectedPath}
        setDashFile={setDashFile}
        channelInfoMap={channelInfoMap}
        embeddedImages={embeddedImages}
        designerMode={designerMode}
        legacyMode={legacyMode}
        scale={scale}
        aspectRatio={dashboardBounds.aspectRatio}
        bgColor={bgColor}
        backgroundImageLayers={backgroundImageLayers}
        backgroundSizeLayers={backgroundSizeLayers}
        backgroundRepeatLayers={backgroundRepeatLayers}
        sweepActive={sweepActive}
        sweepValues={sweepValues}
        gaugeDemoActive={gaugeDemoActive}
        demoValues={demoValues}
        wrapperRef={dashboardWrapperRef}
        onContextMenu={handleContextMenu}
      />

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
