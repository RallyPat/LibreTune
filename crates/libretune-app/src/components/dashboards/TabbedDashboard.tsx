import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Plus,
  XCircle,
  Settings,
  Trash2,
  LayoutDashboard,
  FileDown,
} from 'lucide-react';
import './TabbedDashboard.css';
import GaugeRenderer, { GaugeConfig, GaugeType } from '../gauges/GaugeRenderer';
import IndicatorRow, { FrontPageIndicator } from './IndicatorRow';

export interface DashboardLayout {
  name: string;
  gauges: GaugeConfig[];
  is_fullscreen: boolean;
  background_image?: string;
}

interface FrontPageInfo {
  gauges: string[];
  indicators: FrontPageIndicator[];
}

// Backend gauge configuration from INI [GaugeConfigurations]
interface BackendGaugeInfo {
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

// Dashboard template info from backend
interface DashboardTemplateInfo {
  id: string;
  name: string;
  description: string;
}

interface TabbedDashboardProps {
  onClose?: () => void;
  realtimeData?: Record<string, number>;
  constantValues?: Record<string, number>;
  indicatorColumnCount?: number | 'auto';
  indicatorFillEmpty?: boolean;
  indicatorTextFit?: 'scale' | 'wrap';
}

export default function TabbedDashboard({ onClose: _onClose, realtimeData = {}, constantValues = {}, indicatorColumnCount = 'auto', indicatorFillEmpty = false, indicatorTextFit = 'scale' }: TabbedDashboardProps) {
  const [dashboards, setDashboards] = useState<DashboardLayout[]>([]);
  const [currentDashboard, setCurrentDashboard] = useState<number>(0);
  const [isDesignerMode, setIsDesignerMode] = useState(false);
  const [selectedGauge, setSelectedGauge] = useState<string | null>(null);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [frontpageIndicators, setFrontpageIndicators] = useState<FrontPageIndicator[]>([]);
  const [gaugeTypes] = useState<GaugeType[]>([
    GaugeType.AnalogDial,
    GaugeType.DigitalReadout,
    GaugeType.BarGauge,
    GaugeType.SweepGauge,
    GaugeType.LEDIndicator,
    GaugeType.WarningLight,
  ]);
  const [showGaugeSelector, setShowGaugeSelector] = useState(false);
  const [showTemplateSelector, setShowTemplateSelector] = useState(false);
  const [showSwitchDashboard, setShowSwitchDashboard] = useState(false);
  const [savedDashboardNames, _setSavedDashboardNames] = useState<string[]>([]);
  const [templates, setTemplates] = useState<DashboardTemplateInfo[]>([]);

  const gaugeTypeMap: Record<GaugeType, { icon: string; name: string }> = {
    [GaugeType.AnalogDial]: { icon: '🎚', name: 'Analog Dial' },
    [GaugeType.DigitalReadout]: { icon: '📊', name: 'Digital Readout' },
    [GaugeType.BarGauge]: { icon: '▮', name: 'Bar Gauge' },
    [GaugeType.SweepGauge]: { icon: '◑', name: 'Sweep Gauge' },
    [GaugeType.LEDIndicator]: { icon: '🔴', name: 'LED Indicator' },
    [GaugeType.WarningLight]: { icon: '⚠', name: 'Warning Light' },
  };

  // Default gauge type mapping based on channel/title patterns
  // This maps channel name patterns to preferred gauge types
  const GAUGE_TYPE_DEFAULTS: Record<string, GaugeType> = {
    // RPM - always analog dial (primary instrument)
    'rpm': GaugeType.AnalogDial,
    'rpmvalue': GaugeType.AnalogDial,
    // Temperatures - vertical bar gauges (thermometer-like)
    'coolant': GaugeType.BarGauge,
    'clt': GaugeType.BarGauge,
    'iat': GaugeType.BarGauge,
    'intake': GaugeType.BarGauge,
    'oiltemp': GaugeType.BarGauge,
    'fueltemp': GaugeType.BarGauge,
    // TPS - horizontal bar (pedal travel metaphor)
    'tps': GaugeType.BarGauge,
    'tpsvalue': GaugeType.BarGauge,
    'throttle': GaugeType.BarGauge,
    // AFR/Lambda - digital for precision
    'afr': GaugeType.DigitalReadout,
    'afrvalue': GaugeType.DigitalReadout,
    'lambda': GaugeType.DigitalReadout,
    'lambdavalue': GaugeType.DigitalReadout,
    // Voltage - digital for precision
    'vbatt': GaugeType.DigitalReadout,
    'battery': GaugeType.DigitalReadout,
    // MAP - sweep gauge (boost/vacuum visualization)
    'map': GaugeType.SweepGauge,
    'mapvalue': GaugeType.SweepGauge,
    'manifold': GaugeType.SweepGauge,
    // Timing/Dwell - digital for precision
    'advance': GaugeType.DigitalReadout,
    'timing': GaugeType.DigitalReadout,
    'dwell': GaugeType.DigitalReadout,
    'ignadvance': GaugeType.DigitalReadout,
    // Duty cycles - bar gauges
    'duty': GaugeType.BarGauge,
    'pwm': GaugeType.BarGauge,
    've': GaugeType.BarGauge,
    // Default for percentage values
    'percent': GaugeType.BarGauge,
  };

  // Determine the preferred gauge type based on channel name and title
  const getPreferredGaugeType = (channel: string, title: string): GaugeType => {
    const channelLower = channel.toLowerCase();
    const titleLower = title.toLowerCase();
    
    // Check channel name first (most specific)
    for (const [pattern, gaugeType] of Object.entries(GAUGE_TYPE_DEFAULTS)) {
      if (channelLower.includes(pattern)) {
        return gaugeType;
      }
    }
    
    // Check title for keywords
    if (titleLower.includes('rpm') || titleLower.includes('engine speed')) {
      return GaugeType.AnalogDial;
    }
    if (titleLower.includes('temp') || titleLower.includes('temperature')) {
      return GaugeType.BarGauge;
    }
    if (titleLower.includes('throttle') || titleLower.includes('tps')) {
      return GaugeType.BarGauge;
    }
    if (titleLower.includes('afr') || titleLower.includes('lambda') || titleLower.includes('air/fuel')) {
      return GaugeType.DigitalReadout;
    }
    if (titleLower.includes('voltage') || titleLower.includes('battery')) {
      return GaugeType.DigitalReadout;
    }
    if (titleLower.includes('timing') || titleLower.includes('advance') || titleLower.includes('dwell')) {
      return GaugeType.DigitalReadout;
    }
    if (titleLower.includes('map') || titleLower.includes('manifold') || titleLower.includes('boost')) {
      return GaugeType.SweepGauge;
    }
    if (titleLower.includes('duty') || titleLower.includes('%') || titleLower.includes('percent')) {
      return GaugeType.BarGauge;
    }
    
    // Default fallback
    return GaugeType.DigitalReadout;
  };

  // Default layout positions for 8 gauges (4x2 grid matching TunerStudio FrontPage)
  const DEFAULT_GAUGE_POSITIONS = [
    // Row 1: gauge1-4
    { x: 0.01, y: 0.01, width: 0.24, height: 0.48 },  // gauge1 - top-left
    { x: 0.26, y: 0.01, width: 0.24, height: 0.48 },  // gauge2
    { x: 0.51, y: 0.01, width: 0.24, height: 0.48 },  // gauge3
    { x: 0.76, y: 0.01, width: 0.23, height: 0.48 },  // gauge4 - top-right
    // Row 2: gauge5-8
    { x: 0.01, y: 0.50, width: 0.24, height: 0.48 },  // gauge5 - bottom-left
    { x: 0.26, y: 0.50, width: 0.24, height: 0.48 },  // gauge6
    { x: 0.51, y: 0.50, width: 0.24, height: 0.48 },  // gauge7
    { x: 0.76, y: 0.50, width: 0.23, height: 0.48 },  // gauge8 - bottom-right
  ];

  // Build a gauge config from backend gauge info
  // Uses smart gauge type detection and default layout positions
  // @ts-expect-error - Reserved for future use when loading INI-based dashboards
  const _buildGaugeConfig = (
    info: BackendGaugeInfo, 
    index: number, 
    overrideGaugeType?: GaugeType
  ): GaugeConfig => {
    // Get position from default layout (or calculate if beyond 8 gauges)
    const pos = index < DEFAULT_GAUGE_POSITIONS.length 
      ? DEFAULT_GAUGE_POSITIONS[index]
      : {
          x: 0.02 + (index % 4) * 0.245,
          y: 0.02 + Math.floor(index / 4) * 0.48,
          width: 0.23,
          height: 0.45,
        };
    
    // Determine gauge type: use override, or smart detection
    const gaugeType = overrideGaugeType ?? getPreferredGaugeType(info.channel, info.title);
    
    return {
      id: `gauge_${index + 1}`,
      gauge_type: gaugeType,
      channel: info.channel,
      label: info.title || info.name,
      x: pos.x,
      y: pos.y,
      width: pos.width,
      height: pos.height,
      z_index: index,
      min_value: info.lo,
      max_value: info.hi,
      low_warning: info.low_warning,
      high_warning: info.high_warning,
      high_critical: info.high_danger,
      decimals: info.digits,
      units: info.units,
      font_color: '#FFFFFF',
      needle_color: '#FF6600',
      trim_color: '#666999',
      show_history: false,
      show_min_max: gaugeType === GaugeType.DigitalReadout, // Show min/max on digital gauges
    };
  };

  // Create a minimal fallback dashboard when no INI data available
  const createFallbackDashboard = (): DashboardLayout => ({
    name: 'Default Dashboard',
    gauges: [{
      id: 'gauge_1',
      gauge_type: GaugeType.DigitalReadout,
      channel: 'rpm',
      label: 'RPM',
      x: 0.35,
      y: 0.35,
      width: 0.30,
      height: 0.30,
      z_index: 0,
      min_value: 0,
      max_value: 8000,
      decimals: 0,
      units: 'RPM',
      font_color: '#FFFFFF',
      needle_color: '#FF6600',
      trim_color: '#666999',
      show_history: false,
      show_min_max: false,
    }],
    is_fullscreen: false,
  });

  // Load saved dashboards or create from template
  useEffect(() => {
    const loadDashboards = async () => {
      try {
        // First, load FrontPage indicators (these are shown regardless of dashboard)
        try {
          const frontpage = await invoke<FrontPageInfo | null>('get_frontpage');
          if (frontpage?.indicators) {
            setFrontpageIndicators(frontpage.indicators);
          }
        } catch {
          // Ignore indicator loading errors
        }

        // FOR TESTING: Load TunerStudio Speeduino_LED.dash directly
        try {
          console.log('[TabbedDashboard] Loading TunerStudio Speeduino_LED.dash for testing...');
          const layout = await invoke<DashboardLayout>('load_tunerstudio_dash', {
            path: '/home/pat/codingprojects/libretune/reference/TunerStudioMS/Dash/Speeduino_LED.dash'
          });
          console.log('[TabbedDashboard] Loaded TunerStudio dash with', layout.gauges?.length, 'gauges');
          if (layout && layout.gauges && layout.gauges.length > 0) {
            setDashboards([layout]);
            setCurrentDashboard(0);
            return;
          }
        } catch (e) {
          console.error('Failed to load TunerStudio dash:', e);
        }

        // Fallback to minimal dashboard
        console.log('[TabbedDashboard] Falling back to minimal dashboard');
        setDashboards([createFallbackDashboard()]);
        setCurrentDashboard(0);
      } catch (e) {
        console.error('Failed to load dashboards:', e);
        setDashboards([createFallbackDashboard()]);
        setCurrentDashboard(0);
      }
    };
    
    loadDashboards();
  }, []);

  // Load available templates
  useEffect(() => {
    const loadTemplates = async () => {
      try {
        const templateList = await invoke<DashboardTemplateInfo[]>('get_dashboard_templates');
        setTemplates(templateList);
      } catch (e) {
        console.error('Failed to load templates:', e);
      }
    };
    loadTemplates();
  }, []);

  // Create dashboard from template
  const handleCreateFromTemplate = async (templateId: string) => {
    try {
      const projectName = `template_${templateId}_${Date.now()}`;
      const layout = await invoke<DashboardLayout>('create_default_dashboard', { 
        project_name: projectName, 
        template: templateId 
      });
      setDashboards([...dashboards, layout]);
      setCurrentDashboard(dashboards.length);
      setShowTemplateSelector(false);
    } catch (e) {
      console.error('Failed to create dashboard from template:', e);
      alert(`Failed to create dashboard: ${e}`);
    }
  };

  // Switch to a different dashboard (replace current)
  const handleSwitchToDashboard = async (dashboardName: string) => {
    try {
      const layout = await invoke<DashboardLayout>('load_dashboard_layout', { 
        project_name: 'default', 
        dashboard_name: dashboardName 
      });
      // Replace current dashboard with the loaded one
      const newDashboards = [...dashboards];
      newDashboards[currentDashboard] = layout;
      setDashboards(newDashboards);
      setShowSwitchDashboard(false);
    } catch (e) {
      console.error('Failed to load dashboard:', e);
      alert(`Failed to load dashboard: ${e}`);
    }
  };

  // Switch to a template (replace current)
  const handleSwitchToTemplate = async (templateId: string) => {
    try {
      const layout = await invoke<DashboardLayout>('create_default_dashboard', { 
        project_name: 'default', 
        template: templateId 
      });
      // Replace current dashboard with the template
      const newDashboards = [...dashboards];
      newDashboards[currentDashboard] = layout;
      setDashboards(newDashboards);
      setShowSwitchDashboard(false);
    } catch (e) {
      console.error('Failed to create dashboard from template:', e);
      alert(`Failed to switch dashboard: ${e}`);
    }
  };

  // Load a TunerStudio .dash file directly (for testing)
  const handleLoadTunerStudioDash = async () => {
    try {
      const layout = await invoke<DashboardLayout>('load_tunerstudio_dash', {
        path: '/home/pat/codingprojects/libretune/reference/TunerStudioMS/Dash/Speeduino_LED.dash'
      });
      console.log('[TabbedDashboard] Loaded TunerStudio dash:', JSON.stringify(layout, null, 2));
      setDashboards([layout]);
      setCurrentDashboard(0);
    } catch (e) {
      console.error('Failed to load TunerStudio dash:', e);
      alert(`Failed to load TunerStudio dash: ${e}`);
    }
  };

  const handleSaveDashboard = async () => {
    if (currentDashboard >= dashboards.length) {
      const newDashboard = {
        ...createFallbackDashboard(),
        name: `Dashboard ${dashboards.length + 1}`,
      };
      await invoke('save_dashboard_layout', { project_name: 'default', layout: newDashboard });
      setDashboards([...dashboards, newDashboard]);
      setCurrentDashboard(dashboards.length);
      alert('Dashboard saved successfully!');
    } else {
      const dashboard = dashboards[currentDashboard];
      await invoke('save_dashboard_layout', { project_name: 'default', layout: dashboard });
      alert('Dashboard saved successfully!');
    }
  };

  const handleDeleteDashboard = async (index: number) => {
    if (index === 0) return;

    setDashboards(dashboards.filter((_, i) => i !== index));

    if (currentDashboard === index) {
      setCurrentDashboard(0);
    } else if (currentDashboard > index) {
      setCurrentDashboard(currentDashboard - 1);
    }
    alert('Dashboard deleted!');
  };

  const handleAddGauge = (type: GaugeType) => {
    const dashboard = dashboards[currentDashboard];
    const newGauge: GaugeConfig = {
      id: `gauge_${dashboard.gauges.length + 1}`,
      gauge_type: type,
      channel: 'rpm',
      label: 'New Gauge',
      x: 0.5 + Math.random() * 0.3,
      y: 0.5 + Math.random() * 0.3,
      width: 0.15,
      height: 0.15,
      z_index: dashboard.gauges.length,
      min_value: 0,
      max_value: 10000,
      low_warning: undefined,
      high_warning: undefined,
      high_critical: undefined,
      decimals: 0,
      units: '',
      font_color: '#FFFFFF',
      needle_color: '#FF6600',
      trim_color: '#666999',
      show_history: false,
      show_min_max: false,
    };

    const newDashboard = {
      ...dashboard,
      gauges: [...dashboard.gauges, newGauge],
    };
    setDashboards([...dashboards.slice(0, currentDashboard), newDashboard, ...dashboards.slice(currentDashboard + 1)]);
  };

  const handleDeleteGauge = (gaugeId: string) => {
    const dashboard = dashboards[currentDashboard];
    const newGauges = dashboard.gauges.filter(g => g.id !== gaugeId);
    const newDashboard = { ...dashboard, gauges: newGauges };
    setDashboards([...dashboards.slice(0, currentDashboard), newDashboard, ...dashboards.slice(currentDashboard + 1)]);
    setShowGaugeSelector(false);
    setSelectedGauge(null);
  };

  const handleGaugeDragStart = (e: React.DragEvent, gaugeId: string) => {
    e.dataTransfer.setData('gaugeId', gaugeId);
  };

  const handleGaugeDrop = (e: React.DragEvent, targetIndex: number) => {
    const gaugeId = e.dataTransfer.getData('gaugeId') as string;
    if (!gaugeId) return;

    const dashboard = dashboards[targetIndex];
    const sourceGauge = dashboard.gauges.find(g => g.id === gaugeId);
    if (!sourceGauge) return;

    const gauges = dashboard.gauges.filter(g => g.id !== gaugeId);
    const sourceIndex = dashboard.gauges.findIndex(g => g.id === gaugeId);
    if (sourceIndex === -1) return;

    const newGauges = [...gauges.slice(0, sourceIndex), ...gauges.slice(sourceIndex + 1)];
    if (targetIndex <= sourceIndex) {
      newGauges.splice(targetIndex, 0, sourceGauge);
    } else {
      newGauges.push(sourceGauge, ...gauges.slice(sourceIndex));
    }
    const newDashboard = { ...dashboard, gauges: newGauges };
    setDashboards([...dashboards.slice(0, targetIndex), newDashboard, ...dashboards.slice(targetIndex + 1)]);
    setShowGaugeSelector(false);
  };

  const handleGaugeSelect = (gauge: GaugeConfig) => {
    setSelectedGauge(gauge.id);
    setShowGaugeSelector(false);
  };

  // @ts-expect-error - Reserved for future drag & drop functionality
  const _handleGaugeMove = (dx: number, dy: number, gaugeId: string) => {
    const dashboard = dashboards[currentDashboard];
    const gauge = dashboard.gauges.find(g => g.id === gaugeId);
    if (!gauge) return;

    const newGauges = dashboard.gauges.map(g => {
      if (g.id === gaugeId) {
        return { ...g, x: Math.max(0, Math.min(1, g.x + dx)), y: Math.max(0, Math.min(1, g.y + dy)) };
      }
      return g;
    });

    const newDashboard = { ...dashboard, gauges: newGauges };
    setDashboards([...dashboards.slice(0, currentDashboard), newDashboard, ...dashboards.slice(currentDashboard + 1)]);
  };

  // @ts-expect-error - Reserved for future drag & drop functionality
  const _handleGaugeResize = (width: number, height: number, gaugeId: string) => {
    const dashboard = dashboards[currentDashboard];
    const newGauges = dashboard.gauges.map(g => {
      if (g.id === gaugeId) {
        return { ...g, width: width / 100, height: height / 100 };
      }
      return g;
    });

    const newDashboard = { ...dashboard, gauges: newGauges };
    setDashboards([...dashboards.slice(0, currentDashboard), newDashboard, ...dashboards.slice(currentDashboard + 1)]);
  };

  const handleDoubleClick = () => {
    setIsFullscreen(!isFullscreen);
  };

  return (
    <div className={`tabbed-dashboard ${isFullscreen ? 'fullscreen' : ''}`}>
      <div className="dashboard-header">
        <h2>Dashboard {currentDashboard + 1} / {dashboards.length}</h2>
        <div className="dashboard-tabs">
          {dashboards.map((dashboard, index) => (
            <button
              key={index}
              className={`tab ${currentDashboard === index ? 'active' : ''}`}
              onClick={() => setCurrentDashboard(index)}
            >
              {dashboard.name}
            </button>
          ))}
          <button
            className={`tab-add-btn ${isDesignerMode ? 'active' : ''}`}
            onClick={() => setIsDesignerMode(!isDesignerMode)}
          >
            <Settings size={16} />
            {isDesignerMode ? 'Done' : 'Designer Mode'}
          </button>
        </div>

        <div className="dashboard-actions">
          <button className="action-btn" onClick={handleSaveDashboard}>
            <LayoutDashboard size={16} />
            <span>Save</span>
          </button>
          <button 
            className="action-btn"
            onClick={() => setShowSwitchDashboard(true)}
            title="Switch to a different dashboard or template"
          >
            <Settings size={16} />
            <span>Switch</span>
          </button>
          <button 
            className="action-btn"
            onClick={() => setShowTemplateSelector(true)}
          >
            <FileDown size={16} />
            <span>New from Template</span>
          </button>
          <button 
            className="action-btn"
            onClick={handleLoadTunerStudioDash}
            title="Load TunerStudio Speeduino_LED.dash for testing"
          >
            <FileDown size={16} />
            <span>Load TS Dash</span>
          </button>
          <button 
            className="action-btn"
            onClick={() => handleDeleteDashboard(currentDashboard)}
            disabled={dashboards.length <= 1}
          >
            <Trash2 size={16} />
            <span>Delete</span>
          </button>
          <button 
            className="action-btn"
            onClick={() => setIsDesignerMode(!isDesignerMode)}
          >
            <Plus size={16} />
            <span>Add Gauge</span>
          </button>
        </div>
      </div>

      <div className={`dashboard-content ${isDesignerMode ? 'designer-mode' : ''}`}>
        <div className="dashboard-gauges-container">
          {dashboards[currentDashboard]?.gauges.length === 0 && (
            <div style={{ color: 'white', padding: '2rem' }}>No gauges in this dashboard</div>
          )}
          {dashboards[currentDashboard]?.gauges.map((gauge) => {
            const isAnalogDial = gauge.gauge_type === GaugeType.AnalogDial;
            return (
              <div
                key={gauge.id}
                className={`gauge-container ${selectedGauge === gauge.id ? 'selected' : ''} ${isDesignerMode ? 'draggable' : ''}`}
                onDoubleClick={() => handleDoubleClick()}
                draggable={isDesignerMode}
                onDragStart={e => handleGaugeDragStart(e, gauge.id)}
                onDrop={e => handleGaugeDrop(e, currentDashboard)}
                onClick={() => handleGaugeSelect(gauge)}
                style={{
                  position: 'absolute',
                  left: `${gauge.x * 100}%`,
                  top: `${gauge.y * 100}%`,
                  width: `${gauge.width * 100}%`,
                  height: isAnalogDial ? 'auto' : `${gauge.height * 100}%`,
                  aspectRatio: isAnalogDial ? '1 / 1' : undefined,
                }}
              >
                <GaugeRenderer config={gauge} realtimeData={realtimeData} />
                {isDesignerMode && selectedGauge === gauge.id && (
                  <button className="delete-gauge-btn" onClick={(e) => { e.stopPropagation(); handleDeleteGauge(gauge.id); }}>
                    <XCircle size={14} />
                  </button>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* FrontPage Indicator Row at bottom */}
      {frontpageIndicators.length > 0 && (
        <IndicatorRow
          indicators={frontpageIndicators}
          realtimeData={realtimeData}
          constantValues={constantValues}
          columnCount={indicatorColumnCount}
          fillEmptyCells={indicatorFillEmpty}
          textFitMode={indicatorTextFit}
        />
      )}

      {isDesignerMode && (
        <button 
          className="add-gauge-fab"
          onClick={() => setShowGaugeSelector(!showGaugeSelector)}
        >
          <Plus size={24} />
        </button>
      )}

      {showGaugeSelector && (
        <div className="gauge-selector-overlay" onClick={() => setShowGaugeSelector(false)}>
          <div className="gauge-selector-dialog">
            <h3>Select Gauge Type</h3>
            <div className="gauge-options">
              {gaugeTypes.map((type) => (
                <button
                  key={type}
                  className="gauge-type-option"
                  onClick={() => handleAddGauge(type)}
                >
                  <span className="gauge-icon">{gaugeTypeMap[type].icon}</span>
                  <span>{gaugeTypeMap[type].name}</span>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      {showTemplateSelector && (
        <div className="gauge-selector-overlay" onClick={() => setShowTemplateSelector(false)}>
          <div className="gauge-selector-dialog template-selector">
            <h3>Create Dashboard from Template</h3>
            <p className="template-hint">Select a pre-configured dashboard layout</p>
            <div className="template-options">
              {templates.map((template) => (
                <button
                  key={template.id}
                  className="template-option"
                  onClick={() => handleCreateFromTemplate(template.id)}
                >
                  <span className="template-name">{template.name}</span>
                  <span className="template-description">{template.description}</span>
                </button>
              ))}
            </div>
            <div className="template-actions">
              <button className="cancel-btn" onClick={() => setShowTemplateSelector(false)}>
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {showSwitchDashboard && (
        <div className="gauge-selector-overlay" onClick={() => setShowSwitchDashboard(false)}>
          <div className="gauge-selector-dialog switch-dashboard-dialog" onClick={e => e.stopPropagation()}>
            <h3>Switch Dashboard</h3>
            <p className="template-hint">Replace current dashboard with a saved layout or template</p>
            
            {savedDashboardNames.length > 0 && (
              <div className="switch-section">
                <h4>Saved Dashboards</h4>
                <div className="template-options">
                  {savedDashboardNames.map((name) => (
                    <button
                      key={name}
                      className="template-option"
                      onClick={() => handleSwitchToDashboard(name)}
                    >
                      <span className="template-name">{name}</span>
                      <span className="template-description">User-saved dashboard</span>
                    </button>
                  ))}
                </div>
              </div>
            )}
            
            <div className="switch-section">
              <h4>Templates</h4>
              <div className="template-options">
                {templates.map((template) => (
                  <button
                    key={template.id}
                    className="template-option"
                    onClick={() => handleSwitchToTemplate(template.id)}
                  >
                    <span className="template-name">{template.name}</span>
                    <span className="template-description">{template.description}</span>
                  </button>
                ))}
              </div>
            </div>
            
            <div className="template-actions">
              <button className="cancel-btn" onClick={() => setShowSwitchDashboard(false)}>
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
