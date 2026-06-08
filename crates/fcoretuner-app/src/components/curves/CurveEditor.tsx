/**
 * CurveEditor - Unified curve editing component
 * 
 * Renders and allows editing of 2D curves from INI CurveEditor definitions.
 * Supports both embedded mode (in dialogs) and standalone mode (as a tab).
 */

import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ArrowLeft, Save, Zap, Undo2, Redo2 } from 'lucide-react';
import TsGauge from '../gauges/TsGauge';
import { TsGaugeConfig } from '../dashboards/dashTypes';
import { valueToHeatmapColor } from '../../utils/heatmapColors';
import './CurveEditor.css';

/** Simple gauge info from backend INI [GaugeConfigurations] */
export interface SimpleGaugeInfo {
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

/** Convert SimpleGaugeInfo to TsGaugeConfig for rendering */
function toTsGaugeConfig(gauge: SimpleGaugeInfo): TsGaugeConfig {
  return {
    id: gauge.name,
    gauge_painter: 'AnalogGauge',
    gauge_style: '',
    output_channel: gauge.channel,
    title: gauge.title,
    units: gauge.units,
    value: 0,
    min: gauge.lo,
    max: gauge.hi,
    min_vp: null,
    max_vp: null,
    default_min: null,
    default_max: null,
    peg_limits: false,
    low_warning: gauge.low_warning,
    high_warning: gauge.high_warning,
    low_critical: gauge.low_danger,
    high_critical: gauge.high_danger,
    low_warning_vp: null,
    high_warning_vp: null,
    low_critical_vp: null,
    high_critical_vp: null,
    back_color: { alpha: 255, red: 40, green: 40, blue: 40 },
    font_color: { alpha: 255, red: 255, green: 255, blue: 255 },
    trim_color: { alpha: 255, red: 192, green: 192, blue: 192 },
    warn_color: { alpha: 255, red: 255, green: 200, blue: 0 },
    critical_color: { alpha: 255, red: 255, green: 0, blue: 0 },
    needle_color: { alpha: 255, red: 255, green: 0, blue: 0 },
    value_digits: gauge.digits,
    label_digits: 0,
    font_family: 'sans-serif',
    font_size_adjustment: 0,
    italic_font: false,
    start_angle: 225,
    sweep_angle: 270,
    face_angle: 0,
    sweep_begin_degree: 0,
    counter_clockwise: false,
    major_ticks: 10,
    minor_ticks: 5,
    relative_x: 0,
    relative_y: 0,
    relative_width: 1,
    relative_height: 1,
    border_width: 0,
    shortest_size: 100,
    shape_locked_to_aspect: true,
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
}

/** Extended curve data from backend */
export interface CurveData {
  name: string;
  title: string;
  x_bins: number[];
  y_bins: number[];
  x_label: string;
  y_label: string;
  x_axis?: [number, number, number] | null; // [min, max, step]
  y_axis?: [number, number, number] | null;
  x_output_channel?: string | null;
  gauge?: string | null;
}

interface CurveEditorProps {
  /** Curve data from backend */
  data: CurveData;
  /** Whether this is embedded in a dialog (compact mode) */
  embedded?: boolean;
  /** Realtime data for live cursor and gauge */
  realtimeData?: Record<string, number>;
  /** Full TsGaugeConfig for embedded display (optional) */
  gaugeConfig?: TsGaugeConfig | null;
  /** Simple gauge info from INI (alternative to gaugeConfig) */
  simpleGaugeInfo?: SimpleGaugeInfo | null;
  /** Callback when Y values are modified */
  onValuesChange?: (yBins: number[]) => void;
  /** Callback when user wants to go back (standalone mode) */
  onBack?: () => void;
  /** Menu label for display in title */
  menuLabel?: string;
}

export default function CurveEditor({
  data,
  embedded = false,
  realtimeData,
  gaugeConfig,
  simpleGaugeInfo,
  onValuesChange,
  onBack,
  menuLabel,
}: CurveEditorProps) {
  // Local copy of Y values for editing
  const [localYBins, setLocalYBins] = useState<number[]>([...data.y_bins]);
  // Selected point index
  const [selectedPoint, setSelectedPoint] = useState<number | null>(null);
  // Dragging state
  const [isDragging, setIsDragging] = useState(false);
  const [dragPointIndex, setDragPointIndex] = useState<number | null>(null);
  // Table input value for editing
  const [editingCell, setEditingCell] = useState<number | null>(null);
  const [editValue, setEditValue] = useState<string>('');
  // Undo/Redo history
  const [history, setHistory] = useState<number[][]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  // Context menu state
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  // Axis override state (for manual scaling)
  const [yAxisOverride, setYAxisOverride] = useState<{ min?: number; max?: number; auto: boolean }>({ auto: true });
  const [xAxisOverride, setXAxisOverride] = useState<{ min?: number; max?: number; auto: boolean }>({ auto: true });
  // SVG container ref
  const svgRef = useRef<SVGSVGElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Update local values when data changes
  useEffect(() => {
    setLocalYBins([...data.y_bins]);
  }, [data.y_bins]);

  // Click-outside handler for context menu
  useEffect(() => {
    if (!contextMenu) return;
    
    const handleClickOutside = () => closeContextMenu();
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') closeContextMenu();
    };
    
    document.addEventListener('click', handleClickOutside);
    document.addEventListener('keydown', handleEscape);
    
    return () => {
      document.removeEventListener('click', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [contextMenu]);

  // Helper to compute cell background color based on value position in range
  // Uses centralized heatmap color utility for consistent styling
  const getCellColor = useCallback((value: number, min: number, max: number): string => {
    return valueToHeatmapColor(value, min, max, 'tunerstudio');
  }, []);

  // Chart dimensions based on mode
  const chartWidth = embedded ? 500 : 500;
  const chartHeight = embedded ? 280 : 350;
  const padding = { top: 30, right: 20, bottom: 40, left: 50 };

  // Calculate axis bounds (respecting overrides)
  const xAxis = useMemo(() => {
    const base = data.x_axis 
      ? { min: data.x_axis[0], max: data.x_axis[1], step: data.x_axis[2] }
      : { min: Math.min(...data.x_bins), max: Math.max(...data.x_bins), step: 10 };
    
    if (!xAxisOverride.auto) {
      return {
        min: xAxisOverride.min ?? base.min,
        max: xAxisOverride.max ?? base.max,
        step: base.step
      };
    }
    return base;
  }, [data.x_axis, data.x_bins, xAxisOverride]);

  const yAxis = useMemo(() => {
    const yMin = Math.min(...localYBins);
    const yMax = Math.max(...localYBins);
    const dataPadding = (yMax - yMin) * 0.1 || 0.5;
    
    const base = data.y_axis 
      ? { min: data.y_axis[0], max: data.y_axis[1], step: data.y_axis[2] }
      : { min: yMin - dataPadding, max: yMax + dataPadding, step: 10 };
    
    if (!yAxisOverride.auto) {
      return {
        min: yAxisOverride.min ?? base.min,
        max: yAxisOverride.max ?? base.max,
        step: base.step
      };
    }
    return base;
  }, [data.y_axis, localYBins, yAxisOverride]);

  // Scale functions
  const scaleX = useCallback((x: number) => {
    const range = xAxis.max - xAxis.min || 1;
    return padding.left + ((x - xAxis.min) / range) * (chartWidth - padding.left - padding.right);
  }, [xAxis, chartWidth, padding]);

  const scaleY = useCallback((y: number) => {
    const range = yAxis.max - yAxis.min || 1;
    return chartHeight - padding.bottom - ((y - yAxis.min) / range) * (chartHeight - padding.top - padding.bottom);
  }, [yAxis, chartHeight, padding]);

  const unscaleY = useCallback((screenY: number) => {
    const range = yAxis.max - yAxis.min || 1;
    const normalized = (chartHeight - padding.bottom - screenY) / (chartHeight - padding.top - padding.bottom);
    return yAxis.min + normalized * range;
  }, [yAxis, chartHeight, padding]);

  // Generate grid lines with limited labels for readability
  const gridLines = useMemo(() => {
    const lines: { x1: number; y1: number; x2: number; y2: number; label?: string; isAxis?: boolean }[] = [];
    
    // X-axis: INI step value is the number of divisions, not the step size
    // Limit to ~7 labels max for readability
    const xRange = xAxis.max - xAxis.min;
    const xDivisions = Math.min(xAxis.step || 10, 10); // step is actually division count
    const xStep = xRange / xDivisions;
    
    // Calculate a nice round step value
    const xNiceStep = Math.ceil(xStep / 10) * 10 || xStep; // Round to nearest 10
    const xLabelStep = xRange / Math.min(7, Math.ceil(xRange / xNiceStep));
    
    for (let x = xAxis.min; x <= xAxis.max + 0.001; x += xLabelStep) {
      const roundedX = Math.round(x);
      lines.push({
        x1: scaleX(roundedX), y1: padding.top,
        x2: scaleX(roundedX), y2: chartHeight - padding.bottom,
        label: roundedX.toFixed(0),
        isAxis: roundedX === xAxis.min
      });
    }
    
    // Y-axis: Similar treatment - step is division count
    const yRange = yAxis.max - yAxis.min;
    const yDivisions = Math.min(yAxis.step || 10, 10);
    const yLabelStep = yRange / yDivisions;
    
    for (let y = yAxis.min; y <= yAxis.max + 0.001; y += yLabelStep) {
      lines.push({
        x1: padding.left, y1: scaleY(y),
        x2: chartWidth - padding.right, y2: scaleY(y),
        label: y.toFixed(2),
        isAxis: Math.abs(y - yAxis.min) < 0.001
      });
    }
    
    return lines;
  }, [xAxis, yAxis, scaleX, scaleY, chartWidth, chartHeight, padding]);

  // Polyline points
  const polylinePoints = useMemo(() => {
    return data.x_bins.map((x, i) => `${scaleX(x)},${scaleY(localYBins[i])}`).join(' ');
  }, [data.x_bins, localYBins, scaleX, scaleY]);

  // Live cursor position
  const liveCursor = useMemo(() => {
    if (!realtimeData || !data.x_output_channel) return null;
    const xValue = realtimeData[data.x_output_channel];
    if (xValue === undefined) return null;
    
    // Find the interpolated Y value
    let yValue = localYBins[0];
    for (let i = 0; i < data.x_bins.length - 1; i++) {
      if (xValue >= data.x_bins[i] && xValue <= data.x_bins[i + 1]) {
        const t = (xValue - data.x_bins[i]) / (data.x_bins[i + 1] - data.x_bins[i]);
        yValue = localYBins[i] + t * (localYBins[i + 1] - localYBins[i]);
        break;
      }
    }
    if (xValue > data.x_bins[data.x_bins.length - 1]) {
      yValue = localYBins[localYBins.length - 1];
    }
    
    return { x: xValue, y: yValue, screenX: scaleX(xValue), screenY: scaleY(yValue) };
  }, [realtimeData, data.x_output_channel, data.x_bins, localYBins, scaleX, scaleY]);

  // Persist changes to backend
  const persistCurveValues = useCallback(async (yBins: number[]) => {
    try {
      await invoke('update_curve_data', { curveName: data.name, yValues: yBins });
      onValuesChange?.(yBins);
    } catch (err) {
      console.error('Failed to update curve:', err);
    }
  }, [data.name, onValuesChange]);

  // Push current state to history before making changes
  const pushHistory = useCallback(() => {
    const newHistory = history.slice(0, historyIndex + 1);
    newHistory.push([...localYBins]);
    setHistory(newHistory);
    setHistoryIndex(newHistory.length - 1);
  }, [localYBins, history, historyIndex]);

  // Undo last change
  const undo = useCallback(() => {
    if (historyIndex >= 0) {
      const previousState = history[historyIndex];
      setLocalYBins(previousState);
      setHistoryIndex(historyIndex - 1);
      persistCurveValues(previousState);
    }
  }, [history, historyIndex, persistCurveValues]);

  // Redo last undone change
  const redo = useCallback(() => {
    if (historyIndex < history.length - 1) {
      const nextState = history[historyIndex + 1];
      setHistoryIndex(historyIndex + 1);
      setLocalYBins(nextState);
      persistCurveValues(nextState);
    }
  }, [history, historyIndex, persistCurveValues]);

  // Keyboard shortcuts for undo/redo
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'z' && !e.shiftKey) {
        e.preventDefault();
        undo();
      } else if ((e.ctrlKey || e.metaKey) && (e.key === 'y' || (e.key === 'z' && e.shiftKey))) {
        e.preventDefault();
        redo();
      }
    };
    
    const container = containerRef.current;
    if (container) {
      container.addEventListener('keydown', handleKeyDown);
      return () => container.removeEventListener('keydown', handleKeyDown);
    }
  }, [undo, redo]);

