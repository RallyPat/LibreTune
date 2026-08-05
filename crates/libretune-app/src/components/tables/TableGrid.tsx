import {
  Fragment,
  useState,
  useRef,
  useMemo,
  useCallback,
  useLayoutEffect,
  type CSSProperties,
} from 'react';
import { valueToHeatmapColor, contrastTextColor, HeatmapScheme } from '../../utils/heatmapColors';

export interface SelectionRange {
  start: [number, number];
  end: [number, number];
}

interface TableGridProps {
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  onCellChange: (x: number, y: number, value: number) => void;
  onAxisChange: (axis: 'x' | 'y', index: number, value: number) => void;
  selectionRange: SelectionRange | null;
  onSelectionChange: (range: SelectionRange | null) => void;
  onCellDoubleClick?: (x: number, y: number) => void;
  historyTrail?: [number, number][];
  lockedCells?: Set<string>;
  onCellLock?: (x: number, y: number, locked: boolean) => void;
  isEditing?: boolean;
  canEditZ?: boolean;
  // Live cursor props - shows current ECU operating point
  liveCursorX?: number; // Current X-axis value (e.g., RPM)
  liveCursorY?: number; // Current Y-axis value (e.g., MAP/TPS)
  showLiveCursor?: boolean;
  /** Render rows bottom-up so the origin is at the bottom-left (display only) */
  yAxisBottom?: boolean;
  // Heatmap color props
  showColorShade?: boolean; // Whether to show heatmap colors
  heatmapScheme?: HeatmapScheme | string[]; // Scheme name or custom color stops
  /** Embedded/dialog tables: fixed compact layout (GP#1–4, etc.) */
  compact?: boolean;
  /**
   * VE main table only: shrink cell geometry to fit the parent viewport.
   * Other tables must keep fixed rem sizing.
   */
  fitViewport?: boolean;
}

/** Clean axis/bin display — drop noisy decimals when values are whole. */
function formatBinLabel(val: number): string {
  if (!Number.isFinite(val)) return '';
  const rounded = Math.round(val);
  if (Math.abs(val - rounded) < 1e-6) return String(rounded);
  if (Math.abs(val) >= 100) return val.toFixed(0);
  return val.toFixed(1);
}

/** How often to paint axis labels so they don't overlap (~36px apart). */
function axisLabelStep(count: number, cellPx: number): number {
  if (count <= 12) return 1;
  const maxLabels = Math.max(6, Math.floor((count * cellPx) / 36));
  if (count <= maxLabels) return 1;
  return Math.ceil(count / maxLabels);
}

function shouldShowAxisLabel(index: number, count: number, step: number): boolean {
  return index === 0 || index === count - 1 || index % step === 0;
}

