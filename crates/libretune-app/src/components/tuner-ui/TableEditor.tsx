import { useState, useCallback, useRef, useEffect, KeyboardEvent, useMemo } from 'react';
import './TableEditor.css';
import TableEditor3DNew from '../tables/TableEditor3DNew';

export interface TableData {
  name: string;
  xAxis: number[];
  yAxis: number[];
  zValues: number[][];
  xLabel?: string;
  yLabel?: string;
  zLabel?: string;
  xUnits?: string;
  yUnits?: string;
  zUnits?: string;
  min?: number;
  max?: number;
  precision?: number;
  /** Output channel name for X-axis (used for live cell highlighting) */
  xOutputChannel?: string;
  /** Output channel name for Y-axis (used for live cell highlighting) */
  yOutputChannel?: string;
}

export interface CellPosition {
  row: number;
  col: number;
}

interface TableEditorProps {
  data: TableData;
  onChange: (newData: TableData) => void;
  onBurn?: () => void;
  followMode?: boolean;
  livePosition?: CellPosition | null;
  showHistoryTrail?: boolean;
  /** Real-time data dictionary for live cell highlighting */
  realtimeData?: Record<string, number>;
}

// Selection can be a single cell or a range
interface Selection {
  start: CellPosition;
  end: CellPosition;
}

// Context menu state
interface ContextMenuState {
  x: number;
  y: number;
  visible: boolean;
}

// Increment settings for step operations
interface IncrementSettings {
  stepAmount: number;      // Amount for > < keys
  stepCount: number;       // Multiplier when Ctrl is held
  stepPercent: number;     // Percentage for Shift operations
}

