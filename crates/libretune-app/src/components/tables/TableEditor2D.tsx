import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ArrowLeft, Save, Zap, ExternalLink } from 'lucide-react';
import TableToolbar from './TableToolbar';
import TableGrid, { SelectionRange } from './TableGrid';
import TableContextMenu from './TableContextMenu';
import RebinDialog from '../dialogs/RebinDialog';
import CellEditDialog from '../dialogs/CellEditDialog';
import { useHeatmapSettings } from '../../utils/useHeatmapSettings';
import { useChannels } from '../../stores/realtimeStore';
import { useToast } from '../ToastContext';
import { getHotkeyManager } from '../../services/hotkeyService';
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
  const [localXBins, setLocalXBins] = useState<number[]>([...safeXBins]);
  const [localYBins, setLocalYBins] = useState<number[]>([...safeYBins]);
  
  const [selectionRange, setSelectionRange] = useState<SelectionRange | null>(null);
  const [lockedCells, setLockedCells] = useState<Set<string>>(new Set());
  const [historyTrail, setHistoryTrail] = useState<[number, number][]>([]);
  const [showColorShade, setShowColorShade] = useState(true);
  const [showHistoryTrail, setShowHistoryTrail] = useState(false);
  
  // History Stack
  type HistorySnapshot = {
    z: number[][];
    x: number[];
    y: number[];
  };
  const [history, setHistory] = useState<HistorySnapshot[]>([{ 
    z: [...safeZValues.map(row => [...row])],
    x: [...safeXBins],
    y: [...safeYBins] 
  }]);
  const [historyIndex, setHistoryIndex] = useState(0);

  const canUndo = historyIndex > 0;
  const canRedo = historyIndex < history.length - 1;

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

  const [alertLargeChangeEnabled, setAlertLargeChangeEnabled] = useState(true);
  const [alertLargeChangeAbs, setAlertLargeChangeAbs] = useState(5);
  const [alertLargeChangePercent, setAlertLargeChangePercent] = useState(10);
  
  // Get heatmap scheme from user settings
  const { settings: heatmapSettings } = useHeatmapSettings();

  const selectedCellsCoords = useMemo(() => {
    if (!selectionRange) return [];
    
    // Normalize coordinates (start logic might result in > end logic)
    const minX = Math.min(selectionRange.start[0], selectionRange.end[0]);
    const maxX = Math.max(selectionRange.start[0], selectionRange.end[0]);
    const minY = Math.min(selectionRange.start[1], selectionRange.end[1]);
    const maxY = Math.max(selectionRange.start[1], selectionRange.end[1]);
    
    const coords: [number, number][] = [];
    for (let y = minY; y <= maxY; y++) {
      for (let x = minX; x <= maxX; x++) {
        coords.push([x, y]);
      }
    }
    return coords;
  }, [selectionRange]);

  const selectedCellsPayload = useMemo(
    () => selectedCellsCoords.map(([x, y]) => [y, x] as [number, number]), // Backend expects (row, col)
    [selectedCellsCoords]
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
    invoke('get_settings')
      .then((settings: any) => {
        if (settings.alert_large_change_enabled !== undefined) {
          setAlertLargeChangeEnabled(!!settings.alert_large_change_enabled);
        }
        if (settings.alert_large_change_abs !== undefined) {
          setAlertLargeChangeAbs(settings.alert_large_change_abs);
        }
        if (settings.alert_large_change_percent !== undefined) {
          setAlertLargeChangePercent(settings.alert_large_change_percent);
        }
      })
      .catch((err) => console.warn('[TableEditor2D] Failed to load alert settings:', err));
  }, []);

  const warnIfLargeChange = useCallback(
    (prevValue: number, nextValue: number, operation: string) => {
      if (!alertLargeChangeEnabled) return;
      const absDelta = Math.abs(nextValue - prevValue);
      const denom = Math.max(Math.abs(prevValue), 1e-6);
      const pctDelta = (absDelta / denom) * 100;

      if (absDelta >= alertLargeChangeAbs || pctDelta >= alertLargeChangePercent) {
        showToast(
          `${operation}: large change detected (Δ ${absDelta.toFixed(2)}, ${pctDelta.toFixed(1)}%)`,
          'warning'
        );
      }
    },
    [alertLargeChangeEnabled, alertLargeChangeAbs, alertLargeChangePercent, showToast]
  );

  const warnIfLargeChangeBatch = useCallback(
    (
      previousValues: number[][],
      nextValues: number[][],
      coords: Array<[number, number]> | null,
      operation: string
    ) => {
      if (!alertLargeChangeEnabled) return;

      let maxAbs = 0;
      let maxPct = 0;
      let hits = 0;

      const checkCell = (x: number, y: number) => {
        const prev = previousValues?.[y]?.[x];
        const next = nextValues?.[y]?.[x];
        if (prev === undefined || next === undefined) return;
        const absDelta = Math.abs(next - prev);
        const denom = Math.max(Math.abs(prev), 1e-6);
        const pctDelta = (absDelta / denom) * 100;
        if (absDelta >= alertLargeChangeAbs || pctDelta >= alertLargeChangePercent) {
          hits += 1;
          maxAbs = Math.max(maxAbs, absDelta);
          maxPct = Math.max(maxPct, pctDelta);
        }
      };

      if (coords && coords.length > 0) {
        coords.forEach(([x, y]) => checkCell(x, y));
      } else {
        for (let y = 0; y < nextValues.length; y += 1) {
          for (let x = 0; x < nextValues[y].length; x += 1) {
            checkCell(x, y);
          }
        }
      }

      if (hits > 0) {
        showToast(
          `${operation}: ${hits} cells exceeded thresholds (max Δ ${maxAbs.toFixed(2)}, ${maxPct.toFixed(1)}%)`,
          'warning'
        );
      }
    },
    [alertLargeChangeEnabled, alertLargeChangeAbs, alertLargeChangePercent, showToast]
  );

  const pushHistory = useCallback((newZ: number[][], newX: number[], newY: number[]) => {
    setHistory(prev => {
      // Remove any redo history if we make a new change
      const newHistory = prev.slice(0, historyIndex + 1);
      newHistory.push({ 
        z: newZ.map(row => [...row]), 
        x: [...newX], 
        y: [...newY] 
      });
      // Limit history size (optional, e.g. 50 steps)
      if (newHistory.length > 50) newHistory.shift();
      return newHistory;
    });
    setHistoryIndex(prev => Math.min(prev + 1, 49));
  }, [historyIndex]);

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
      
      // Build composite key for Ctrl+key combinations
      const getKeyCombo = (key: string): string => {
        if (isCtrl && key.length === 1) {
          return `Ctrl+${key.toUpperCase()}`;
        }
        return key;
      };
      
      const keyCombo = getKeyCombo(e.key);
      const hotkeyManager = getHotkeyManager();

      // Check if keyboard event matches a custom binding or default
      const matchesAction = (actionId: string): boolean => {
        return hotkeyManager.matchesBinding(actionId, keyCombo) || 
               (hotkeyManager.getBindingForAction(actionId) === '' && matchesDefaultBinding(actionId, e.key));
      };
      
      const matchesDefaultBinding = (actionId: string, key: string): boolean => {
        // Fallback to default bindings if no custom binding is set
        const defaults: Record<string, string[]> = {
          'table.navigateUp': ['ArrowUp'],
          'table.navigateDown': ['ArrowDown'],
          'table.navigateLeft': ['ArrowLeft'],
          'table.navigateRight': ['ArrowRight'],
          'table.setEqual': ['='],
          'table.increase': ['>', '.', 'q'],
          'table.decrease': ['<', ',', '-', '_'],
          'table.increaseMultiple': ['+'],
          'table.scale': ['*'],
          'table.interpolate': ['/'],
          'table.smooth': ['s', 'S'],
          'table.toggleFollowMode': ['f', 'F'],
          'table.jumpToActive': ['g', 'G'],
          'table.copy': ['Ctrl+C'],
          'table.paste': ['Ctrl+V'],
          'table.undo': ['Ctrl+Z'],
          'table.escape': ['Escape'],
        };
        return (defaults[actionId] || []).includes(key);
      };

      // Navigation
      if (matchesAction('table.navigateUp') || e.key === 'ArrowUp') {
        e.preventDefault();
        handleArrowNavigation('ArrowUp', isShift);
        return;
      }
      if (matchesAction('table.navigateDown') || e.key === 'ArrowDown') {
        e.preventDefault();
        handleArrowNavigation('ArrowDown', isShift);
        return;
      }
      if (matchesAction('table.navigateLeft') || e.key === 'ArrowLeft') {
        e.preventDefault();
        handleArrowNavigation('ArrowLeft', isShift);
        return;
      }
      if (matchesAction('table.navigateRight') || e.key === 'ArrowRight') {
        e.preventDefault();
        handleArrowNavigation('ArrowRight', isShift);
        return;
      }

      // Cell operations
      if (matchesAction('table.setEqual') || e.key === '=') {
        e.preventDefault();
        handleSetEqual();
        return;
      }
      if (matchesAction('table.increase') || ['>', '.', 'q'].includes(e.key)) {
        e.preventDefault();
        handleIncrease(multiplier);
        return;
      }
      if (matchesAction('table.decrease') || ['<', ',', '-', '_'].includes(e.key)) {
        e.preventDefault();
        handleDecrease(multiplier);
        return;
      }
      if (matchesAction('table.increaseMultiple') || e.key === '+') {
        e.preventDefault();
        handleIncrease(10 * multiplier);
        return;
      }
      if (matchesAction('table.scale') || e.key === '*') {
        e.preventDefault();
        handleScale(1.0);
        return;
      }
      if (matchesAction('table.interpolate') || e.key === '/') {
        e.preventDefault();
        handleInterpolate();
        return;
      }
      if ((matchesAction('table.smooth') || ['s', 'S'].includes(e.key)) && !isCtrl) {
        e.preventDefault();
        handleSmooth();
        return;
      }

      // View controls
      if ((matchesAction('table.toggleFollowMode') || ['f', 'F'].includes(e.key)) && !isCtrl) {
        e.preventDefault();
        setFollowMode(!followMode);
        return;
      }
      if (matchesAction('table.jumpToActive') || ['g', 'G'].includes(e.key)) {
        e.preventDefault();
        // Go to live position (jump to active cell)
        if (activeCell) {
          setSelectionRange({ start: activeCell, end: activeCell });
        }
        return;
      }

      // Copy/Paste
      if (matchesAction('table.copy') || (isCtrl && (e.key === 'c' || e.key === 'C'))) {
        e.preventDefault();
        handleCopy();
        return;
      }
      if (matchesAction('table.paste') || (isCtrl && (e.key === 'v' || e.key === 'V'))) {
        e.preventDefault();
        handlePaste();
        return;
      }
      if (matchesAction('table.undo') || (isCtrl && (e.key === 'z' || e.key === 'Z'))) {
        e.preventDefault();
        if (isShift) {
          handleRedo();
        } else {
          handleUndo();
        }
        return;
      }
      if (isCtrl && (e.key === 'y' || e.key === 'Y')) {
        e.preventDefault();
        handleRedo();
        return;
      }

      // Escape to clear selection
      if (matchesAction('table.escape') || e.key === 'Escape') {
        e.preventDefault();
        setSelectionRange(null);
        setContextMenu({ visible: false, x: 0, y: 0, value: 0 });
        return;
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [selectionRange, followMode, activeCell, localZValues]);

  // Arrow key navigation helper
  const handleArrowNavigation = (key: string, extendSelection: boolean) => {
    if (!selectionRange) {
      // Start from top-left if no selection
      setSelectionRange({ start: [0, 0], end: [0, 0] });
      return;
    }

    // Use current end as base for movement
    const [currentX, currentY] = selectionRange.end;

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
      // Extend selection: keep start, update end
      setSelectionRange({ ...selectionRange, end: [newX, newY] });
    } else {
      // Move selection: both start and end move to new cell
      setSelectionRange({ start: [newX, newY], end: [newX, newY] });
    }
    setActiveCell([newX, newY]);
  };

  const handleCellChange = (
    x: number,
    y: number,
    value: number,
    options?: { suppressAlert?: boolean; operation?: string }
  ) => {
    const prevValue = localZValues[y][x];
    const newValues = localZValues.map(row => [...row]);
    newValues[y][x] = value;
    
    setLocalZValues(newValues);
    setSelectionRange({ start: [x,y], end: [x,y] });
    pushHistory(newValues, localXBins, localYBins);
    onValuesChange?.(newValues);

    if (!options?.suppressAlert) {
      warnIfLargeChange(prevValue, value, options?.operation ?? 'Cell edit');
    }
  };

  const handleAxisChange = (axis: 'x' | 'y', index: number, value: number) => {
    if (axis === 'x') {
      const newBins = [...localXBins];
      newBins[index] = value;
      setLocalXBins(newBins);
      setRebinDialog(prev => ({ ...prev, newXBins: newBins }));
      pushHistory(localZValues, newBins, localYBins);
    } else {
      const newBins = [...localYBins];
      newBins[index] = value;
      setLocalYBins(newBins);
      setRebinDialog(prev => ({ ...prev, newYBins: newBins }));
      pushHistory(localZValues, localXBins, newBins);
    }
  };

  const handleSetEqual = async () => {
    const values = selectedCellsCoords.map(([x, y]) => {
      return { x, y, value: localZValues[y][x] };
    });

    if (values.length === 0) return;

    const avgValue = values.reduce((sum, v) => sum + v.value, 0) / values.length;

    const previousValues = localZValues.map((row) => [...row]);

    try {
      const result = await invoke<TableOperationResult>('set_cells_equal', {
        table_name,
        selected_cells: selectedCellsPayload,
        value: avgValue
      });
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        onValuesChange?.(result.z_values);
        pushHistory(result.z_values, localXBins, localYBins);
        warnIfLargeChangeBatch(previousValues, result.z_values, selectedCellsCoords, 'Set equal');
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

  const handleContextMenuSetEqual = (_value: number) => {
    setContextMenu({ visible: false, x: 0, y: 0, value: 0 });
    handleSetEqual();
  };

  const handleContextMenuScale = (factor: number) => {
    setContextMenu({ visible: false, x: 0, y: 0, value: 0 });
    handleScale(factor);
  };

  const handleIncrease = (amount: number) => {
    const values = selectedCellsCoords.map(([x, y]) => {
      return { x, y, value: localZValues[y][x] };
    });
    
    values.forEach(({ x, y, value }) => {
      handleCellChange(x, y, value * (1 + amount), { suppressAlert: true });
    });
  };

  const handleDecrease = (amount: number) => {
    const values = selectedCellsCoords.map(([x, y]) => {
      return { x, y, value: localZValues[y][x] };
    });
    
    values.forEach(({ x, y, value }) => {
      handleCellChange(x, y, value * (1 - amount), { suppressAlert: true });
    });
  };

  const handleScale = async (factor: number) => {
    const previousValues = localZValues.map((row) => [...row]);
    try {
      const result = await invoke<TableOperationResult>('scale_cells', {
        table_name,
        selected_cells: selectedCellsPayload,
        scale_factor: factor
      });
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        onValuesChange?.(result.z_values);
        pushHistory(result.z_values, localXBins, localYBins);
        warnIfLargeChangeBatch(previousValues, result.z_values, selectedCellsCoords, 'Scale');
      }
    } catch (err) {
      handleOperationError('Scale', err);
    }
  };

  const handleSmooth = async () => {
    const previousValues = localZValues.map((row) => [...row]);
    try {
      const result = await invoke<TableOperationResult>('smooth_table', {
        table_name,
        selected_cells: selectedCellsPayload,
        factor: 1.0
      });
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        onValuesChange?.(result.z_values);
        pushHistory(result.z_values, localXBins, localYBins);
        warnIfLargeChangeBatch(previousValues, result.z_values, selectedCellsCoords, 'Smooth');
      }
    } catch (err) {
      handleOperationError('Smooth', err);
    }
  };

  const handleInterpolate = async () => {
    const previousValues = localZValues.map((row) => [...row]);
    try {
      const result = await invoke<TableOperationResult>('interpolate_cells', {
        table_name,
        selected_cells: selectedCellsPayload
      });
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        onValuesChange?.(result.z_values);
        pushHistory(result.z_values, localXBins, localYBins);
        warnIfLargeChangeBatch(previousValues, result.z_values, selectedCellsCoords, 'Interpolate');
      }
    } catch (err) {
      handleOperationError('Interpolate', err);
    }
  };

  const handleAddOffset = async (offset: number) => {
    const previousValues = localZValues.map((row) => [...row]);
    try {
      const result = await invoke<TableOperationResult>('add_offset', {
        table_name,
        selected_cells: selectedCellsPayload,
        offset: offset
      });
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        onValuesChange?.(result.z_values);
        pushHistory(result.z_values, localXBins, localYBins);
        warnIfLargeChangeBatch(previousValues, result.z_values, selectedCellsCoords, 'Offset');
      }
    } catch (err) {
      handleOperationError('Offset', err);
    }
  };

  const handleInterpolateLinear = async (axis: 'row' | 'col') => {
    const previousValues = localZValues.map((row) => [...row]);
    try {
      const result = await invoke<TableOperationResult>('interpolate_linear', {
        table_name,
        selected_cells: selectedCellsPayload,
        axis: axis
      });
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        onValuesChange?.(result.z_values);
        pushHistory(result.z_values, localXBins, localYBins);
        warnIfLargeChangeBatch(previousValues, result.z_values, selectedCellsCoords, 'Linear Interpolate');
      }
    } catch (err) {
      handleOperationError('Linear Interpolate', err);
    }
  };

  const handleFill = async (direction: 'right' | 'down') => {
    const previousValues = localZValues.map((row) => [...row]);
    try {
      const result = await invoke<TableOperationResult>('fill_region', {
        table_name,
        selected_cells: selectedCellsPayload,
        direction: direction
      });
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        onValuesChange?.(result.z_values);
        pushHistory(result.z_values, localXBins, localYBins);
        warnIfLargeChangeBatch(previousValues, result.z_values, selectedCellsCoords, 'Fill Region');
      }
    } catch (err) {
      handleOperationError('Fill Region', err);
    }
  };

  const handleNudge = (up: boolean, large: boolean) => {
    const amount = large ? 0.05 : 0.01; 
    if (up) handleIncrease(amount);
    else handleDecrease(amount);
  };

  const handleRebin = async (newXBins: number[], newYBins: number[], interpolateZ: boolean) => {
    setRebinDialog({ show: false, newXBins, newYBins });

    const previousValues = localZValues.map((row) => [...row]);

    try {
      const result = await invoke<TableOperationResult>('rebin_table', {
        table_name,
        new_x_bins: newXBins,
        new_y_bins: newYBins,
        interpolate_z: interpolateZ
      });
      if (result && result.z_values) {
        setLocalZValues(result.z_values);
        setLocalXBins(newXBins);
        setLocalYBins(newYBins);
        onValuesChange?.(result.z_values);
        setSelectionRange(null);
        pushHistory(result.z_values, newXBins, newYBins);
        warnIfLargeChangeBatch(previousValues, result.z_values, null, 'Rebin');
      }
    } catch (err) {
      handleOperationError('Rebin', err);
    }
  };

  const handleCellEditApply = (value: number) => {
    handleCellChange(cellEditDialog.col, cellEditDialog.row, value, { operation: 'Cell edit' });
  };

  const handleCellDoubleClick = (x: number, y: number) => {
    setCellEditDialog({
      show: true,
      row: y,
      col: x,
      value: localZValues[y][x],
    });
  };

  const handleCopy = async () => {
    if (!selectionRange) return;

    const minX = Math.min(selectionRange.start[0], selectionRange.end[0]);
    const maxX = Math.max(selectionRange.start[0], selectionRange.end[0]);
    const minY = Math.min(selectionRange.start[1], selectionRange.end[1]);
    const maxY = Math.max(selectionRange.start[1], selectionRange.end[1]);

    const rows: string[] = [];
    for (let y = minY; y <= maxY; y++) {
      const rowValues: string[] = [];
      for (let x = minX; x <= maxX; x++) {
        rowValues.push(localZValues[y][x].toString());
      }
      rows.push(rowValues.join('\t'));
    }

    const text = rows.join('\n');
    try {
      await navigator.clipboard.writeText(text);
      // Optional: Visual feedback or toast could go here
    } catch (err) {
      console.error('Failed to copy to clipboard:', err);
    }
  };

  const handlePaste = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (!text) return;

      // Determine paste anchor (top-left of selection)
      const startX = selectionRange 
        ? Math.min(selectionRange.start[0], selectionRange.end[0]) 
        : (activeCell ? activeCell[0] : 0);
      const startY = selectionRange 
        ? Math.min(selectionRange.start[1], selectionRange.end[1]) 
        : (activeCell ? activeCell[1] : 0);

      // Parse data (handle standard TSV/CSV)
      const rows = text.split(/\r?\n/).filter(line => line.trim() !== '');
      if (rows.length === 0) return;

      // Create new table state
      const newValues = localZValues.map(row => [...row]);
      let hasChanges = false;

      rows.forEach((line, r) => {
        const y = startY + r;
        if (y >= y_bins.length) return;

        // Auto-detect delimiter (tab preferred, then comma)
        const delimiter = line.includes('\t') ? '\t' : ',';
        const cols = line.split(delimiter);

        cols.forEach((valStr, c) => {
          const x = startX + c;
          if (x >= x_bins.length) return;

          const val = parseFloat(valStr.trim());
          if (!isNaN(val)) {
            const targetKey = `${x},${y}`;
            if (!lockedCells.has(targetKey)) {
              if (newValues[y][x] !== val) {
                newValues[y][x] = val;
                hasChanges = true;
              }
            }
          }
        });
      });

      if (hasChanges) {
        setLocalZValues(newValues);
        pushHistory(newValues, localXBins, localYBins);
        // Persist to backend without triggering n*m alerts
        invoke('update_table_data', {
          table_name,
          z_values: newValues
        });
        
        // Update selection to cover pasted area
        const endY = Math.min(startY + rows.length - 1, y_bins.length - 1);
        const maxCols = Math.max(...rows.map(r => r.split(r.includes('\t') ? '\t' : ',').length));
        const endX = Math.min(startX + maxCols - 1, x_bins.length - 1);
        
        setSelectionRange({
          start: [startX, startY],
          end: [endX, endY]
        });
      }
    } catch (err) {
      console.error('Failed to paste from clipboard:', err);
    }
  };

  const handleUndo = () => {
    if (historyIndex > 0) {
      const prevIndex = historyIndex - 1;
      const snapshot = history[prevIndex];
      
      setLocalZValues(snapshot.z);
      setLocalXBins(snapshot.x);
      setLocalYBins(snapshot.y);
      setRebinDialog(prev => ({ ...prev, newXBins: snapshot.x, newYBins: snapshot.y }));
      
      setHistoryIndex(prevIndex);
      onValuesChange?.(snapshot.z);
      
      // Update backend
      invoke('update_table_data', {
        table_name,
        z_values: snapshot.z,
        // Backend update for axis not yet available via simple set command
        // but local state is reverted
      });
    }
  };

  const handleRedo = () => {
    if (historyIndex < history.length - 1) {
      const nextIndex = historyIndex + 1;
      const snapshot = history[nextIndex];
      
      setLocalZValues(snapshot.z);
      setLocalXBins(snapshot.x);
      setLocalYBins(snapshot.y);
      setRebinDialog(prev => ({ ...prev, newXBins: snapshot.x, newYBins: snapshot.y }));
      
      setHistoryIndex(nextIndex);
      onValuesChange?.(snapshot.z);

      invoke('update_table_data', {
        table_name,
        z_values: snapshot.z
      });
    }
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

  const handleSelectionChange = (range: SelectionRange | null) => {
    setSelectionRange(range);
    if (range) {
      setActiveCell(range.end);
      setContextMenu({ visible: false, x: 0, y: 0, value: 0 });
    }
  };

  const handleSave = () => {
    invoke('update_table_data', {
      table_name,
      z_values: localZValues
    }).then(() => {
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
          onRedo={handleRedo}
          canUndo={canUndo}
          canRedo={canRedo}
          canPaste={true}
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
          x_bins={localXBins}
          y_bins={localYBins}
          z_values={localZValues}
          onCellChange={handleCellChange}
          onAxisChange={handleAxisChange}
          selectionRange={selectionRange}
          onSelectionChange={handleSelectionChange}
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
        onInterpolateLinear={(axis) => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); handleInterpolateLinear(axis); }}
        onSmooth={() => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); handleSmooth(); }}
        onAddOffset={(offset) => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); handleAddOffset(offset); }}
        onNudge={(up, large) => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); handleNudge(up, large); }}
        onFill={(direction) => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); handleFill(direction); }}
        onLock={() => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); handleCellLock(contextMenu.x, contextMenu.y, true); }}
        onUnlock={() => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); handleCellLock(contextMenu.x, contextMenu.y, false); }}
        isLocked={lockedCells.has(`${contextMenu.x},${contextMenu.y}`)}
        onCopy={() => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); handleCopy(); }}
        onPaste={() => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); handlePaste(); }}
        onToggleHeatmap={() => { setContextMenu({ visible: false, x: 0, y: 0, value: 0 }); setShowColorShade(prev => !prev); }}
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
