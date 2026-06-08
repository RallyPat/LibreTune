/**
 * Dashboard Designer Mode
 * 
 * Provides interactive editing capabilities for dashboard layouts:
 * - Drag gauges to reposition
 * - Resize handles on corners and edges
 * - Property editor panel for gauge configuration
 * - Snap-to-grid alignment
 * - Multi-select with shift-click
 * - Copy/paste gauges
 * - Undo/redo support
 */

import { useState, useCallback, useRef, useEffect } from 'react';
import { 
  Grid3X3, 
  Save, 
  Undo2, 
  Redo2, 
  Copy, 
  Clipboard, 
  Trash2,
  X,
} from 'lucide-react';
import { DashFile, TsGaugeConfig, TsIndicatorConfig, DashComponent, isGauge, isIndicator } from './dashTypes';
import './DashboardDesigner.css';

interface DashboardDesignerProps {
  dashFile: DashFile;
  onDashFileChange: (file: DashFile) => void;
  selectedGaugeId: string | null;
  onSelectGauge: (id: string | null) => void;
  gridSnap: number; // Grid snap size in percentage (e.g., 5 = 5%)
  onGridSnapChange: (snap: number) => void;
  showGrid: boolean;
  onShowGridChange: (show: boolean) => void;
  onSave: () => void;
  onExit: () => void;
}

interface HistoryEntry {
  dashFile: DashFile;
  description: string;
}

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

type ResizeHandle = 'n' | 's' | 'e' | 'w' | 'ne' | 'nw' | 'se' | 'sw';

