/**
 * TableEditor3DNew - 3D Table Visualization using react-three-fiber
 * 
 * Provides an interactive 3D surface mesh visualization of ECU tuning tables
 * with orbit controls, cell highlighting, and heatmap coloring.
 */

import { useRef, useMemo, useState, useCallback } from 'react';
import { Canvas, useFrame, ThreeEvent } from '@react-three/fiber';
import { OrbitControls, Text, Html, PerspectiveCamera } from '@react-three/drei';
import * as THREE from 'three';
import { ArrowLeft, Maximize2, Minimize2 } from 'lucide-react';
import { valueToHeatmapColor, HeatmapScheme } from '../../utils/heatmapColors';
import './TableEditor3D.css';

interface TableEditor3DNewProps {
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

interface SurfaceMeshProps {
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  heatmapScheme: HeatmapScheme | string[];
  onCellClick?: (x: number, y: number) => void;
  selectedCell?: { x: number; y: number } | null;
  liveCell?: { x: number; y: number } | null;
  wireframe: boolean;
}

/** Creates the 3D surface mesh geometry from table data */
function SurfaceMesh({ 
  x_bins, 
  y_bins, 
  z_values, 
  heatmapScheme,
  onCellClick, 
  selectedCell,
  liveCell,
  wireframe
}: SurfaceMeshProps) {
  const meshRef = useRef<THREE.Mesh>(null);
  
  const { geometry } = useMemo(() => {
    const xSize = x_bins.length;
    const ySize = y_bins.length;
    
    // Normalize bins to 0-10 range for better visualization
    const xMin = Math.min(...x_bins);
    const xMax = Math.max(...x_bins);
    const yMin = Math.min(...y_bins);
    const yMax = Math.max(...y_bins);
    const zFlat = z_values.flat();
    const zMin = Math.min(...zFlat);
    const zMax = Math.max(...zFlat);
    
    const normalizeX = (v: number) => ((v - xMin) / (xMax - xMin || 1)) * 10;
    const normalizeY = (v: number) => ((v - yMin) / (yMax - yMin || 1)) * 10;
    const normalizeZ = (v: number) => ((v - zMin) / (zMax - zMin || 1)) * 5;
    
    // Create vertices
    const vertices: number[] = [];
    const colorArray: number[] = [];
    const indices: number[] = [];
    
    for (let yi = 0; yi < ySize; yi++) {
      for (let xi = 0; xi < xSize; xi++) {
        const x = normalizeX(x_bins[xi]);
        const y = normalizeY(y_bins[yi]);
        const z = normalizeZ(z_values[yi]?.[xi] ?? 0);
        
        vertices.push(x, z, y); // Y-up convention in Three.js
        
        // Get color from heatmap
        const colorHex = valueToHeatmapColor(z_values[yi]?.[xi] ?? 0, zMin, zMax, heatmapScheme);
        const color = new THREE.Color(colorHex);
        colorArray.push(color.r, color.g, color.b);
      }
    }
    
    // Create triangle indices for the mesh
    for (let yi = 0; yi < ySize - 1; yi++) {
      for (let xi = 0; xi < xSize - 1; xi++) {
        const topLeft = yi * xSize + xi;
        const topRight = topLeft + 1;
        const bottomLeft = (yi + 1) * xSize + xi;
        const bottomRight = bottomLeft + 1;
        
        // Two triangles per quad
        indices.push(topLeft, bottomLeft, topRight);
        indices.push(topRight, bottomLeft, bottomRight);
      }
    }
    
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.Float32BufferAttribute(vertices, 3));
    geo.setAttribute('color', new THREE.Float32BufferAttribute(colorArray, 3));
    geo.setIndex(indices);
    geo.computeVertexNormals();
    
    return { geometry: geo, colors: colorArray, minZ: zMin, maxZ: zMax };
  }, [x_bins, y_bins, z_values, heatmapScheme]);

