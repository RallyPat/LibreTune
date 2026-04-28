/**
 * Dashboard Designer Mode
 * 
 * Provides interactive editing capabilities for dashboard layouts:
 * - Drag gauges to reposition
 * - Resize handles on corners and edges
 * - Property editor panel for gauge configuration
 * - Snap-to-grid alignment
 * - Multi-select with shift-click
 * - Copy/paste gauges
 * - Undo/redo support
 */

import { useCallback, useRef } from 'react';
import { DashFile, TsGaugeConfig, DashComponent, isGauge, isIndicator } from './dashTypes';
import PropertyEditor from './designer/PropertyEditor';
import DesignerToolbar from './designer/DesignerToolbar';
import { useDesignerHistory } from './designer/useDesignerHistory';
import { useDesignerDragResize, ResizeHandle } from './designer/useDesignerDragResize';
import { useDesignerKeyboard } from './designer/useDesignerKeyboard';
import './DashboardDesigner.css';

interface ChannelInfo {
  name: string;
  label?: string | null;
  units: string;
  scale: number;
  translate: number;
}

interface DashboardDesignerProps {
  dashFile: DashFile;
  onDashFileChange: (file: DashFile) => void;
  selectedGaugeId: string | null;
  onSelectGauge: (id: string | null) => void;
  onContextMenu: (e: React.MouseEvent, gaugeId: string | null) => void;
  gridSnap: number; // Grid snap size in percentage (e.g., 5 = 5%)
  onGridSnapChange: (snap: number) => void;
  showGrid: boolean;
  onShowGridChange: (show: boolean) => void;
  onSave: () => void;
  onExit: () => void;
  channelInfoMap?: Record<string, ChannelInfo>; // INI channel metadata for gauge creation
}




