import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ArrowLeft, Save, Zap, ExternalLink } from 'lucide-react';
import TableToolbar from './TableToolbar';
import TableGrid from './TableGrid';
import TableContextMenu from './TableContextMenu';
import RebinDialog from '../dialogs/RebinDialog';
import CellEditDialog from '../dialogs/CellEditDialog';
import { useHeatmapSettings } from '../../utils/useHeatmapSettings';
import { useChannels } from '../../stores/realtimeStore';
import { useToast } from '../ToastContext';
import './TableComponents.css';
import './TableEditor2D.css';

type TableOperationResult = {
  table_name: string;
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
};

/**
 * Props for the TableEditor2D component.
 */
interface TableEditor2DProps {
  /** Display title for the table */
  title: string;
  /** Internal name used for backend operations */
  table_name: string;
  /** Label for the X axis (columns) */
  x_axis_name: string;
  /** Label for the Y axis (rows) */
  y_axis_name: string;
  /** X axis bin values (column headers) */
  x_bins: number[];
  /** Y axis bin values (row headers) */
  y_bins: number[];
  /** 2D array of Z values [row][col] */
  z_values: number[][];
  /** Output channel name for X-axis (used for live cursor) */
  x_output_channel?: string | null;
  /** Output channel name for Y-axis (used for live cursor) */
  y_output_channel?: string | null;
  /** Callback when back button is clicked (optional for embedded mode) */
  onBack?: () => void;
  /** Compact mode for embedding in dialogs */
  embedded?: boolean;
  /** Callback to open this table in a separate tab */
  onOpenInTab?: () => void;
  /** Callback when cell values are modified */
  onValuesChange?: (values: number[][]) => void;
}

/**
 * State for the re-bin dialog.
 */
interface RebinDialogState {
  show: boolean;
  newXBins: number[];
  newYBins: number[];
}

/**
 * State for the cell edit dialog.
 */
interface CellEditDialogState {
  show: boolean;
  row: number;
  col: number;
  value: number;
}

/**
 * TableEditor2D - A comprehensive 2D table editor for ECU calibration data.
 * 
 * Features:
 * - Cell selection (click, shift-click, ctrl-click, drag)
 * - Value editing (direct input, increment/decrement)
 * - Bulk operations (set equal, scale, smooth, interpolate)
 * - Copy/paste with smart selection
 * - Undo/redo support
 * - Color-coded cell values with heatmap visualization
 * - Live cursor showing current ECU operating point
 * - History trail showing recent operating positions
 * - Re-binning with automatic Z-value interpolation
 * - Context menu for additional operations
 * 
 * @example
 * ```tsx
 * <TableEditor2D
 *   title="VE Table 1"
 *   table_name="veTable1Tbl"
 *   x_axis_name="RPM"
 *   y_axis_name="MAP"
 *   x_bins={[500, 1000, 1500, ...]}
 *   y_bins={[20, 40, 60, ...]}
 *   z_values={[[50, 52, ...], ...]}
 *   onBack={() => closeTab()}
 * />
 * ```
 */