  // Handle click on mesh
  const handleClick = useCallback((event: ThreeEvent<MouseEvent>) => {
    if (!onCellClick || !event.face) return;
    
    event.stopPropagation();
    
    // Get the face indices to find which cell was clicked
    const faceIndex = event.faceIndex;
    if (faceIndex === undefined || faceIndex === null) return;
    
    // Each quad has 2 triangles, so divide by 2 to get quad index
    const quadIndex = Math.floor(faceIndex / 2);
    const xSize = x_bins.length - 1;
    const yi = Math.floor(quadIndex / xSize);
    const xi = quadIndex % xSize;
    
    onCellClick(xi, yi);
  }, [onCellClick, x_bins.length]);

  return (
    <>
      <mesh 
        ref={meshRef}
        geometry={geometry}
        onClick={handleClick}
      >
        <meshStandardMaterial 
          vertexColors 
          side={THREE.DoubleSide}
          wireframe={wireframe}
          roughness={0.6}
          metalness={0.1}
        />
      </mesh>
      
      {/* Show selected cell marker */}
      {selectedCell && (
        <SelectedCellMarker 
          x_bins={x_bins}
          y_bins={y_bins}
          z_values={z_values}
          cellX={selectedCell.x}
          cellY={selectedCell.y}
        />
      )}
      
      {/* Show live cell indicator (triangle + outline) */}
      {liveCell && (
        <LiveCellIndicator 
          x_bins={x_bins}
          y_bins={y_bins}
          z_values={z_values}
          cellX={liveCell.x}
          cellY={liveCell.y}
        />
      )}
    </>
  );
}

/** Helper to get normalized position for a cell */
function getNormalizedPosition(
  x_bins: number[],
  y_bins: number[],
  z_values: number[][],
  cellX: number,
  cellY: number
): THREE.Vector3 {
  const xMin = Math.min(...x_bins);
  const xMax = Math.max(...x_bins);
  const yMin = Math.min(...y_bins);
  const yMax = Math.max(...y_bins);
  const zFlat = z_values.flat();
  const zMin = Math.min(...zFlat);
  const zMax = Math.max(...zFlat);
  
  const normalizeX = (v: number) => ((v - xMin) / (xMax - xMin || 1)) * 10;
  const normalizeY = (v: number) => ((v - yMin) / (yMax - yMin || 1)) * 10;
  const normalizeZ = (v: number) => ((v - zMin) / (zMax - zMin || 1)) * 5;
  
  const x = normalizeX(x_bins[cellX] ?? 0);
  const y = normalizeY(y_bins[cellY] ?? 0);
  const z = normalizeZ(z_values[cellY]?.[cellX] ?? 0);
  
  return new THREE.Vector3(x, z, y);
}