export function TableEditor({
  data,
  onChange,
  onBurn,
  followMode: _followMode = false,
  livePosition = null,
  showHistoryTrail: _showHistoryTrail = false,
  realtimeData,
}: TableEditorProps) {
  const [selection, setSelection] = useState<Selection | null>(null);
  const [isSelecting, setIsSelecting] = useState(false);
  const [editingCell, setEditingCell] = useState<CellPosition | null>(null);
  const [editValue, setEditValue] = useState('');
  const [clipboard, setClipboard] = useState<number[][] | null>(null);
  const [history, setHistory] = useState<TableData[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const tableRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  
  // Follow mode state
  const [followMode, setFollowMode] = useState(false);
  const [historyTrail, setHistoryTrail] = useState<Array<{ row: number; col: number; time: number }>>([]);
  const TRAIL_DURATION_MS = 3000; // 3 second trail
  
  // Context menu state
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({ x: 0, y: 0, visible: false });
  
  // Original values for reset functionality
  const [originalData, setOriginalData] = useState<TableData | null>(null);
  
  // Increment settings
  const [incrementSettings, setIncrementSettings] = useState<IncrementSettings>({
    stepAmount: 0.1,
    stepCount: 10,
    stepPercent: 1,
  });
  
  // Track if heatmap coloring is enabled
  const [heatmapEnabled, setHeatmapEnabled] = useState(true);
  
  // Track if 3D view is enabled
  const [show3D, setShow3D] = useState(false);
  
  // Store original data on first render
  useEffect(() => {
    if (!originalData) {
      setOriginalData(JSON.parse(JSON.stringify(data)));
    }
  }, [data, originalData]);

  // Helper: Find nearest bin index for a given value
  const findNearestBinIndex = useCallback((value: number, bins: number[]): number => {
    if (bins.length === 0) return 0;
    let nearestIdx = 0;
    let minDiff = Math.abs(bins[0] - value);
    for (let i = 1; i < bins.length; i++) {
      const diff = Math.abs(bins[i] - value);
      if (diff < minDiff) {
        minDiff = diff;
        nearestIdx = i;
      }
    }
    return nearestIdx;
  }, []);

  // Calculate live cursor position from realtime data
  const calculatedLivePosition = useMemo((): CellPosition | null => {
    if (!followMode || !realtimeData) return null;
    
    const xChannel = data.xOutputChannel;
    const yChannel = data.yOutputChannel;
    
    if (!xChannel) return null;
    
    const xValue = realtimeData[xChannel];
    if (xValue === undefined) return null;
    
    const col = findNearestBinIndex(xValue, data.xAxis);
    
    // For 1D curves (single row), row is always 0
    if (!yChannel || data.yAxis.length <= 1) {
      return { row: 0, col };
    }
    
    const yValue = realtimeData[yChannel];
    if (yValue === undefined) return { row: 0, col };
    
    const row = findNearestBinIndex(yValue, data.yAxis);
    return { row, col };
  }, [followMode, realtimeData, data.xOutputChannel, data.yOutputChannel, data.xAxis, data.yAxis, findNearestBinIndex]);

  // Merge prop-passed livePosition with calculated one (calculated takes precedence when followMode is on)
  const effectiveLivePosition = followMode ? calculatedLivePosition : livePosition;

  // Update history trail when live position changes
  useEffect(() => {
    if (!followMode || !effectiveLivePosition) return;
    
    const now = Date.now();
    const newEntry = { row: effectiveLivePosition.row, col: effectiveLivePosition.col, time: now };
    
    setHistoryTrail((prev) => {
      // Remove old entries
      const filtered = prev.filter((entry) => now - entry.time < TRAIL_DURATION_MS);
      
      // Only add if position changed from last entry
      const last = filtered[filtered.length - 1];
      if (last && last.row === newEntry.row && last.col === newEntry.col) {
        return filtered;
      }
      
      return [...filtered, newEntry];
    });
  }, [followMode, effectiveLivePosition, TRAIL_DURATION_MS]);

  // Periodically clean up old trail entries
  useEffect(() => {
    if (!followMode) {
      setHistoryTrail([]);
      return;
    }
    
    const interval = setInterval(() => {
      const now = Date.now();
      setHistoryTrail((prev) => prev.filter((entry) => now - entry.time < TRAIL_DURATION_MS));
    }, 200);
    
    return () => clearInterval(interval);
  }, [followMode, TRAIL_DURATION_MS]);

  // Calculate color for value based on min/max
  const getValueColor = useCallback((value: number) => {
    if (!heatmapEnabled) return 'var(--table-cell-bg)';
    
    const min = data.min ?? Math.min(...data.zValues.flat());
    const max = data.max ?? Math.max(...data.zValues.flat());
    const range = max - min;
    if (range === 0) return 'var(--table-cell-bg)';
    
    const normalized = (value - min) / range;
    // Blue (rich) to Red (lean) gradient
    const r = Math.round(normalized * 200);
    const b = Math.round((1 - normalized) * 200);
    return `rgb(${r}, 60, ${b})`;
  }, [data.min, data.max, data.zValues, heatmapEnabled]);

  // Get selected cells as array of positions
  const getSelectedCells = useCallback((): CellPosition[] => {
    if (!selection) return [];
    
    const cells: CellPosition[] = [];
    const minRow = Math.min(selection.start.row, selection.end.row);
    const maxRow = Math.max(selection.start.row, selection.end.row);
    const minCol = Math.min(selection.start.col, selection.end.col);
    const maxCol = Math.max(selection.start.col, selection.end.col);
    
    for (let row = minRow; row <= maxRow; row++) {
      for (let col = minCol; col <= maxCol; col++) {
        cells.push({ row, col });
      }
    }
    return cells;
  }, [selection]);

  // Push to history before making changes
  const pushHistory = useCallback(() => {
    const newHistory = history.slice(0, historyIndex + 1);
    newHistory.push(JSON.parse(JSON.stringify(data)));
    setHistory(newHistory);
    setHistoryIndex(newHistory.length - 1);
  }, [data, history, historyIndex]);

  // Undo
  const undo = useCallback(() => {
    if (historyIndex >= 0) {
      onChange(history[historyIndex]);
      setHistoryIndex(historyIndex - 1);
    }
  }, [history, historyIndex, onChange]);

  // Redo
  const redo = useCallback(() => {
    if (historyIndex < history.length - 1) {
      setHistoryIndex(historyIndex + 1);
      onChange(history[historyIndex + 1]);
    }
  }, [history, historyIndex, onChange]);

  // Table operations
  const setEqual = useCallback((value: number) => {
    const cells = getSelectedCells();
    if (cells.length === 0) return;
    
    pushHistory();
    const newZValues = data.zValues.map((row) => [...row]);
    cells.forEach(({ row, col }) => {
      newZValues[row][col] = value;
    });
    onChange({ ...data, zValues: newZValues });
  }, [data, getSelectedCells, onChange, pushHistory]);

  const adjustValues = useCallback((delta: number) => {
    const cells = getSelectedCells();
    if (cells.length === 0) return;
    
    pushHistory();
    const newZValues = data.zValues.map((row) => [...row]);
    cells.forEach(({ row, col }) => {
      newZValues[row][col] = Number((newZValues[row][col] + delta).toFixed(data.precision ?? 2));
    });
    onChange({ ...data, zValues: newZValues });
  }, [data, getSelectedCells, onChange, pushHistory]);

  const scaleValues = useCallback((factor: number) => {
    const cells = getSelectedCells();
    if (cells.length === 0) return;
    
    pushHistory();
    const newZValues = data.zValues.map((row) => [...row]);
    cells.forEach(({ row, col }) => {
      newZValues[row][col] = Number((newZValues[row][col] * factor).toFixed(data.precision ?? 2));
    });
    onChange({ ...data, zValues: newZValues });
  }, [data, getSelectedCells, onChange, pushHistory]);

  const interpolate = useCallback(() => {
    const cells = getSelectedCells();
    if (cells.length < 3) return; // Need at least 3 cells
    
    pushHistory();
    const newZValues = data.zValues.map((row) => [...row]);
    
    // Determine if horizontal or vertical interpolation
    const rows = [...new Set(cells.map((c) => c.row))].sort((a, b) => a - b);
    const cols = [...new Set(cells.map((c) => c.col))].sort((a, b) => a - b);
    
    if (rows.length === 1 && cols.length > 2) {
      // Horizontal interpolation
      const row = rows[0];
      const startVal = newZValues[row][cols[0]];
      const endVal = newZValues[row][cols[cols.length - 1]];
      const step = (endVal - startVal) / (cols.length - 1);
      
      cols.forEach((col, i) => {
        newZValues[row][col] = Number((startVal + step * i).toFixed(data.precision ?? 2));
      });
    } else if (cols.length === 1 && rows.length > 2) {
      // Vertical interpolation
      const col = cols[0];
      const startVal = newZValues[rows[0]][col];
      const endVal = newZValues[rows[rows.length - 1]][col];
      const step = (endVal - startVal) / (rows.length - 1);
      
      rows.forEach((row, i) => {
        newZValues[row][col] = Number((startVal + step * i).toFixed(data.precision ?? 2));
      });
    }
    
    onChange({ ...data, zValues: newZValues });
  }, [data, getSelectedCells, onChange, pushHistory]);

  const smooth = useCallback(() => {
    const cells = getSelectedCells();
    if (cells.length === 0) return;
    
    pushHistory();
    const newZValues = data.zValues.map((row) => [...row]);
    
    // Apply 3x3 weighted average smoothing
    cells.forEach(({ row, col }) => {
      let sum = 0;
      let weight = 0;
      
      for (let dr = -1; dr <= 1; dr++) {
        for (let dc = -1; dc <= 1; dc++) {
          const r = row + dr;
          const c = col + dc;
          if (r >= 0 && r < data.zValues.length && c >= 0 && c < data.zValues[0].length) {
            const w = dr === 0 && dc === 0 ? 2 : 1;
            sum += data.zValues[r][c] * w;
            weight += w;
          }
        }
      }
      
      newZValues[row][col] = Number((sum / weight).toFixed(data.precision ?? 2));
    });
    
    onChange({ ...data, zValues: newZValues });
  }, [data, getSelectedCells, onChange, pushHistory]);

  // Interpolate horizontal only (H key)
  const interpolateHorizontal = useCallback(() => {
    const cells = getSelectedCells();
    if (cells.length < 2) return;
    
    pushHistory();
    const newZValues = data.zValues.map((row) => [...row]);
    
    // Group by row and interpolate each row independently
    const rowGroups = new Map<number, number[]>();
    cells.forEach(({ row, col }) => {
      if (!rowGroups.has(row)) rowGroups.set(row, []);
      rowGroups.get(row)!.push(col);
    });
    
    rowGroups.forEach((cols, row) => {
      cols.sort((a, b) => a - b);
      if (cols.length < 2) return;
      
      const startVal = newZValues[row][cols[0]];
      const endVal = newZValues[row][cols[cols.length - 1]];
      const step = (endVal - startVal) / (cols.length - 1);
      
      cols.forEach((col, i) => {
        newZValues[row][col] = Number((startVal + step * i).toFixed(data.precision ?? 2));
      });
    });
    
    onChange({ ...data, zValues: newZValues });
  }, [data, getSelectedCells, onChange, pushHistory]);

  // Interpolate vertical only (V key)
  const interpolateVertical = useCallback(() => {
    const cells = getSelectedCells();
    if (cells.length < 2) return;
    
    pushHistory();
    const newZValues = data.zValues.map((row) => [...row]);
    
    // Group by column and interpolate each column independently
    const colGroups = new Map<number, number[]>();
    cells.forEach(({ row, col }) => {
      if (!colGroups.has(col)) colGroups.set(col, []);
      colGroups.get(col)!.push(row);
    });
    
    colGroups.forEach((rows, col) => {
      rows.sort((a, b) => a - b);
      if (rows.length < 2) return;
      
      const startVal = newZValues[rows[0]][col];
      const endVal = newZValues[rows[rows.length - 1]][col];
      const step = (endVal - startVal) / (rows.length - 1);
      
      rows.forEach((row, i) => {
        newZValues[row][col] = Number((startVal + step * i).toFixed(data.precision ?? 2));
      });
    });
    
    onChange({ ...data, zValues: newZValues });
  }, [data, getSelectedCells, onChange, pushHistory]);

  // Flood fill (fill up and right from selection) - F key
  const floodFill = useCallback(() => {
    if (!selection) return;
    
    pushHistory();
    const newZValues = data.zValues.map((row) => [...row]);
    const sourceRow = Math.min(selection.start.row, selection.end.row);
    const sourceCol = Math.min(selection.start.col, selection.end.col);
    const value = data.zValues[sourceRow][sourceCol];
    
    // Fill from source cell up to top-right corner
    for (let row = sourceRow; row >= 0; row--) {
      for (let col = sourceCol; col < data.zValues[0].length; col++) {
        newZValues[row][col] = value;
      }
    }
    
    onChange({ ...data, zValues: newZValues });
  }, [selection, data, onChange, pushHistory]);

  // Reset to original values (Escape with selection)
  const resetToOriginal = useCallback(() => {
    const cells = getSelectedCells();
    if (cells.length === 0 || !originalData) return;
    
    pushHistory();
    const newZValues = data.zValues.map((row) => [...row]);
    cells.forEach(({ row, col }) => {
      if (row < originalData.zValues.length && col < originalData.zValues[0].length) {
        newZValues[row][col] = originalData.zValues[row][col];
      }
    });
    
    onChange({ ...data, zValues: newZValues });
  }, [data, getSelectedCells, onChange, pushHistory, originalData]);

  // Select all cells (Ctrl+A)
  const selectAll = useCallback(() => {
    if (data.zValues.length === 0 || data.zValues[0].length === 0) return;
    setSelection({
      start: { row: 0, col: 0 },
      end: { row: data.zValues.length - 1, col: data.zValues[0].length - 1 }
    });
  }, [data.zValues]);

  // Copy/paste
  const copySelection = useCallback(() => {
    if (!selection) return;
    
    const minRow = Math.min(selection.start.row, selection.end.row);
    const maxRow = Math.max(selection.start.row, selection.end.row);
    const minCol = Math.min(selection.start.col, selection.end.col);
    const maxCol = Math.max(selection.start.col, selection.end.col);
    
    const copied: number[][] = [];
    for (let row = minRow; row <= maxRow; row++) {
      const rowData: number[] = [];
      for (let col = minCol; col <= maxCol; col++) {
        rowData.push(data.zValues[row][col]);
      }
      copied.push(rowData);
    }
    setClipboard(copied);
  }, [selection, data.zValues]);

  const pasteSelection = useCallback(() => {
    if (!selection || !clipboard) return;
    
    pushHistory();
    const newZValues = data.zValues.map((row) => [...row]);
    const startRow = Math.min(selection.start.row, selection.end.row);
    const startCol = Math.min(selection.start.col, selection.end.col);
    
    clipboard.forEach((row, dr) => {
      row.forEach((value, dc) => {
        const r = startRow + dr;
        const c = startCol + dc;
        if (r < newZValues.length && c < newZValues[0].length) {
          newZValues[r][c] = value;
        }
      });
    });
    
    onChange({ ...data, zValues: newZValues });
  }, [selection, clipboard, data, onChange, pushHistory]);

  // Handle cell click
  const handleCellMouseDown = useCallback((row: number, col: number, e: React.MouseEvent) => {
    if (e.shiftKey && selection) {
      // Extend selection
      setSelection({ ...selection, end: { row, col } });
    } else {
      // Start new selection
      setSelection({ start: { row, col }, end: { row, col } });
      setIsSelecting(true);
    }
  }, [selection]);

  const handleCellMouseEnter = useCallback((row: number, col: number) => {
    if (isSelecting && selection) {
      setSelection({ ...selection, end: { row, col } });
    }
  }, [isSelecting, selection]);

  const handleMouseUp = useCallback(() => {
    setIsSelecting(false);
  }, []);

  // Handle cell double-click for editing
  const handleCellDoubleClick = useCallback((row: number, col: number) => {
    setEditingCell({ row, col });
    setEditValue(String(data.zValues[row][col]));
  }, [data.zValues]);

  // Handle edit completion
  const finishEdit = useCallback((save: boolean) => {
    if (editingCell && save) {
      const value = parseFloat(editValue);
      if (!isNaN(value)) {
        pushHistory();
        const newZValues = data.zValues.map((row) => [...row]);
        newZValues[editingCell.row][editingCell.col] = Number(value.toFixed(data.precision ?? 2));
        onChange({ ...data, zValues: newZValues });
      }
    }
    setEditingCell(null);
    setEditValue('');
  }, [editingCell, editValue, data, onChange, pushHistory]);

  // Keyboard navigation
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (editingCell) {
      if (e.key === 'Enter') {
        finishEdit(true);
      } else if (e.key === 'Escape') {
        finishEdit(false);
      }
      return;
    }

    if (!selection) return;

    const { row, col } = selection.end;
    const multiplier = e.ctrlKey ? 5 : 1;
    const delta = e.shiftKey ? 1 : 0.1;

    switch (e.key) {
      case 'ArrowUp':
        e.preventDefault();
        if (row > 0) {
          const newPos = { row: row - 1, col };
          setSelection({ start: e.shiftKey ? selection.start : newPos, end: newPos });
        }
        break;
      case 'ArrowDown':
        e.preventDefault();
        if (row < data.zValues.length - 1) {
          const newPos = { row: row + 1, col };
          setSelection({ start: e.shiftKey ? selection.start : newPos, end: newPos });
        }
        break;
      case 'ArrowLeft':
        e.preventDefault();
        if (col > 0) {
          const newPos = { row, col: col - 1 };
          setSelection({ start: e.shiftKey ? selection.start : newPos, end: newPos });
        }
        break;
      case 'ArrowRight':
        e.preventDefault();
        if (col < data.zValues[0].length - 1) {
          const newPos = { row, col: col + 1 };
          setSelection({ start: e.shiftKey ? selection.start : newPos, end: newPos });
        }
        break;
      case '=':
        e.preventDefault();
        const avgValue = getSelectedCells().reduce((sum, c) => sum + data.zValues[c.row][c.col], 0) / getSelectedCells().length;
        setEqual(Number(avgValue.toFixed(data.precision ?? 2)));
        break;
      case '>':
      case '.':
        e.preventDefault();
        adjustValues(delta * multiplier);
        break;
      case '<':
      case ',':
        e.preventDefault();
        adjustValues(-delta * multiplier);
        break;
      case '+':
        e.preventDefault();
        adjustValues(1 * multiplier);
        break;
      case '-':
        e.preventDefault();
        adjustValues(-1 * multiplier);
        break;
      case '*':
        e.preventDefault();
        scaleValues(1.01 * multiplier);
        break;
      case '/':
        e.preventDefault();
        interpolate();
        break;
      case 's':
      case 'S':
        e.preventDefault();
        smooth();
        break;
      case 'f':
      case 'F':
        if (!e.ctrlKey) {
          e.preventDefault();
          if (data.xOutputChannel) {
            setFollowMode(!followMode);
          }
        }
        break;
      case 'c':
        if (e.ctrlKey) {
          e.preventDefault();
          copySelection();
        }
        break;
      case 'z':
        if (e.ctrlKey) {
          e.preventDefault();
          undo();
        }
        break;
      case 'y':
        if (e.ctrlKey) {
          e.preventDefault();
          redo();
        }
        break;
      case 'a':
        if (e.ctrlKey) {
          e.preventDefault();
          selectAll();
        }
        break;
      case 'h':
      case 'H':
        e.preventDefault();
        interpolateHorizontal();
        break;
      case 'v':
        if (e.ctrlKey) {
          e.preventDefault();
          pasteSelection();
        } else {
          e.preventDefault();
          interpolateVertical();
        }
        break;
      case 'Escape':
        e.preventDefault();
        resetToOriginal();
        break;
      case 'Enter':
        e.preventDefault();
        handleCellDoubleClick(row, col);
        break;
    }
  }, [
    selection, editingCell, data, finishEdit, getSelectedCells, setEqual,
    adjustValues, scaleValues, interpolate, interpolateHorizontal, interpolateVertical,
    smooth, copySelection, pasteSelection, selectAll, resetToOriginal, floodFill,
    undo, redo, handleCellDoubleClick, followMode, setFollowMode
  ]);

  // Focus input when editing
  useEffect(() => {
    if (editingCell && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [editingCell]);

  // Add mouse up listener
  useEffect(() => {
    document.addEventListener('mouseup', handleMouseUp);
    return () => document.removeEventListener('mouseup', handleMouseUp);
  }, [handleMouseUp]);

  // Check if cell is selected
  const isCellSelected = useCallback((row: number, col: number) => {
    if (!selection) return false;
    const minRow = Math.min(selection.start.row, selection.end.row);
    const maxRow = Math.max(selection.start.row, selection.end.row);
    const minCol = Math.min(selection.start.col, selection.end.col);
    const maxCol = Math.max(selection.start.col, selection.end.col);
    return row >= minRow && row <= maxRow && col >= minCol && col <= maxCol;
  }, [selection]);

  // Format value for display
  const formatValue = useCallback((value: number) => {
    return value.toFixed(data.precision ?? 1);
  }, [data.precision]);

  // Context menu handler
  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
  }, []);

  // Close context menu
  const closeContextMenu = useCallback(() => {
    setContextMenu({ ...contextMenu, visible: false });
  }, [contextMenu]);

  // Close context menu on click outside
  useEffect(() => {
    const handleClick = () => closeContextMenu();
    if (contextMenu.visible) {
      document.addEventListener('click', handleClick);
      return () => document.removeEventListener('click', handleClick);
    }
  }, [contextMenu.visible, closeContextMenu]);

  return (
    <div 
      className="table-editor" 
      ref={tableRef} 
      tabIndex={0} 
      onKeyDown={handleKeyDown}
      onContextMenu={handleContextMenu}
    >
      {/* Context Menu */}
      {contextMenu.visible && (
        <TableContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={closeContextMenu}
          hasSelection={!!selection}
          hasClipboard={!!clipboard}
          onResetToOriginal={resetToOriginal}
          onSetValue={() => {
            const value = prompt('Enter value:');
            if (value) setEqual(parseFloat(value));
            closeContextMenu();
          }}
          onStepUp={() => { adjustValues(incrementSettings.stepAmount); closeContextMenu(); }}
          onStepDown={() => { adjustValues(-incrementSettings.stepAmount); closeContextMenu(); }}
          onAddAmount={() => {
            const amt = prompt('Enter amount to add:');
            if (amt) adjustValues(parseFloat(amt));
            closeContextMenu();
          }}
          onSubtractAmount={() => {
            const amt = prompt('Enter amount to subtract:');
            if (amt) adjustValues(-parseFloat(amt));
            closeContextMenu();
          }}
          onMultiplyBy={() => {
            const factor = prompt('Enter multiplier (e.g., 1.02 for +2%):');
            if (factor) scaleValues(parseFloat(factor));
            closeContextMenu();
          }}
          onInterpolate={() => { interpolate(); closeContextMenu(); }}
          onInterpolateHorizontal={() => { interpolateHorizontal(); closeContextMenu(); }}
          onInterpolateVertical={() => { interpolateVertical(); closeContextMenu(); }}
          onSmooth={() => { smooth(); closeContextMenu(); }}
          onFloodFill={() => { floodFill(); closeContextMenu(); }}
          onCopy={() => { copySelection(); closeContextMenu(); }}
          onPaste={() => { pasteSelection(); closeContextMenu(); }}
          onSetStepAmount={() => {
            const amt = prompt('Enter step amount:', String(incrementSettings.stepAmount));
            if (amt) setIncrementSettings({ ...incrementSettings, stepAmount: parseFloat(amt) });
            closeContextMenu();
          }}
          onSetStepCount={() => {
            const count = prompt('Enter step multiplier (Ctrl key):', String(incrementSettings.stepCount));
            if (count) setIncrementSettings({ ...incrementSettings, stepCount: parseInt(count) });
            closeContextMenu();
          }}
          onSetStepPercent={() => {
            const pct = prompt('Enter step percent (Shift key):', String(incrementSettings.stepPercent));
            if (pct) setIncrementSettings({ ...incrementSettings, stepPercent: parseFloat(pct) });
            closeContextMenu();
          }}
          onToggleHeatmap={() => { setHeatmapEnabled(!heatmapEnabled); closeContextMenu(); }}
          heatmapEnabled={heatmapEnabled}
        />
      )}

      {/* Toolbar */}
      <TableToolbar
        onSetEqual={() => {
          const value = prompt('Enter value:');
          if (value) setEqual(parseFloat(value));
        }}
        onIncrease={() => adjustValues(0.1)}
        onDecrease={() => adjustValues(-0.1)}
        onIncreaseMore={() => adjustValues(1)}
        onDecreaseMore={() => adjustValues(-1)}
        onScale={() => {
          const factor = prompt('Enter scale factor (e.g., 1.02 for +2%):');
          if (factor) scaleValues(parseFloat(factor));
        }}
        onInterpolate={interpolate}
        onSmooth={smooth}
        onCopy={copySelection}
        onPaste={pasteSelection}
        onUndo={undo}
        onRedo={redo}
        onBurn={onBurn}
        hasSelection={!!selection}
        hasClipboard={!!clipboard}
        canUndo={historyIndex >= 0}
        canRedo={historyIndex < history.length - 1}
        followMode={followMode}
        onToggleFollowMode={() => setFollowMode(!followMode)}
        hasOutputChannels={!!data.xOutputChannel}
        show3D={show3D}
        onToggle3D={() => setShow3D(!show3D)}
      />

      {/* 3D View */}
      {show3D && (
        <TableEditor3DNew
          title={data.name}
          x_bins={data.xAxis}
          y_bins={data.yAxis}
          z_values={data.zValues}
          x_label={data.xLabel}
          y_label={data.yLabel}
          z_label={data.zLabel}
          x_units={data.xUnits}
          y_units={data.yUnits}
          z_units={data.zUnits}
          onBack={() => setShow3D(false)}
          selectedCell={selection ? { x: selection.start.col, y: selection.start.row } : null}
          liveCell={livePosition ? { x: livePosition.col, y: livePosition.row } : null}
        />
      )}

      {/* Table */}
      {!show3D && (
      <div className="table-grid-container">
        <table className="table-grid">
          <thead>
            <tr>
              <th className="table-corner">
                {data.yLabel || 'Y'} / {data.xLabel || 'X'}
              </th>
              {data.xAxis.map((x, i) => (
                <th key={i} className="table-x-header">
                  {x}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.yAxis.map((y, rowIndex) => (
              <tr key={rowIndex}>
                <th className="table-y-header">{y}</th>
                {data.xAxis.map((_, colIndex) => {
                  const value = data.zValues[rowIndex][colIndex];
                  const isSelected = isCellSelected(rowIndex, colIndex);
                  const isEditing = editingCell?.row === rowIndex && editingCell?.col === colIndex;
                  const isLive = effectiveLivePosition?.row === rowIndex && effectiveLivePosition?.col === colIndex;
                  
                  // Check if cell is in trail and calculate opacity
                  const now = Date.now();
                  const trailEntry = historyTrail.find((e) => e.row === rowIndex && e.col === colIndex);
                  const trailOpacity = trailEntry ? Math.max(0, 1 - (now - trailEntry.time) / TRAIL_DURATION_MS) : 0;
                  const isInTrail = trailOpacity > 0 && !isLive;

                  return (
                    <td
                      key={colIndex}
                      className={`table-cell ${isSelected ? 'selected' : ''} ${isLive ? 'live' : ''} ${isInTrail ? 'trail' : ''}`}
                      style={{ 
                        backgroundColor: getValueColor(value),
                        ...(isInTrail && { '--trail-opacity': trailOpacity } as React.CSSProperties)
                      }}
                      onMouseDown={(e) => handleCellMouseDown(rowIndex, colIndex, e)}
                      onMouseEnter={() => handleCellMouseEnter(rowIndex, colIndex)}
                      onDoubleClick={() => handleCellDoubleClick(rowIndex, colIndex)}
                    >
                      {isEditing ? (
                        <input
                          ref={inputRef}
                          type="text"
                          className="table-cell-input"
                          value={editValue}
                          onChange={(e) => setEditValue(e.target.value)}
                          onBlur={() => finishEdit(true)}
                        />
                      ) : (
                        formatValue(value)
                      )}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      )}

      {/* Status */}
      <div className="table-status">
        <span>{data.name}</span>
        {selection && (
          <span>
            Selected: {getSelectedCells().length} cell(s)
          </span>
        )}
        {data.zUnits && <span>Units: {data.zUnits}</span>}
      </div>
    </div>
  );
}

interface TableToolbarProps {
  onSetEqual: () => void;
  onIncrease: () => void;
  onDecrease: () => void;
  onIncreaseMore: () => void;
  onDecreaseMore: () => void;
  onScale: () => void;
  onInterpolate: () => void;
  onSmooth: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onBurn?: () => void;
  hasSelection: boolean;
  hasClipboard: boolean;
  canUndo: boolean;
  canRedo: boolean;
  followMode: boolean;
  onToggleFollowMode: () => void;
  hasOutputChannels: boolean;
  show3D: boolean;
  onToggle3D: () => void;
}

function TableToolbar({
  onSetEqual,
  onIncrease,
  onDecrease,
  onIncreaseMore,
  onDecreaseMore,
  onScale,
  onInterpolate,
  onSmooth,
  onCopy,
  onPaste,
  onUndo,
  onRedo,
  onBurn,
  hasSelection,
  hasClipboard,
  canUndo,
  canRedo,
  followMode,
  onToggleFollowMode,
  hasOutputChannels,
  show3D,
  onToggle3D,
}: TableToolbarProps) {
  return (
    <div className="table-toolbar">
      <div className="table-toolbar-group">
        <button
          className="table-toolbar-btn"
          onClick={onSetEqual}
          disabled={!hasSelection}
          title="Set Equal (=)"
        >
          =
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onDecrease}
          disabled={!hasSelection}
          title="Decrease (&lt;)"
        >
          &lt;
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onIncrease}
          disabled={!hasSelection}
          title="Increase (&gt;)"
        >
          &gt;
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onDecreaseMore}
          disabled={!hasSelection}
          title="Decrease More (-)"
        >
          −
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onIncreaseMore}
          disabled={!hasSelection}
          title="Increase More (+)"
        >
          +
        </button>
      </div>

      <div className="table-toolbar-separator" />

      <div className="table-toolbar-group">
        <button
          className="table-toolbar-btn"
          onClick={onScale}
          disabled={!hasSelection}
          title="Scale (*)"
        >
          ×
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onInterpolate}
          disabled={!hasSelection}
          title="Interpolate (/)"
        >
          /
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onSmooth}
          disabled={!hasSelection}
          title="Smooth (s)"
        >
          s
        </button>
      </div>

      <div className="table-toolbar-separator" />

      <div className="table-toolbar-group">
        <button
          className="table-toolbar-btn"
          onClick={onCopy}
          disabled={!hasSelection}
          title="Copy (Ctrl+C)"
        >
          📋
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onPaste}
          disabled={!hasClipboard}
          title="Paste (Ctrl+V)"
        >
          📄
        </button>
      </div>

      <div className="table-toolbar-separator" />

      <div className="table-toolbar-group">
        <button
          className="table-toolbar-btn"
          onClick={onUndo}
          disabled={!canUndo}
          title="Undo (Ctrl+Z)"
        >
          ↩
        </button>
        <button
          className="table-toolbar-btn"
          onClick={onRedo}
          disabled={!canRedo}
          title="Redo (Ctrl+Y)"
        >
          ↪
        </button>
      </div>

      {onBurn && (
        <>
          <div className="table-toolbar-separator" />
          <button
            className="table-toolbar-btn table-toolbar-btn-burn"
            onClick={onBurn}
            title="Burn to ECU"
          >
            🔥 Burn
          </button>
        </>
      )}

      <div className="table-toolbar-separator" />
      
      <button
        className={`table-toolbar-btn table-toolbar-btn-follow ${followMode ? 'active' : ''}`}
        onClick={onToggleFollowMode}
        disabled={!hasOutputChannels}
        title={hasOutputChannels ? `Follow Mode (F) - ${followMode ? 'ON' : 'OFF'}` : 'Follow Mode unavailable (no output channels defined)'}
      >
        🎯 Follow
      </button>
      
      <button
        className={`table-toolbar-btn table-toolbar-btn-3d ${show3D ? 'active' : ''}`}
        onClick={onToggle3D}
        title={`3D View - ${show3D ? 'ON' : 'OFF'}`}
      >
        🧊 3D
      </button>
    </div>
  );
}

// Context menu component for table operations
interface TableContextMenuProps {
  x: number;
  y: number;
  onClose: () => void;
  hasSelection: boolean;
  hasClipboard: boolean;
  onResetToOriginal: () => void;
  onSetValue: () => void;
  onStepUp: () => void;
  onStepDown: () => void;
  onAddAmount: () => void;
  onSubtractAmount: () => void;
  onMultiplyBy: () => void;
  onInterpolate: () => void;
  onInterpolateHorizontal: () => void;
  onInterpolateVertical: () => void;
  onSmooth: () => void;
  onFloodFill: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onSetStepAmount: () => void;
  onSetStepCount: () => void;
  onSetStepPercent: () => void;
  onToggleHeatmap: () => void;
  heatmapEnabled: boolean;
}

function TableContextMenu({
  x,
  y,
  onClose: _onClose,
  hasSelection,
  hasClipboard,
  onResetToOriginal,
  onSetValue,
  onStepUp,
  onStepDown,
  onAddAmount,
  onSubtractAmount,
  onMultiplyBy,
  onInterpolate,
  onInterpolateHorizontal,
  onInterpolateVertical,
  onSmooth,
  onFloodFill,
  onCopy,
  onPaste,
  onSetStepAmount,
  onSetStepCount,
  onSetStepPercent,
  onToggleHeatmap,
  heatmapEnabled,
}: TableContextMenuProps) {
  return (
    <div 
      className="table-context-menu" 
      style={{ left: x, top: y }}
      onClick={(e) => e.stopPropagation()}
    >
      <button className="context-menu-item" onClick={onResetToOriginal} disabled={!hasSelection}>
        Reset to Original <span className="shortcut">Esc</span>
      </button>
      
      <div className="context-menu-separator" />
      
      <button className="context-menu-item" onClick={onSetValue} disabled={!hasSelection}>
        Set Value <span className="shortcut">=</span>
      </button>
      <button className="context-menu-item" onClick={onStepUp} disabled={!hasSelection}>
        Step Up <span className="shortcut">&gt; or ,</span>
      </button>
      <button className="context-menu-item" onClick={onStepDown} disabled={!hasSelection}>
        Step Down <span className="shortcut">&lt; or .</span>
      </button>
      <button className="context-menu-item" onClick={onAddAmount} disabled={!hasSelection}>
        Add Amount <span className="shortcut">+</span>
      </button>
      <button className="context-menu-item" onClick={onSubtractAmount} disabled={!hasSelection}>
        Subtract Amount <span className="shortcut">-</span>
      </button>
      <button className="context-menu-item" onClick={onMultiplyBy} disabled={!hasSelection}>
        Multiply By <span className="shortcut">*</span>
      </button>
      
      <div className="context-menu-separator" />
      
      <button className="context-menu-item" onClick={onInterpolate} disabled={!hasSelection}>
        Auto-Fill <span className="shortcut">/</span>
      </button>
      <button className="context-menu-item" onClick={onInterpolateHorizontal} disabled={!hasSelection}>
        Fill Horizontal <span className="shortcut">H</span>
      </button>
      <button className="context-menu-item" onClick={onInterpolateVertical} disabled={!hasSelection}>
        Fill Vertical <span className="shortcut">V</span>
      </button>
      <button className="context-menu-item" onClick={onSmooth} disabled={!hasSelection}>
        Blend Selection <span className="shortcut">S</span>
      </button>
      <button className="context-menu-item" onClick={onFloodFill} disabled={!hasSelection}>
        Flood Fill <span className="shortcut">F</span>
      </button>
      
      <div className="context-menu-separator" />
      
      <button className="context-menu-item" onClick={onCopy} disabled={!hasSelection}>
        Copy <span className="shortcut">Ctrl+C</span>
      </button>
      <button className="context-menu-item" onClick={onPaste} disabled={!hasClipboard}>
        Paste <span className="shortcut">Ctrl+V</span>
      </button>
      
      <div className="context-menu-separator" />
      
      <button className="context-menu-item" onClick={onSetStepAmount}>
        Set Step Amount...
      </button>
      <button className="context-menu-item" onClick={onSetStepCount}>
        Set Step Multiplier (Ctrl)...
      </button>
      <button className="context-menu-item" onClick={onSetStepPercent}>
        Set Step Percent (Shift)...
      </button>
      
      <div className="context-menu-separator" />
      
      <button className="context-menu-item" onClick={onToggleHeatmap}>
        {heatmapEnabled ? '✓ ' : ''}Cell Color By Value
      </button>
    </div>
  );
}
