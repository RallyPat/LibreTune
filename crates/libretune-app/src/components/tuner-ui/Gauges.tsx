import { useRef, useEffect, useMemo } from 'react';
import './Gauges.css';

export interface GaugeConfig {
  type: 'analog' | 'digital' | 'bar' | 'sweep' | 'led';
  channel: string;
  title: string;
  min: number;
  max: number;
  units?: string;
  lowWarning?: number;
  highWarning?: number;
  lowCritical?: number;
  highCritical?: number;
  precision?: number;
  sweepAngle?: number; // Default 270 for analog
  startAngle?: number; // Default -135 for analog
}

interface GaugeProps {
  config: GaugeConfig;
  value: number;
  width?: number;
  height?: number;
}

export function Gauge({ config, value, width = 200, height = 200 }: GaugeProps) {
  switch (config.type) {
    case 'analog':
      return <AnalogGauge config={config} value={value} size={Math.min(width, height)} />;
    case 'digital':
      return <DigitalGauge config={config} value={value} width={width} height={height} />;
    case 'bar':
      return <BarGauge config={config} value={value} width={width} height={height} />;
    case 'sweep':
      return <SweepGauge config={config} value={value} size={Math.min(width, height)} />;
    case 'led':
      return <LedIndicator config={config} value={value} size={Math.min(width, height)} />;
    default:
      return <DigitalGauge config={config} value={value} width={width} height={height} />;
  }
}