/** Live cell indicator with inverted triangle and cell outline */
function LiveCellIndicator({ 
  x_bins, 
  y_bins, 
  z_values, 
  cellX, 
  cellY
}: {
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  cellX: number;
  cellY: number;
}) {
  const coneRef = useRef<THREE.Mesh>(null);
  const outlineRef = useRef<THREE.LineSegments>(null);
  
  const { position, cellOutlineGeometry } = useMemo(() => {
    const pos = getNormalizedPosition(x_bins, y_bins, z_values, cellX, cellY);
    
    // Calculate cell bounds for outline
    const xMin = Math.min(...x_bins);
    const xMax = Math.max(...x_bins);
    const yMin = Math.min(...y_bins);
    const yMax = Math.max(...y_bins);
    const zFlat = z_values.flat();
    const zMin = Math.min(...zFlat);
    const zMax = Math.max(...zFlat);
    
    const normalizeX = (v: number) => ((v - xMin) / (xMax - xMin || 1)) * 10;
    const normalizeY = (v: number) => ((v - yMin) / (yMax - yMin || 1)) * 10;
    const normalizeZ = (v: number) => ((v - zMin) / (zMax - zMin || 1)) * 5;
    
    // Get half cell size
    const xStep = x_bins.length > 1 ? normalizeX(x_bins[1]) - normalizeX(x_bins[0]) : 1;
    const yStep = y_bins.length > 1 ? normalizeY(y_bins[1]) - normalizeY(y_bins[0]) : 1;
    const halfX = xStep / 2;
    const halfY = yStep / 2;
    
    // Get z values at cell corners (or use center value for all corners if at edge)
    const getZ = (xi: number, yi: number) => {
      const clampedX = Math.max(0, Math.min(xi, x_bins.length - 1));
      const clampedY = Math.max(0, Math.min(yi, y_bins.length - 1));
      return normalizeZ(z_values[clampedY]?.[clampedX] ?? 0);
    };
    
    // Create outline box on surface
    const z = getZ(cellX, cellY);
    const vertices = new Float32Array([
      // Bottom square (slightly above surface)
      pos.x - halfX, z + 0.05, pos.z - halfY,
      pos.x + halfX, z + 0.05, pos.z - halfY,
      pos.x + halfX, z + 0.05, pos.z - halfY,
      pos.x + halfX, z + 0.05, pos.z + halfY,
      pos.x + halfX, z + 0.05, pos.z + halfY,
      pos.x - halfX, z + 0.05, pos.z + halfY,
      pos.x - halfX, z + 0.05, pos.z + halfY,
      pos.x - halfX, z + 0.05, pos.z - halfY,
    ]);
    
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(vertices, 3));
    
    return { position: pos, cellOutlineGeometry: geo };
  }, [x_bins, y_bins, z_values, cellX, cellY]);

  // Pulsing animation
  useFrame((state) => {
    if (coneRef.current) {
      const scale = 1 + Math.sin(state.clock.elapsedTime * 4) * 0.15;
      coneRef.current.scale.setScalar(scale);
    }
    if (outlineRef.current && outlineRef.current.material instanceof THREE.LineBasicMaterial) {
      const intensity = 0.7 + Math.sin(state.clock.elapsedTime * 4) * 0.3;
      outlineRef.current.material.opacity = intensity;
    }
  });

  return (
    <>
      {/* Cell outline on surface */}
      <lineSegments ref={outlineRef} geometry={cellOutlineGeometry}>
        <lineBasicMaterial color="#00ff00" linewidth={2} transparent opacity={1} />
      </lineSegments>
      
      {/* Inverted triangle (cone) floating above */}
      <mesh 
        ref={coneRef} 
        position={[position.x, position.y + 1.2, position.z]}
        rotation={[Math.PI, 0, 0]} // Inverted (pointing down)
      >
        <coneGeometry args={[0.25, 0.5, 4]} />
        <meshStandardMaterial 
          color="#00ff00" 
          emissive="#00ff00" 
          emissiveIntensity={0.6}
          transparent
          opacity={0.9}
        />
      </mesh>
    </>
  );
}

