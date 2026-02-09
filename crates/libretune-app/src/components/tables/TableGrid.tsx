import { useState, useRef, useMemo, useCallback } from 'react';
import { valueToHeatmapColor, HeatmapScheme } from '../../utils/heatmapColors';

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
  // Heatmap color props
  showColorShade?: boolean; // Whether to show heatmap colors
  heatmapScheme?: HeatmapScheme | string[]; // Scheme name or custom color stops
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
}: TableGridProps) {
  const gridRef = useRef<HTMLDivElement>(null);
  const [editingCell, setEditingCell] = useState<[number, number] | null>(null);
  const [editValue, setEditValue] = useState<string>('');
  // cellDrag stores the anchor point of the selection
  const [dragAnchor, setDragAnchor] = useState<[number, number] | null>(null);
  
  // Header interaction state
  const [editingAxis, setEditingAxis] = useState<{ axis: 'x' | 'y', index: number } | null>(null);
  const [headerDragStart, setHeaderDragStart] = useState<{ axis: 'x' | 'y', index: number } | null>(null);

  const x_size = x_bins.length;
  const y_size = y_bins.length;

  // Calculate live cursor position (fractional cell indices)
  const liveCursorPosition = useMemo(() => {
    if (!showLiveCursor || liveCursorX === undefined || liveCursorY === undefined) {
      return null;
    }

    const computePosition = (value: number, bins: number[]) => {
      if (bins.length === 0) return 0;
      const ascending = bins[0] <= bins[bins.length - 1];

      for (let i = 0; i < bins.length - 1; i++) {
        const start = bins[i];
        const end = bins[i + 1];
        const inRange = ascending
          ? value >= start && value <= end
          : value <= start && value >= end;
        if (inRange) {
          const denom = end - start;
          const ratio = denom !== 0 ? (value - start) / denom : 0;
          return i + ratio;
        }
      }

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
    return { background: color };
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

    // Check if we should extend the selection (Shift key + existing compatible selection)
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
      const y = Math.floor((e.clientY - rect.top) / (rect.height / y_size));
      
      if (x >= 0 && x < x_size && y >= 0 && y < y_size) {
        onSelectionChange({ start: dragAnchor, end: [x, y] });
      }
    }
  };


  const renderHistoryTrail = () => {
    if (!historyTrail || historyTrail.length === 0) return null;

    const points = historyTrail.map(([x, y]) => {
      const cellKey = `${x},${y}`;
      if (lockedCells?.has(cellKey)) return null;

      const left = (x / x_size) * 100;
      const top = (y / y_size) * 100;

      return `${left},${top}`;
    }).filter(Boolean) as string[];

    const pointElements = historyTrail.map(([x, y], i) => {
      const cellKey = `${x},${y}`;
      if (lockedCells?.has(cellKey)) return null;

      const left = (x / x_size) * 100;
      const top = (y / y_size) * 100;

      return (
        <div
          key={`trail-${i}`}
          className="history-trail-point"
          style={{ left: `${left}%`, top: `${top}%` }}
        />
      );
    }).filter(Boolean);

    if (points.length === 0) return null;

    return (
      <svg className="history-trail-svg">
        <polyline
          points={points.join(' ')}
          fill="none"
          stroke="#4A90E2"
          strokeWidth="2"
          strokeOpacity="0.7"
        />
        {pointElements}
      </svg>
    );
  };

  return (
    <div 
      ref={gridRef}
      className="table-grid-container"
      onMouseUp={handleMouseUp}
      onMouseMove={handleCellMouseMove}
    >
      <div className="y-axis-labels">
        {y_bins.map((val, i) => {
          const isEditingThis = editingAxis?.axis === 'y' && editingAxis.index === i;
          const isHeaderSelected = selectionRange && 
            i >= Math.min(selectionRange.start[1], selectionRange.end[1]) && 
            i <= Math.max(selectionRange.start[1], selectionRange.end[1]);
          
          if (isEditingThis) {
            return (
              <input
                key={`y-${i}`}
                type="number"
                step="any"
                value={editValue}
                className="axis-bin-label y-bin editing"
                autoFocus
                onChange={e => setEditValue(e.target.value)}
                onBlur={handleHeaderBlur}
                onKeyDown={handleHeaderKeyDown}
              />
            );
          }

          return (
            <div
              key={`y-${i}`}
              className={`axis-bin-label y-bin ${isHeaderSelected ? 'selected' : ''}`}
              onMouseDown={e => handleHeaderMouseDown(e, 'y', i)}
              onMouseEnter={() => handleHeaderMouseEnter('y', i)}
              onDoubleClick={() => handleHeaderDoubleClick('y', i)}
            >
              {val}
            </div>
          );
        })}
      </div>

      <div 
        className="x-axis-bins"
        style={{ marginLeft: '3.5rem' }}
      >
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

          return (
            <div
              key={`x-${i}`}
              className={`axis-bin-label x-bin ${isHeaderSelected ? 'selected' : ''}`}
              onMouseDown={e => handleHeaderMouseDown(e, 'x', i)}
              onMouseEnter={() => handleHeaderMouseEnter('x', i)}
              onDoubleClick={() => handleHeaderDoubleClick('x', i)}
            >
              {val}
            </div>
          );
        })}
      </div>

      <div 
        className="table-cells"
        style={{ gridTemplateColumns: `repeat(${x_bins.length}, 3rem)` }}
      >
        {z_values.map((row, y) => (
          <div key={`row-${y}`} className="table-row">
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
                    <span className={`cell-value ${isSelected ? 'value-selected' : ''}`}>
                      {value.toFixed(1)}
                    </span>
                  )}
                  {isLocked && <div className="lock-indicator" />}
                </div>
              );
            })}
          </div>
        ))}
      </div>

      {renderHistoryTrail()}
      
      {/* Live Cursor Overlay - shows current ECU operating point */}
      {liveCursorPosition && (
        <div 
          className="live-cursor-overlay"
          style={{
            '--cursor-x': liveCursorPosition.x,
            '--cursor-y': liveCursorPosition.y,
            '--cols': x_size,
            '--rows': y_size,
          } as React.CSSProperties}
        >
          <div className="live-cursor-marker" />
        </div>
      )}
    </div>
  );
}
