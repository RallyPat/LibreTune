import { useRef, useEffect, useState } from 'react';
import { ArrowLeft } from 'lucide-react';

interface TableEditor3DProps {
  title: string;
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  onBack: () => void;
}

export default function TableEditor3D({ title, x_bins, y_bins, z_values, onBack }: TableEditor3DProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [yawAngle, setYawAngle] = useState(250);
  const [rollAngle, setRollAngle] = useState(0);
  const [pitchAngle, setPitchAngle] = useState(30);
  const [zScale, setZScale] = useState(1.0);
  const [followMode, setFollowMode] = useState(false);
  const [showColorShade, setShowColorShade] = useState(true);

  const x_size = x_bins.length;
  const y_size = y_bins.length;

  useEffect(() => {
    render();
  }, [z_values, yawAngle, rollAngle, pitchAngle, zScale, showColorShade]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      switch(e.key) {
        case 'm':
          setYawAngle(prev => Math.min(360, prev + 5));
          break;
        case 'M':
          setYawAngle(prev => Math.max(0, prev - 5));
          break;
        case 'n':
          setRollAngle(prev => Math.min(90, prev + 5));
          break;
        case 'N':
          setRollAngle(prev => Math.max(-90, prev - 5));
          break;
        case 'ArrowUp':
          setPitchAngle(prev => Math.min(85, prev + 2));
          break;
        case 'ArrowDown':
          setPitchAngle(prev => Math.max(10, prev - 2));
          break;
        case 'f':
          setFollowMode(prev => !prev);
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  const _getCellColor = (_value: number, _x: number, _y: number) => {
    if (!showColorShade) return '#4CAF50';

    const minVal = Math.min(...z_values.flat());
    const maxVal = Math.max(...z_values.flat());
    const range = maxVal - minVal;

    if (range === 0) return '#888888';

    const ratio = (_value - minVal) / range;
    const hue = (1 - ratio) * 240;
    return `hsl(${hue}, 60%, 45%)`;
  };

  const project3D = (x: number, y: number, z: number) => {
    const x_bin = x_bins[Math.min(x, x_size - 1)];
    const y_bin = y_bins[Math.min(y, y_size - 1)];
    // @ts-expect-error - Reserved for future interpolation feature
    const _next_x_bin = x_bins[Math.min(x + 1, x_size - 1)] || x_bins[x_size - 1];
    // @ts-expect-error - Reserved for future interpolation feature
    const _next_y_bin = y_bins[Math.min(y + 1, y_size - 1)] || y_bins[y_size - 1];

    const cell_width = 1.0 / x_size;
    const cell_height = 1.0 / y_size;

    const center_x = x * cell_width + cell_width / 2;
    const center_y = y * cell_height + cell_height / 2;

    const x_range = x_bins[x_size - 1] - x_bins[0];
    const y_range = y_bins[y_size - 1] - y_bins[0];

    const normalized_x = (x_bin - x_bins[0]) / x_range;
    const normalized_y = (y_bin - y_bins[0]) / y_range;

    const z_scale_factor = (z - Math.min(...z_values.flat())) / (Math.max(...z_values.flat()) - Math.min(...z_values.flat())) * zScale;

    const projected_x = center_x + normalized_x * 0.4;
    const projected_y = center_y + normalized_y * 0.4;

    const rotated = rotateX(projected_x, projected_y, toRad(yawAngle));
    const rolled = rotateZ(rotated.x, rotated.y * z_scale_factor, 0, toRad(rollAngle));
    const pitched = rotateY(rolled.x, rolled.y, toRad(pitchAngle));

    return {
      x: pitched.x * 200 + 200,
      y: -pitched.y * 200 + 150,
      z: 0,
    };
  };

  const render = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const rect = canvas.parentElement?.getBoundingClientRect();
    if (!rect) return;

    canvas.width = rect.width;
    canvas.height = rect.height;

    ctx.fillStyle = '#1e293b';
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    ctx.lineWidth = 1;

    // Draw horizontal lines (Y-axis)
    for (let y = 0; y <= y_size; y++) {
      ctx.beginPath();
      ctx.moveTo(0, (y - 1) * (canvas.height / y_size));

      for (let x = 0; x <= x_size; x++) {
        if (x <= x_size && y <= y_size) {
          const zVal = z_values[Math.min(y - 1, y_size - 1)][Math.min(x - 1, x_size - 1)];
          const projected = project3D(x, y, zVal);
          ctx.lineTo(projected.x, projected.y);
        }
      }

      // Use color shading if enabled
      ctx.strokeStyle = showColorShade ? _getCellColor(z_values[Math.max(0, y - 2)][0], 0, y) : '#4a4a4a';
      ctx.stroke();
    }

    // Draw vertical lines (X-axis)
    for (let x = 0; x <= x_size; x++) {
      ctx.beginPath();
      ctx.moveTo((x - 1) * (canvas.width / x_size), 0);

      for (let y = 0; y <= y_size; y++) {
        if (x <= x_size && y <= y_size) {
          const zVal = z_values[y - 1][x - 1];
          const _projected = project3D(x, y, zVal);
          ctx.lineTo(_projected.x, _projected.y);
        }
      }

      // Use color shading if enabled
      ctx.strokeStyle = showColorShade ? _getCellColor(z_values[0][Math.max(0, x - 2)], x, 0) : '#4a4a4a';
      ctx.stroke();
    }

    // Draw filled cells with color shading
    if (showColorShade) {
      for (let y = 0; y < y_size; y++) {
        for (let x = 0; x < x_size; x++) {
          const zVal = z_values[y][x];

          // Get the 4 corners of the cell
          const p1 = project3D(x, y, zVal);
          const p2 = project3D(x + 1, y, zVal);
          const p3 = project3D(x + 1, y + 1, zVal);
          const p4 = project3D(x, y + 1, zVal);

          // Fill the cell with gradient color
          ctx.beginPath();
          ctx.moveTo(p1.x, p1.y);
          ctx.lineTo(p2.x, p2.y);
          ctx.lineTo(p3.x, p3.y);
          ctx.lineTo(p4.x, p4.y);
          ctx.closePath();

          // Get cell color based on Z value
          ctx.fillStyle = _getCellColor(zVal, x, y);
          ctx.fill();
        }
      }
    }
  };

  const toRad = (degrees: number) => degrees * Math.PI / 180;

  const rotateX = (x: number, y: number, angle: number) => ({
    x: x * Math.cos(angle) - y * Math.sin(angle),
    y: x * Math.sin(angle) + y * Math.cos(angle),
  });

  const rotateY = (x: number, y: number, angle: number) => ({
    x: x,
    y: y * Math.cos(angle) - x * Math.sin(angle),
  });

  const rotateZ = (x: number, y: number, z: number, angle: number) => ({
    x: x * Math.cos(angle) - y * Math.sin(angle),
    y: x * Math.sin(angle) + y * Math.cos(angle),
    z: z,
  });

  return (
    <div className="table-editor-3d">
      <div className="editor-header">
        <button className="back-btn" onClick={onBack}>
          <ArrowLeft size={18} />
          <span>Back</span>
        </button>
        <h1>{title} - 3D View</h1>
      </div>

      <div className="controls-panel">
        <div className="control-section">
          <h3>Rotation</h3>
          <div className="control-row">
            <label>Yaw (M/K)</label>
            <input
              type="range"
              min="0"
              max="360"
              value={yawAngle}
              onChange={e => setYawAngle(Number(e.target.value))}
            />
            <span>{yawAngle}°</span>
          </div>
          <div className="control-row">
            <label>Roll (N/J)</label>
            <input
              type="range"
              min="-90"
              max="90"
              value={rollAngle}
              onChange={e => setRollAngle(Number(e.target.value))}
            />
            <span>{rollAngle}°</span>
          </div>
          <div className="control-row">
            <label>Pitch (↑/↓)</label>
            <input
              type="range"
              min="10"
              max="85"
              value={pitchAngle}
              onChange={e => setPitchAngle(Number(e.target.value))}
            />
            <span>{pitchAngle}°</span>
          </div>
        </div>

        <div className="control-section">
          <h3>Scale</h3>
          <div className="control-row">
            <label>Z Scale</label>
            <input
              type="range"
              min="0.1"
              max="3"
              step="0.1"
              value={zScale}
              onChange={e => setZScale(Number(e.target.value))}
            />
            <span>{zScale.toFixed(1)}x</span>
          </div>
        </div>

        <div className="control-section">
          <h3>View</h3>
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={showColorShade}
              onChange={e => setShowColorShade(e.target.checked)}
            />
            Color Shade (Z)
          </label>
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={followMode}
              onChange={e => setFollowMode(e.target.checked)}
            />
            Follow Mode (F)
          </label>
          <label className="checkbox-label">
            <input
              type="checkbox"
              readOnly
              checked={true}
            />
            Wireframe
          </label>
        </div>
      </div>

      <div className="canvas-container">
        <div className="canvas-wrapper">
          <canvas ref={canvasRef} className="table-canvas" />
          <div className="active-position-display">
            <div>Active Position</div>
            <div>RPM: {x_bins[0]?.toFixed(0)}-{x_bins[x_size - 1]?.toFixed(0)}</div>
            <div>MAP: {y_bins[0]?.toFixed(0)}-{y_bins[y_size - 1]?.toFixed(0)}</div>
          </div>
        </div>
      </div>
    </div>
  );
}
