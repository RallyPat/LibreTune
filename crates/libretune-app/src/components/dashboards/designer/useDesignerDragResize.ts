import { useState, useCallback, useEffect, RefObject } from 'react';
import { DashFile, DashComponent, isGauge, isIndicator } from '../dashTypes';

export type ResizeHandle = 'n' | 's' | 'e' | 'w' | 'ne' | 'nw' | 'se' | 'sw';

interface DragState {
  isDragging: boolean;
  startX: number;
  startY: number;
  startRelativeX: number;
  startRelativeY: number;
  gaugeId: string | null;
}

interface ResizeState {
  isResizing: boolean;
  handle: ResizeHandle | null;
  startX: number;
  startY: number;
  startWidth: number;
  startHeight: number;
  startRelativeX: number;
  startRelativeY: number;
  gaugeId: string | null;
}

interface Options {
  dashFile: DashFile;
  containerRef: RefObject<HTMLDivElement>;
  snapToGrid: (value: number) => number;
  pushHistory: (newFile: DashFile, description: string) => void;
  onDashFileChange: (file: DashFile) => void;
  onSelectGauge: (id: string | null) => void;
}

/**
 * Designer drag/resize interactions: gauge mousedown, resize-handle mousedown,
 * window-level mousemove/mouseup that mutate positions/sizes during a drag.
 * Extracted from DashboardDesigner during Phase D.
 */
