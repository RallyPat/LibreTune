/**
 * TunerStudio Dashboard Component
 * 
 * Renders a complete TunerStudio-compatible dashboard from a DashFile structure.
 * Supports all gauge types, indicators, embedded images, and proper positioning.
 */

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  DashFile,
  DashFileInfo,
  isGauge,
  isIndicator,
  buildEmbeddedImageMap,
  tsColorToRgba,
} from './dashTypes';
import TsGauge from '../gauges/TsGauge';
import TsIndicator from '../gauges/TsIndicator';
import GaugeContextMenu, { ContextMenuState } from './GaugeContextMenu';
import './TsDashboard.css';

interface TsDashboardProps {
  realtimeData?: Record<string, number>;
  initialDashPath?: string;
}

export default function TsDashboard({ realtimeData = {}, initialDashPath }: TsDashboardProps) {
  const [dashFile, setDashFile] = useState<DashFile | null>(null);
  const [availableDashes, setAvailableDashes] = useState<DashFileInfo[]>([]);
  const [selectedPath, setSelectedPath] = useState<string>(initialDashPath || '');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showSelector, setShowSelector] = useState(false);

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
  const [demoValues, setDemoValues] = useState<Record<string, number>>({});

  // Build embedded images map
  const embeddedImages = dashFile 
    ? buildEmbeddedImageMap(dashFile.gauge_cluster.embedded_images)
    : new Map<string, string>();

  // Gauge demo animation
  useEffect(() => {
    if (!gaugeDemoActive || !dashFile) return;

    const interval = setInterval(() => {
      const time = Date.now() / 1000;
      const newValues: Record<string, number> = {};
      
      dashFile.gauge_cluster.components.forEach((comp) => {
        if (isGauge(comp)) {
          const gauge = comp.Gauge;
          const range = gauge.max - gauge.min;
          // Sinusoidal demo with random phase per gauge
          const phase = gauge.id.charCodeAt(0) / 10;
          const value = gauge.min + (range / 2) * (1 + Math.sin(time * 0.5 + phase));
          newValues[gauge.output_channel] = value;
        }
      });
      
      setDemoValues(newValues);
    }, 50);

    return () => clearInterval(interval);
  }, [gaugeDemoActive, dashFile]);

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

  // Load available dashboards
  useEffect(() => {
    const loadAvailableDashes = async () => {
      try {
        const dashes = await invoke<DashFileInfo[]>('list_available_dashes');
        setAvailableDashes(dashes);
        
        // If no initial path, select first available
        if (!selectedPath && dashes.length > 0) {
          // Prefer Basic.ltdash.xml as the default
          const basicDash = dashes.find(d => d.name === 'Basic.ltdash.xml');
          setSelectedPath(basicDash?.path || dashes[0].path);
        }
      } catch (e) {
        console.error('Failed to load available dashes:', e);
      }
    };
    loadAvailableDashes();
  }, []);

  // Load selected dashboard
  useEffect(() => {
    const loadDashboard = async () => {
      if (!selectedPath) {
        setLoading(false);
        return;
      }

      setLoading(true);
      setError(null);

      try {
        console.log('[TsDashboard] Loading:', selectedPath);
        const file = await invoke<DashFile>('get_dash_file', { path: selectedPath });
        console.log('[TsDashboard] Loaded:', file.gauge_cluster.components.length, 'components');
        setDashFile(file);
      } catch (e) {
        console.error('Failed to load dashboard:', e);
        setError(String(e));
      } finally {
        setLoading(false);
      }
    };

    loadDashboard();
  }, [selectedPath]);

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

  return (
    <div className="ts-dashboard-container">
      {/* Header with dashboard selector */}
      <div className="ts-dashboard-header">
        <span className="ts-dashboard-title">
          {dashFile.bibliography.author} Dashboard
        </span>
        <button 
          className="ts-dashboard-selector-btn"
          onClick={() => setShowSelector(!showSelector)}
        >
          Change Dashboard ▼
        </button>
      </div>

      {/* Dashboard selector dropdown */}
      {showSelector && (
        <div className="ts-dashboard-selector-overlay" onClick={() => setShowSelector(false)}>
          <div className="ts-dashboard-selector" onClick={e => e.stopPropagation()}>
            <h3>Select Dashboard</h3>
            <div className="ts-dashboard-list">
              {availableDashes.map((dash) => (
                <button
                  key={dash.path}
                  className={`ts-dashboard-option ${dash.path === selectedPath ? 'selected' : ''}`}
                  onClick={() => handleDashSelect(dash.path)}
                >
                  {dash.name}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Dashboard canvas */}
      <div 
        className={`ts-dashboard ${designerMode ? 'designer-mode' : ''}`}
        style={{
          backgroundColor: bgColor,
          backgroundImage: bgImageUrl ? `url(${bgImageUrl})` : undefined,
          backgroundSize: cluster.cluster_background_image_style === 'Stretch' ? 'cover' 
                        : cluster.cluster_background_image_style === 'Fit' ? 'contain'
                        : cluster.cluster_background_image_style === 'Center' ? 'auto'
                        : undefined,
          backgroundRepeat: cluster.cluster_background_image_style === 'Tile' ? 'repeat' : 'no-repeat',
          backgroundPosition: 'center',
        }}
        onContextMenu={(e) => handleContextMenu(e, null)}
      >
        {cluster.components.map((component, index) => {
          // Helper to clamp relative positions to 0-1 range
          const clampPercent = (v: number | undefined | null) => 
            Math.max(0, Math.min(1, v ?? 0)) * 100;

          if (isGauge(component)) {
            const gauge = component.Gauge;
            // Use demo values if demo mode is active, otherwise realtime data
            const value = gaugeDemoActive 
              ? (demoValues[gauge.output_channel] ?? gauge.value)
              : (realtimeData[gauge.output_channel] ?? gauge.value);
            
            return (
              <div
                key={gauge.id || `gauge-${index}`}
                className={`ts-component ts-gauge ${designerMode ? 'editable' : ''}`}
                style={{
                  left: `${clampPercent(gauge.relative_x)}%`,
                  top: `${clampPercent(gauge.relative_y)}%`,
                  width: `${clampPercent(gauge.relative_width)}%`,
                  height: `${clampPercent(gauge.relative_height)}%`,
                }}
                onContextMenu={(e) => handleContextMenu(e, gauge.id)}
              >
                <TsGauge 
                  config={gauge}
                  value={value}
                  embeddedImages={embeddedImages}
                />
              </div>
            );
          }

          if (isIndicator(component)) {
            const indicator = component.Indicator;
            const value = realtimeData[indicator.output_channel] ?? indicator.value;
            const isOn = value !== 0;
            
            return (
              <div
                key={indicator.id || `indicator-${index}`}
                className={`ts-component ts-indicator ${designerMode ? 'editable' : ''}`}
                style={{
                  left: `${clampPercent(indicator.relative_x)}%`,
                  top: `${clampPercent(indicator.relative_y)}%`,
                  width: `${clampPercent(indicator.relative_width)}%`,
                  height: `${clampPercent(indicator.relative_height)}%`,
                }}
                onContextMenu={(e) => handleContextMenu(e, indicator.id)}
              >
                <TsIndicator
                  config={indicator}
                  isOn={isOn}
                  embeddedImages={embeddedImages}
                />
              </div>
            );
          }

          return null;
        })}
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
          // Reset value for the targeted gauge (if needed)
          console.log('Reset value for gauge:', contextMenu.targetGaugeId);
        }}
        onReplaceGauge={(channel, gaugeInfo) => {
          // Replace the targeted gauge with a new one
          console.log('Replace gauge with:', channel, gaugeInfo);
        }}
      />
    </div>
  );
}