/** Selected cell marker (simple outline) */
function SelectedCellMarker({ 
  x_bins, 
  y_bins, 
  z_values, 
  cellX, 
  cellY
}: {
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  cellX: number;
  cellY: number;
}) {
  const outlineGeometry = useMemo(() => {
    const pos = getNormalizedPosition(x_bins, y_bins, z_values, cellX, cellY);
    
    const xMin = Math.min(...x_bins);
    const xMax = Math.max(...x_bins);
    const yMin = Math.min(...y_bins);
    const yMax = Math.max(...y_bins);
    const zFlat = z_values.flat();
    const zMin = Math.min(...zFlat);
    const zMax = Math.max(...zFlat);
    
    const normalizeX = (v: number) => ((v - xMin) / (xMax - xMin || 1)) * 10;
    const normalizeY = (v: number) => ((v - yMin) / (yMax - yMin || 1)) * 10;
    const normalizeZ = (v: number) => ((v - zMin) / (zMax - zMin || 1)) * 5;
    
    const xStep = x_bins.length > 1 ? normalizeX(x_bins[1]) - normalizeX(x_bins[0]) : 1;
    const yStep = y_bins.length > 1 ? normalizeY(y_bins[1]) - normalizeY(y_bins[0]) : 1;
    const halfX = xStep / 2;
    const halfY = yStep / 2;
    const z = normalizeZ(z_values[cellY]?.[cellX] ?? 0);
    
    const vertices = new Float32Array([
      pos.x - halfX, z + 0.05, pos.z - halfY,
      pos.x + halfX, z + 0.05, pos.z - halfY,
      pos.x + halfX, z + 0.05, pos.z - halfY,
      pos.x + halfX, z + 0.05, pos.z + halfY,
      pos.x + halfX, z + 0.05, pos.z + halfY,
      pos.x - halfX, z + 0.05, pos.z + halfY,
      pos.x - halfX, z + 0.05, pos.z + halfY,
      pos.x - halfX, z + 0.05, pos.z - halfY,
    ]);
    
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(vertices, 3));
    return geo;
  }, [x_bins, y_bins, z_values, cellX, cellY]);

  return (
    <lineSegments geometry={outlineGeometry}>
      <lineBasicMaterial color="#ffff00" linewidth={2} />
    </lineSegments>
  );
}

/** Smooth trail line with fading opacity */
function TrailLine({
  x_bins,
  y_bins,
  z_values,
  historyTrail
}: {
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  historyTrail: Array<{ row: number; col: number; time: number }>;
}) {
  const TRAIL_DURATION_MS = 3000;

  const curveGeometry = useMemo(() => {
    if (historyTrail.length < 2) return null;
    
    const now = Date.now();
    const points: THREE.Vector3[] = [];
    const colorArray: number[] = [];
    
    // Get positions for each trail point
    for (const entry of historyTrail) {
      const age = now - entry.time;
      if (age > TRAIL_DURATION_MS) continue;
      
      const pos = getNormalizedPosition(x_bins, y_bins, z_values, entry.col, entry.row);
      pos.y += 0.1; // Slightly above surface
      points.push(pos);
      
      // Fade from green (newest) to transparent (oldest)
      const alpha = 1 - (age / TRAIL_DURATION_MS);
      colorArray.push(0, 1, 0, alpha); // RGBA
    }
    
    if (points.length < 2) return null;
    
    // Create smooth curve through points
    const splineCurve = new THREE.CatmullRomCurve3(points);
    const curvePoints = splineCurve.getPoints(points.length * 10);
    
    // Interpolate colors for curve points
    const interpolatedColors: number[] = [];
    for (let i = 0; i < curvePoints.length; i++) {
      const t = i / (curvePoints.length - 1);
      const colorIndex = Math.min(Math.floor(t * (colorArray.length / 4 - 1)), colorArray.length / 4 - 2);
      const localT = (t * (colorArray.length / 4 - 1)) - colorIndex;
      
      const r = colorArray[colorIndex * 4] * (1 - localT) + colorArray[(colorIndex + 1) * 4] * localT;
      const g = colorArray[colorIndex * 4 + 1] * (1 - localT) + colorArray[(colorIndex + 1) * 4 + 1] * localT;
      const b = colorArray[colorIndex * 4 + 2] * (1 - localT) + colorArray[(colorIndex + 1) * 4 + 2] * localT;
      interpolatedColors.push(r, g, b);
    }
    
    const geo = new THREE.BufferGeometry().setFromPoints(curvePoints);
    geo.setAttribute('color', new THREE.Float32BufferAttribute(interpolatedColors, 3));
    
    return geo;
  }, [x_bins, y_bins, z_values, historyTrail]);

  if (!curveGeometry) return null;

  // Use primitive to create THREE.Line directly
  return (
    <primitive object={new THREE.Line(curveGeometry, new THREE.LineBasicMaterial({ 
      vertexColors: true, 
      transparent: true, 
      opacity: 0.8 
    }))} />
  );
}