export default function DashboardDesigner({
  dashFile,
  onDashFileChange,
  selectedGaugeId,
  onSelectGauge,
  onContextMenu,
  gridSnap,
  onGridSnapChange,
  showGrid,
  onShowGridChange,
  onSave,
  onExit,
  channelInfoMap = {},
}: DashboardDesignerProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  // Undo/redo + clipboard + delete + selected component (extracted hook).
  const {
    selectedComponent,
    pushHistory,
    undo: handleUndo,
    redo: handleRedo,
    remove: handleDelete,
    copy: handleCopy,
    paste: handlePaste,
    canUndo,
    canRedo,
    hasClipboard,
  } = useDesignerHistory({ dashFile, selectedGaugeId, onDashFileChange, onSelectGauge });

  // Snap value to grid
  const snapToGrid = useCallback((value: number): number => {
    if (gridSnap <= 0) return value;
    return Math.round(value / (gridSnap / 100)) * (gridSnap / 100);
  }, [gridSnap]);

  // Drag/resize interactions extracted into a hook.
  const {
    dragState,
    resizeState,
    onGaugeMouseDown: handleGaugeMouseDown,
    onResizeMouseDown: handleResizeMouseDown,
  } = useDesignerDragResize({
    dashFile,
    containerRef,
    snapToGrid,
    pushHistory,
    onDashFileChange,
    onSelectGauge,
  });

  // Keyboard shortcuts
  const handleDeselect = useCallback(() => onSelectGauge(null), [onSelectGauge]);
  useDesignerKeyboard({
    onDelete: handleDelete,
    onUndo: handleUndo,
    onRedo: handleRedo,
    onCopy: handleCopy,
    onPaste: handlePaste,
    onSave,
    onDeselect: handleDeselect,
  });

  // Handle drag-and-drop channel to canvas
  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = 'copy';
    e.currentTarget.classList.add('drag-over-dropzone');
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.currentTarget.classList.remove('drag-over-dropzone');
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    e.currentTarget.classList.remove('drag-over-dropzone');

    try {
      const data = e.dataTransfer.getData('application/json');
      if (!data) return;
      
      const channel = JSON.parse(data);
      if (channel.type !== 'channel' || !dashFile) return;

      // Calculate relative drop position (0.0-1.0)
      const rect = e.currentTarget.getBoundingClientRect();
      let relX = (e.clientX - rect.left) / rect.width;
      let relY = (e.clientY - rect.top) / rect.height;

      // Offset to center on cursor
      relX = Math.max(0, Math.min(0.9, relX - 0.1));
      relY = Math.max(0, Math.min(0.9, relY - 0.1));

      // Apply grid snap if enabled
      if (gridSnap > 0) {
        relX = snapToGrid(relX);
        relY = snapToGrid(relY);
      }

      // Get channel info from map (with INI defaults)
      const info = channelInfoMap[channel.id];
      const units = info?.units || '';
      const label = info?.label || channel.label;
      const minVal = info ? Math.min(0, info.translate) : 0;
      const maxVal = info ? Math.max(100, info.translate + (100 * info.scale)) : 100;

      // Create default gauge config with INI data
      const defaultGauge: TsGaugeConfig = {
        id: `gauge_${Date.now()}`,
        gauge_painter: 'BasicReadout',
        gauge_style: '',
        output_channel: channel.id,
        title: label || channel.label,
        units: units,
        value: 0,
        min: minVal,
        max: maxVal,
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
        relative_x: relX,
        relative_y: relY,
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
      
      pushHistory(updatedFile, `Add gauge from ${channel.label}`);
      onDashFileChange(updatedFile);
    } catch (err) {
      console.error('Drop handler error:', err);
    }
  }, [dashFile, gridSnap, snapToGrid, channelInfoMap, pushHistory, onDashFileChange]);

  // Render resize handles for selected gauge
  const renderResizeHandles = (gaugeId: string, component: DashComponent) => {
    if (selectedGaugeId !== gaugeId) return null;
    
    const handles: ResizeHandle[] = ['n', 's', 'e', 'w', 'ne', 'nw', 'se', 'sw'];
    
    return handles.map(handle => (
      <div
        key={handle}
        className={`resize-handle resize-handle-${handle}`}
        onMouseDown={(e) => handleResizeMouseDown(e, handle, gaugeId, component)}
      />
    ));
  };

  return (
    <div className="dashboard-designer">
      <DesignerToolbar
        canUndo={canUndo}
        canRedo={canRedo}
        hasClipboard={hasClipboard}
        hasSelection={!!selectedGaugeId}
        showGrid={showGrid}
        gridSnap={gridSnap}
        onUndo={handleUndo}
        onRedo={handleRedo}
        onCopy={handleCopy}
        onPaste={handlePaste}
        onDelete={handleDelete}
        onShowGridChange={onShowGridChange}
        onGridSnapChange={onGridSnapChange}
        onSave={onSave}
        onExit={onExit}
      />

      {/* Main designer area */}
      <div className="designer-content">
        {/* Canvas with gauges */}
        <div 
          ref={containerRef}
          className={`designer-canvas ${showGrid ? 'show-grid' : ''}`}
          style={{
            '--grid-size': `${gridSnap}%`,
          } as React.CSSProperties}
          onClick={() => onSelectGauge(null)}
          onContextMenu={(e) => onContextMenu(e, null)}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
        >
          {dashFile.gauge_cluster.components.map((component, index) => {
            let id: string, relX: number, relY: number, width: number, height: number;
            
            if (isGauge(component)) {
              const g = component.Gauge;
              id = g.id || `gauge-${index}`;
              relX = g.relative_x ?? 0;
              relY = g.relative_y ?? 0;
              width = g.relative_width ?? 0.25;
              height = g.relative_height ?? 0.25;
            } else if (isIndicator(component)) {
              const i = component.Indicator;
              id = i.id || `indicator-${index}`;
              relX = i.relative_x ?? 0;
              relY = i.relative_y ?? 0;
              width = i.relative_width ?? 0.1;
              height = i.relative_height ?? 0.05;
            } else {
              return null;
            }
            
            const isSelected = selectedGaugeId === id;
            const isDraggingThis = dragState.isDragging && dragState.gaugeId === id;
            const isResizingThis = resizeState.isResizing && resizeState.gaugeId === id;
            
            return (
              <div
                key={id}
                className={`designer-gauge ${isSelected ? 'selected' : ''} ${isDraggingThis ? 'dragging' : ''} ${isResizingThis ? 'resizing' : ''}`}
                style={{
                  left: `${relX * 100}%`,
                  top: `${relY * 100}%`,
                  width: `${width * 100}%`,
                  height: `${height * 100}%`,
                }}
                onMouseDown={(e) => handleGaugeMouseDown(e, id, component)}
                onClick={(e) => {
                  e.stopPropagation();
                  onSelectGauge(id);
                }}
                onContextMenu={(e) => onContextMenu(e, id)}
              >
                <div className="gauge-preview">
                  {isGauge(component) && (
                    <span className="gauge-label">{component.Gauge.title || component.Gauge.output_channel}</span>
                  )}
                  {isIndicator(component) && (
                    <span className="gauge-label">{component.Indicator.on_text || component.Indicator.output_channel}</span>
                  )}
                </div>
                {renderResizeHandles(id, component)}
              </div>
            );
          })}
        </div>

        {/* Property editor panel */}
        <div className="designer-properties">
          <h3>Properties</h3>
          {selectedComponent ? (
            <PropertyEditor
              component={selectedComponent}
              onChange={(updated) => {
                const newComponents = dashFile.gauge_cluster.components.map(c => {
                  if (isGauge(c) && isGauge(updated) && c.Gauge.id === updated.Gauge.id) {
                    return updated;
                  }
                  if (isIndicator(c) && isIndicator(updated) && c.Indicator.id === updated.Indicator.id) {
                    return updated;
                  }
                  return c;
                });
                
                const newFile = {
                  ...dashFile,
                  gauge_cluster: { ...dashFile.gauge_cluster, components: newComponents },
                };
                pushHistory(newFile, 'Edit property');
                onDashFileChange(newFile);
              }}
            />
          ) : (
            <p className="no-selection">Select a gauge to edit its properties</p>
          )}
        </div>
      </div>
    </div>
  );
}
