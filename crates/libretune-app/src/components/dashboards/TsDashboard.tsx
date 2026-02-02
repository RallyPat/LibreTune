/**
 * TsDashboard - TunerStudio-compatible dashboard renderer.
 * 
 * Renders a complete dashboard from a DashFile structure, supporting all
 * TunerStudio gauge types, indicators, embedded images, and proper positioning.
 * 
 * Features:
 * - 13 gauge types (Analog, Digital, Bar, Sweep, Line Graph, etc.)
 * - Boolean indicators (LED, image-based)
 * - Gauge sweep animation on load (sportscar-style)
 * - Designer mode for layout editing
 * - Context menu for gauge configuration
 * - Dashboard selector with categories
 * - Import from TunerStudio .dash files
 * - Responsive scaling for different screen sizes
 * - Realtime data from Zustand store (per-channel subscription for efficiency)
 * 
 * @example
 * ```tsx
 * <TsDashboard
 *   isConnected={connectionStatus.state === 'Connected'}
 * />
 * ```
 */

import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useChannels } from '../../stores/realtimeStore';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { save } from '@tauri-apps/plugin-dialog';
import { useRealtimeStore } from '../../stores/realtimeStore';
import {
  DashFile,
  DashFileInfo,
  TsGaugeConfig,
  isGauge,
  isIndicator,
  buildEmbeddedImageMap,
  tsColorToRgba,
} from './dashTypes';
import { createLibreTuneDefaultDashboard } from './LibreTuneDefaultDashboard';
import TsGauge from '../gauges/TsGauge';
import TsIndicator from '../gauges/TsIndicator';
import GaugeContextMenu, { ContextMenuState } from './GaugeContextMenu';
import ImportDashboardDialog from '../dialogs/ImportDashboardDialog';
import DashboardDesigner from './DashboardDesigner';
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
  const [rpmChannel, setRpmChannel] = useState<string | null>(null);
  const [channelInfoMap, setChannelInfoMap] = useState<Record<string, ChannelInfo>>({});

  // Subscribe only to RPM channel value to avoid re-rendering on unrelated channel updates
  useRealtimeStore((state) => (rpmChannel ? state.channels[rpmChannel] : undefined));
  // For cases where we need an instant snapshot (on dashboard load), use the store getter
  const realtimeStoreGet = useRealtimeStore.getState;
  
  // Gauge sweep animation state (sportscar-style min→max→min on load)
  const [sweepActive, setSweepActive] = useState(false);
  const [sweepValues, setSweepValues] = useState<Record<string, number>>({});

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
  
  // Dynamic scaling state for responsive dashboard sizing
  const [scale, setScale] = useState(1);
  const dashboardWrapperRef = useRef<HTMLDivElement>(null);
  const initialSyncDoneRef = useRef(false);

  // Build embedded images map
  const embeddedImages = dashFile 
    ? buildEmbeddedImageMap(dashFile.gauge_cluster.embedded_images)
    : new Map<string, string>();

  // Determine which channels this dashboard uses (subscribe only to these)
  const usedChannels = useMemo(() => {
    if (!dashFile) return [] as string[];
    const set = new Set<string>();
    dashFile.gauge_cluster.components.forEach((comp) => {
      if (isGauge(comp)) {
        const g = comp.Gauge;
        if (g.output_channel) set.add(g.output_channel);
      }
    });
    return Array.from(set);
  }, [dashFile]);

  const channelValues = useChannels(usedChannels);

  // Resolve RPM channel from INI output channels (INI-driven, no hardcoded names)
  useEffect(() => {
    const loadChannels = async () => {
      try {
        const channels = await invoke<ChannelInfo[]>('get_available_channels');
        const map: Record<string, ChannelInfo> = {};
        channels.forEach((ch) => {
          map[ch.name] = ch;
        });
        setChannelInfoMap(map);
        const rpmByUnits = channels.find((ch) => ch.units?.toLowerCase() === 'rpm');
        const rpmByLabel = channels.find((ch) => ch.label?.toLowerCase().includes('rpm'));
        const resolved = rpmByUnits?.name || rpmByLabel?.name || null;
        setRpmChannel(resolved);
      } catch (e) {
        console.warn('[TsDashboard] Failed to load available channels:', e);
        setRpmChannel(null);
        setChannelInfoMap({});
      }
    };
    loadChannels();
  }, []);

  // Calculate dashboard aspect ratio from gauge bounding box
  // Find the maximum extent of all gauges to determine the design's aspect ratio
  // NOTE: This must be before early returns to comply with React Rules of Hooks
  const dashboardBounds = useMemo(() => {
    if (!dashFile) {
      return { maxX: 1.0, maxY: 1.0, aspectRatio: 1.0, minSize: 50 };
    }
    
    const components = dashFile.gauge_cluster.components;
    let maxX = 0;
    let maxY = 0;
    let minShortestSize = Infinity;
    
    components.forEach((comp) => {
      if (isGauge(comp)) {
        const g = comp.Gauge;
        maxX = Math.max(maxX, (g.relative_x ?? 0) + (g.relative_width ?? 0.25));
        maxY = Math.max(maxY, (g.relative_y ?? 0) + (g.relative_height ?? 0.25));
        if (g.shortest_size > 0) {
          minShortestSize = Math.min(minShortestSize, g.shortest_size);
        }
      } else if (isIndicator(comp)) {
        const i = comp.Indicator;
        maxX = Math.max(maxX, (i.relative_x ?? 0) + (i.relative_width ?? 0.1));
        maxY = Math.max(maxY, (i.relative_y ?? 0) + (i.relative_height ?? 0.05));
      }
    });
    
    // Clamp to reasonable bounds (at least 1.0 to cover the full area)
    maxX = Math.max(1.0, maxX);
    maxY = Math.max(1.0, maxY);
    
    const forceAspect = dashFile.gauge_cluster.force_aspect
      && dashFile.gauge_cluster.force_aspect_width > 0
      && dashFile.gauge_cluster.force_aspect_height > 0;
    const forcedRatio = forceAspect
      ? dashFile.gauge_cluster.force_aspect_width / dashFile.gauge_cluster.force_aspect_height
      : null;

    // Aspect ratio is width / height (use forced aspect for legacy TunerStudio dashboards)
    const aspectRatio = forcedRatio ?? (maxX / maxY);
    
    // Minimum dashboard size based on smallest gauge requirement
    const minSize = minShortestSize === Infinity ? 50 : minShortestSize;
    
    return { maxX, maxY, aspectRatio, minSize };
  }, [dashFile]);

  const isLegacyPath = useMemo(() => {
    const lower = (selectedPath ?? '').toLowerCase();
    return lower.endsWith('.dash') || lower.endsWith('.gauge');
  }, [selectedPath]);

  const compatibilityReport = useMemo(() => {
    if (!dashFile) return null;

    const supportedGaugePainters = new Set([
      'AnalogGauge',
      'BasicAnalogGauge',
      'CircleAnalogGauge',
      'AsymmetricSweepGauge',
      'BasicReadout',
      'HorizontalBarGauge',
      'HorizontalDashedBar',
      'VerticalBarGauge',
      'HorizontalLineGauge',
      'VerticalDashedBar',
      'AnalogBarGauge',
      'AnalogMovingBarGauge',
      'Histogram',
      'LineGraph',
      'RoundGauge',
      'RoundDashedGauge',
      'FuelMeter',
      'Tachometer',
    ]);
    const supportedIndicatorPainters = new Set([
      'BasicRectangleIndicator',
      'BulbIndicator',
    ]);

    const gaugePainters: Record<string, number> = {};
    const indicatorPainters: Record<string, number> = {};
    const unsupportedGaugePainters = new Set<string>();
    const unsupportedIndicatorPainters = new Set<string>();

    let gauges = 0;
    let indicators = 0;

    dashFile.gauge_cluster.components.forEach((comp) => {
      if (isGauge(comp)) {
        gauges += 1;
        const painter = comp.Gauge.gauge_painter || 'BasicReadout';
        gaugePainters[painter] = (gaugePainters[painter] || 0) + 1;
        if (!supportedGaugePainters.has(painter)) {
          unsupportedGaugePainters.add(painter);
        }
      } else if (isIndicator(comp)) {
        indicators += 1;
        const painter = comp.Indicator.indicator_painter || 'BasicRectangleIndicator';
        indicatorPainters[painter] = (indicatorPainters[painter] || 0) + 1;
        if (!supportedIndicatorPainters.has(painter)) {
          unsupportedIndicatorPainters.add(painter);
        }
      }
    });

    return {
      total_components: dashFile.gauge_cluster.components.length,
      gauges,
      indicators,
      gauge_painters: gaugePainters,
      indicator_painters: indicatorPainters,
      unsupported_gauge_painters: Array.from(unsupportedGaugePainters),
      unsupported_indicator_painters: Array.from(unsupportedIndicatorPainters),
    };
  }, [dashFile]);

  const hasCompatibilityIssues = useMemo(() => {
    if (!compatibilityReport) return false;
    return (
      compatibilityReport.unsupported_gauge_painters.length > 0 ||
      compatibilityReport.unsupported_indicator_painters.length > 0
    );
  }, [compatibilityReport]);



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

  // Sportscar-style gauge sweep animation (min → max → min)
  const sweepActiveRef = useRef(false);
  const sweepAnimRef = useRef<number | null>(null);

  const startGaugeSweep = useCallback((file: DashFile) => {
    // Guard against overlapping sweeps
    if (sweepActiveRef.current) return;
    sweepActiveRef.current = true;
    setSweepActive(true);

    // Cancel any previous animation frame (cleanup)
    if (sweepAnimRef.current !== null) {
      cancelAnimationFrame(sweepAnimRef.current);
      sweepAnimRef.current = null;
    }

    const duration = 1500; // 1.5 seconds total
    const startTime = performance.now();

    // Easing function: ease-in-out for smooth acceleration/deceleration
    const easeInOut = (t: number) => t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;

    const animate = (currentTime: number) => {
      const elapsed = currentTime - startTime;
      const rawProgress = Math.min(elapsed / duration, 1);

      // Convert to sweep position: 0→1 (rising) then 1→0 (falling)
      // 0-0.5 progress = sweep up (0→1), 0.5-1 progress = sweep down (1→0)
      const sweepPosition = rawProgress < 0.5 
        ? easeInOut(rawProgress * 2) // 0→1 with easing
        : easeInOut(1 - (rawProgress - 0.5) * 2); // 1→0 with easing

      const newValues: Record<string, number> = {};

      file.gauge_cluster.components.forEach((comp) => {
        if (isGauge(comp)) {
          const gauge = comp.Gauge;
          const range = gauge.max - gauge.min;
          // Interpolate from min to max based on sweep position
          const value = gauge.min + range * sweepPosition;
          newValues[gauge.output_channel] = value;
        }
      });

      setSweepValues(newValues);

      if (rawProgress < 1) {
        sweepAnimRef.current = requestAnimationFrame(animate);
      } else {
        // Animation complete
        sweepAnimRef.current = null;
        sweepActiveRef.current = false;
        setSweepActive(false);
        setSweepValues({});
      }
    };

    sweepAnimRef.current = requestAnimationFrame(animate);
  }, []);

  // Cleanup any running animation on unmount
  useEffect(() => {
    return () => {
      if (sweepAnimRef.current !== null) {
        cancelAnimationFrame(sweepAnimRef.current);
        sweepAnimRef.current = null;
      }
      sweepActiveRef.current = false;
    };
  }, []);

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
  const refreshDashboardList = useCallback(async () => {
    try {
      let dashes = await invoke<DashFileInfo[]>('list_available_dashes');
      if (!dashes || dashes.length === 0) {
        dashes = [{
          name: 'LibreTune Default',
          path: '__libretune_default__',
          category: 'Bundled',
        }];
      }
      setAvailableDashes(dashes);
      return dashes;
    } catch (e) {
      setAvailableDashes([
        {
          name: 'LibreTune Default',
          path: '__libretune_default__',
          category: 'Bundled',
        },
      ]);
      return [
        {
          name: 'LibreTune Default',
          path: '__libretune_default__',
          category: 'Bundled',
        },
      ];
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

  const computeScale = useCallback(() => {
    const wrapper = dashboardWrapperRef.current;
    if (!wrapper) return;

    const { width: containerWidth, height: containerHeight } = wrapper.getBoundingClientRect();

    // Calculate minimum size needed based on aspect ratio and minimum gauge sizes
    // Assume dashboards need at least 600px width at scale 1.0 for readability
    const minDashWidth = 600;
    const minDashHeight = minDashWidth / Math.max(0.1, dashboardBounds.aspectRatio);

    // Calculate scale factor based on container size
    const scaleX = containerWidth / minDashWidth;
    const scaleY = containerHeight / minDashHeight;
    const newScale = Math.min(1, Math.min(scaleX, scaleY));

    setScale(Math.max(0.5, newScale)); // Minimum 50% scale
  }, [dashboardBounds.aspectRatio]);

  // Dynamic scaling - scale dashboard down when viewport is too small
  useEffect(() => {
    if (!dashboardWrapperRef.current) return;

    const wrapper = dashboardWrapperRef.current;

    const resizeObserver = new ResizeObserver(() => {
      computeScale();
    });

    resizeObserver.observe(wrapper);
    computeScale();
    return () => resizeObserver.disconnect();
  }, [computeScale]);

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
        let file: DashFile;
        if (selectedPath === '__libretune_default__') {
          file = createLibreTuneDefaultDashboard();
        } else {
          file = await invoke<DashFile>('get_dash_file', { path: selectedPath });
        }
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

  // On dashboard file load, decide whether to run the initial sweep using a snapshot of realtime data
  useEffect(() => {
    if (!dashFile) return;

    // Snapshot current RPM value (no subscription) to avoid re-triggering on rapid realtime updates
    const rpmSnapshot = rpmChannel ? realtimeStoreGet().channels[rpmChannel] : undefined;
    const isEngineRunning = typeof rpmSnapshot === 'number' && rpmSnapshot > 50;

    if (!isConnected || !isEngineRunning) {
      startGaugeSweep(dashFile);
    }
  }, [dashFile, rpmChannel, isConnected]);

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
            ➕ New
          </button>
          <button 
            className="ts-dashboard-action-btn"
            onClick={handleDuplicateDashboard}
            title="Duplicate Dashboard"
          >
            📋 Duplicate
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
            ✏️ Rename
          </button>
          <button 
            className="ts-dashboard-action-btn danger"
            onClick={() => setShowDeleteConfirm(true)}
            title="Delete Dashboard"
          >
            🗑️ Delete
          </button>
          <button 
            className="ts-dashboard-action-btn"
            onClick={handleExportDashboard}
            title="Export Dashboard"
          >
            💾 Export
          </button>
          <button 
            className="ts-dashboard-action-btn"
            onClick={handleSyncGaugeRanges}
            title="Sync gauge ranges from INI"
          >
            🔄 Sync Ranges
          </button>
          <button
            className={`ts-dashboard-action-btn ${legacyMode ? 'active' : ''}`}
            onClick={() => setLegacyMode(prev => !prev)}
            title={legacyMode ? 'Legacy TS layout enabled' : 'Enable legacy TS layout'}
          >
            🧭 Legacy: {legacyMode ? 'On' : 'Off'}
          </button>
        </div>
      </div>

      {compatibilityReport && compatBarVisible && hasCompatibilityIssues && (
        <div className={`ts-dashboard-compat warn`}>
          <span>
            Compatibility: some features not yet supported
          </span>
          <button
            className="ts-dashboard-compat-close"
            onClick={() => setCompatBarVisible(false)}
            title="Dismiss"
          >
            ✕
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
                📁 Import TS Dashboard Files...
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
      {showNewDialog && (
        <div className="ts-dashboard-dialog-overlay" onClick={() => setShowNewDialog(false)}>
          <div className="ts-dashboard-dialog" onClick={e => e.stopPropagation()}>
            <h3>New Dashboard</h3>
            <div className="ts-dashboard-dialog-content">
              <label>Dashboard Name:</label>
              <input
                type="text"
                value={newDashName}
                onChange={(e) => setNewDashName(e.target.value)}
                placeholder="My Dashboard"
                autoFocus
                onKeyDown={(e) => e.key === 'Enter' && handleNewDashboard()}
              />
            </div>
            <div className="ts-dashboard-dialog-buttons">
              <button onClick={() => setShowNewDialog(false)}>Cancel</button>
              <button 
                className="primary" 
                onClick={handleNewDashboard}
                disabled={!newDashName.trim()}
              >
                Create
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Rename Dashboard Dialog */}
      {showRenameDialog && (
        <div className="ts-dashboard-dialog-overlay" onClick={() => setShowRenameDialog(false)}>
          <div className="ts-dashboard-dialog" onClick={e => e.stopPropagation()}>
            <h3>Rename Dashboard</h3>
            <div className="ts-dashboard-dialog-content">
              <label>New Name:</label>
              <input
                type="text"
                value={renameName}
                onChange={(e) => setRenameName(e.target.value)}
                placeholder="Dashboard Name"
                autoFocus
                onKeyDown={(e) => e.key === 'Enter' && handleRenameDashboard()}
              />
            </div>
            <div className="ts-dashboard-dialog-buttons">
              <button onClick={() => setShowRenameDialog(false)}>Cancel</button>
              <button 
                className="primary" 
                onClick={handleRenameDashboard}
                disabled={!renameName.trim()}
              >
                Rename
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Delete Confirmation Dialog */}
      {showDeleteConfirm && (
        <div className="ts-dashboard-dialog-overlay" onClick={() => setShowDeleteConfirm(false)}>
          <div className="ts-dashboard-dialog" onClick={e => e.stopPropagation()}>
            <h3>Delete Dashboard?</h3>
            <div className="ts-dashboard-dialog-content">
              <p>Are you sure you want to delete "{selectedPath.split('/').pop()?.replace(/\.(ltdash\.xml|dash)$/i, '')}"?</p>
              <p className="warning">This action cannot be undone.</p>
            </div>
            <div className="ts-dashboard-dialog-buttons">
              <button onClick={() => setShowDeleteConfirm(false)}>Cancel</button>
              <button className="danger" onClick={handleDeleteDashboard}>Delete</button>
            </div>
          </div>
        </div>
      )}

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
            const defaultGauge: TsGaugeConfig = {
              id: `gauge_${Date.now()}`,
              gauge_painter: 'BasicReadout',
              gauge_style: '',
              output_channel: channel.id,
              title: label || channel.label,
              units: units,
              value: 0,
              min: 0,
              max: 100,
              min_vp: null,
              max_vp: null,
              default_min: null,
              default_max: null,
              peg_limits: false,
              low_warning: null,
              high_warning: null,
              low_critical: null,
              high_critical: null,
              low_warning_vp: null,
              high_warning_vp: null,
              low_critical_vp: null,
              high_critical_vp: null,
              back_color: { alpha: 0, red: 40, green: 40, blue: 40 },
              font_color: { alpha: 0, red: 255, green: 255, blue: 255 },
              trim_color: { alpha: 0, red: 100, green: 100, blue: 100 },
              warn_color: { alpha: 0, red: 255, green: 165, blue: 0 },
              critical_color: { alpha: 0, red: 255, green: 0, blue: 0 },
              needle_color: { alpha: 0, red: 200, green: 200, blue: 200 },
              value_digits: 2,
              label_digits: 0,
              font_family: 'Arial',
              font_size_adjustment: 0,
              italic_font: false,
              sweep_angle: 270,
              start_angle: 225,
              face_angle: 0,
              sweep_begin_degree: 0,
              counter_clockwise: false,
              major_ticks: 5,
              minor_ticks: 0,
              relative_x: relX - 0.1,
              relative_y: relY - 0.1,
              relative_width: 0.2,
              relative_height: 0.2,
              border_width: 1,
              shortest_size: 0,
              shape_locked_to_aspect: false,
              antialiasing_on: true,
              background_image_file_name: null,
              needle_image_file_name: null,
              show_history: false,
              history_value: 0,
              history_delay: 0,
              needle_smoothing: 0,
              short_click_action: null,
              long_click_action: null,
              display_value_at_180: false,
            };

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
            // Priority: sweep animation > demo mode > realtime data > default
            const hasChannel = !!channelInfoMap[gauge.output_channel];
            const value = sweepActive
              ? (sweepValues[gauge.output_channel] ?? gauge.min)
              : gaugeDemoActive 
                ? (demoValues[gauge.output_channel] ?? gauge.value)
                : (hasChannel ? (channelValues[gauge.output_channel] ?? gauge.value) : gauge.value);
            
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
                />
              </div>
            );
          }

          if (isIndicator(component)) {
            const indicator = component.Indicator;
            const hasChannel = !!channelInfoMap[indicator.output_channel];
            // Use the subscribed channel values map instead of an undefined variable
            const value = hasChannel ? (channelValues[indicator.output_channel] ?? indicator.value) : indicator.value;
            const isOn = value !== 0;
            
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