/** Cell grid overlay (2D grid lines on surface) */
function CellGridOverlay({
  x_bins,
  y_bins,
  z_values
}: {
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
}) {
  const linesGeometry = useMemo(() => {
    const xMin = Math.min(...x_bins);
    const xMax = Math.max(...x_bins);
    const yMin = Math.min(...y_bins);
    const yMax = Math.max(...y_bins);
    const zFlat = z_values.flat();
    const zMin = Math.min(...zFlat);
    const zMax = Math.max(...zFlat);
    
    const normalizeX = (v: number) => ((v - xMin) / (xMax - xMin || 1)) * 10;
    const normalizeY = (v: number) => ((v - yMin) / (yMax - yMin || 1)) * 10;
    const normalizeZ = (v: number) => ((v - zMin) / (zMax - zMin || 1)) * 5;
    
    const vertices: number[] = [];
    
    // Draw horizontal lines (along X axis for each Y)
    for (let yi = 0; yi < y_bins.length; yi++) {
      for (let xi = 0; xi < x_bins.length - 1; xi++) {
        const x1 = normalizeX(x_bins[xi]);
        const x2 = normalizeX(x_bins[xi + 1]);
        const y = normalizeY(y_bins[yi]);
        const z1 = normalizeZ(z_values[yi]?.[xi] ?? 0) + 0.02;
        const z2 = normalizeZ(z_values[yi]?.[xi + 1] ?? 0) + 0.02;
        
        vertices.push(x1, z1, y, x2, z2, y);
      }
    }
    
    // Draw vertical lines (along Y axis for each X)
    for (let xi = 0; xi < x_bins.length; xi++) {
      for (let yi = 0; yi < y_bins.length - 1; yi++) {
        const x = normalizeX(x_bins[xi]);
        const y1 = normalizeY(y_bins[yi]);
        const y2 = normalizeY(y_bins[yi + 1]);
        const z1 = normalizeZ(z_values[yi]?.[xi] ?? 0) + 0.02;
        const z2 = normalizeZ(z_values[yi + 1]?.[xi] ?? 0) + 0.02;
        
        vertices.push(x, z1, y1, x, z2, y2);
      }
    }
    
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.Float32BufferAttribute(vertices, 3));
    return geo;
  }, [x_bins, y_bins, z_values]);

  return (
    <lineSegments geometry={linesGeometry}>
      <lineBasicMaterial color="#ffffff" transparent opacity={0.3} />
    </lineSegments>
  );
}

/** Axis labels and grid */
function AxisLabels({ 
  x_label, 
  y_label, 
  z_label,
  x_bins,
  y_bins
}: { 
  x_label?: string; 
  y_label?: string; 
  z_label?: string;
  x_bins: number[];
  y_bins: number[];
}) {
  return (
    <>
      {/* X-axis label */}
      <Text
        position={[5, -0.5, -0.5]}
        fontSize={0.4}
        color="#ffffff"
        anchorX="center"
      >
        {x_label || 'X'}
      </Text>
      
      {/* Y-axis label */}
      <Text
        position={[-0.5, -0.5, 5]}
        fontSize={0.4}
        color="#ffffff"
        anchorX="center"
        rotation={[0, Math.PI / 2, 0]}
      >
        {y_label || 'Y'}
      </Text>
      
      {/* Z-axis label */}
      <Text
        position={[-0.8, 2.5, -0.8]}
        fontSize={0.4}
        color="#ffffff"
        anchorX="center"
        rotation={[0, 0, Math.PI / 2]}
      >
        {z_label || 'Z'}
      </Text>
      
      {/* Grid floor */}
      <gridHelper args={[10, 10, '#444444', '#333333']} position={[5, 0, 5]} />
      
      {/* Axis value labels (sample) */}
      <Text position={[0, -0.3, -0.3]} fontSize={0.25} color="#888888">
        {x_bins[0]?.toFixed(0)}
      </Text>
      <Text position={[10, -0.3, -0.3]} fontSize={0.25} color="#888888">
        {x_bins[x_bins.length - 1]?.toFixed(0)}
      </Text>
      <Text position={[-0.3, -0.3, 0]} fontSize={0.25} color="#888888">
        {y_bins[0]?.toFixed(0)}
      </Text>
      <Text position={[-0.3, -0.3, 10]} fontSize={0.25} color="#888888">
        {y_bins[y_bins.length - 1]?.toFixed(0)}
      </Text>
    </>
  );
}