// ============================================
// ANALOG GAUGE (Classic dial with needle)
// ============================================
function AnalogGauge({ config, value, size }: { config: GaugeConfig; value: number; size: number }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  
  const sweepAngle = config.sweepAngle ?? 270;
  const startAngle = config.startAngle ?? -135;
  const precision = config.precision ?? 0;
  
  // Calculate needle angle
  const normalizedValue = Math.max(config.min, Math.min(config.max, value));
  const valuePercent = (normalizedValue - config.min) / (config.max - config.min);
  const needleAngle = startAngle + valuePercent * sweepAngle;
  
  // Get status color
  const getStatusColor = useMemo(() => {
    if (config.lowCritical !== undefined && value < config.lowCritical) return 'var(--gauge-critical)';
    if (config.highCritical !== undefined && value > config.highCritical) return 'var(--gauge-critical)';
    if (config.lowWarning !== undefined && value < config.lowWarning) return 'var(--gauge-warning)';
    if (config.highWarning !== undefined && value > config.highWarning) return 'var(--gauge-warning)';
    return 'var(--gauge-normal)';
  }, [config, value]);
  
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    
    const dpr = window.devicePixelRatio || 1;
    canvas.width = size * dpr;
    canvas.height = size * dpr;
    ctx.scale(dpr, dpr);
    
    const centerX = size / 2;
    const centerY = size / 2;
    const radius = size * 0.4;
    
    // Clear
    ctx.clearRect(0, 0, size, size);
    
    // Draw bezel
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius + 8, 0, Math.PI * 2);
    ctx.fillStyle = '#333';
    ctx.fill();
    
    // Draw face
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius + 2, 0, Math.PI * 2);
    ctx.fillStyle = '#1a1a1a';
    ctx.fill();
    
    // Draw arc background
    const startRad = (startAngle - 90) * Math.PI / 180;
    const endRad = (startAngle + sweepAngle - 90) * Math.PI / 180;
    
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius - 5, startRad, endRad);
    ctx.strokeStyle = '#2a2a2a';
    ctx.lineWidth = 8;
    ctx.stroke();
    
    // Draw warning zones
    const drawZone = (start: number, end: number, color: string) => {
      const startPercent = (start - config.min) / (config.max - config.min);
      const endPercent = (end - config.min) / (config.max - config.min);
      const zoneStartAngle = (startAngle + startPercent * sweepAngle - 90) * Math.PI / 180;
      const zoneEndAngle = (startAngle + endPercent * sweepAngle - 90) * Math.PI / 180;
      
      ctx.beginPath();
      ctx.arc(centerX, centerY, radius - 5, zoneStartAngle, zoneEndAngle);
      ctx.strokeStyle = color;
      ctx.lineWidth = 8;
      ctx.stroke();
    };
    
    if (config.lowCritical !== undefined) {
      drawZone(config.min, config.lowCritical, '#ff4444');
    }
    if (config.lowWarning !== undefined && config.lowCritical !== undefined) {
      drawZone(config.lowCritical, config.lowWarning, '#ffaa00');
    } else if (config.lowWarning !== undefined) {
      drawZone(config.min, config.lowWarning, '#ffaa00');
    }
    if (config.highCritical !== undefined) {
      drawZone(config.highCritical, config.max, '#ff4444');
    }
    if (config.highWarning !== undefined && config.highCritical !== undefined) {
      drawZone(config.highWarning, config.highCritical, '#ffaa00');
    } else if (config.highWarning !== undefined) {
      drawZone(config.highWarning, config.max, '#ffaa00');
    }
    
    // Draw tick marks
    const numMajorTicks = 10;
    const numMinorTicks = 50;
    
    for (let i = 0; i <= numMinorTicks; i++) {
      const angle = (startAngle + (i / numMinorTicks) * sweepAngle - 90) * Math.PI / 180;
      const isMajor = i % (numMinorTicks / numMajorTicks) === 0;
      const innerRadius = radius - (isMajor ? 20 : 12);
      const outerRadius = radius - 2;
      
      ctx.beginPath();
      ctx.moveTo(centerX + innerRadius * Math.cos(angle), centerY + innerRadius * Math.sin(angle));
      ctx.lineTo(centerX + outerRadius * Math.cos(angle), centerY + outerRadius * Math.sin(angle));
      ctx.strokeStyle = isMajor ? '#c0c0c0' : '#606060';
      ctx.lineWidth = isMajor ? 2 : 1;
      ctx.stroke();
      
      // Draw labels for major ticks
      if (isMajor) {
        const labelValue = config.min + (i / numMinorTicks) * (config.max - config.min);
        const labelRadius = radius - 30;
        ctx.fillStyle = '#a0a0a0';
        ctx.font = `${size * 0.06}px sans-serif`;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(
          labelValue.toFixed(precision),
          centerX + labelRadius * Math.cos(angle),
          centerY + labelRadius * Math.sin(angle)
        );
      }
    }
    
    // Draw needle
    const needleRad = (needleAngle - 90) * Math.PI / 180;
    const needleLength = radius - 15;
    
    ctx.save();
    ctx.shadowColor = getStatusColor;
    ctx.shadowBlur = 10;
    
    ctx.beginPath();
    ctx.moveTo(centerX, centerY);
    ctx.lineTo(
      centerX + needleLength * Math.cos(needleRad),
      centerY + needleLength * Math.sin(needleRad)
    );
    ctx.strokeStyle = getStatusColor;
    ctx.lineWidth = 3;
    ctx.lineCap = 'round';
    ctx.stroke();
    ctx.restore();
    
    // Draw center cap
    ctx.beginPath();
    ctx.arc(centerX, centerY, 8, 0, Math.PI * 2);
    ctx.fillStyle = '#444';
    ctx.fill();
    
    // Draw title
    ctx.fillStyle = '#ffffff';
    ctx.font = `bold ${size * 0.08}px sans-serif`;
    ctx.textAlign = 'center';
    ctx.fillText(config.title, centerX, centerY + radius * 0.4);
    
    // Draw value
    ctx.font = `bold ${size * 0.12}px monospace`;
    ctx.fillStyle = getStatusColor;
    ctx.fillText(normalizedValue.toFixed(precision), centerX, centerY + radius * 0.6);
    
    // Draw units
    if (config.units) {
      ctx.font = `${size * 0.06}px sans-serif`;
      ctx.fillStyle = '#808080';
      ctx.fillText(config.units, centerX, centerY + radius * 0.75);
    }
    
  }, [config, value, size, sweepAngle, startAngle, needleAngle, normalizedValue, precision, getStatusColor]);
  
  return (
    <div className="gauge gauge-analog">
      <canvas
        ref={canvasRef}
        style={{ width: size, height: size }}
      />
    </div>
  );
}

