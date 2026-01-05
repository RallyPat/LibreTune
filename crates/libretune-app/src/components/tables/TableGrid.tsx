import { useState, useRef, useMemo } from 'react';

interface TableGridProps {
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  onCellChange: (x: number, y: number, value: number) => void;
  onAxisChange: (axis: 'x' | 'y', index: number, value: number) => void;
  selectedCell: [number, number] | null;
  onCellSelect: (x: number, y: number) => void;
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
}

export default function TableGrid({
  x_bins,
  y_bins,
  z_values,
  onCellChange,
  onAxisChange,
  selectedCell,
  onCellSelect,
  onCellDoubleClick,
  historyTrail,
  lockedCells,
  onCellLock,
  isEditing = true,
  canEditZ = true,
  liveCursorX,
  liveCursorY,
  showLiveCursor = false,
}: TableGridProps) {
  const gridRef = useRef<HTMLDivElement>(null);
  const [editingCell, setEditingCell] = useState<[number, number] | null>(null);
  const [editValue, setEditValue] = useState<string>('');
  const [cellDrag, setCellDrag] = useState<[number, number] | null>(null);

  const x_size = x_bins.length;
  const y_size = y_bins.length;

  // Calculate live cursor position (fractional cell indices)
  const liveCursorPosition = useMemo(() => {
    if (!showLiveCursor || liveCursorX === undefined || liveCursorY === undefined) {
      return null;
    }

    // Find X position (interpolate between bins)
    let xPos = 0;
    for (let i = 0; i < x_bins.length - 1; i++) {
      if (liveCursorX >= x_bins[i] && liveCursorX <= x_bins[i + 1]) {
        const ratio = (liveCursorX - x_bins[i]) / (x_bins[i + 1] - x_bins[i]);
        xPos = i + ratio;
        break;
      } else if (liveCursorX < x_bins[0]) {
        xPos = 0;
        break;
      } else if (liveCursorX > x_bins[x_bins.length - 1]) {
        xPos = x_bins.length - 1;
        break;
      }
    }

    // Find Y position (interpolate between bins) - Y axis is usually reversed (high values at top)
    let yPos = 0;
    for (let i = 0; i < y_bins.length - 1; i++) {
      if (liveCursorY >= y_bins[i] && liveCursorY <= y_bins[i + 1]) {
        const ratio = (liveCursorY - y_bins[i]) / (y_bins[i + 1] - y_bins[i]);
        yPos = i + ratio;
        break;
      } else if (liveCursorY < y_bins[0]) {
        yPos = 0;
        break;
      } else if (liveCursorY > y_bins[y_bins.length - 1]) {
        yPos = y_bins.length - 1;
        break;
      }
    }

    return { x: xPos, y: yPos };
  }, [showLiveCursor, liveCursorX, liveCursorY, x_bins, y_bins]);

  const getCellColor = (value: number, x: number, y: number) => {
    const cellKey = `${x},${y}`;
    const isLocked = lockedCells?.has(cellKey);

    if (isLocked) {
      return { background: 'var(--surface-dim)' };
    }

    const minVal = Math.min(...z_values.flat());
    const maxVal = Math.max(...z_values.flat());
    const range = maxVal - minVal;

    if (range === 0) return { background: 'var(--surface)' };

    const ratio = (value - minVal) / range;
    const hue = (1 - ratio) * 240;
    return { background: `hsl(${hue}, 60%, 45%)` };
  };

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
      setCellDrag([x, y]);
      const cellKey = `${x},${y}`;
      const isLocked = lockedCells?.has(cellKey);
      if (e.shiftKey && isLocked !== undefined) {
        onCellLock?.(x, y, !isLocked);
      } else if (!e.shiftKey) {
        onCellSelect(x, y);
      }
    }
  };

  const handleMouseUp = () => {
    setCellDrag(null);
  };

  const handleCellMouseMove = (e: React.MouseEvent) => {
    if (cellDrag && gridRef.current) {
      const rect = gridRef.current.getBoundingClientRect();
      const x = Math.floor((e.clientX - rect.left) / (rect.width / x_size));
      const y = Math.floor((e.clientY - rect.top) / (rect.height / y_size));
      
      if (x >= 0 && x < x_size && y >= 0 && y < y_size) {
        onCellSelect(x, y);
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
        {y_bins.map((val, i) => (
          <input
            key={`y-${i}`}
            type="number"
            step="1"
            value={val}
            className="axis-bin-label y-bin"
            disabled={!isEditing}
            onChange={e => onAxisChange('y', i, parseFloat(e.target.value))}
          />
        ))}
      </div>

      <div className="x-axis-bins">
        {x_bins.map((val, i) => (
          <input
            key={`x-${i}`}
            type="number"
            step="1"
            value={val}
            className="axis-bin-label x-bin"
            disabled={!isEditing}
            onChange={e => onAxisChange('x', i, parseFloat(e.target.value))}
          />
        ))}
      </div>

      <div className="table-cells">
        {z_values.map((row, y) => (
          <div key={`row-${y}`} className="table-row">
            {row.map((value, x) => {
              const cellKey = `${x},${y}`;
              const isLocked = lockedCells?.has(cellKey);
              const isSelected = selectedCell?.[0] === x && selectedCell?.[1] === y;
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
