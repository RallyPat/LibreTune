import {
  DashFile,
  isGauge,
  isIndicator,
  buildEmbeddedImageMap,
  tsColorToRgba,
} from './dashTypes';

import React, { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { useRealtimeStore } from '../../stores/realtimeStore';
import {
  useDashboardStore,
  selectLegacyMode,
} from '../../stores/dashboardStore';
import GaugeContextMenu from './GaugeContextMenu';
import ImportDashboardDialog from '../dialogs/ImportDashboardDialog';
import DashboardDesigner from './DashboardDesigner';
import DashboardHeader from './components/DashboardHeader';
import ValidationPanel, { type ChannelRemap } from './components/ValidationPanel';
import CompatibilityBar from './components/CompatibilityBar';
import DashboardSelectorOverlay from './components/DashboardSelectorOverlay';
import DashboardManagementDialogs from './components/DashboardManagementDialogs';
import DashboardCanvas from './components/DashboardCanvas';
import {
  computeCompatibilityReport,
  hasCompatibilityIssues as hasCompatIssues,
} from './utils/compatibility';
import { computeDashboardBounds } from './utils/dashboardBounds';
import { dashBaseName } from './shared/dashFilename';
import { useChannelInfoMap } from './shared/channelInfo';
import { useGaugeSweep } from './hooks/useGaugeSweep';
import { useGaugeDemo } from './hooks/useGaugeDemo';
import { useDashboardScale } from './hooks/useDashboardScale';
import { useDashboardValidation } from './hooks/useDashboardValidation';
import { useGaugeRangeSync } from './hooks/useGaugeRangeSync';
import './TsDashboard.css';

/**
 * Props for the TsDashboard component.
 */
interface TsDashboardProps {
  /** Whether ECU is connected (enables data display) */
  isConnected?: boolean;
}

/**
 * Dashboard shell — layout and wiring only. All dashboard state lives in
 * the dashboardStore (see stores/dashboardStore.ts); realtime channel data
 * lives in realtimeStore and never flows through this component. Each
 * TsGauge subscribes to its own channel directly inside its rAF loop.
 */
export default function TsDashboard({ isConnected = false }: TsDashboardProps) {
  // --- Store state -----------------------------------------------------------
  const dashFile = useDashboardStore((s) => s.dashFile);
  const selectedPath = useDashboardStore((s) => s.selectedPath);
  const loading = useDashboardStore((s) => s.loading);
  const error = useDashboardStore((s) => s.error);
  const availableDashes = useDashboardStore((s) => s.availableDashes);
  const designerMode = useDashboardStore((s) => s.designerMode);
  const selectedGaugeId = useDashboardStore((s) => s.selectedGaugeId);
  const gridSnap = useDashboardStore((s) => s.gridSnap);
  const showGrid = useDashboardStore((s) => s.showGrid);
  const demoActive = useDashboardStore((s) => s.demoActive);
  const contextMenu = useDashboardStore((s) => s.contextMenu);
  const legacyMode = useDashboardStore(selectLegacyMode);

  const init = useDashboardStore((s) => s.init);
  const selectDashboard = useDashboardStore((s) => s.selectDashboard);
  const reloadCurrent = useDashboardStore((s) => s.reloadCurrent);
  const saveDashboard = useDashboardStore((s) => s.save);
  const createDashboard = useDashboardStore((s) => s.createDashboard);
  const renameDashboard = useDashboardStore((s) => s.renameDashboard);
  const deleteDashboard = useDashboardStore((s) => s.deleteDashboard);
  const duplicateDashboard = useDashboardStore((s) => s.duplicateDashboard);
  const exportDashboard = useDashboardStore((s) => s.exportDashboard);
  const importCompleted = useDashboardStore((s) => s.importCompleted);
  const resetToDefaults = useDashboardStore((s) => s.resetToDefaults);
  const setDashFile = useDashboardStore((s) => s.setDashFile);
  const setDesignerMode = useDashboardStore((s) => s.setDesignerMode);
  const setSelectedGaugeId = useDashboardStore((s) => s.setSelectedGaugeId);
  const setGridSnap = useDashboardStore((s) => s.setGridSnap);
  const setShowGrid = useDashboardStore((s) => s.setShowGrid);
  const setDemoActive = useDashboardStore((s) => s.setDemoActive);
  const setLegacyModeOverride = useDashboardStore((s) => s.setLegacyModeOverride);
  const openContextMenu = useDashboardStore((s) => s.openContextMenu);
  const closeContextMenu = useDashboardStore((s) => s.closeContextMenu);

  // --- Local UI-only state ---------------------------------------------------
  const [showSelector, setShowSelector] = useState(false);
  const [showImportDialog, setShowImportDialog] = useState(false);
  const [showNewDialog, setShowNewDialog] = useState(false);
  const [showRenameDialog, setShowRenameDialog] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [compatBarVisible, setCompatBarVisible] = useState(true);
  const [showValidationPanel, setShowValidationPanel] = useState(false);
  const channelInfoMap = useChannelInfoMap();

  // --- Init: load dashboard list and initial selection -----------------------
  useEffect(() => {
    void init();
  }, [init]);

  // --- Derived data -----------------------------------------------------------
  // Build embedded images map — memoized so TsGauge's React.memo doesn't
  // re-run on every TsDashboard render.
  const embeddedImages = useMemo(
    () => dashFile
      ? buildEmbeddedImageMap(dashFile.gauge_cluster.embedded_images)
      : new Map<string, string>(),
    [dashFile]
  );

  // Calculate dashboard aspect ratio from gauge bounding box.
  // Must be before any early returns to comply with React Rules of Hooks.
  const dashboardBounds = useMemo(
    () => computeDashboardBounds(dashFile),
    [dashFile],
  );

  const compatibilityReport = useMemo(
    () => (dashFile ? computeCompatibilityReport(dashFile) : null),
    [dashFile],
  );

  const hasCompatibilityIssues = useMemo(
    () => hasCompatIssues(compatibilityReport),
    [compatibilityReport],
  );

  // Dynamic scaling: shrink the dashboard when the viewport is too small.
  const { scale, scrollable, wrapperRef: dashboardWrapperRef, recompute: computeScale } =
    useDashboardScale(dashboardBounds.aspectRatio);

  // Validation: re-runs whenever the dash file changes.
  const validationReport = useDashboardValidation(dashFile);

  // --- Animations (non-reactive override module; no container re-renders) ----
  const { startGaugeSweep } = useGaugeSweep();
  useGaugeDemo(demoActive, dashFile);

  const isConnectedRef = useRef(isConnected);
  isConnectedRef.current = isConnected;
  const lastSweepDashRef = useRef<DashFile | null>(null);

  useEffect(() => {
    if (!dashFile) return;
    // Fire at most once per loaded file (the guard makes the effect
    // idempotent if it re-runs due to callback identity changes).
    if (lastSweepDashRef.current === dashFile) return;
    lastSweepDashRef.current = dashFile;

    // Try common RPM channel names directly from the store (no async dependency)
    const channels = useRealtimeStore.getState().channels;
    const rpm = channels['rpm'] ?? channels['RPM'] ?? channels['RPMValue'] ?? channels['engineSpeed'] ?? undefined;
    const isEngineRunning = typeof rpm === 'number' && rpm > 50;

    if (!isConnectedRef.current || !isEngineRunning) {
      startGaugeSweep(dashFile);
    }
  }, [dashFile, startGaugeSweep]);

  // Sync gauge ranges from INI GaugeConfigurations (manual trigger + auto).
  const { syncGaugeRanges: handleSyncGaugeRanges } =
    useGaugeRangeSync(dashFile, setDashFile);

  // Recompute scale when validation panel visibility changes
  useEffect(() => {
    // Small delay to ensure DOM has updated and layout has settled
    const timer = setTimeout(() => computeScale(), 100);
    return () => clearTimeout(timer);
  }, [showValidationPanel, computeScale]);

  // Handle right-click context menu
  const handleContextMenu = useCallback((e: React.MouseEvent, gaugeId: string | null) => {
    e.preventDefault();
    e.stopPropagation();
    openContextMenu(e.clientX, e.clientY, gaugeId);
  }, [openContextMenu]);

  const handleImportComplete = useCallback(async (imported: Parameters<typeof importCompleted>[0]) => {
    await importCompleted(imported);
    setShowImportDialog(false);
  }, [importCompleted]);

  const handleNewDashboard = useCallback(async (name: string, template: string) => {
    if (!name.trim()) return;
    await createDashboard(name, template);
    setShowNewDialog(false);
  }, [createDashboard]);

  const handleRenameDashboard = useCallback(async (name: string) => {
    if (!name.trim() || !selectedPath) return;
    await renameDashboard(name);
    setShowRenameDialog(false);
  }, [selectedPath, renameDashboard]);

  const handleDeleteDashboard = useCallback(async () => {
    await deleteDashboard();
    setShowDeleteConfirm(false);
  }, [deleteDashboard]);

  const handleResetDefaults = useCallback(async () => {
    await resetToDefaults();
    setShowResetConfirm(false);
    setShowSelector(false);
  }, [resetToDefaults]);

  const handleApplyRemap = useCallback((remap: ChannelRemap) => {
    if (!dashFile) return;
    const updatedComponents = dashFile.gauge_cluster.components.map((comp) => {
      if (isGauge(comp) && comp.Gauge.id === remap.component_id) {
        return { Gauge: { ...comp.Gauge, output_channel: remap.to_channel } };
      }
      if (isIndicator(comp) && comp.Indicator.id === remap.component_id) {
        return { Indicator: { ...comp.Indicator, output_channel: remap.to_channel } };
      }
      return comp;
    });
    setDashFile({
      ...dashFile,
      gauge_cluster: { ...dashFile.gauge_cluster, components: updatedComponents },
    });
  }, [dashFile, setDashFile]);

  const currentName = dashBaseName(selectedPath) || 'Dashboard';

  // --- Render -----------------------------------------------------------------
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
        title={dashFile.bibliography.author || currentName}
        showSelector={showSelector}
        onToggleSelector={() => setShowSelector(!showSelector)}
        onNew={() => setShowNewDialog(true)}
        onDuplicate={() => duplicateDashboard()}
        onRename={() => setShowRenameDialog(true)}
        onDelete={() => setShowDeleteConfirm(true)}
        onExport={() => exportDashboard()}
        onSyncRanges={handleSyncGaugeRanges}
        validationReport={validationReport}
        onToggleValidationPanel={() => setShowValidationPanel((prev) => !prev)}
        legacyMode={legacyMode}
        onToggleLegacyMode={() => setLegacyModeOverride(!legacyMode)}
      />

      {showValidationPanel && validationReport && (
        <ValidationPanel
          report={validationReport}
          dashFile={dashFile}
          onApplyRemap={handleApplyRemap}
          onClose={() => setShowValidationPanel(false)}
        />
      )}

      {compatibilityReport && compatBarVisible && hasCompatibilityIssues && (
        <CompatibilityBar onClose={() => setCompatBarVisible(false)} />
      )}

      {/* Dashboard selector dropdown */}
      {showSelector && (
        <DashboardSelectorOverlay
          availableDashes={availableDashes}
          selectedPath={selectedPath}
          onSelect={(path) => {
            selectDashboard(path);
            setShowSelector(false);
          }}
          onClose={() => setShowSelector(false)}
          onImportClick={() => {
            setShowSelector(false);
            setShowImportDialog(true);
          }}
          onResetDefaultsClick={() => {
            setShowSelector(false);
            setShowResetConfirm(true);
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
        onNewClose={() => setShowNewDialog(false)}
        onNewCreate={handleNewDashboard}
        renameOpen={showRenameDialog}
        renameInitialValue={currentName}
        onRenameClose={() => setShowRenameDialog(false)}
        onRenameConfirm={handleRenameDashboard}
        deleteOpen={showDeleteConfirm}
        deleteTargetName={currentName}
        onDeleteClose={() => setShowDeleteConfirm(false)}
        onDeleteConfirm={handleDeleteDashboard}
        resetOpen={showResetConfirm}
        onResetClose={() => setShowResetConfirm(false)}
        onResetConfirm={handleResetDefaults}
      />

      {/* Designer Mode - full screen editor */}
      {designerMode ? (
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
          onSave={saveDashboard}
          onExit={() => setDesignerMode(false)}
          channelInfoMap={channelInfoMap}
          embeddedImages={embeddedImages}
          isConnected={isConnected}
        />
      ) : (
        <DashboardCanvas
          dashFile={dashFile}
          embeddedImages={embeddedImages}
          legacyMode={legacyMode}
          scale={scale}
          scrollable={scrollable}
          aspectRatio={dashboardBounds.aspectRatio}
          bgColor={bgColor}
          backgroundImageLayers={backgroundImageLayers}
          backgroundSizeLayers={backgroundSizeLayers}
          backgroundRepeatLayers={backgroundRepeatLayers}
          isConnected={isConnected}
          wrapperRef={dashboardWrapperRef}
          onContextMenu={handleContextMenu}
        />
      )}

      {/* Context Menu — rendered in both branches so right-click also works in designer mode */}
      <GaugeContextMenu
        state={contextMenu}
        onClose={closeContextMenu}
        designerMode={designerMode}
        onDesignerModeChange={setDesignerMode}
        antialiasingEnabled={cluster.anti_aliasing}
        onAntialiasingChange={(enabled) => {
          setDashFile({
            ...dashFile,
            gauge_cluster: { ...dashFile.gauge_cluster, anti_aliasing: enabled }
          });
        }}
        gaugeDemoActive={demoActive}
        onGaugeDemoToggle={() => setDemoActive(!demoActive)}
        backgroundColor={cluster.cluster_background_color}
        onBackgroundColorChange={(color) => {
          setDashFile({
            ...dashFile,
            gauge_cluster: { ...dashFile.gauge_cluster, cluster_background_color: color }
          });
        }}
        backgroundDitherColor={cluster.background_dither_color}
        onBackgroundDitherColorChange={(color) => {
          setDashFile({
            ...dashFile,
            gauge_cluster: { ...dashFile.gauge_cluster, background_dither_color: color }
          });
        }}
        onReloadDefaultGauges={() => reloadCurrent()}
        onReplaceGauge={(channel, gaugeInfo) => {
          // Replace the targeted gauge with a new one from INI
          if (!contextMenu.targetGaugeId) return;

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

          setDashFile({
            ...dashFile,
            gauge_cluster: { ...dashFile.gauge_cluster, components: updatedComponents },
          });
          closeContextMenu();
        }}
      />
    </div>
  );
}