export default function DashboardDesigner({
  dashFile,
  onDashFileChange,
  selectedGaugeId,
  onSelectGauge,
  gridSnap,
  onGridSnapChange,
  showGrid,
  onShowGridChange,
  onSave,
  onExit,
}: DashboardDesignerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  
  // History for undo/redo
  const [history, setHistory] = useState<HistoryEntry[]>([{ dashFile, description: 'Initial' }]);
  const [historyIndex, setHistoryIndex] = useState(0);
  
  // Drag and resize states
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
  
  // Clipboard for copy/paste
  const [clipboard, setClipboard] = useState<DashComponent | null>(null);
  
  // Get selected gauge/indicator config
  const selectedComponent = selectedGaugeId 
    ? dashFile.gauge_cluster.components.find(c => {
        if (isGauge(c)) return c.Gauge.id === selectedGaugeId;
        if (isIndicator(c)) return c.Indicator.id === selectedGaugeId;
        return false;
      })
    : null;

  // Snap value to grid
  const snapToGrid = useCallback((value: number): number => {
    if (gridSnap <= 0) return value;
    return Math.round(value / (gridSnap / 100)) * (gridSnap / 100);
  }, [gridSnap]);

  // Add history entry
  const pushHistory = useCallback((newFile: DashFile, description: string) => {
    setHistory(prev => {
      const newHistory = prev.slice(0, historyIndex + 1);
      newHistory.push({ dashFile: newFile, description });
      return newHistory;
    });
    setHistoryIndex(prev => prev + 1);
  }, [historyIndex]);

  // Undo
  const handleUndo = useCallback(() => {
    if (historyIndex > 0) {
      const newIndex = historyIndex - 1;
      setHistoryIndex(newIndex);
      onDashFileChange(history[newIndex].dashFile);
    }
  }, [historyIndex, history, onDashFileChange]);

  // Redo
  const handleRedo = useCallback(() => {
    if (historyIndex < history.length - 1) {
      const newIndex = historyIndex + 1;
      setHistoryIndex(newIndex);
      onDashFileChange(history[newIndex].dashFile);
    }
  }, [historyIndex, history, onDashFileChange]);

  // Delete selected gauge
  const handleDelete = useCallback(() => {
    if (!selectedGaugeId) return;
    
    const newComponents = dashFile.gauge_cluster.components.filter(c => {
      if (isGauge(c)) return c.Gauge.id !== selectedGaugeId;
      if (isIndicator(c)) return c.Indicator.id !== selectedGaugeId;
      return true;
    });
    
    const newFile: DashFile = {
      ...dashFile,
      gauge_cluster: {
        ...dashFile.gauge_cluster,
        components: newComponents,
      },
    };
    
    pushHistory(newFile, `Delete ${selectedGaugeId}`);
    onDashFileChange(newFile);
    onSelectGauge(null);
  }, [selectedGaugeId, dashFile, pushHistory, onDashFileChange, onSelectGauge]);

  // Copy selected gauge
  const handleCopy = useCallback(() => {
    if (!selectedComponent) return;
    setClipboard(JSON.parse(JSON.stringify(selectedComponent)));
  }, [selectedComponent]);

  // Paste from clipboard
  const handlePaste = useCallback(() => {
    if (!clipboard) return;
    
    // Create new component with unique ID and offset position
    let newComponent: DashComponent;
    
    if (isGauge(clipboard)) {
      const gauge = clipboard.Gauge;
      newComponent = {
        Gauge: {
          ...gauge,
          id: `gauge-${Date.now()}`,
          relative_x: (gauge.relative_x ?? 0) + 0.05,
          relative_y: (gauge.relative_y ?? 0) + 0.05,
        },
      };
    } else if (isIndicator(clipboard)) {
      const indicator = clipboard.Indicator;
      newComponent = {
        Indicator: {
          ...indicator,
          id: `indicator-${Date.now()}`,
          relative_x: (indicator.relative_x ?? 0) + 0.05,
          relative_y: (indicator.relative_y ?? 0) + 0.05,
        },
      };
    } else {
      return;
    }
    
    const newFile: DashFile = {
      ...dashFile,
      gauge_cluster: {
        ...dashFile.gauge_cluster,
        components: [...dashFile.gauge_cluster.components, newComponent],
      },
    };
    
    pushHistory(newFile, 'Paste gauge');
    onDashFileChange(newFile);
  }, [clipboard, dashFile, pushHistory, onDashFileChange]);

  // Handle mouse down on gauge for dragging
  const handleGaugeMouseDown = useCallback((e: React.MouseEvent, gaugeId: string, component: DashComponent) => {
    e.preventDefault();
    e.stopPropagation();
    
    onSelectGauge(gaugeId);
    
    // Get relative position
    let relX = 0, relY = 0;
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
  }, [onSelectGauge]);

  // Handle mouse down on resize handle
  const handleResizeMouseDown = useCallback((e: React.MouseEvent, handle: ResizeHandle, gaugeId: string, component: DashComponent) => {
    e.preventDefault();
    e.stopPropagation();
    
    let relX = 0, relY = 0, width = 0.25, height = 0.25;
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
  }, []);

  // Handle mouse move for drag/resize
  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!containerRef.current) return;
      
      const rect = containerRef.current.getBoundingClientRect();
      
      if (dragState.isDragging && dragState.gaugeId) {
        const deltaX = (e.clientX - dragState.startX) / rect.width;
        const deltaY = (e.clientY - dragState.startY) / rect.height;
        
        let newRelX = snapToGrid(dragState.startRelativeX + deltaX);
        let newRelY = snapToGrid(dragState.startRelativeY + deltaY);
        
        // Clamp to bounds
        newRelX = Math.max(0, Math.min(1, newRelX));
        newRelY = Math.max(0, Math.min(1, newRelY));
        
        // Update component position
        const newComponents = dashFile.gauge_cluster.components.map(c => {
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
        
        // Calculate new dimensions based on handle
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
        
        // Enforce minimum size
        const minSize = 0.05;
        newWidth = Math.max(minSize, newWidth);
        newHeight = Math.max(minSize, newHeight);
        
        // Clamp position
        newX = Math.max(0, Math.min(1 - newWidth, newX));
        newY = Math.max(0, Math.min(1 - newHeight, newY));
        
        // Update component
        const newComponents = dashFile.gauge_cluster.components.map(c => {
          if (isGauge(c) && c.Gauge.id === resizeState.gaugeId) {
            return { 
              Gauge: { 
                ...c.Gauge, 
                relative_x: newX, 
                relative_y: newY,
                relative_width: newWidth,
                relative_height: newHeight,
              } 
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
              } 
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
      
      setDragState(prev => ({ ...prev, isDragging: false, gaugeId: null }));
      setResizeState(prev => ({ ...prev, isResizing: false, gaugeId: null, handle: null }));
    };
    
    if (dragState.isDragging || resizeState.isResizing) {
      window.addEventListener('mousemove', handleMouseMove);
      window.addEventListener('mouseup', handleMouseUp);
      return () => {
        window.removeEventListener('mousemove', handleMouseMove);
        window.removeEventListener('mouseup', handleMouseUp);
      };
    }
  }, [dragState, resizeState, dashFile, snapToGrid, pushHistory, onDashFileChange]);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Delete' || e.key === 'Backspace') {
        handleDelete();
      } else if (e.ctrlKey || e.metaKey) {
        if (e.key === 'z' && !e.shiftKey) {
          e.preventDefault();
          handleUndo();
        } else if ((e.key === 'z' && e.shiftKey) || e.key === 'y') {
          e.preventDefault();
          handleRedo();
        } else if (e.key === 'c') {
          e.preventDefault();
          handleCopy();
        } else if (e.key === 'v') {
          e.preventDefault();
          handlePaste();
        } else if (e.key === 's') {
          e.preventDefault();
          onSave();
        }
      } else if (e.key === 'Escape') {
        onSelectGauge(null);
      }
    };
    
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleDelete, handleUndo, handleRedo, handleCopy, handlePaste, onSave, onSelectGauge]);

  // Render resize handles for selected gauge
  const renderResizeHandles = (gaugeId: string, component: DashComponent) => {
    if (selectedGaugeId !== gaugeId) return null;
    
    const handles: ResizeHandle[] = ['n', 's', 'e', 'w', 'ne', 'nw', 'se', 'sw'];
    
    return handles.map(handle => (
      <div
        key={handle}
        className={`resize-handle resize-handle-${handle}`}
        onMouseDown={(e) => handleResizeMouseDown(e, handle, gaugeId, component)}
      />
    ));
  };

  return (
    <div className="dashboard-designer">
      {/* Toolbar */}
      <div className="designer-toolbar">
        <div className="toolbar-group">
          <button 
            className="toolbar-btn"
            onClick={handleUndo}
            disabled={historyIndex <= 0}
            title="撤销 (Ctrl+Z)"
          >
            <Undo2 size={16} />
          </button>
          <button 
            className="toolbar-btn"
            onClick={handleRedo}
            disabled={historyIndex >= history.length - 1}
            title="重做 (Ctrl+Y)"
          >
            <Redo2 size={16} />
          </button>
        </div>
        
        <div className="toolbar-separator" />
        
        <div className="toolbar-group">
          <button 
            className="toolbar-btn"
            onClick={handleCopy}
            disabled={!selectedGaugeId}
            title="复制 (Ctrl+C)"
          >
            <Copy size={16} />
          </button>
          <button 
            className="toolbar-btn"
            onClick={handlePaste}
            disabled={!clipboard}
            title="粘贴 (Ctrl+V)"
          >
            <Clipboard size={16} />
          </button>
          <button 
            className="toolbar-btn danger"
            onClick={handleDelete}
            disabled={!selectedGaugeId}
            title="删除 (Del)"
          >
            <Trash2 size={16} />
          </button>
        </div>
        
        <div className="toolbar-separator" />
        
        <div className="toolbar-group">
            <button 
              className={`toolbar-btn ${showGrid ? 'active' : ''}`}
              onClick={() => onShowGridChange(!showGrid)}
              title="切换网格"
            >
            <Grid3X3 size={16} />
          </button>
          <select 
            className="toolbar-select"
            value={gridSnap}
            onChange={(e) => onGridSnapChange(parseInt(e.target.value))}
            title="网格吸附大小"
          >
            <option value={0}>无吸附</option>
            <option value={1}>1%</option>
            <option value={2}>2%</option>
            <option value={5}>5%</option>
            <option value={10}>10%</option>
          </select>
        </div>
        
        <div className="toolbar-separator" />
        
        <div className="toolbar-group">
          <button 
            className="toolbar-btn primary"
            onClick={onSave}
            title="保存仪表盘 (Ctrl+S)"
          >
            <Save size={16} />
            <span>保存</span>
          </button>
          <button 
            className="toolbar-btn"
            onClick={onExit}
            title="退出设计模式"
          >
            <X size={16} />
            <span>退出</span>
          </button>
        </div>
      </div>

      {/* Main designer area */}
      <div className="designer-content">
        {/* Canvas with gauges */}
        <div 
          ref={containerRef}
          className={`designer-canvas ${showGrid ? 'show-grid' : ''}`}
          style={{
            '--grid-size': `${gridSnap}%`,
          } as React.CSSProperties}
          onClick={() => onSelectGauge(null)}
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
                onMouseDown={(e) => handleGaugeMouseDown(e, id, component)}
              >
                <div className="gauge-preview">
                  {isGauge(component) && (
                    <span className="gauge-label">{component.Gauge.title || component.Gauge.output_channel}</span>
                  )}
                  {isIndicator(component) && (
                    <span className="gauge-label">{component.Indicator.on_text || component.Indicator.output_channel}</span>
                  )}
                </div>
                {renderResizeHandles(id, component)}
              </div>
            );
          })}
        </div>

        {/* Property editor panel */}
        <div className="designer-properties">
          <h3>属性</h3>
          {selectedComponent ? (
            <PropertyEditor
              component={selectedComponent}
              onChange={(updated) => {
                const newComponents = dashFile.gauge_cluster.components.map(c => {
                  if (isGauge(c) && isGauge(updated) && c.Gauge.id === updated.Gauge.id) {
                    return updated;
                  }
                  if (isIndicator(c) && isIndicator(updated) && c.Indicator.id === updated.Indicator.id) {
                    return updated;
                  }
                  return c;
                });
                
                const newFile = {
                  ...dashFile,
                  gauge_cluster: { ...dashFile.gauge_cluster, components: newComponents },
                };
                pushHistory(newFile, 'Edit property');
                onDashFileChange(newFile);
              }}
            />
          ) : (
            <p className="no-selection">选择一个仪表以编辑其属性</p>
          )}
        </div>
      </div>
    </div>
  );
}