// ============================================
// DIGITAL GAUGE (LCD-style numeric display)
// ============================================
function DigitalGauge({ config, value, width, height }: { config: GaugeConfig; value: number; width: number; height: number }) {
  const precision = config.precision ?? 0;
  const normalizedValue = Math.max(config.min, Math.min(config.max, value));
  
  const getStatusClass = () => {
    if (config.lowCritical !== undefined && value < config.lowCritical) return 'critical';
    if (config.highCritical !== undefined && value > config.highCritical) return 'critical';
    if (config.lowWarning !== undefined && value < config.lowWarning) return 'warning';
    if (config.highWarning !== undefined && value > config.highWarning) return 'warning';
    return 'normal';
  };
  
  return (
    <div className="gauge gauge-digital" style={{ width, height }}>
      <div className="gauge-digital-title">{config.title}</div>
      <div className={`gauge-digital-value ${getStatusClass()}`}>
        {normalizedValue.toFixed(precision)}
      </div>
      {config.units && (
        <div className="gauge-digital-units">{config.units}</div>
      )}
    </div>
  );
}

// ============================================
// BAR GAUGE (Horizontal or vertical bar)
// ============================================
function BarGauge({ config, value, width, height }: { config: GaugeConfig; value: number; width: number; height: number }) {
  const precision = config.precision ?? 0;
  const normalizedValue = Math.max(config.min, Math.min(config.max, value));
  const percent = ((normalizedValue - config.min) / (config.max - config.min)) * 100;
  
  const isVertical = height > width;
  
  const getBarColor = () => {
    if (config.lowCritical !== undefined && value < config.lowCritical) return 'var(--gauge-critical)';
    if (config.highCritical !== undefined && value > config.highCritical) return 'var(--gauge-critical)';
    if (config.lowWarning !== undefined && value < config.lowWarning) return 'var(--gauge-warning)';
    if (config.highWarning !== undefined && value > config.highWarning) return 'var(--gauge-warning)';
    return 'var(--gauge-normal)';
  };
  
  return (
    <div className={`gauge gauge-bar ${isVertical ? 'vertical' : 'horizontal'}`} style={{ width, height }}>
      <div className="gauge-bar-label">
        <span className="gauge-bar-title">{config.title}</span>
        <span className="gauge-bar-value">{normalizedValue.toFixed(precision)} {config.units}</span>
      </div>
      <div className="gauge-bar-track">
        <div
          className="gauge-bar-fill"
          style={{
            [isVertical ? 'height' : 'width']: `${percent}%`,
            backgroundColor: getBarColor(),
          }}
        />
        {/* Warning markers */}
        {config.lowWarning !== undefined && (
          <div
            className="gauge-bar-marker warning"
            style={{
              [isVertical ? 'bottom' : 'left']:
                `${((config.lowWarning - config.min) / (config.max - config.min)) * 100}%`,
            }}
          />
        )}
        {config.highWarning !== undefined && (
          <div
            className="gauge-bar-marker warning"
            style={{
              [isVertical ? 'bottom' : 'left']:
                `${((config.highWarning - config.min) / (config.max - config.min)) * 100}%`,
            }}
          />
        )}
      </div>
      <div className="gauge-bar-range">
        <span>{config.min}</span>
        <span>{config.max}</span>
      </div>
    </div>
  );
}

