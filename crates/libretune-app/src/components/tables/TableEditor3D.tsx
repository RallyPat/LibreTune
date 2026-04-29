/**
 * TableEditor3D - 3D Table Visualization using react-three-fiber
 * 
 * Provides an interactive 3D surface mesh visualization of ECU tuning tables
 * with orbit controls, cell highlighting, and heatmap coloring.
 */

import { useRef, useState, useCallback } from 'react';
import { Canvas } from '@react-three/fiber';
import { ArrowLeft, Maximize2, Minimize2 } from 'lucide-react';
import { HeatmapScheme } from '../../utils/heatmapColors';
import './TableEditor3D.css';

interface TableEditor3DProps {
  title: string;
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  x_label?: string;
  y_label?: string;
  z_label?: string;
  x_units?: string;
  y_units?: string;
  z_units?: string;
  onBack: () => void;
  onCellSelect?: (x: number, y: number) => void;
  selectedCell?: { x: number; y: number } | null;
  liveCell?: { x: number; y: number } | null;
  historyTrail?: Array<{ row: number; col: number; time: number }>;
  heatmapScheme?: HeatmapScheme | string[];
}

import { Scene } from './3d/SceneComponents';

export default function TableEditor3D({
  title,
  x_bins,
  y_bins,
  z_values,
  x_label,
  y_label,
  z_label,
  x_units,
  y_units,
  z_units,
  onBack,
  onCellSelect,
  selectedCell,
  liveCell,
  historyTrail,
  heatmapScheme = 'tunerstudio'
}: TableEditor3DProps) {
  const [wireframe, setWireframe] = useState(false);
  const [showCells, setShowCells] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const canRender3D = x_bins.length > 1 && y_bins.length > 1 && z_values.length > 0 && z_values[0]?.length > 0;

  const handleCellClick = useCallback((x: number, y: number) => {
    onCellSelect?.(x, y);
  }, [onCellSelect]);

  const toggleFullscreen = useCallback(() => {
    if (!containerRef.current) return;
    
    if (!fullscreen) {
      containerRef.current.requestFullscreen?.();
    } else {
      document.exitFullscreen?.();
    }
    setFullscreen(!fullscreen);
  }, [fullscreen]);

  // Calculate min/max for display
  const zFlat = z_values.flat();
  const zMin = Math.min(...zFlat);
  const zMax = Math.max(...zFlat);

  if (!canRender3D) {
    return (
      <div ref={containerRef} className={`table-editor-3d ${fullscreen ? 'fullscreen' : ''}`}>
        <div className="table3d-header">
          <button className="table3d-back-btn" onClick={onBack}>
            <ArrowLeft size={16} />
            Back
          </button>
          <div className="table3d-title">{title}</div>
        </div>
        <div className="table3d-empty">
          3D view requires at least 2 bins on both axes.
        </div>
      </div>
    );
  }

  return (
    <div ref={containerRef} className={`table-editor-3d ${fullscreen ? 'fullscreen' : ''}`}>
      {/* Header toolbar */}
      <div className="table3d-header">
        <button className="table3d-back-btn" onClick={onBack}>
          <ArrowLeft size={16} />
          Back
        </button>
        
        <h3 className="table3d-title">{title}</h3>
        
        <div className="table3d-controls">
          <label className="table3d-checkbox">
            <input
              type="checkbox"
              checked={wireframe}
              onChange={(e) => setWireframe(e.target.checked)}
            />
            Wireframe
          </label>
          
          <label className="table3d-checkbox">
            <input
              type="checkbox"
              checked={showCells}
              onChange={(e) => setShowCells(e.target.checked)}
            />
            Show Cells
          </label>
          
          <button className="table3d-btn" onClick={toggleFullscreen} title="Toggle fullscreen">
            {fullscreen ? <Minimize2 size={16} /> : <Maximize2 size={16} />}
          </button>
        </div>
      </div>

      {/* 3D Canvas */}
      <div className="table3d-canvas-container">
        <Canvas shadows>
          <Scene
            x_bins={x_bins}
            y_bins={y_bins}
            z_values={z_values}
            x_label={`${x_label || 'X'}${x_units ? ` (${x_units})` : ''}`}
            y_label={`${y_label || 'Y'}${y_units ? ` (${y_units})` : ''}`}
            z_label={`${z_label || 'Z'}${z_units ? ` (${z_units})` : ''}`}
            heatmapScheme={heatmapScheme}
            onCellClick={handleCellClick}
            selectedCell={selectedCell}
            liveCell={liveCell}
            historyTrail={historyTrail}
            wireframe={wireframe}
            showCells={showCells}
          />
        </Canvas>
      </div>

      {/* Info panel */}
      <div className="table3d-info">
        <span>Range: {zMin.toFixed(2)} - {zMax.toFixed(2)}</span>
        <span>Size: {x_bins.length} × {y_bins.length}</span>
        <span className="table3d-hint">Drag to rotate • Scroll to zoom • Click cells to select</span>
      </div>
    </div>
  );
}
