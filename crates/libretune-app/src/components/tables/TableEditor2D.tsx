import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ArrowLeft, Save, Zap } from 'lucide-react';
import TableToolbar from './TableToolbar';
import TableGrid from './TableGrid';
import TableContextMenu from './TableContextMenu';
import RebinDialog from '../dialogs/RebinDialog';
import CellEditDialog from '../dialogs/CellEditDialog';
import './TableComponents.css';
import './TableEditor2D.css';

interface TableEditor2DProps {
  title: string;
  table_name: string;
  x_axis_name: string;
  y_axis_name: string;
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  onBack: () => void;
  realtimeData?: Record<string, number>; // For live cursor position
}

interface RebinDialogState {
  show: boolean;
  newXBins: number[];
  newYBins: number[];
}

interface CellEditDialogState {
  show: boolean;
  row: number;
  col: number;
  value: number;
}

export default function TableEditor2D({
  title,
  table_name,
  x_axis_name,
  y_axis_name,
  x_bins,
  y_bins,
  z_values,
  onBack,
  realtimeData,
}: TableEditor2DProps) {
  const [localZValues, setLocalZValues] = useState<number[][]>([...z_values]);
  const [selectedCells, setSelectedCells] = useState<Set<string>>(new Set());
  const [lockedCells, setLockedCells] = useState<Set<string>>(new Set());
  const [historyTrail, setHistoryTrail] = useState<[number, number][]>([]);
  const [showColorShade, setShowColorShade] = useState(false);
  const [showHistoryTrail, setShowHistoryTrail] = useState(false);
  const [clipboard, setClipboard] = useState<[number, number][]>([]);
  const [canUndo, setCanUndo] = useState(false);
  const [rebinDialog, setRebinDialog] = useState<RebinDialogState>({
    show: false,
    newXBins: [...x_bins],
    newYBins: [...y_bins],
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

  useEffect(() => {
    const interval = setInterval(() => {
      if (followMode && activeCell) {
        const trail = [...historyTrail.slice(-50), activeCell];
        setHistoryTrail(trail);
      }
    }, 500);
    return () => clearInterval(interval);
  }, [followMode, activeCell]);

  // Keyboard event handling for TunerStudio-style hotkeys
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

  const handleSetEqual = () => {
    const values = Array.from(selectedCells).map(key => {
      const [x, y] = key.split(',').map(Number);
      return { x, y, value: localZValues[y][x] };
    });

    if (values.length === 0) return;

    const avgValue = values.reduce((sum, v) => sum + v.value, 0) / values.length;

    values.forEach(({ x, y }) => {
      handleCellChange(x, y, avgValue);
    });
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

  const handleScale = (factor: number) => {
    const values = Array.from(selectedCells).map(key => {
      const [x, y] = key.split(',').map(Number);
      return { x, y, value: localZValues[y][x] };
    });
    
    values.forEach(({ x, y, value }) => {
      handleCellChange(x, y, value * factor);
    });
  };

  const handleSmooth = () => {
    invoke('smooth_table', {
      table_name,
      selected_cells: Array.from(selectedCells).map(key => {
        const [x, y] = key.split(',').map(Number);
        return [x, y] as [number, number];
      }),
      factor: 1.0
    }).then((result: any) => {
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        setSelectedCells(new Set());
      }
    });
  };

  const handleInterpolate = () => {
    invoke('interpolate_cells', {
      table_name,
      selected_cells: Array.from(selectedCells).map(key => {
        const [x, y] = key.split(',').map(Number);
        return [x, y] as [number, number];
      })
    }).then((result: any) => {
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        setSelectedCells(new Set());
      }
    });
  };

  const handleRebin = (newXBins: number[], newYBins: number[], interpolateZ: boolean) => {
    setRebinDialog({ show: false, newXBins, newYBins });
    
    invoke('rebin_table', {
      table_name,
      new_x_bins: newXBins,
      new_y_bins: newYBins,
      interpolate_z: interpolateZ
    }).then((result: any) => {
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        // Note: x_bins and y_bins from props won't update, but newXBins/newYBins are stored in rebinDialog
        setSelectedCells(new Set());
      }
    });
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

  return (
    <div className="table-editor-2d">
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
          liveCursorX={realtimeData?.rpm}  // Use RPM for X-axis (common for VE tables)
          liveCursorY={realtimeData?.map}  // Use MAP for Y-axis
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
