import { useState, useCallback, useEffect, useRef, RefObject } from 'react';
import { DashFile, DashComponent, isGauge, isIndicator } from '../dashTypes';

// 8 resize handles, named after the compass edge/corner they sit on:
//   n/s = north/south (top/bottom edges, control height)
//   e/w = east/west   (right/left edges, control width)
//   ne/nw/se/sw = corners (control both, and also move the opposite corner).
// The mousemove handler below decodes each handle by checking which of
// 'e'/'w'/'n'/'s' it `.includes()` — see that handler for the per-axis logic.
export type ResizeHandle = 'n' | 's' | 'e' | 'w' | 'ne' | 'nw' | 'se' | 'sw';

// In-progress drag. Positions are captured in CLIENT pixels at mousedown and
// converted to relative (0-1) deltas during mousemove by dividing by the
// container's pixel size — that's why the containerRef is required.
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

  // DashFile as it was when the current drag/resize began. Compared by
  // identity at mouseup — any movement produces a new object — so a plain
  // select-click (mousedown + mouseup, no movement) pushes no history entry.
  // Without this, every click stacked a no-op entry that made the next undo
  // appear to do nothing.
  const dragStartFileRef = useRef<DashFile | null>(null);

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
      dragStartFileRef.current = dashFile;
    },
    [onSelectGauge, dashFile],
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
      dragStartFileRef.current = dashFile;
    },
    [dashFile],
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

        // Decode the handle into per-axis effects. Deltas are already in
        // relative (0-1) units, so adding them to width/height/x/y keeps the
        // component consistent. Key insight: dragging an EAST edge grows width
        // (left edge stays), but dragging a WEST edge SHRINKS width AND moves
        // the left edge (x) by the same amount — otherwise the right edge
        // would drift. Same mirror logic for s (bottom) vs n (top).
        const handle = resizeState.handle;
        if (handle.includes('e')) newWidth = snapToGrid(resizeState.startWidth + deltaX);
        if (handle.includes('w')) newWidth = snapToGrid(resizeState.startWidth - deltaX);
        if (handle.includes('s')) newHeight = snapToGrid(resizeState.startHeight + deltaY);
        if (handle.includes('n')) newHeight = snapToGrid(resizeState.startHeight - deltaY);

        const minSize = 0.05;
        newWidth = Math.max(minSize, newWidth);
        newHeight = Math.max(minSize, newHeight);

        // Derive x/y from the already-floor-clamped width/height instead of
        // from the raw mouse delta, so the fixed edge (right edge for a west
        // drag, bottom edge for a north drag) never drifts once minSize is
        // hit. Computing x from the unclamped delta let the right/bottom edge
        // keep sliding with the mouse even after width/height pinned at the
        // floor, since x and width stopped moving in lockstep at that point.
        if (handle.includes('w')) {
          newX = snapToGrid(resizeState.startRelativeX + resizeState.startWidth - newWidth);
        }
        if (handle.includes('n')) {
          newY = snapToGrid(resizeState.startRelativeY + resizeState.startHeight - newHeight);
        }

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
      const startFile = dragStartFileRef.current;
      dragStartFileRef.current = null;

      // `dashFile` here is the post-drag state (the last mousemove's
      // onDashFileChange). Record it only when the interaction actually
      // changed something; identity comparison works because every mousemove
      // produces a fresh object.
      if (startFile && startFile !== dashFile) {
        if (dragState.isDragging) {
          pushHistory(dashFile, `Move ${dragState.gaugeId}`);
        }
        if (resizeState.isResizing) {
          pushHistory(dashFile, `Resize ${resizeState.gaugeId}`);
        }
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