  // Handle mouse down on a point - push history first
  const handlePointMouseDown = (e: React.MouseEvent, index: number) => {
    e.preventDefault();
    e.stopPropagation();
    pushHistory(); // Save state before editing
    setIsDragging(true);
    setDragPointIndex(index);
    setSelectedPoint(index);
  };

  // Handle mouse move for dragging
  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!isDragging || dragPointIndex === null || !svgRef.current) return;
    
    const rect = svgRef.current.getBoundingClientRect();
    const screenY = e.clientY - rect.top;
    let newY = unscaleY(screenY);
    
    // Clamp to axis bounds
    newY = Math.max(yAxis.min, Math.min(yAxis.max, newY));
    
    const newYBins = [...localYBins];
    newYBins[dragPointIndex] = newY;
    setLocalYBins(newYBins);
  }, [isDragging, dragPointIndex, unscaleY, yAxis, localYBins]);

  // Handle mouse up to end dragging
  const handleMouseUp = useCallback(() => {
    if (isDragging && dragPointIndex !== null) {
      // Commit the change to backend
      persistCurveValues(localYBins);
    }
    setIsDragging(false);
    setDragPointIndex(null);
  }, [isDragging, dragPointIndex, localYBins, persistCurveValues]);

  // Handle table cell edit
  const handleCellDoubleClick = (index: number) => {
    pushHistory(); // Save state before editing
    setEditingCell(index);
    setEditValue(localYBins[index].toFixed(2));
  };

  const handleCellKeyDown = (e: React.KeyboardEvent, index: number) => {
    if (e.key === 'Enter') {
      const parsed = parseFloat(editValue);
      if (!isNaN(parsed)) {
        let clamped = parsed;
        clamped = Math.max(yAxis.min, Math.min(yAxis.max, clamped));
        const newYBins = [...localYBins];
        newYBins[index] = clamped;
        setLocalYBins(newYBins);
        persistCurveValues(newYBins);
      }
      setEditingCell(null);
    } else if (e.key === 'Escape') {
      setEditingCell(null);
    }
  };

  const handleCellBlur = (index: number) => {
    const parsed = parseFloat(editValue);
    if (!isNaN(parsed)) {
      let clamped = parsed;
      clamped = Math.max(yAxis.min, Math.min(yAxis.max, clamped));
      const newYBins = [...localYBins];
      newYBins[index] = clamped;
      setLocalYBins(newYBins);
      persistCurveValues(newYBins);
    }
    setEditingCell(null);
  };

  // Handle row click to select
  const handleRowClick = (index: number) => {
    setSelectedPoint(index);
  };

  // Context menu handlers
  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY });
  };

  const closeContextMenu = () => {
    setContextMenu(null);
  };

  const setYAxisMin = () => {
    const value = prompt('Set Y Axis Minimum:', yAxis.min.toString());
    if (value !== null) {
      const num = parseFloat(value);
      if (!isNaN(num)) {
        setYAxisOverride(prev => ({ ...prev, min: num, auto: false }));
      }
    }
    closeContextMenu();
  };

  const setYAxisMax = () => {
    const value = prompt('Set Y Axis Maximum:', yAxis.max.toString());
    if (value !== null) {
      const num = parseFloat(value);
      if (!isNaN(num)) {
        setYAxisOverride(prev => ({ ...prev, max: num, auto: false }));
      }
    }
    closeContextMenu();
  };

  const setXAxisMin = () => {
    const value = prompt('Set X Axis Minimum:', xAxis.min.toString());
    if (value !== null) {
      const num = parseFloat(value);
      if (!isNaN(num)) {
        setXAxisOverride(prev => ({ ...prev, min: num, auto: false }));
      }
    }
    closeContextMenu();
  };

  const setXAxisMax = () => {
    const value = prompt('Set X Axis Maximum:', xAxis.max.toString());
    if (value !== null) {
      const num = parseFloat(value);
      if (!isNaN(num)) {
        setXAxisOverride(prev => ({ ...prev, max: num, auto: false }));
      }
    }
    closeContextMenu();
  };

  const toggleYAxisAuto = () => {
    setYAxisOverride(prev => ({ ...prev, auto: !prev.auto }));
    closeContextMenu();
  };

  const toggleXAxisAuto = () => {
    setXAxisOverride(prev => ({ ...prev, auto: !prev.auto }));
    closeContextMenu();
  };

  // Display title
  const displayTitle = menuLabel 
    ? `${menuLabel} (${data.name})` 
    : data.title || data.name;

  // Gauge value
  const gaugeValue = realtimeData && data.x_output_channel 
    ? realtimeData[data.x_output_channel] ?? 0 
    : 0;

  return (
    <div 
      className={`curve-editor ${embedded ? 'embedded' : 'standalone'}`}
      ref={containerRef}
      tabIndex={0} // Enable keyboard focus for undo/redo shortcuts
    >
      {/* Header - only for standalone mode */}
      {!embedded && (
        <div className="curve-editor-header">
          <button className="back-button" onClick={onBack} title="Back">
            <ArrowLeft size={18} />
          </button>
          <h2 className="curve-title">{displayTitle}</h2>
          <div className="curve-toolbar">
            <button 
              className="toolbar-btn" 
              title="Undo (Ctrl+Z)" 
              onClick={undo}
              disabled={historyIndex < 0}
            >
              <Undo2 size={16} />
            </button>
            <button 
              className="toolbar-btn" 
              title="Redo (Ctrl+Y)" 
              onClick={redo}
              disabled={historyIndex >= history.length - 1}
            >
              <Redo2 size={16} />
            </button>
            <div className="toolbar-separator" />
            <button className="toolbar-btn" title="Save">
              <Save size={16} />
            </button>
            <button className="toolbar-btn" title="Burn to ECU">
              <Zap size={16} />
            </button>
          </div>
        </div>
      )}

      {/* Title for embedded mode */}
      {embedded && (
        <div className="curve-embedded-title">{displayTitle}</div>
      )}

      <div className="curve-content">
        {/* Chart area */}
        <div className="curve-chart-container" onContextMenu={handleContextMenu}>
          <svg
            ref={svgRef}
            width={chartWidth}
            height={chartHeight}
            className="curve-svg"
            onMouseMove={handleMouseMove}
            onMouseUp={handleMouseUp}
            onMouseLeave={handleMouseUp}
          >
            {/* Background */}
            <rect
              x={padding.left}
              y={padding.top}
              width={chartWidth - padding.left - padding.right}
              height={chartHeight - padding.top - padding.bottom}
              fill="#1a1a1a"
            />

            {/* Grid lines */}
            {gridLines.map((line, i) => (
              <line
                key={i}
                x1={line.x1}
                y1={line.y1}
                x2={line.x2}
                y2={line.y2}
                stroke={line.isAxis ? '#666' : '#333'}
                strokeWidth={line.isAxis ? 2 : 1}
              />
            ))}

            {/* X axis labels */}
            {gridLines
              .filter(l => l.x1 === l.x2 && l.label) // Vertical lines = X axis
              .map((line, i) => (
                <text
                  key={`x-${i}`}
                  x={line.x1}
                  y={chartHeight - padding.bottom + 15}
                  textAnchor="middle"
                  fill="#888"
                  fontSize="10"
                >
                  {line.label}
                </text>
              ))}

            {/* Y axis labels */}
            {gridLines
              .filter(l => l.y1 === l.y2 && l.label) // Horizontal lines = Y axis
              .map((line, i) => (
                <text
                  key={`y-${i}`}
                  x={padding.left - 5}
                  y={line.y1 + 3}
                  textAnchor="end"
                  fill="#888"
                  fontSize="10"
                >
                  {line.label}
                </text>
              ))}

            {/* Axis titles */}
            <text
              x={chartWidth / 2}
              y={chartHeight - 5}
              textAnchor="middle"
              fill="#aaa"
              fontSize="12"
            >
              {data.x_label}
            </text>
            <text
              x={12}
              y={chartHeight / 2}
              textAnchor="middle"
              fill="#aaa"
              fontSize="12"
              transform={`rotate(-90, 12, ${chartHeight / 2})`}
            >
              {data.y_label}
            </text>

            {/* Data line */}
            <polyline
              points={polylinePoints}
              fill="none"
              stroke="#f5d742"
              strokeWidth="2"
            />

            {/* Data points */}
            {data.x_bins.map((x, i) => (
              <circle
                key={i}
                cx={scaleX(x)}
                cy={scaleY(localYBins[i])}
                r={selectedPoint === i ? 8 : 6}
                fill={selectedPoint === i ? '#fff' : '#f5d742'}
                stroke="#000"
                strokeWidth="2"
                style={{ cursor: 'ns-resize' }}
                onMouseDown={(e) => handlePointMouseDown(e, i)}
              />
            ))}

            {/* Live cursor */}
            {liveCursor && (
              <>
                {/* Vertical line */}
                <line
                  x1={liveCursor.screenX}
                  y1={padding.top}
                  x2={liveCursor.screenX}
                  y2={chartHeight - padding.bottom}
                  stroke="#ff4444"
                  strokeWidth="1"
                  strokeDasharray="4,2"
                />
                {/* Highlight point */}
                <circle
                  cx={liveCursor.screenX}
                  cy={liveCursor.screenY}
                  r="5"
                  fill="#ff4444"
                  stroke="#fff"
                  strokeWidth="2"
                />
              </>
            )}
          </svg>
        </div>

        {/* Bottom section: gauge + data table (embedded only uses stacked layout) */}
        {embedded ? (
          <div className="curve-bottom-section">
            {/* Gauge */}
            {(gaugeConfig || simpleGaugeInfo) && (
              <div className="curve-gauge-container">
                <TsGauge 
                  config={gaugeConfig || toTsGaugeConfig(simpleGaugeInfo!)} 
                  value={gaugeValue} 
                />
              </div>
            )}
            {/* Data table */}
            <div className="curve-data-table">
          <table>
            <thead>
              <tr>
                <th>{data.x_label}</th>
                <th>{data.y_label}</th>
              </tr>
            </thead>
            <tbody>
              {data.x_bins.map((x, i) => {
                const yValue = localYBins[i];
                const yColor = getCellColor(yValue, yAxis.min, yAxis.max);
                const xColor = getCellColor(x, xAxis.min, xAxis.max);
                
                return (
                  <tr
                    key={i}
                    className={selectedPoint === i ? 'selected' : ''}
                    onClick={() => handleRowClick(i)}
                  >
                    <td className="x-cell" style={{ backgroundColor: xColor }}>{x.toFixed(2)}</td>
                    <td
                      className="y-cell"
                      style={{ backgroundColor: yColor }}
                      onDoubleClick={() => handleCellDoubleClick(i)}
                    >
                      {editingCell === i ? (
                        <input
                          type="text"
                          value={editValue}
                          onChange={(e) => setEditValue(e.target.value)}
                          onKeyDown={(e) => handleCellKeyDown(e, i)}
                          onBlur={() => handleCellBlur(i)}
                          autoFocus
                        />
                      ) : (
                        yValue.toFixed(2)
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
          </div>
        ) : (
          /* Standalone mode: table beside chart */
          <div className="curve-data-table">
            <table>
              <thead>
                <tr>
                  <th>{data.x_label}</th>
                  <th>{data.y_label}</th>
                </tr>
              </thead>
              <tbody>
                {data.x_bins.map((x, i) => {
                  const yValue = localYBins[i];
                  const yColor = getCellColor(yValue, yAxis.min, yAxis.max);
                  const xColor = getCellColor(x, xAxis.min, xAxis.max);
                  
                  return (
                    <tr
                      key={i}
                      className={selectedPoint === i ? 'selected' : ''}
                      onClick={() => handleRowClick(i)}
                    >
                      <td className="x-cell" style={{ backgroundColor: xColor }}>{x.toFixed(2)}</td>
                      <td
                        className="y-cell"
                        style={{ backgroundColor: yColor }}
                        onDoubleClick={() => handleCellDoubleClick(i)}
                      >
                        {editingCell === i ? (
                          <input
                            type="text"
                            value={editValue}
                            onChange={(e) => setEditValue(e.target.value)}
                            onKeyDown={(e) => handleCellKeyDown(e, i)}
                            onBlur={() => handleCellBlur(i)}
                            autoFocus
                          />
                        ) : (
                          yValue.toFixed(2)
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Context menu for axis scaling */}
      {contextMenu && (
        <div 
          className="curve-context-menu" 
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="context-menu-section">
            <div className="context-menu-header">Y Axis</div>
            <div className="context-menu-item" onClick={setYAxisMin}>Set Minimum...</div>
            <div className="context-menu-item" onClick={setYAxisMax}>Set Maximum...</div>
            <div className="context-menu-item" onClick={toggleYAxisAuto}>
              <input type="checkbox" checked={yAxisOverride.auto} readOnly /> Auto Scale
            </div>
          </div>
          <div className="context-menu-divider" />
          <div className="context-menu-section">
            <div className="context-menu-header">X Axis</div>
            <div className="context-menu-item" onClick={setXAxisMin}>Set Minimum...</div>
            <div className="context-menu-item" onClick={setXAxisMax}>Set Maximum...</div>
            <div className="context-menu-item" onClick={toggleXAxisAuto}>
              <input type="checkbox" checked={xAxisOverride.auto} readOnly /> Auto Scale
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