// ============================================
// SWEEP GAUGE (Arc/pie style gauge)
// ============================================
function SweepGauge({ config, value, size }: { config: GaugeConfig; value: number; size: number }) {
  const sweepAngle = config.sweepAngle ?? 270;
  const startAngle = config.startAngle ?? -135;
  const precision = config.precision ?? 0;
  
  const normalizedValue = Math.max(config.min, Math.min(config.max, value));
  const valuePercent = (normalizedValue - config.min) / (config.max - config.min);
  const currentSweep = valuePercent * sweepAngle;
  
  const radius = size * 0.4;
  const centerX = size / 2;
  const centerY = size / 2;
  
  // Convert angle to SVG arc parameters
  const polarToCartesian = (angle: number, r: number) => {
    const rad = (angle - 90) * Math.PI / 180;
    return {
      x: centerX + r * Math.cos(rad),
      y: centerY + r * Math.sin(rad),
    };
  };
  
  const describeArc = (startAngleDeg: number, endAngleDeg: number, r: number) => {
    const start = polarToCartesian(endAngleDeg, r);
    const end = polarToCartesian(startAngleDeg, r);
    const largeArc = endAngleDeg - startAngleDeg > 180 ? 1 : 0;
    return `M ${start.x} ${start.y} A ${r} ${r} 0 ${largeArc} 0 ${end.x} ${end.y}`;
  };
  
  const getColor = () => {
    if (config.lowCritical !== undefined && value < config.lowCritical) return 'var(--gauge-critical)';
    if (config.highCritical !== undefined && value > config.highCritical) return 'var(--gauge-critical)';
    if (config.lowWarning !== undefined && value < config.lowWarning) return 'var(--gauge-warning)';
    if (config.highWarning !== undefined && value > config.highWarning) return 'var(--gauge-warning)';
    return 'var(--gauge-normal)';
  };
  
  return (
    <div className="gauge gauge-sweep" style={{ width: size, height: size }}>
      <svg width={size} height={size}>
        {/* Background arc */}
        <path
          d={describeArc(startAngle, startAngle + sweepAngle, radius)}
          fill="none"
          stroke="#2a2a2a"
          strokeWidth={size * 0.08}
          strokeLinecap="round"
        />
        {/* Value arc */}
        {currentSweep > 0 && (
          <path
            d={describeArc(startAngle, startAngle + currentSweep, radius)}
            fill="none"
            stroke={getColor()}
            strokeWidth={size * 0.08}
            strokeLinecap="round"
          />
        )}
        {/* Title */}
        <text
          x={centerX}
          y={centerY - 10}
          textAnchor="middle"
          fill="white"
          fontSize={size * 0.08}
          fontWeight="bold"
        >
          {config.title}
        </text>
        {/* Value */}
        <text
          x={centerX}
          y={centerY + 15}
          textAnchor="middle"
          fill={getColor()}
          fontSize={size * 0.15}
          fontWeight="bold"
          fontFamily="monospace"
        >
          {normalizedValue.toFixed(precision)}
        </text>
        {/* Units */}
        {config.units && (
          <text
            x={centerX}
            y={centerY + 35}
            textAnchor="middle"
            fill="#808080"
            fontSize={size * 0.06}
          >
            {config.units}
          </text>
        )}
      </svg>
    </div>
  );
}

// ============================================
// LED INDICATOR (On/off or threshold-based)
// ============================================
function LedIndicator({ config, value, size }: { config: GaugeConfig; value: number; size: number }) {
  const isOn = value > (config.min + config.max) / 2;
  const isCritical = (config.lowCritical !== undefined && value < config.lowCritical) ||
                     (config.highCritical !== undefined && value > config.highCritical);
  const isWarning = (config.lowWarning !== undefined && value < config.lowWarning) ||
                    (config.highWarning !== undefined && value > config.highWarning);
  
  const getColor = () => {
    if (isCritical) return 'var(--gauge-critical)';
    if (isWarning) return 'var(--gauge-warning)';
    return isOn ? 'var(--gauge-normal)' : '#333';
  };
  
  return (
    <div className="gauge gauge-led" style={{ width: size, height: size }}>
      <div
        className={`gauge-led-light ${isOn ? 'on' : 'off'}`}
        style={{
          backgroundColor: getColor(),
          boxShadow: isOn ? `0 0 ${size * 0.2}px ${getColor()}` : 'none',
        }}
      />
      <div className="gauge-led-label">{config.title}</div>
    </div>
  );
}