// Property editor sub-component
interface PropertyEditorProps {
  component: DashComponent;
  onChange: (component: DashComponent) => void;
}

function PropertyEditor({ component, onChange }: PropertyEditorProps) {
  if (isGauge(component)) {
    const gauge = component.Gauge;
    
    const updateGauge = (updates: Partial<TsGaugeConfig>) => {
      onChange({ Gauge: { ...gauge, ...updates } });
    };
    
    return (
      <div className="property-editor">
        <div className="property-group">
          <label>标题</label>
          <input 
            type="text" 
            value={gauge.title || ''} 
            onChange={(e) => updateGauge({ title: e.target.value })}
          />
        </div>
        
        <div className="property-group">
          <label>输出通道</label>
          <input 
            type="text" 
            value={gauge.output_channel} 
            onChange={(e) => updateGauge({ output_channel: e.target.value })}
          />
        </div>
        
        <div className="property-row">
          <div className="property-group half">
            <label>最小值</label>
            <input 
              type="number" 
              value={gauge.min} 
              onChange={(e) => updateGauge({ min: parseFloat(e.target.value) || 0 })}
            />
          </div>
          <div className="property-group half">
            <label>最大值</label>
            <input 
              type="number" 
              value={gauge.max} 
              onChange={(e) => updateGauge({ max: parseFloat(e.target.value) || 100 })}
            />
          </div>
        </div>
        
        <div className="property-group">
          <label>单位</label>
          <input 
            type="text" 
            value={gauge.units || ''} 
            onChange={(e) => updateGauge({ units: e.target.value })}
          />
        </div>
        
        <div className="property-row">
          <div className="property-group half">
            <label>警告</label>
            <input 
              type="number" 
              value={gauge.high_warning ?? ''} 
              onChange={(e) => updateGauge({ high_warning: e.target.value ? parseFloat(e.target.value) : null })}
            />
          </div>
          <div className="property-group half">
            <label>临界</label>
            <input 
              type="number" 
              value={gauge.high_critical ?? ''} 
              onChange={(e) => updateGauge({ high_critical: e.target.value ? parseFloat(e.target.value) : null })}
            />
          </div>
        </div>
        
        <div className="property-group">
          <label>仪表类型</label>
          <select 
            value={gauge.gauge_painter || 'AnalogGauge'}
            onChange={(e) => updateGauge({ gauge_painter: e.target.value as TsGaugeConfig['gauge_painter'] })}
          >
            <option value="AnalogGauge">模拟仪表</option>
            <option value="BasicReadout">数字读数</option>
            <option value="HorizontalBarGauge">水平条</option>
            <option value="VerticalBarGauge">垂直条</option>
            <option value="AsymmetricSweepGauge">扫描仪表</option>
            <option value="RoundGauge">圆形仪表</option>
            <option value="Tachometer">转速表</option>
            <option value="FuelMeter">油量表</option>
            <option value="LineGraph">折线图</option>
            <option value="Histogram">直方图</option>
          </select>
        </div>
        
        <div className="property-group">
          <label>小数位数</label>
          <input 
            type="number" 
            min={0}
            max={5}
            value={gauge.value_digits ?? 1} 
            onChange={(e) => updateGauge({ value_digits: parseInt(e.target.value) || 0 })}
          />
        </div>
        
        <div className="property-group">
          <label className="checkbox-label">
            <input 
              type="checkbox" 
              checked={gauge.shape_locked_to_aspect ?? false} 
              onChange={(e) => updateGauge({ shape_locked_to_aspect: e.target.checked })}
            />
            锁定宽高比
          </label>
        </div>
        
        <div className="property-section">
          <h4>位置与大小</h4>
          <div className="property-row">
            <div className="property-group half">
              <label>X (%)</label>
              <input 
                type="number" 
                step={0.01}
                value={((gauge.relative_x ?? 0) * 100).toFixed(1)} 
                onChange={(e) => updateGauge({ relative_x: parseFloat(e.target.value) / 100 })}
              />
            </div>
            <div className="property-group half">
              <label>Y (%)</label>
              <input 
                type="number" 
                step={0.01}
                value={((gauge.relative_y ?? 0) * 100).toFixed(1)} 
                onChange={(e) => updateGauge({ relative_y: parseFloat(e.target.value) / 100 })}
              />
            </div>
          </div>
          <div className="property-row">
            <div className="property-group half">
              <label>宽度 (%)</label>
              <input 
                type="number" 
                step={0.01}
                value={((gauge.relative_width ?? 0.25) * 100).toFixed(1)} 
                onChange={(e) => updateGauge({ relative_width: parseFloat(e.target.value) / 100 })}
              />
            </div>
            <div className="property-group half">
              <label>高度 (%)</label>
              <input 
                type="number" 
                step={0.01}
                value={((gauge.relative_height ?? 0.25) * 100).toFixed(1)} 
                onChange={(e) => updateGauge({ relative_height: parseFloat(e.target.value) / 100 })}
              />
            </div>
          </div>
        </div>
      </div>
    );
  }
  
  if (isIndicator(component)) {
    const indicator = component.Indicator;
    
    const updateIndicator = (updates: Partial<TsIndicatorConfig>) => {
      onChange({ Indicator: { ...indicator, ...updates } });
    };
    
    return (
      <div className="property-editor">
        <div className="property-group">
          <label>输出通道</label>
          <input 
            type="text" 
            value={indicator.output_channel} 
            onChange={(e) => updateIndicator({ output_channel: e.target.value })}
          />
        </div>
        
        <div className="property-group">
          <label>开启标签</label>
          <input 
            type="text" 
            value={indicator.on_text || ''} 
            onChange={(e) => updateIndicator({ on_text: e.target.value })}
          />
        </div>
        
        <div className="property-group">
          <label>关闭标签</label>
          <input 
            type="text" 
            value={indicator.off_text || ''} 
            onChange={(e) => updateIndicator({ off_text: e.target.value })}
          />
        </div>
        
        <div className="property-section">
          <h4>位置与大小</h4>
          <div className="property-row">
            <div className="property-group half">
              <label>X (%)</label>
              <input 
                type="number" 
                step={0.01}
                value={((indicator.relative_x ?? 0) * 100).toFixed(1)} 
                onChange={(e) => updateIndicator({ relative_x: parseFloat(e.target.value) / 100 })}
              />
            </div>
            <div className="property-group half">
              <label>Y (%)</label>
              <input 
                type="number" 
                step={0.01}
                value={((indicator.relative_y ?? 0) * 100).toFixed(1)} 
                onChange={(e) => updateIndicator({ relative_y: parseFloat(e.target.value) / 100 })}
              />
            </div>
          </div>
          <div className="property-row">
            <div className="property-group half">
              <label>宽度 (%)</label>
              <input 
                type="number" 
                step={0.01}
                value={((indicator.relative_width ?? 0.1) * 100).toFixed(1)} 
                onChange={(e) => updateIndicator({ relative_width: parseFloat(e.target.value) / 100 })}
              />
            </div>
            <div className="property-group half">
              <label>高度 (%)</label>
              <input 
                type="number" 
                step={0.01}
                value={((indicator.relative_height ?? 0.05) * 100).toFixed(1)} 
                onChange={(e) => updateIndicator({ relative_height: parseFloat(e.target.value) / 100 })}
              />
            </div>
          </div>
        </div>
      </div>
    );
  }
  
  return <p>未知组件类型</p>;
}