/** Tooltip showing cell value on hover */
function CellTooltip({ 
  x_bins, 
  y_bins, 
  z_values, 
  hoveredCell 
}: { 
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  hoveredCell: { x: number; y: number } | null;
}) {
  if (!hoveredCell) return null;
  
  const value = z_values[hoveredCell.y]?.[hoveredCell.x];
  const xVal = x_bins[hoveredCell.x];
  const yVal = y_bins[hoveredCell.y];
  
  return (
    <Html position={[5, 6, 5]} center>
      <div className="table3d-tooltip">
        <div>X: {xVal?.toFixed(1)}</div>
        <div>Y: {yVal?.toFixed(1)}</div>
        <div>Value: {value?.toFixed(2)}</div>
      </div>
    </Html>
  );
}

/** Main 3D scene */
function Scene({ 
  x_bins, 
  y_bins, 
  z_values,
  x_label,
  y_label,
  z_label,
  heatmapScheme,
  onCellClick,
  selectedCell,
  liveCell,
  historyTrail,
  wireframe,
  showCells
}: {
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  x_label?: string;
  y_label?: string;
  z_label?: string;
  heatmapScheme: HeatmapScheme | string[];
  onCellClick?: (x: number, y: number) => void;
  selectedCell?: { x: number; y: number } | null;
  liveCell?: { x: number; y: number } | null;
  historyTrail?: Array<{ row: number; col: number; time: number }>;
  wireframe: boolean;
  showCells: boolean;
}) {
  const [hoveredCell, _setHoveredCell] = useState<{ x: number; y: number } | null>(null);

  return (
    <>
      <PerspectiveCamera makeDefault position={[15, 10, 15]} fov={45} />
      <OrbitControls 
        target={[5, 2, 5]} 
        enableDamping 
        dampingFactor={0.1}
        minDistance={5}
        maxDistance={40}
      />
      
      {/* Lighting */}
      <ambientLight intensity={0.4} />
      <directionalLight position={[10, 15, 10]} intensity={0.8} castShadow />
      <directionalLight position={[-5, 10, -5]} intensity={0.3} />
      
      {/* Surface mesh */}
      <SurfaceMesh
        x_bins={x_bins}
        y_bins={y_bins}
        z_values={z_values}
        heatmapScheme={heatmapScheme}
        onCellClick={onCellClick}
        selectedCell={selectedCell}
        liveCell={liveCell}
        wireframe={wireframe}
      />
      
      {/* Cell grid overlay */}
      {showCells && (
        <CellGridOverlay
          x_bins={x_bins}
          y_bins={y_bins}
          z_values={z_values}
        />
      )}
      
      {/* History trail line */}
      {historyTrail && historyTrail.length > 1 && (
        <TrailLine
          x_bins={x_bins}
          y_bins={y_bins}
          z_values={z_values}
          historyTrail={historyTrail}
        />
      )}
      
      {/* Axis labels and grid */}
      <AxisLabels 
        x_label={x_label}
        y_label={y_label}
        z_label={z_label}
        x_bins={x_bins}
        y_bins={y_bins}
      />
      
      {/* Tooltip */}
      <CellTooltip
        x_bins={x_bins}
        y_bins={y_bins}
        z_values={z_values}
        hoveredCell={hoveredCell}
      />
    </>
  );
}

export default function TableEditor3DNew({
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
}: TableEditor3DNewProps) {
  const [wireframe, setWireframe] = useState(false);
  const [showCells, setShowCells] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

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