export function useDesignerDragResize({
  dashFile,
  containerRef,
  snapToGrid,
  pushHistory,
  onDashFileChange,
  onSelectGauge,
}: Options) {
  const [dragState, setDragState] = useState<DragState>({
    isDragging: false,
    startX: 0,
    startY: 0,
    startRelativeX: 0,
    startRelativeY: 0,
    gaugeId: null,
  });

  const [resizeState, setResizeState] = useState<ResizeState>({
    isResizing: false,
    handle: null,
    startX: 0,
    startY: 0,
    startWidth: 0,
    startHeight: 0,
    startRelativeX: 0,
    startRelativeY: 0,
    gaugeId: null,
  });

  const onGaugeMouseDown = useCallback(
    (e: React.MouseEvent, gaugeId: string, component: DashComponent) => {
      const target = e.target as HTMLElement;
      if (
        target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.tagName === 'SELECT' ||
        target.tagName === 'BUTTON'
      ) {
        return;
      }
      if (e.button !== 0) return;

      onSelectGauge(gaugeId);
      e.preventDefault();
      e.stopPropagation();

      let relX = 0;
      let relY = 0;
      if (isGauge(component)) {
        relX = component.Gauge.relative_x ?? 0;
        relY = component.Gauge.relative_y ?? 0;
      } else if (isIndicator(component)) {
        relX = component.Indicator.relative_x ?? 0;
        relY = component.Indicator.relative_y ?? 0;
      }

      setDragState({
        isDragging: true,
        startX: e.clientX,
        startY: e.clientY,
        startRelativeX: relX,
        startRelativeY: relY,
        gaugeId,
      });
    },
    [onSelectGauge],
  );

  const onResizeMouseDown = useCallback(
    (e: React.MouseEvent, handle: ResizeHandle, gaugeId: string, component: DashComponent) => {
      e.preventDefault();
      e.stopPropagation();

      let relX = 0;
      let relY = 0;
      let width = 0.25;
      let height = 0.25;
      if (isGauge(component)) {
        relX = component.Gauge.relative_x ?? 0;
        relY = component.Gauge.relative_y ?? 0;
        width = component.Gauge.relative_width ?? 0.25;
        height = component.Gauge.relative_height ?? 0.25;
      } else if (isIndicator(component)) {
        relX = component.Indicator.relative_x ?? 0;
        relY = component.Indicator.relative_y ?? 0;
        width = component.Indicator.relative_width ?? 0.1;
        height = component.Indicator.relative_height ?? 0.05;
      }

      setResizeState({
        isResizing: true,
        handle,
        startX: e.clientX,
        startY: e.clientY,
        startWidth: width,
        startHeight: height,
        startRelativeX: relX,
        startRelativeY: relY,
        gaugeId,
      });
    },
    [],
  );

  // Window-level mousemove / mouseup handlers while dragging or resizing.
  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();

      if (dragState.isDragging && dragState.gaugeId) {
        const deltaX = (e.clientX - dragState.startX) / rect.width;
        const deltaY = (e.clientY - dragState.startY) / rect.height;

        let newRelX = snapToGrid(dragState.startRelativeX + deltaX);
        let newRelY = snapToGrid(dragState.startRelativeY + deltaY);
        newRelX = Math.max(0, Math.min(1, newRelX));
        newRelY = Math.max(0, Math.min(1, newRelY));

        const newComponents = dashFile.gauge_cluster.components.map((c) => {
          if (isGauge(c) && c.Gauge.id === dragState.gaugeId) {
            return { Gauge: { ...c.Gauge, relative_x: newRelX, relative_y: newRelY } };
          }
          if (isIndicator(c) && c.Indicator.id === dragState.gaugeId) {
            return { Indicator: { ...c.Indicator, relative_x: newRelX, relative_y: newRelY } };
          }
          return c;
        });

        onDashFileChange({
          ...dashFile,
          gauge_cluster: { ...dashFile.gauge_cluster, components: newComponents },
        });
      }

      if (resizeState.isResizing && resizeState.gaugeId && resizeState.handle) {
        const deltaX = (e.clientX - resizeState.startX) / rect.width;
        const deltaY = (e.clientY - resizeState.startY) / rect.height;

        let newWidth = resizeState.startWidth;
        let newHeight = resizeState.startHeight;
        let newX = resizeState.startRelativeX;
        let newY = resizeState.startRelativeY;

        const handle = resizeState.handle;
        if (handle.includes('e')) newWidth = snapToGrid(resizeState.startWidth + deltaX);
        if (handle.includes('w')) {
          newWidth = snapToGrid(resizeState.startWidth - deltaX);
          newX = snapToGrid(resizeState.startRelativeX + deltaX);
        }
        if (handle.includes('s')) newHeight = snapToGrid(resizeState.startHeight + deltaY);
        if (handle.includes('n')) {
          newHeight = snapToGrid(resizeState.startHeight - deltaY);
          newY = snapToGrid(resizeState.startRelativeY + deltaY);
        }

        const minSize = 0.05;
        newWidth = Math.max(minSize, newWidth);
        newHeight = Math.max(minSize, newHeight);
        newX = Math.max(0, Math.min(1 - newWidth, newX));
        newY = Math.max(0, Math.min(1 - newHeight, newY));

        const newComponents = dashFile.gauge_cluster.components.map((c) => {
          if (isGauge(c) && c.Gauge.id === resizeState.gaugeId) {
            return {
              Gauge: {
                ...c.Gauge,
                relative_x: newX,
                relative_y: newY,
                relative_width: newWidth,
                relative_height: newHeight,
              },
            };
          }
          if (isIndicator(c) && c.Indicator.id === resizeState.gaugeId) {
            return {
              Indicator: {
                ...c.Indicator,
                relative_x: newX,
                relative_y: newY,
                relative_width: newWidth,
                relative_height: newHeight,
              },
            };
          }
          return c;
        });

        onDashFileChange({
          ...dashFile,
          gauge_cluster: { ...dashFile.gauge_cluster, components: newComponents },
        });
      }
    };

    const handleMouseUp = () => {
      if (dragState.isDragging) {
        pushHistory(dashFile, `Move ${dragState.gaugeId}`);
      }
      if (resizeState.isResizing) {
        pushHistory(dashFile, `Resize ${resizeState.gaugeId}`);
      }

      setDragState((prev) => ({ ...prev, isDragging: false, gaugeId: null }));
      setResizeState((prev) => ({ ...prev, isResizing: false, gaugeId: null, handle: null }));
    };

    if (dragState.isDragging || resizeState.isResizing) {
      window.addEventListener('mousemove', handleMouseMove);
      window.addEventListener('mouseup', handleMouseUp);
      return () => {
        window.removeEventListener('mousemove', handleMouseMove);
        window.removeEventListener('mouseup', handleMouseUp);
      };
    }
  }, [dragState, resizeState, dashFile, snapToGrid, pushHistory, onDashFileChange, containerRef]);

  return {
    dragState,
    resizeState,
    onGaugeMouseDown,
    onResizeMouseDown,
  };
}