export default function TableEditor2D({
  title,
  table_name,
  x_axis_name,
  y_axis_name,
  x_bins,
  y_bins,
  z_values,
  x_output_channel,
  y_output_channel,
  onBack,
  embedded = false,
  onOpenInTab,
  onValuesChange,
}: TableEditor2DProps) {
  // Determine if data is valid - used for conditional rendering after hooks
  const hasValidData = 
    z_values && Array.isArray(z_values) && z_values.length > 0 &&
    x_bins && Array.isArray(x_bins) && x_bins.length > 0 &&
    y_bins && Array.isArray(y_bins) && y_bins.length > 0;

  // Get realtime data from Zustand store for live cursor (ECU-agnostic)
  const outputChannels = useMemo(() => {
    const channels: string[] = [];
    if (x_output_channel) channels.push(x_output_channel);
    if (y_output_channel) channels.push(y_output_channel);
    if (channels.length === 0) {
      channels.push('rpm', 'map');
    }
    return channels;
  }, [x_output_channel, y_output_channel]);
  const realtimeData = useChannels(outputChannels);
  
  // Use safe fallback values for hooks when data is invalid
  const safeZValues = hasValidData ? z_values : [[0]];
  const safeXBins = hasValidData ? x_bins : [0];
  const safeYBins = hasValidData ? y_bins : [0];
  
  const [localZValues, setLocalZValues] = useState<number[][]>([...safeZValues]);
  const [selectedCells, setSelectedCells] = useState<Set<string>>(new Set());
  const [lockedCells, setLockedCells] = useState<Set<string>>(new Set());
  const [historyTrail, setHistoryTrail] = useState<[number, number][]>([]);
  const [showColorShade, setShowColorShade] = useState(true);
  const [showHistoryTrail, setShowHistoryTrail] = useState(false);
  const [clipboard, setClipboard] = useState<[number, number][]>([]);
  const [canUndo, setCanUndo] = useState(false);
  const [rebinDialog, setRebinDialog] = useState<RebinDialogState>({
    show: false,
    newXBins: [...safeXBins],
    newYBins: [...safeYBins],
  });
  const [contextMenu, setContextMenu] = useState<{
    visible: boolean;
    x: number;
    y: number;
    value: number;
    position?: { top: number; left: number };
  }>({ visible: false, x: 0, y: 0, value: 0 });

  const [cellEditDialog, setCellEditDialog] = useState<CellEditDialogState>({
    show: false,
    row: 0,
    col: 0,
    value: 0,
  });

  const [followMode, setFollowMode] = useState(false);
  const [activeCell, setActiveCell] = useState<[number, number] | null>(null);

  const { showToast } = useToast();
  
  // Get heatmap scheme from user settings
  const { settings: heatmapSettings } = useHeatmapSettings();

  const selectedCellsPayload = useMemo(
    () => Array.from(selectedCells).map((key) => {
      const [x, y] = key.split(',').map(Number);
      return [y, x] as [number, number]; // Backend expects (row, col)
    }),
    [selectedCells]
  );

  const handleOperationError = useCallback(
    (operation: string, err: unknown) => {
      console.error(`${operation} failed:`, err);
      const message = err instanceof Error ? err.message : String(err ?? 'Unknown error');
      showToast(`${operation} failed: ${message}`, 'error');
    },
    [showToast]
  );

  useEffect(() => {
    const interval = setInterval(() => {
      if (followMode && activeCell) {
        const trail = [...historyTrail.slice(-50), activeCell];
        setHistoryTrail(trail);
      }
    }, 500);
    return () => clearInterval(interval);
  }, [followMode, activeCell]);

  // Keyboard event handling for TS-style hotkeys
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't handle if typing in an input
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return;
      }

      const isCtrl = e.ctrlKey || e.metaKey;
      const isShift = e.shiftKey;
      const multiplier = isCtrl ? 5 : 1; // Ctrl = 5x increment

      switch (e.key) {
        // Navigation
        case 'ArrowUp':
        case 'ArrowDown':
        case 'ArrowLeft':
        case 'ArrowRight':
          e.preventDefault();
          handleArrowNavigation(e.key, isShift);
          break;

        // Cell operations
        case '=':
          e.preventDefault();
          handleSetEqual();
          break;
        case '>':
        case '.':
        case 'q':
          e.preventDefault();
          handleIncrease(multiplier);
          break;
        case '<':
        case ',':
          e.preventDefault();
          handleDecrease(multiplier);
          break;
        case '+':
          e.preventDefault();
          handleIncrease(10 * multiplier);
          break;
        case '-':
        case '_':
          e.preventDefault();
          handleDecrease(10 * multiplier);
          break;
        case '*':
          e.preventDefault();
          handleScale(1.0);
          break;
        case '/':
          e.preventDefault();
          handleInterpolate();
          break;
        case 's':
        case 'S':
          if (!isCtrl) {
            e.preventDefault();
            handleSmooth();
          }
          break;

        // View controls
        case 'f':
        case 'F':
          if (!isCtrl) {
            e.preventDefault();
            setFollowMode(!followMode);
          }
          break;
        case 'g':
        case 'G':
          e.preventDefault();
          // Go to live position (jump to active cell)
          if (activeCell) {
            setSelectedCells(new Set([`${activeCell[0]},${activeCell[1]}`]));
          }
          break;

        // Copy/Paste
        case 'c':
        case 'C':
          if (isCtrl) {
            e.preventDefault();
            handleCopy();
          }
          break;
        case 'v':
        case 'V':
          if (isCtrl) {
            e.preventDefault();
            handlePaste();
          }
          break;
        case 'z':
        case 'Z':
          if (isCtrl) {
            e.preventDefault();
            handleUndo();
          }
          break;

        // Escape to clear selection
        case 'Escape':
          setSelectedCells(new Set());
          setContextMenu({ visible: false, x: 0, y: 0, value: 0 });
          break;
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [selectedCells, followMode, activeCell, localZValues]);

  // Arrow key navigation helper
  const handleArrowNavigation = (key: string, extendSelection: boolean) => {
    if (selectedCells.size === 0) {
      // Start from top-left if no selection
      setSelectedCells(new Set(['0,0']));
      return;
    }

    // Get current anchor cell (first selected)
    const firstCell = Array.from(selectedCells)[0];
    const [currentX, currentY] = firstCell.split(',').map(Number);

    let newX = currentX;
    let newY = currentY;

    switch (key) {
      case 'ArrowUp':
        newY = Math.max(0, currentY - 1);
        break;
      case 'ArrowDown':
        newY = Math.min(y_bins.length - 1, currentY + 1);
        break;
      case 'ArrowLeft':
        newX = Math.max(0, currentX - 1);
        break;
      case 'ArrowRight':
        newX = Math.min(x_bins.length - 1, currentX + 1);
        break;
    }

    if (extendSelection) {
      // Extend selection to include new cell
      const newSelection = new Set(selectedCells);
      newSelection.add(`${newX},${newY}`);
      setSelectedCells(newSelection);
    } else {
      // Move to new cell
      setSelectedCells(new Set([`${newX},${newY}`]));
    }
  };

  const handleCellChange = (x: number, y: number, value: number) => {
    const newValues = [...localZValues];
    newValues[y][x] = value;
    setLocalZValues(newValues);
    setSelectedCells(new Set([`${x},${y}`]));
    setCanUndo(true);
    onValuesChange?.(newValues);
  };

  const handleAxisChange = (axis: 'x' | 'y', index: number, value: number) => {
    if (axis === 'x') {
      const newBins = [...x_bins];
      newBins[index] = value;
      setRebinDialog(prev => ({ ...prev, newXBins: newBins }));
    } else {
      const newBins = [...y_bins];
      newBins[index] = value;
      setRebinDialog(prev => ({ ...prev, newYBins: newBins }));
    }
  };

  const handleSetEqual = async () => {
    const values = Array.from(selectedCells).map(key => {
      const [x, y] = key.split(',').map(Number);
      return { x, y, value: localZValues[y][x] };
    });

    if (values.length === 0) return;

    const avgValue = values.reduce((sum, v) => sum + v.value, 0) / values.length;

    try {
      const result = await invoke<TableOperationResult>('set_cells_equal', {
        table_name,
        selected_cells: selectedCellsPayload,
        value: avgValue
      });
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        onValuesChange?.(result.z_values);
        setCanUndo(true);
      }
    } catch (err) {
      handleOperationError('Set equal', err);
    }
  };

  const handleSetEqualWrapper = () => {
    setContextMenu({ visible: false, x: 0, y: 0, value: 0 });
    handleSetEqual();
  };

  const handleScaleWrapper = () => {
    handleScale(1.0);
  };

  const handleCellSelectWrapper = (x: number, y: number) => {
    handleCellSelect(x, y, false);
  };

  const handleContextMenuSetEqual = (_value: number) => {
    setContextMenu({ visible: false, x: 0, y: 0, value: 0 });
    handleSetEqual();
  };

  const handleContextMenuScale = (factor: number) => {
    setContextMenu({ visible: false, x: 0, y: 0, value: 0 });
    handleScale(factor);
  };

  const handleIncrease = (amount: number) => {
    const values = Array.from(selectedCells).map(key => {
      const [x, y] = key.split(',').map(Number);
      return { x, y, value: localZValues[y][x] };
    });
    
    values.forEach(({ x, y, value }) => {
      handleCellChange(x, y, value * (1 + amount));
    });
  };

  const handleDecrease = (amount: number) => {
    const values = Array.from(selectedCells).map(key => {
      const [x, y] = key.split(',').map(Number);
      return { x, y, value: localZValues[y][x] };
    });
    
    values.forEach(({ x, y, value }) => {
      handleCellChange(x, y, value * (1 - amount));
    });
  };

  const handleScale = async (factor: number) => {
    try {
      const result = await invoke<TableOperationResult>('scale_cells', {
        table_name,
        selected_cells: selectedCellsPayload,
        scale_factor: factor
      });
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        onValuesChange?.(result.z_values);
        setCanUndo(true);
      }
    } catch (err) {
      handleOperationError('Scale', err);
    }
  };

  const handleSmooth = async () => {
    try {
      const result = await invoke<TableOperationResult>('smooth_table', {
        table_name,
        selected_cells: selectedCellsPayload,
        factor: 1.0
      });
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        onValuesChange?.(result.z_values);
        setCanUndo(true);
      }
    } catch (err) {
      handleOperationError('Smooth', err);
    }
  };

  const handleInterpolate = async () => {
    try {
      const result = await invoke<TableOperationResult>('interpolate_cells', {
        table_name,
        selected_cells: selectedCellsPayload
      });
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        onValuesChange?.(result.z_values);
        setCanUndo(true);
      }
    } catch (err) {
      handleOperationError('Interpolate', err);
    }
  };

  const handleRebin = async (newXBins: number[], newYBins: number[], interpolateZ: boolean) => {
    setRebinDialog({ show: false, newXBins, newYBins });
    
    try {
      const result = await invoke<TableOperationResult>('rebin_table', {
        table_name,
        new_x_bins: newXBins,
        new_y_bins: newYBins,
        interpolate_z: interpolateZ
      });
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        onValuesChange?.(result.z_values);
        setCanUndo(true);
        setSelectedCells(new Set());
      }
    } catch (err) {
      handleOperationError('Rebin', err);
    }
  };

  const handleCellEditApply = (value: number) => {
    handleCellChange(cellEditDialog.col, cellEditDialog.row, value);
  };

  const handleCellDoubleClick = (x: number, y: number) => {
    setCellEditDialog({
      show: true,
      row: y,
      col: x,
      value: localZValues[y][x],
    });
  };

  const handleCopy = () => {
    const cells: [number, number][] = Array.from(selectedCells).map(key => {
      const parts = key.split(',').map(Number);
      return [parts[0], parts[1]] as [number, number];
    });
    setClipboard(cells);
  };

  const handlePaste = () => {
    if (clipboard.length === 0) return;

    clipboard.forEach(([x, y]) => {
      if (x >= 0 && x < x_bins.length && y >= 0 && y < y_bins.length) {
        const targetKey = `${x},${y}`;
        if (!lockedCells.has(targetKey)) {
          handleCellChange(x, y, localZValues[y][x]);
        }
      }
    });
  };

  const handleUndo = () => {
    invoke('update_table_data', {
      table_name,
      z_values
    }).then(() => {
      setLocalZValues(z_values);
      setSelectedCells(new Set());
      setCanUndo(false);
    });
  };

  const handleCellLock = (x: number, y: number, locked: boolean) => {
    const key = `${x},${y}`;
    const newLocked = new Set(lockedCells);
    if (locked) {
      newLocked.add(key);
    } else {
      newLocked.delete(key);
    }
    setLockedCells(newLocked);
  };

  const handleCellSelect = (x: number, y: number, shiftKey: boolean) => {
    if (shiftKey) {
      const newSelected = new Set(selectedCells);
      newSelected.add(`${x},${y}`);
      setSelectedCells(newSelected);
    } else {
      setSelectedCells(new Set([`${x},${y}`]));
    }
    setActiveCell([x, y]);
    setContextMenu({ visible: false, x: 0, y: 0, value: 0 });
  };

  const handleSave = () => {
    invoke('update_table_data', {
      table_name,
      z_values: localZValues
    }).then(() => {
      setCanUndo(false);
    });
  };

  const handleRightClick = (e: React.MouseEvent, x: number, y: number) => {
    e.preventDefault();
    const rect = (e.target as HTMLElement).getBoundingClientRect();
    setContextMenu({
      visible: true,
      x,
      y,
      value: localZValues[y][x],
      position: {
        top: rect.top,
        left: rect.left
      }
    });
  };

  // Render error state if data is invalid (after all hooks have been called)
  if (!hasValidData) {
    const getErrorMessage = () => {
      if (!z_values || !Array.isArray(z_values) || z_values.length === 0) {
        return `No Z-values available for table "${title || table_name}". The table data may be missing or improperly formatted in the tune file.`;
      }
      if (!x_bins || !Array.isArray(x_bins) || x_bins.length === 0) {
        return `No X-axis bins available for table "${title || table_name}".`;
      }
      if (!y_bins || !Array.isArray(y_bins) || y_bins.length === 0) {
        return `No Y-axis bins available for table "${title || table_name}".`;
      }
      return 'Unknown table data error.';
    };

    return (
      <div className="table-editor" style={{ padding: '20px', textAlign: 'center' }}>
        <h3 style={{ color: 'var(--error)', marginBottom: '8px' }}>⚠️ Table Data Error</h3>
        <p style={{ color: 'var(--text-muted)' }}>{getErrorMessage()}</p>
        {onBack && (
          <button onClick={onBack} style={{ marginTop: '16px' }} className="btn btn-secondary">
            ← Go Back
          </button>
        )}
      </div>
    );
  }

  return (
    <div className={`table-editor-2d ${embedded ? 'embedded' : 'standalone'}`}>
      {/* Embedded mode: compact title bar with pop-out button */}
      {embedded && (
        <div className="embedded-header">
          <span className="embedded-title">{title}</span>
          <button 
            className={`embedded-toggle ${showColorShade ? 'active' : ''}`}
            onClick={() => setShowColorShade(!showColorShade)}
            title="Toggle Color Shade"
          >
            🎨
          </button>
          {onOpenInTab && (
            <button 
              className="pop-out-btn" 
              onClick={onOpenInTab}
              title="Open in new tab"
            >
              <ExternalLink size={14} />
            </button>
          )}
        </div>
      )}

      {/* Standalone mode: full header with back button and actions */}
      {!embedded && (
        <div className="editor-header">
          <button className="back-btn" onClick={onBack}>
            <ArrowLeft size={18} />
            <span>Back</span>
          </button>
          <h1>{title}</h1>
          <div className="editor-actions">
            <button 
              className={`action-btn ${showColorShade ? 'active' : ''}`}
              onClick={() => setShowColorShade(!showColorShade)}
              title="Toggle Color Shade"
            >
              <span className="action-icon">🎨</span>
            </button>
            <button 
              className={`action-btn ${showHistoryTrail ? 'active' : ''}`}
              onClick={() => setShowHistoryTrail(!showHistoryTrail)}
              title="Toggle History Trail"
            >
              <span className="action-icon">📍</span>
            </button>
            <button 
              className={`action-btn ${followMode ? 'active' : ''}`}
              onClick={() => setFollowMode(!followMode)}
              title="Follow Mode (F)"
            >
              <span className="action-icon">🎯</span>
            </button>
            <button className="action-btn" onClick={handleSave} title="Save (S)">
              <Save size={18} />
            </button>
            <button className="action-btn" onClick={handleUndo} disabled={!canUndo} title="Undo (Ctrl+Z)">
              <Zap size={18} />
            </button>
          </div>
        </div>
      )}

      {/* Toolbar: only in standalone mode */}
      {!embedded && (
        <TableToolbar
          onSetEqual={handleSetEqualWrapper}
          onIncrease={handleIncrease}
          onDecrease={handleDecrease}
          onScale={handleScaleWrapper}
          onInterpolate={handleInterpolate}
          onSmooth={handleSmooth}
          onRebin={() => setRebinDialog({ ...rebinDialog, show: true })}
          onCopy={handleCopy}
          onPaste={handlePaste}
          onUndo={handleUndo}
          canUndo={canUndo}
          canPaste={clipboard.length > 0}
          followMode={followMode}
          onFollowModeToggle={() => setFollowMode(!followMode)}
          showColorShade={showColorShade}
          onColorShadeToggle={() => setShowColorShade(!showColorShade)}
        />
      )}

      <div 
        className="editor-content"
        onContextMenu={e => {
          const target = e.target as HTMLElement;
          if (target.classList.contains('table-cell')) {
            const cell = target.closest('.table-cell') as HTMLElement;
            if (cell) {
              const x = parseInt(cell.dataset.x || '0');
              const y = parseInt(cell.dataset.y || '0');
              handleRightClick(e, x, y);
            }
          }
        }}
      >
        <TableGrid
          x_bins={rebinDialog.newXBins}
          y_bins={rebinDialog.newYBins}
          z_values={localZValues}
          onCellChange={handleCellChange}
          onAxisChange={handleAxisChange}
          selectedCell={Array.from(selectedCells).map(key => {
            const [x, y] = key.split(',').map(Number);
            return [x, y] as [number, number];
          })[0] || null}
          onCellSelect={handleCellSelectWrapper}
          onCellDoubleClick={handleCellDoubleClick}
          historyTrail={showHistoryTrail ? historyTrail : []}
          lockedCells={lockedCells}
          onCellLock={handleCellLock}
          // Live cursor - maps realtime values to table position
          showLiveCursor={followMode && realtimeData !== undefined}
          liveCursorX={x_output_channel ? realtimeData?.[x_output_channel] : realtimeData?.rpm}
          liveCursorY={y_output_channel ? realtimeData?.[y_output_channel] : realtimeData?.map}
          // Heatmap color settings
          showColorShade={showColorShade}
          heatmapScheme={heatmapSettings.valueScheme}
        />
      </div>

      <TableContextMenu
        visible={contextMenu.visible}
        x={contextMenu.x}
        y={contextMenu.y}
        cellValue={contextMenu.value}
        position={contextMenu.position || { top: contextMenu.y, left: contextMenu.x }}
        onClose={() => setContextMenu({ visible: false, x: 0, y: 0, value: 0 })}
        onSetEqual={handleContextMenuSetEqual}
        onScale={handleContextMenuScale}
        onInterpolate={() => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); handleInterpolate(); }}
        onSmooth={() => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); handleSmooth(); }}
        onLock={() => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); handleCellLock(contextMenu.x, contextMenu.y, true); }}
        onUnlock={() => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); handleCellLock(contextMenu.x, contextMenu.y, false); }}
        isLocked={lockedCells.has(`${contextMenu.x},${contextMenu.y}`)}
      />

      <RebinDialog
        isOpen={rebinDialog.show}
        onClose={() => setRebinDialog({ ...rebinDialog, show: false })}
        onApply={handleRebin}
        currentXBins={rebinDialog.newXBins}
        currentYBins={rebinDialog.newYBins}
        xAxisName={x_axis_name}
        yAxisName={y_axis_name}
      />

      <CellEditDialog
        isOpen={cellEditDialog.show}
        onClose={() => setCellEditDialog({ ...cellEditDialog, show: false })}
        onApply={handleCellEditApply}
        currentValue={cellEditDialog.value}
        cellRow={cellEditDialog.row}
        cellCol={cellEditDialog.col}
        xBinValue={rebinDialog.newXBins[cellEditDialog.col] ?? 0}
        yBinValue={rebinDialog.newYBins[cellEditDialog.row] ?? 0}
        xAxisName={x_axis_name}
        yAxisName={y_axis_name}
      />
    </div>
  );
}
