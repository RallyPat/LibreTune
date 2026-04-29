import { RefObject } from 'react';
import { DashFile, DashComponent, isGauge, isIndicator } from '../dashTypes';
import { ResizeHandle } from './useDesignerDragResize';

interface DragState {
  isDragging: boolean;
  gaugeId: string | null;
}

interface ResizeState {
  isResizing: boolean;
  gaugeId: string | null;
}

interface DesignerCanvasProps {
  containerRef: RefObject<HTMLDivElement>;
  dashFile: DashFile;
  showGrid: boolean;
  gridSnap: number;
  selectedGaugeId: string | null;
  dragState: DragState;
  resizeState: ResizeState;
  onSelectGauge: (id: string | null) => void;
  onContextMenu: (e: React.MouseEvent, id: string | null) => void;
  onDragOver: (e: React.DragEvent) => void;
  onDragLeave: (e: React.DragEvent) => void;
  onDrop: (e: React.DragEvent) => void;
  onGaugeMouseDown: (e: React.MouseEvent, id: string, component: DashComponent) => void;
  onResizeMouseDown: (e: React.MouseEvent, handle: ResizeHandle, id: string, component: DashComponent) => void;
}

const RESIZE_HANDLES: ResizeHandle[] = ['n', 's', 'e', 'w', 'ne', 'nw', 'se', 'sw'];

export default function DesignerCanvas({
  containerRef,
  dashFile,
  showGrid,
  gridSnap,
  selectedGaugeId,
  dragState,
  resizeState,
  onSelectGauge,
  onContextMenu,
  onDragOver,
  onDragLeave,
  onDrop,
  onGaugeMouseDown,
  onResizeMouseDown,
}: DesignerCanvasProps) {
  return (
    <div
      ref={containerRef}
      className={`designer-canvas ${showGrid ? 'show-grid' : ''}`}
      style={{ '--grid-size': `${gridSnap}%` } as React.CSSProperties}
      onClick={() => onSelectGauge(null)}
      onContextMenu={(e) => onContextMenu(e, null)}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
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
            onMouseDown={(e) => onGaugeMouseDown(e, id, component)}
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
            {isSelected && RESIZE_HANDLES.map(handle => (
              <div
                key={handle}
                className={`resize-handle resize-handle-${handle}`}
                onMouseDown={(e) => onResizeMouseDown(e, handle, id, component)}
              />
            ))}
          </div>
        );
      })}
    </div>
  );
}
