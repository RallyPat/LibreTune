import { useState, useRef, useEffect } from 'react';
import './GaugeRenderer.css';

export interface GaugeConfig {
  id: string;
  gauge_type: GaugeType;
  channel: string;
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
  z_index: number;
  min_value: number;
  max_value: number;
  low_warning?: number;
  high_warning?: number;
  high_critical?: number;
  decimals: number;
  units: string;
  font_color: string;
  needle_color: string;
  trim_color: string;
  show_history: boolean;
  show_min_max: boolean;
  // Warning light specific
  on_condition?: string;
  on_color?: string;
  off_color?: string;
  blink?: boolean;
}

export enum GaugeType {
  AnalogDial = 'analog_dial',
  DigitalReadout = 'digital_readout',
  BarGauge = 'bar_gauge',
  SweepGauge = 'sweep_gauge',
  LEDIndicator = 'led_indicator',
  WarningLight = 'warning_light',
}

export default function GaugeRenderer({ config, realtimeData = {}, onConfigChange }: { config: GaugeConfig; realtimeData?: Record<string, number>; onConfigChange?: (config: GaugeConfig) => void }) {
  const [value, setValue] = useState(config.min_value);
  const [blinkState, setBlinkState] = useState(true);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const barCanvasRef = useRef<HTMLCanvasElement>(null);
  const sweepCanvasRef = useRef<HTMLCanvasElement>(null);
  const ledCanvasRef = useRef<HTMLCanvasElement>(null);
  const warningLightRef = useRef<HTMLCanvasElement>(null);

  const currentValue = realtimeData[config.channel] ?? value;

  useEffect(() => {
    setValue(currentValue);
  }, [currentValue]);

  // Blink timer for warning lights
  useEffect(() => {
    if (config.gauge_type === GaugeType.WarningLight && config.blink) {
      const interval = setInterval(() => {
        setBlinkState((prev) => !prev);
      }, 500);
      return () => clearInterval(interval);
    }
  }, [config.gauge_type, config.blink]);

  // Draw analog dial
  useEffect(() => {
    if (config.gauge_type === GaugeType.AnalogDial && canvasRef.current) {
      drawAnalogDial(canvasRef.current, currentValue);
    }
  }, [currentValue, config, config.gauge_type]);

  // Draw bar gauge
  useEffect(() => {
    if (config.gauge_type === GaugeType.BarGauge && barCanvasRef.current) {
      drawBarGauge(barCanvasRef.current, currentValue);
    }
  }, [currentValue, config, config.gauge_type]);

  // Draw sweep gauge
  useEffect(() => {
    if (config.gauge_type === GaugeType.SweepGauge && sweepCanvasRef.current) {
      drawSweepGauge(sweepCanvasRef.current, currentValue);
    }
  }, [currentValue, config, config.gauge_type]);

  // Draw LED gauge
  useEffect(() => {
    if (config.gauge_type === GaugeType.LEDIndicator && ledCanvasRef.current) {
      drawLEDIndicator(ledCanvasRef.current, currentValue);
    }
  }, [currentValue, config, config.gauge_type]);

  // Draw warning light
  useEffect(() => {
    if (config.gauge_type === GaugeType.WarningLight && warningLightRef.current) {
      drawWarningLight(warningLightRef.current, currentValue, blinkState);
    }
  }, [currentValue, config, config.gauge_type, blinkState]);

  const drawAnalogDial = (canvas: HTMLCanvasElement, value: number) => {
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;
    const centerX = width / 2;
    const centerY = height / 2;
    const radius = Math.min(width, height) / 2 - 20;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Draw TS-style gray metallic bezel
    const bezelWidth = 18;
    const outerRadius = radius + bezelWidth / 2;
    const innerRadius = radius - bezelWidth / 2;
    
    // Outer bezel ring with gradient
    const bezelGradient = ctx.createRadialGradient(
      centerX - radius * 0.3, centerY - radius * 0.3, 0,
      centerX, centerY, outerRadius
    );
    bezelGradient.addColorStop(0, '#a0a0a0');
    bezelGradient.addColorStop(0.3, '#808080');
    bezelGradient.addColorStop(0.6, '#606060');
    bezelGradient.addColorStop(1, '#404040');
    
    ctx.beginPath();
    ctx.arc(centerX, centerY, outerRadius, 0, 2 * Math.PI);
    ctx.arc(centerX, centerY, innerRadius, 0, 2 * Math.PI, true);
    ctx.fillStyle = bezelGradient;
    ctx.fill();
    
    // Inner bevel highlight
    ctx.beginPath();
    ctx.arc(centerX, centerY, innerRadius + 1, 0, 2 * Math.PI);
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.3)';
    ctx.lineWidth = 1;
    ctx.stroke();
    
    // Outer bevel shadow
    ctx.beginPath();
    ctx.arc(centerX, centerY, outerRadius - 1, 0, 2 * Math.PI);
    ctx.strokeStyle = 'rgba(0, 0, 0, 0.4)';
    ctx.lineWidth = 2;
    ctx.stroke();
    
    // Black inner face background
    ctx.beginPath();
    ctx.arc(centerX, centerY, innerRadius - 2, 0, 2 * Math.PI);
    ctx.fillStyle = '#1a1a1a';
    ctx.fill();

    // Draw tick marks and labels
    const range = config.max_value - config.min_value;
    const majorTicks = 10;
    const minorTicks = 50;
    const tickOuterRadius = innerRadius - 4;

    for (let i = 0; i <= minorTicks; i++) {
      const angle = (Math.PI * 0.75) + (i / minorTicks) * (Math.PI * 1.5);
      const isMajor = i % (minorTicks / majorTicks) === 0;
      const tickLength = isMajor ? 15 : 8;
      const startRadius = tickOuterRadius - tickLength;

      const x1 = centerX + Math.cos(angle) * startRadius;
      const y1 = centerY + Math.sin(angle) * startRadius;
      const x2 = centerX + Math.cos(angle) * tickOuterRadius;
      const y2 = centerY + Math.sin(angle) * tickOuterRadius;

      ctx.beginPath();
      ctx.moveTo(x1, y1);
      ctx.lineTo(x2, y2);
      ctx.strokeStyle = isMajor ? '#FFFFFF' : '#888888';
      ctx.lineWidth = isMajor ? 2 : 1;
      ctx.stroke();

      // Draw labels for major ticks
      if (isMajor) {
        const labelValue = config.min_value + (i / minorTicks) * range;
        const labelRadius = innerRadius - 35;
        const labelX = centerX + Math.cos(angle) * labelRadius;
        const labelY = centerY + Math.sin(angle) * labelRadius;

        ctx.fillStyle = config.font_color || '#FFFFFF';
        ctx.font = 'bold 20px sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(Math.round(labelValue).toString(), labelX, labelY);
      }
    }

    // Draw warning zones (TS style - thicker filled arcs)
    const zoneRadius = innerRadius - 8;
    const zoneWidth = 12;
    
    // Draw yellow warning zone
    if (config.high_warning !== undefined) {
      const warningStart = ((config.high_warning - config.min_value) / range) * (Math.PI * 1.5) + (Math.PI * 0.75);
      const warningEnd = config.high_critical !== undefined 
        ? ((config.high_critical - config.min_value) / range) * (Math.PI * 1.5) + (Math.PI * 0.75)
        : ((config.max_value - config.min_value) / range) * (Math.PI * 1.5) + (Math.PI * 0.75);
      
      ctx.beginPath();
      ctx.arc(centerX, centerY, zoneRadius, warningStart, warningEnd);
      ctx.strokeStyle = '#FFD700';
      ctx.lineWidth = zoneWidth;
      ctx.stroke();
    }

    // Draw red critical zone
    if (config.high_critical !== undefined) {
      const criticalStart = ((config.high_critical - config.min_value) / range) * (Math.PI * 1.5) + (Math.PI * 0.75);
      const criticalEnd = ((config.max_value - config.min_value) / range) * (Math.PI * 1.5) + (Math.PI * 0.75);
      
      ctx.beginPath();
      ctx.arc(centerX, centerY, zoneRadius, criticalStart, criticalEnd);
      ctx.strokeStyle = '#FF3333';
      ctx.lineWidth = zoneWidth;
      ctx.stroke();
    }
    
    // Draw low warning zone if defined
    if (config.low_warning !== undefined) {
      const lowWarningEnd = ((config.low_warning - config.min_value) / range) * (Math.PI * 1.5) + (Math.PI * 0.75);
      const lowWarningStart = (Math.PI * 0.75);
      
      ctx.beginPath();
      ctx.arc(centerX, centerY, zoneRadius, lowWarningStart, lowWarningEnd);
      ctx.strokeStyle = '#FFD700';
      ctx.lineWidth = zoneWidth;
      ctx.stroke();
    }

    // Calculate needle angle
    const normalizedValue = Math.max(config.min_value, Math.min(config.max_value, value));
    const percentage = (normalizedValue - config.min_value) / range;
    const needleAngle = (Math.PI * 0.75) + percentage * (Math.PI * 1.5);
    const needleLength = innerRadius - 30;

    // Draw needle shadow
    ctx.beginPath();
    ctx.moveTo(centerX + 2, centerY + 2);
    ctx.lineTo(
      centerX + Math.cos(needleAngle) * needleLength + 2,
      centerY + Math.sin(needleAngle) * needleLength + 2
    );
    ctx.strokeStyle = 'rgba(0, 0, 0, 0.5)';
    ctx.lineWidth = 4;
    ctx.stroke();

    // Draw needle
    ctx.beginPath();
    ctx.moveTo(centerX, centerY);
    ctx.lineTo(
      centerX + Math.cos(needleAngle) * needleLength,
      centerY + Math.sin(needleAngle) * needleLength
    );
    ctx.strokeStyle = config.needle_color || '#FF6600';
    ctx.lineWidth = 4;
    ctx.lineCap = 'round';
    ctx.stroke();

    // Draw center cap
    ctx.beginPath();
    ctx.arc(centerX, centerY, 10, 0, 2 * Math.PI);
    ctx.fillStyle = '#444444';
    ctx.fill();
    ctx.beginPath();
    ctx.arc(centerX, centerY, 6, 0, 2 * Math.PI);
    ctx.fillStyle = config.needle_color || '#FF6600';
    ctx.fill();

    // Draw value text at bottom
    ctx.fillStyle = config.font_color || '#FFFFFF';
    ctx.font = 'bold 36px sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(
      value.toFixed(config.decimals),
      centerX,
      centerY + innerRadius * 0.5
    );

    // Draw units
    ctx.font = 'bold 18px sans-serif';
    ctx.fillText(
      config.units || '',
      centerX,
      centerY + innerRadius * 0.5 + 25
    );
  };

  const renderAnalogDial = () => {
    return (
      <>
        <canvas 
          ref={canvasRef} 
          width={600} 
          height={600} 
          style={{ 
            width: '100%', 
            height: '100%',
            objectFit: 'contain',
            aspectRatio: '1 / 1'
          }} 
        />
      </>
    );
  };

  const renderDigitalReadout = () => {
    const displayValue = currentValue.toFixed(config.decimals);
    const displayMin = config.show_min_max ? config.min_value : currentValue;
    const displayMax = config.show_min_max ? config.max_value : currentValue;

    const isLowWarning = config.low_warning !== undefined && currentValue < config.low_warning;
    const isHighWarning = config.high_warning !== undefined && currentValue > config.high_warning;
    const isCritical = config.high_critical !== undefined && currentValue > config.high_critical;

    const valueColor = isCritical ? '#FF0000'
      : isHighWarning ? '#FF8C00'
        : isLowWarning ? '#FFA500'
          : config.font_color;

    return (
      <>
        <div className="digital-label">{config.label}</div>
        <div className="digital-value" style={{ color: valueColor }}>
          {displayValue}
          {config.units && <span className="digital-units">{config.units}</span>}
        </div>
        {config.show_min_max && (
          <div className="digital-min-max">
            <div>Min: {displayMin.toFixed(config.decimals)}</div>
            <div>Max: {displayMax.toFixed(config.decimals)}</div>
          </div>
        )}
      </>
    );
  };

  const drawBarGauge = (canvas: HTMLCanvasElement, value: number) => {
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;
    const isHorizontal = config.width > config.height;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Calculate percentage
    const range = config.max_value - config.min_value;
    const percentage = Math.max(0, Math.min(1, (value - config.min_value) / range));

    // Determine color based on warnings
    let barColor = '#4CAF50'; // Default green
    if (config.high_critical !== undefined && value >= config.high_critical) {
      barColor = '#FF0000';
    } else if (config.high_warning !== undefined && value >= config.high_warning) {
      barColor = '#FFA500';
    } else if (config.low_warning !== undefined && value <= config.low_warning) {
      barColor = '#FFD700';
    }

    const padding = 40;
    const barThickness = isHorizontal ? Math.min(60, height - padding * 2) : Math.min(60, width - padding * 2);

    if (isHorizontal) {
      const barWidth = width - padding * 2;
      const barHeight = barThickness;
      const barX = padding;
      const barY = (height - barHeight) / 2;

      // Draw background track
      ctx.fillStyle = '#2A2A2A';
      ctx.fillRect(barX, barY, barWidth, barHeight);
      ctx.strokeStyle = '#444';
      ctx.lineWidth = 2;
      ctx.strokeRect(barX, barY, barWidth, barHeight);

      // Draw filled portion
      const fillWidth = barWidth * percentage;
      ctx.fillStyle = barColor;
      ctx.fillRect(barX, barY, fillWidth, barHeight);

      // Draw tick marks
      const numTicks = 5;
      for (let i = 0; i <= numTicks; i++) {
        const tickX = barX + (barWidth * i / numTicks);
        ctx.strokeStyle = '#888';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(tickX, barY - 5);
        ctx.lineTo(tickX, barY + barHeight + 5);
        ctx.stroke();

        // Draw tick labels
        const tickValue = config.min_value + (range * i / numTicks);
        ctx.fillStyle = '#FFFFFF';
        ctx.font = 'bold 14px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(Math.round(tickValue).toString(), tickX, barY + barHeight + 25);
      }

      // Draw value text
      ctx.fillStyle = barColor;
      ctx.font = 'bold 24px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(
        `${value.toFixed(config.decimals)} ${config.units || ''}`,
        width / 2,
        barY - 15
      );
    } else {
      // Vertical bar
      const barWidth = barThickness;
      const barHeight = height - padding * 2;
      const barX = (width - barWidth) / 2;
      const barY = padding;

      // Draw background track
      ctx.fillStyle = '#2A2A2A';
      ctx.fillRect(barX, barY, barWidth, barHeight);
      ctx.strokeStyle = '#444';
      ctx.lineWidth = 2;
      ctx.strokeRect(barX, barY, barWidth, barHeight);

      // Draw filled portion (from bottom up)
      const fillHeight = barHeight * percentage;
      ctx.fillStyle = barColor;
      ctx.fillRect(barX, barY + barHeight - fillHeight, barWidth, fillHeight);

      // Draw tick marks
      const numTicks = 5;
      for (let i = 0; i <= numTicks; i++) {
        const tickY = barY + barHeight - (barHeight * i / numTicks);
        ctx.strokeStyle = '#888';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(barX - 5, tickY);
        ctx.lineTo(barX + barWidth + 5, tickY);
        ctx.stroke();

        // Draw tick labels
        const tickValue = config.min_value + (range * i / numTicks);
        ctx.fillStyle = '#FFFFFF';
        ctx.font = 'bold 14px sans-serif';
        ctx.textAlign = 'right';
        ctx.textBaseline = 'middle';
        ctx.fillText(Math.round(tickValue).toString(), barX - 10, tickY);
      }

      // Draw value text
      ctx.save();
      ctx.translate(barX + barWidth + 15, height / 2);
      ctx.rotate(-Math.PI / 2);
      ctx.fillStyle = barColor;
      ctx.font = 'bold 24px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(
        `${value.toFixed(config.decimals)} ${config.units || ''}`,
        0,
        0
      );
      ctx.restore();
    }
  };

  const renderBarGauge = () => {
    return (
      <canvas 
        ref={barCanvasRef} 
        width={600} 
        height={400}
        style={{ 
          width: '100%', 
          height: '100%',
          objectFit: 'contain'
        }} 
      />
    );
  };

  const drawSweepGauge = (canvas: HTMLCanvasElement, value: number) => {
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;
    const centerX = width / 2;
    const centerY = height * 0.68;
    const radius = Math.min(width, height) * 0.42;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Calculate percentage and angle
    const range = config.max_value - config.min_value;
    const percentage = Math.max(0, Math.min(1, (value - config.min_value) / range));
    const startAngle = Math.PI;
    const endAngle = 2 * Math.PI;
    const currentAngle = startAngle + percentage * (endAngle - startAngle);

    // Determine color based on warnings
    let sweepColor = '#4CAF50';
    if (config.high_critical !== undefined && value >= config.high_critical) {
      sweepColor = '#FF0000';
    } else if (config.high_warning !== undefined && value >= config.high_warning) {
      sweepColor = '#FFA500';
    }

    // Draw background arc
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius, startAngle, endAngle);
    ctx.strokeStyle = '#2A2A2A';
    ctx.lineWidth = 30;
    ctx.lineCap = 'round';
    ctx.stroke();

    // Draw filled arc
    if (percentage > 0) {
      ctx.beginPath();
      ctx.arc(centerX, centerY, radius, startAngle, currentAngle);
      ctx.strokeStyle = sweepColor;
      ctx.lineWidth = 30;
      ctx.lineCap = 'round';
      ctx.stroke();
    }

    // Draw tick marks
    const numTicks = 10;
    for (let i = 0; i <= numTicks; i++) {
      const angle = startAngle + (endAngle - startAngle) * (i / numTicks);
      const isMajor = i % 2 === 0;
      
      const innerRadius = radius + 20;
      const outerRadius = isMajor ? radius + 35 : radius + 28;
      
      const x1 = centerX + Math.cos(angle) * innerRadius;
      const y1 = centerY + Math.sin(angle) * innerRadius;
      const x2 = centerX + Math.cos(angle) * outerRadius;
      const y2 = centerY + Math.sin(angle) * outerRadius;

      ctx.beginPath();
      ctx.moveTo(x1, y1);
      ctx.lineTo(x2, y2);
      ctx.strokeStyle = isMajor ? '#FFFFFF' : '#888888';
      ctx.lineWidth = isMajor ? 2 : 1;
      ctx.stroke();

      // Draw labels for major ticks
      if (isMajor) {
        const labelValue = config.min_value + (i / numTicks) * range;
        const labelRadius = radius + 40;
        const labelX = centerX + Math.cos(angle) * labelRadius;
        const labelY = centerY + Math.sin(angle) * labelRadius;

        ctx.fillStyle = '#FFFFFF';
        ctx.font = 'bold 16px sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(Math.round(labelValue).toString(), labelX, labelY);
      }
    }

    // Draw value text in center
    ctx.fillStyle = sweepColor;
    ctx.font = 'bold 36px sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(value.toFixed(config.decimals), centerX, centerY - 10);

    // Draw units
    ctx.fillStyle = '#FFFFFF';
    ctx.font = 'bold 18px sans-serif';
    ctx.fillText(config.units || '', centerX, centerY + 25);

    // Draw label at top
    ctx.fillStyle = '#AAAAAA';
    ctx.font = 'bold 20px sans-serif';
    ctx.fillText(config.label || '', centerX, 25);
  };

  const renderSweepGauge = () => {
    return (
      <canvas 
        ref={sweepCanvasRef} 
        width={600} 
        height={500}
        style={{ 
          width: '100%', 
          height: '100%',
          objectFit: 'contain'
        }} 
      />
    );
  };

  const drawLEDIndicator = (canvas: HTMLCanvasElement, value: number) => {
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Draw label at top
    ctx.fillStyle = '#FFFFFF';
    ctx.font = 'bold 24px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText(config.label || '', width / 2, 40);

    // Calculate which LEDs should be lit based on value percentage
    const range = config.max_value - config.min_value;
    const percentage = Math.max(0, Math.min(1, (value - config.min_value) / range));
    const numLEDs = 10;
    const litLEDs = Math.round(percentage * numLEDs);

    const ledWidth = 40;
    const ledHeight = 40;
    const spacing = 10;
    const startY = 80;

    // Determine LED layout (vertical stack)
    for (let i = 0; i < numLEDs; i++) {
      const ledIndex = numLEDs - 1 - i; // Reverse so it fills from bottom
      const isLit = litLEDs > ledIndex;
      const y = startY + i * (ledHeight + spacing);
      const x = (width - ledWidth) / 2;

      // Determine color based on position
      let ledColor = '#444444'; // Off color
      if (isLit) {
        if (i < 3) {
          ledColor = '#4CAF50'; // Green (low range)
        } else if (i < 7) {
          ledColor = '#FFD700'; // Yellow (mid range)
        } else {
          ledColor = '#FF0000'; // Red (high range)
        }
      }

      // Draw LED with glow effect if lit
      if (isLit) {
        // Outer glow
        const gradient = ctx.createRadialGradient(x + ledWidth/2, y + ledHeight/2, 0, x + ledWidth/2, y + ledHeight/2, ledWidth);
        gradient.addColorStop(0, ledColor);
        gradient.addColorStop(0.5, ledColor + '88');
        gradient.addColorStop(1, ledColor + '00');
        ctx.fillStyle = gradient;
        ctx.fillRect(x - 10, y - 10, ledWidth + 20, ledHeight + 20);
      }

      // Draw LED rectangle
      ctx.fillStyle = ledColor;
      ctx.fillRect(x, y, ledWidth, ledHeight);
      
      // Draw border
      ctx.strokeStyle = isLit ? '#FFFFFF' : '#666666';
      ctx.lineWidth = 2;
      ctx.strokeRect(x, y, ledWidth, ledHeight);
    }

    // Draw value text at bottom
    ctx.fillStyle = '#FFFFFF';
    ctx.font = 'bold 32px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText(
      value.toFixed(config.decimals),
      width / 2,
      startY + numLEDs * (ledHeight + spacing) + 40
    );

    // Draw units
    ctx.font = 'bold 20px sans-serif';
    ctx.fillText(
      config.units || '',
      width / 2,
      startY + numLEDs * (ledHeight + spacing) + 75
    );
  };

  const renderLEDIndicator = () => {
    return (
      <canvas 
        ref={ledCanvasRef} 
        width={400} 
        height={700}
        style={{ 
          width: '100%', 
          height: '100%',
          objectFit: 'contain'
        }} 
      />
    );
  };

  // Evaluate simple condition expression
  const evaluateCondition = (condition: string, value: number): boolean => {
    // Simple condition parser: "channel < 12", "channel > 100", "channel == 1", etc.
    const normalized = condition.toLowerCase().trim();
    
    // Try to extract operator and threshold
    if (normalized.includes('<') && !normalized.includes('<=')) {
      const match = normalized.match(/<\s*([\d.]+)/);
      if (match) return value < parseFloat(match[1]);
    }
    if (normalized.includes('<=')) {
      const match = normalized.match(/<=\s*([\d.]+)/);
      if (match) return value <= parseFloat(match[1]);
    }
    if (normalized.includes('>') && !normalized.includes('>=')) {
      const match = normalized.match(/>\s*([\d.]+)/);
      if (match) return value > parseFloat(match[1]);
    }
    if (normalized.includes('>=')) {
      const match = normalized.match(/>=\s*([\d.]+)/);
      if (match) return value >= parseFloat(match[1]);
    }
    if (normalized.includes('!=')) {
      const match = normalized.match(/!=\s*([\d.]+)/);
      if (match) return value !== parseFloat(match[1]);
    }
    if (normalized.includes('==')) {
      const match = normalized.match(/==\s*([\d.]+)/);
      if (match) return value === parseFloat(match[1]);
    }
    
    // Default: treat non-zero as true
    return value !== 0;
  };

  const drawWarningLight = (canvas: HTMLCanvasElement, value: number, blink: boolean) => {
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Determine if light should be on
    let isOn = false;
    if (config.on_condition) {
      isOn = evaluateCondition(config.on_condition, value);
    } else {
      // Default: on if value != 0
      isOn = value !== 0;
    }

    // Handle blink
    if (config.blink && isOn && !blink) {
      isOn = false;
    }

    const onColor = config.on_color || '#FF0000';
    const offColor = config.off_color || '#333333';
    const currentColor = isOn ? onColor : offColor;

    const centerX = width / 2;
    const centerY = height / 2 - 10;
    const radius = Math.min(width, height) * 0.35;

    // Draw glow when on
    if (isOn) {
      const glow = ctx.createRadialGradient(centerX, centerY, 0, centerX, centerY, radius * 1.8);
      glow.addColorStop(0, onColor);
      glow.addColorStop(0.4, onColor + 'AA');
      glow.addColorStop(0.7, onColor + '44');
      glow.addColorStop(1, onColor + '00');
      ctx.fillStyle = glow;
      ctx.beginPath();
      ctx.arc(centerX, centerY, radius * 1.8, 0, Math.PI * 2);
      ctx.fill();
    }

    // Draw light bulb/indicator
    const gradient = ctx.createRadialGradient(
      centerX - radius * 0.3, 
      centerY - radius * 0.3, 
      0, 
      centerX, 
      centerY, 
      radius
    );
    gradient.addColorStop(0, isOn ? '#FFFFFF' : '#555555');
    gradient.addColorStop(0.3, currentColor);
    gradient.addColorStop(1, isOn ? currentColor : '#111111');

    ctx.beginPath();
    ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
    ctx.fillStyle = gradient;
    ctx.fill();

    // Draw border
    ctx.strokeStyle = isOn ? '#FFFFFF' : '#444444';
    ctx.lineWidth = 3;
    ctx.stroke();

    // Draw label below
    ctx.fillStyle = isOn ? '#FFFFFF' : '#888888';
    ctx.font = 'bold 20px sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'top';
    ctx.fillText(config.label || '', centerX, centerY + radius + 15);
  };

  const renderWarningLight = () => {
    return (
      <canvas 
        ref={warningLightRef} 
        width={200} 
        height={200}
        style={{ 
          width: '100%', 
          height: '100%',
          objectFit: 'contain'
        }} 
      />
    );
  };

  const handleConfigChange = (updates: Partial<GaugeConfig>) => {
    onConfigChange?.({ ...config, ...updates });
    };

  const getGaugeClassName = () => {
    switch (config.gauge_type) {
      case GaugeType.AnalogDial:
        return 'analog-dial';
      case GaugeType.DigitalReadout:
        return 'digital-readout';
      case GaugeType.BarGauge:
        return 'bar-gauge-wrapper';
      case GaugeType.SweepGauge:
        return 'sweep-gauge';
      case GaugeType.LEDIndicator:
        return 'led-indicator-wrapper';
      case GaugeType.WarningLight:
        return 'warning-light';
      default:
        return '';
    }
  };

  return (
    <div className={`gauge-wrapper ${getGaugeClassName()}`} style={{
      width: '100%',
      height: '100%',
      position: 'relative',
    }}>
      <div className="gauge-controls">
        <button className="gauge-control" onClick={() => handleConfigChange({ ...config, min_value: currentValue })}>
          <span>⚙</span>
        </button>
      </div>

      {config.gauge_type === GaugeType.AnalogDial && renderAnalogDial()}
      {config.gauge_type === GaugeType.DigitalReadout && renderDigitalReadout()}
      {config.gauge_type === GaugeType.BarGauge && renderBarGauge()}
      {config.gauge_type === GaugeType.SweepGauge && renderSweepGauge()}
      {config.gauge_type === GaugeType.LEDIndicator && renderLEDIndicator()}
      {config.gauge_type === GaugeType.WarningLight && renderWarningLight()}
    </div>
  );
}