export default function TableGrid({
  x_bins,
  y_bins,
  z_values,
  onCellChange,
  onAxisChange,
  selectionRange,
  onSelectionChange,
  onCellDoubleClick,
  historyTrail,
  lockedCells,
  isEditing = true,
  canEditZ = true,
  liveCursorX,
  liveCursorY,
  showLiveCursor = false,
  showColorShade = true,
  heatmapScheme = 'tunerstudio',
  compact = false,
  yAxisBottom = false,
  fitViewport = false,
}: TableGridProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const [fitPx, setFitPx] = useState<{ axis: number; col: number; row: number } | null>(null);
  const [editingCell, setEditingCell] = useState<[number, number] | null>(null);
  const [editValue, setEditValue] = useState<string>('');
  // cellDrag stores the anchor point of the selection
  const [dragAnchor, setDragAnchor] = useState<[number, number] | null>(null);
  
  // Header interaction state
  const [editingAxis, setEditingAxis] = useState<{ axis: 'x' | 'y', index: number } | null>(null);
  const [headerDragStart, setHeaderDragStart] = useState<{ axis: 'x' | 'y', index: number } | null>(null);

  const x_size = x_bins.length;
  const y_size = y_bins.length;

  // Live cursor position as FRACTIONAL cell indices. Returns null when the
  // cursor is disabled or has no value. Each axis is converted independently
  // from a raw channel value (e.g. RPM) into a cell index + fraction, so the
  // cursor can sit between cells and animate smoothly.
  const liveCursorPosition = useMemo(() => {
    if (!showLiveCursor || liveCursorX === undefined || liveCursorY === undefined) {
      return null;
    }

    // Map a scalar value onto the bin array as a fractional index. Handles
    // both ascending (normal) and descending (reversed) bin orders without
    // assuming direction. Returns `i + ratio` where i is the lower bin index
    // and ratio is the linear interpolation [0,1) across the bin. Values
    // outside the bin range are clamped to the first/last index so the cursor
    // never leaves the table.
    const computePosition = (value: number, bins: number[]) => {
      if (bins.length === 0) return 0;
      const ascending = bins[0] <= bins[bins.length - 1];

      for (let i = 0; i < bins.length - 1; i++) {
        const start = bins[i];
        const end = bins[i + 1];
        // Direction-aware containment check so reversed bins work too.
        const inRange = ascending
          ? value >= start && value <= end
          : value <= start && value >= end;
        if (inRange) {
          // denom===0 means two equal adjacent bins (degenerate); avoid NaN
          // by falling back to ratio 0 (snap to the lower bin).
          const denom = end - start;
          const ratio = denom !== 0 ? (value - start) / denom : 0;
          return i + ratio;
        }
      }

      // Below the first bin → clamp to index 0; above the last → last index.
      // Direction-aware comparisons again so descending bins clamp correctly.
      if ((ascending && value < bins[0]) || (!ascending && value > bins[0])) {
        return 0;
      }
      if ((ascending && value > bins[bins.length - 1]) || (!ascending && value < bins[bins.length - 1])) {
        return bins.length - 1;
      }

      return 0;
    };

    const xPos = computePosition(liveCursorX, x_bins);
    const yPos = computePosition(liveCursorY, y_bins);

    return { x: xPos, y: yPos };
  }, [showLiveCursor, liveCursorX, liveCursorY, x_bins, y_bins]);

  const getCellColor = useCallback((value: number, x: number, y: number) => {
    const cellKey = `${x},${y}`;
    const isLocked = lockedCells?.has(cellKey);

    if (isLocked) {
      return { background: 'var(--surface-dim)' };
    }

    if (!showColorShade) {
      return { background: 'var(--surface)' };
    }

    const minVal = Math.min(...z_values.flat());
    const maxVal = Math.max(...z_values.flat());

    if (minVal === maxVal) return { background: 'var(--surface)' };

    // Use centralized heatmap utility
    const color = valueToHeatmapColor(value, minVal, maxVal, heatmapScheme);
    return { background: color, color: contrastTextColor(color) };
  }, [lockedCells, showColorShade, z_values, heatmapScheme]);

  const handleKeyDown = (e: KeyboardEvent, x: number, y: number) => {
    if (e.key === 'Enter' && editingCell) {
      const newValue = parseFloat(editValue);
      if (!isNaN(newValue)) {
        onCellChange(x, y, newValue);
      }
      setEditingCell(null);
      setEditValue('');
      e.preventDefault();
    } else if (e.key === 'Escape') {
      setEditingCell(null);
      setEditValue('');
      e.preventDefault();
    }
  };

  const handleCellMouseDown = (e: React.MouseEvent, x: number, y: number) => {
    if (e.button === 0 && canEditZ) {
      let anchor: [number, number];
      
      if (e.shiftKey && selectionRange) {
        // Extend selection from existing anchor
        anchor = selectionRange.start;
      } else {
        // Start new selection
        anchor = [x, y];
      }
      
      setDragAnchor(anchor);
      onSelectionChange({ start: anchor, end: [x, y] });
    }
  };

  const handleMouseUp = () => {
    setDragAnchor(null);
    setHeaderDragStart(null);
  };

  const handleHeaderMouseDown = (e: React.MouseEvent, axis: 'x' | 'y', index: number) => {
    if (e.button !== 0) return;
    if (editingAxis?.axis === axis && editingAxis.index === index) return;

    e.preventDefault();
    
    let anchorIndex = index;

    // Shift-click extends a header selection. But we only extend if the
    // PREVIOUS selection was already a full row/column select (i.e. it spans
    // the entire opposite axis) — otherwise a cell-range select would get
    // silently widened into a row/column select, which is surprising. The
    // full-range check (min===0 && max===size-1) is how we tell header-derived
    // selections apart from drag cell selections.
    if (e.shiftKey && selectionRange) {
      if (axis === 'x') {
        // Only extend if previous selection covers full Y range (i.e. was a column selection)
        const minY = Math.min(selectionRange.start[1], selectionRange.end[1]);
        const maxY = Math.max(selectionRange.start[1], selectionRange.end[1]);
        if (minY === 0 && maxY === y_size - 1) {
          anchorIndex = selectionRange.start[0];
        }
      } else {
        // Only extend if previous selection covers full X range (i.e. was a row selection)
        const minX = Math.min(selectionRange.start[0], selectionRange.end[0]);
        const maxX = Math.max(selectionRange.start[0], selectionRange.end[0]);
        if (minX === 0 && maxX === x_size - 1) {
          anchorIndex = selectionRange.start[1];
        }
      }
    }

    setHeaderDragStart({ axis, index: anchorIndex });

    if (axis === 'x') {
      onSelectionChange({ start: [anchorIndex, 0], end: [index, y_size - 1] });
    } else {
      onSelectionChange({ start: [0, anchorIndex], end: [x_size - 1, index] });
    }
  };

  const handleHeaderMouseEnter = (axis: 'x' | 'y', index: number) => {
    if (!headerDragStart) return;
    if (headerDragStart.axis !== axis) return;

    const startIdx = headerDragStart.index;
    if (axis === 'x') {
      onSelectionChange({ start: [startIdx, 0], end: [index, y_size - 1] });
    } else {
      onSelectionChange({ start: [0, startIdx], end: [x_size - 1, index] });
    }
  };

  const handleHeaderDoubleClick = (axis: 'x' | 'y', index: number) => {
    if (!isEditing) return;
    setEditingAxis({ axis, index });
    setEditValue((axis === 'x' ? x_bins : y_bins)[index].toString());
  };

  const handleHeaderBlur = () => {
    if (!editingAxis) return;
    const { axis, index } = editingAxis;
    const newValue = parseFloat(editValue);
    if (!isNaN(newValue)) {
      onAxisChange(axis, index, newValue);
    }
    setEditingAxis(null);
    setEditValue('');
  };

  const handleHeaderKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleHeaderBlur();
    } else if (e.key === 'Escape') {
      setEditingAxis(null);
      setEditValue('');
    }
  };

  const handleCellMouseMove = (e: React.MouseEvent) => {
    if (dragAnchor && gridRef.current) {
      const rect = gridRef.current.getBoundingClientRect();
      const x = Math.floor((e.clientX - rect.left) / (rect.width / x_size));
      const visualY = Math.floor((e.clientY - rect.top) / (rect.height / y_size));
      // Pixel math yields the visual row; map back to the data row when the
      // grid renders bottom-up.
      const y = yAxisBottom ? y_size - 1 - visualY : visualY;

      if (x >= 0 && x < x_size && visualY >= 0 && visualY < y_size) {
        onSelectionChange({ start: dragAnchor, end: [x, y] });
      }
    }
  };


  // Pixel metrics of the data-cell area (the CSS-var percent math was offset
  // by the axis label column/header row and missed cell centering)
  const [cellMetrics, setCellMetrics] = useState<{ l: number; t: number; w: number; h: number } | null>(null);
  useLayoutEffect(() => {
    const grid = gridRef.current;
    if (!grid) return;
    const measure = () => {
      const cell = grid.querySelector('.table-cell') as HTMLElement | null;
      if (cell) {
        setCellMetrics({ l: cell.offsetLeft, t: cell.offsetTop, w: cell.offsetWidth, h: cell.offsetHeight });
      }
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(grid);
    return () => observer.disconnect();
  }, [x_size, y_size, compact]);

  const cellCenter = (x: number, y: number) => {
    if (!cellMetrics) return null;
    const displayY = yAxisBottom ? y_size - 1 - y : y;
    return {
      cx: cellMetrics.l + (x + 0.5) * cellMetrics.w,
      cy: cellMetrics.t + (displayY + 0.5) * cellMetrics.h,
    };
  };

  const renderHistoryTrail = () => {
    if (!historyTrail || historyTrail.length === 0 || !cellMetrics) return null;

    const centers = historyTrail
      .filter(([x, y]) => !lockedCells?.has(`${x},${y}`))
      .map(([x, y]) => cellCenter(x, y))
      .filter((c): c is { cx: number; cy: number } => c !== null);

    if (centers.length === 0) return null;

    return (
      <svg className="history-trail-svg">
        <polyline
          points={centers.map((c) => `${c.cx},${c.cy}`).join(' ')}
          fill="none"
          style={{ stroke: 'var(--cursor-trail, #4A90E2)' }}
          strokeWidth="2"
          strokeOpacity="0.7"
        />
        {centers.map((c, i) => (
          <circle key={i} cx={c.cx} cy={c.cy} r="3" style={{ fill: 'var(--cursor-trail, #4A90E2)' }} fillOpacity="0.8" />
        ))}
      </svg>
    );
  };

  // Fixed column widths — tuning tables must not stretch with the panel.
  // Exception: fitViewport (veTableTbl only) sizes cells from the parent box.
  useLayoutEffect(() => {
    if (!fitViewport) {
      setFitPx(null);
      return;
    }
    const host = hostRef.current;
    if (!host) return;

    const measure = () => {
      const availW = host.clientWidth;
      const availH = host.clientHeight;
      if (availW < 32 || availH < 32 || x_size < 1 || y_size < 1) return;

      // The grid container has a 1px border on every side (2px total per
      // axis). If we fit columns/rows to the raw clientWidth/clientHeight,
      // the border pushes the grid 2px past the host -> a scrollbar appears
      // -> clientWidth shrinks -> ResizeObserver refits -> scrollbar vanishes
      // -> clientWidth snaps back -> overflow again. That oscillation is the
      // visible "twitch". Subtract the border so the fit never overflows.
      const fitW = availW - 2;
      const fitH = availH - 2;

      // Readability floor: never crush below this when fitting. If the table
      // is denser (e.g. 64 RPM bins), keep this size and scroll instead.
      const minCol = 28;
      const minRow = 18;
      const minAxis = 32;
      const preferredCol = compact ? 56 : 48;
      const preferredRow = 32;
      const preferredAxis = compact ? 52 : 44;

      const naturalCol = Math.floor((fitW - minAxis) / x_size);
      let col: number;
      let axis: number;
      if (naturalCol < minCol) {
        col = minCol;
        axis = Math.max(minAxis, Math.min(preferredAxis, Math.floor(fitW * 0.12)));
      } else {
        col = Math.min(preferredCol, naturalCol);
        axis = Math.floor(fitW - col * x_size);
        axis = Math.max(minAxis, Math.min(preferredAxis, axis));
        if (axis + col * x_size > fitW) {
          col = Math.max(minCol, Math.floor((fitW - minAxis) / x_size));
          axis = Math.max(minAxis, fitW - col * x_size);
        }
      }

      const naturalRow = Math.floor(fitH / (y_size + 1));
      const row =
        naturalRow < minRow
          ? minRow
          : Math.max(minRow, Math.min(preferredRow, naturalRow));

      setFitPx((prev) =>
        prev && prev.axis === axis && prev.col === col && prev.row === row
          ? prev
          : { axis, col, row },
      );
    };

    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(host);
    return () => ro.disconnect();
  }, [fitViewport, x_size, y_size, compact]);

  const axisCol = fitPx ? `${fitPx.axis}px` : compact ? '3.25rem' : '2.75rem';
  const dataCol = fitPx ? `${fitPx.col}px` : compact ? '3.5rem' : '3rem';
  const colPx = fitPx?.col ?? (compact ? 56 : 48);
  const xLabelStep = axisLabelStep(x_size, colPx);
  const yLabelStep = axisLabelStep(y_size, fitPx?.row ?? 32);
  const showCellNumbers = colPx >= 26;
  const cellDecimals = colPx < 34 ? 0 : 1;
  const gridTemplateColumns = `${axisCol} repeat(${x_size}, ${dataCol})`;
  const gridStyle: CSSProperties = fitPx
    ? {
        gridTemplateColumns,
        ['--table-row-h' as string]: `${fitPx.row}px`,
        fontSize: `${Math.max(9, Math.min(13, Math.floor(fitPx.col * 0.32)))}px`,
      }
    : { gridTemplateColumns };

  const grid = (
    <div
      ref={gridRef}
      className={`table-grid-container${compact ? ' table-grid-container--compact' : ''}${fitViewport ? ' table-grid-container--fit-ve' : ''}`}
      onMouseUp={handleMouseUp}
      onMouseMove={handleCellMouseMove}
      style={gridStyle}
    >
      {/* Corner cell (row 0, col 0) */}
      <div className="axis-corner" />

      {/* X-axis headers (row 0, cols 1..N) */}
      {x_bins.map((val, i) => {
        const isEditingThis = editingAxis?.axis === 'x' && editingAxis.index === i;
        const isHeaderSelected = selectionRange && 
          i >= Math.min(selectionRange.start[0], selectionRange.end[0]) && 
          i <= Math.max(selectionRange.start[0], selectionRange.end[0]);

        if (isEditingThis) {
          return (
            <input
              key={`x-${i}`}
              type="number"
              step="any"
              value={editValue}
              className="axis-bin-label x-bin editing"
              autoFocus
              onChange={e => setEditValue(e.target.value)}
              onBlur={handleHeaderBlur}
              onKeyDown={handleHeaderKeyDown}
            />
          );
        }

        const label = formatBinLabel(val);
        return (
          <div
            key={`x-${i}`}
            className={`axis-bin-label x-bin ${isHeaderSelected ? 'selected' : ''}${
              shouldShowAxisLabel(i, x_size, xLabelStep) ? '' : ' axis-bin-label--tick'
            }`}
            title={label}
            onMouseDown={e => handleHeaderMouseDown(e, 'x', i)}
            onMouseEnter={() => handleHeaderMouseEnter('x', i)}
            onDoubleClick={() => handleHeaderDoubleClick('x', i)}
          >
            {shouldShowAxisLabel(i, x_size, xLabelStep) ? label : ''}
          </div>
        );
      })}

      {/* Data rows: each row = y-axis label + data cells.
          Display order only — all coordinates stay in data space. */}
      {(yAxisBottom ? [...z_values.keys()].reverse() : [...z_values.keys()]).map((y) => {
        const row = z_values[y];
        const isEditingYAxis = editingAxis?.axis === 'y' && editingAxis.index === y;
        const isYHeaderSelected = selectionRange && 
          y >= Math.min(selectionRange.start[1], selectionRange.end[1]) && 
          y <= Math.max(selectionRange.start[1], selectionRange.end[1]);

        const yLabel = isEditingYAxis ? (
          <input
            key={`y-${y}`}
            type="number"
            step="any"
            value={editValue}
            className="axis-bin-label y-bin editing"
            autoFocus
            onChange={e => setEditValue(e.target.value)}
            onBlur={handleHeaderBlur}
            onKeyDown={handleHeaderKeyDown}
          />
        ) : (
          <div
            key={`y-${y}`}
            className={`axis-bin-label y-bin ${isYHeaderSelected ? 'selected' : ''}${
              shouldShowAxisLabel(y, y_size, yLabelStep) ? '' : ' axis-bin-label--tick'
            }`}
            title={formatBinLabel(y_bins[y])}
            onMouseDown={e => handleHeaderMouseDown(e, 'y', y)}
            onMouseEnter={() => handleHeaderMouseEnter('y', y)}
            onDoubleClick={() => handleHeaderDoubleClick('y', y)}
          >
            {shouldShowAxisLabel(y, y_size, yLabelStep)
              ? formatBinLabel(y_bins[y])
              : ''}
          </div>
        );

        return (
          <Fragment key={`row-${y}`}>
            {yLabel}
            {row.map((value, x) => {
              const cellKey = `${x},${y}`;
              const isLocked = lockedCells?.has(cellKey);
              
              const isSelected = selectionRange && 
                x >= Math.min(selectionRange.start[0], selectionRange.end[0]) && 
                x <= Math.max(selectionRange.start[0], selectionRange.end[0]) &&
                y >= Math.min(selectionRange.start[1], selectionRange.end[1]) && 
                y <= Math.max(selectionRange.start[1], selectionRange.end[1]);

              const isEditingThisCell = editingCell?.[0] === x && editingCell?.[1] === y;
              
              return (
                <div
                  key={cellKey}
                  className={`
                    table-cell 
                    ${isSelected ? 'selected' : ''} 
                    ${isLocked ? 'locked' : ''}
                  `}
                  data-x={x}
                  data-y={y}
                  style={getCellColor(value, x, y)}
                  onMouseDown={e => handleCellMouseDown(e, x, y)}
                  onDoubleClick={() => onCellDoubleClick?.(x, y)}
                  onKeyDown={(e) => handleKeyDown(e.nativeEvent, x, y)}
                >
                  {isEditingThisCell ? (
                    <input
                      type="number"
                      step="1"
                      value={editValue}
                      className="cell-input"
                      autoFocus
                      onChange={e => setEditValue(e.target.value)}
                      onBlur={() => {
                        const newValue = parseFloat(editValue);
                        if (!isNaN(newValue)) {
                          onCellChange(x, y, newValue);
                        }
                        setEditingCell(null);
                        setEditValue('');
                      }}
                    />
                  ) : (
                    <span
                      className={`cell-value ${isSelected ? 'value-selected' : ''}`}
                      title={value.toFixed(2)}
                    >
                      {(showCellNumbers || isSelected)
                        ? value.toFixed(isSelected ? 1 : cellDecimals)
                        : ''}
                    </span>
                  )}
                  {isLocked && <div className="lock-indicator" />}
                </div>
              );
            })}
          </Fragment>
        );
      })}

      {renderHistoryTrail()}
      
      {/* Live Cursor Overlay - shows current ECU operating point */}
      {liveCursorPosition && cellMetrics && (() => {
        const c = cellCenter(liveCursorPosition.x, liveCursorPosition.y);
        return c && (
          <div className="live-cursor-overlay">
            <div className="live-cursor-marker" style={{ left: c.cx, top: c.cy }} />
          </div>
        );
      })()}
    </div>
  );

  if (!fitViewport) {
    return grid;
  }

  return (
    <div ref={hostRef} className="table-grid-fit-host">
      {grid}
    </div>
  );
}
