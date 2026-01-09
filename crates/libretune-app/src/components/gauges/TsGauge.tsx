/**
 * TunerStudio Gauge Renderer
 * 
 * Renders gauges based on TunerStudio's GaugePainter types.
 * Uses canvas for all gauge rendering.
 */

import { useEffect, useRef } from 'react';
import { TsGaugeConfig, TsColor, tsColorToRgba } from '../dashboards/dashTypes';

interface TsGaugeProps {
  config: TsGaugeConfig;
  value: number;
  embeddedImages?: Map<string, string>;
}

export default function TsGauge({ config, value, embeddedImages }: TsGaugeProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Clamp value to min/max if peg_limits is true
  const clampedValue = config.peg_limits 
    ? Math.max(config.min, Math.min(config.max, value))
    : value;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    // Set canvas size to match container
    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    
    // Reset transform before scaling to prevent accumulation
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.scale(dpr, dpr);

    // Clear canvas
    ctx.clearRect(0, 0, rect.width, rect.height);

    // Render based on gauge painter type
    switch (config.gauge_painter) {
      case 'BasicReadout':
        drawBasicReadout(ctx, rect.width, rect.height);
        break;
      case 'HorizontalBarGauge':
        drawHorizontalBar(ctx, rect.width, rect.height);
        break;
      case 'VerticalBarGauge':
        drawVerticalBar(ctx, rect.width, rect.height);
        break;
      case 'AnalogGauge':
      case 'BasicAnalogGauge':
      case 'CircleAnalogGauge':
        drawAnalogGauge(ctx, rect.width, rect.height);
        break;
      case 'AsymmetricSweepGauge':
        drawSweepGauge(ctx, rect.width, rect.height);
        break;
      case 'HorizontalLineGauge':
        drawHorizontalLine(ctx, rect.width, rect.height);
        break;
      case 'VerticalDashedBar':
        drawVerticalDashedBar(ctx, rect.width, rect.height);
        break;
      default:
        // Fallback to basic readout for unimplemented types
        drawBasicReadout(ctx, rect.width, rect.height);
    }
  }, [config, clampedValue, value, embeddedImages]);

  /** Get color based on value thresholds */
  const getValueColor = (): TsColor => {
    // Use != null to catch both null and undefined
    if (config.high_critical != null && clampedValue >= config.high_critical) {
      return config.critical_color;
    }
    if (config.low_critical != null && clampedValue <= config.low_critical) {
      return config.critical_color;
    }
    if (config.high_warning != null && clampedValue >= config.high_warning) {
      return config.warn_color;
    }
    if (config.low_warning != null && clampedValue <= config.low_warning) {
      return config.warn_color;
    }
    return config.font_color;
  };

  /** Draw digital readout (LCD style) */
  const drawBasicReadout = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = 4;
    const innerHeight = height - padding * 2;

    // Background
    ctx.fillStyle = tsColorToRgba(config.back_color);
    ctx.fillRect(0, 0, width, height);

    // Border
    ctx.strokeStyle = tsColorToRgba(config.trim_color);
    ctx.lineWidth = config.border_width;
    ctx.strokeRect(1, 1, width - 2, height - 2);

    // Title at top
    const titleHeight = innerHeight * 0.25;
    ctx.fillStyle = tsColorToRgba(config.trim_color);
    ctx.font = `${Math.max(10, titleHeight * 0.8)}px ${config.font_family || 'sans-serif'}`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, width / 2, padding);

    // Value in center (large)
    const valueColor = getValueColor();
    const valueHeight = innerHeight * 0.5;
    const valueText = clampedValue.toFixed(config.value_digits);
    ctx.fillStyle = tsColorToRgba(valueColor);
    ctx.font = `bold ${Math.max(12, valueHeight * 0.9)}px ${config.font_family || 'monospace'}`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(valueText, width / 2, height / 2);

    // Units at bottom
    const unitsHeight = innerHeight * 0.2;
    ctx.fillStyle = tsColorToRgba(config.trim_color);
    ctx.font = `${Math.max(8, unitsHeight * 0.8)}px ${config.font_family || 'sans-serif'}`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'bottom';
    ctx.fillText(config.units, width / 2, height - padding);
  };

  /** Draw horizontal bar gauge */
  const drawHorizontalBar = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = 4;
    const barHeight = height * 0.4;
    const barY = (height - barHeight) / 2 + height * 0.1;
    const barWidth = width - padding * 2;

    // Background
    ctx.fillStyle = tsColorToRgba(config.back_color);
    ctx.fillRect(0, 0, width, height);

    // Title
    ctx.fillStyle = tsColorToRgba(config.trim_color);
    ctx.font = `${Math.max(8, height * 0.15)}px sans-serif`;
    ctx.textAlign = 'left';
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, padding, 2);

    // Bar background
    ctx.fillStyle = 'rgba(50, 50, 50, 0.8)';
    ctx.fillRect(padding, barY, barWidth, barHeight);

    // Bar fill
    const fillPercent = (clampedValue - config.min) / (config.max - config.min);
    const fillWidth = barWidth * Math.max(0, Math.min(1, fillPercent));
    const valueColor = getValueColor();
    ctx.fillStyle = tsColorToRgba(valueColor);
    ctx.fillRect(padding, barY, fillWidth, barHeight);

    // Bar border
    ctx.strokeStyle = tsColorToRgba(config.trim_color);
    ctx.lineWidth = 1;
    ctx.strokeRect(padding, barY, barWidth, barHeight);

    // Value text
    ctx.fillStyle = tsColorToRgba(config.font_color);
    ctx.font = `bold ${Math.max(10, height * 0.2)}px monospace`;
    ctx.textAlign = 'right';
    ctx.textBaseline = 'top';
    ctx.fillText(`${clampedValue.toFixed(config.value_digits)} ${config.units}`, width - padding, 2);
  };

  /** Draw vertical bar gauge */
  const drawVerticalBar = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = 4;
    const labelHeight = height * 0.15;
    const barWidth = width * 0.5;
    const barHeight = height - labelHeight * 2 - padding * 2;
    const barX = (width - barWidth) / 2;
    const barY = labelHeight + padding;

    // Background
    ctx.fillStyle = tsColorToRgba(config.back_color);
    ctx.fillRect(0, 0, width, height);

    // Title at top
    ctx.fillStyle = tsColorToRgba(config.trim_color);
    ctx.font = `${Math.max(8, labelHeight * 0.7)}px sans-serif`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, width / 2, 2);

    // Bar background
    ctx.fillStyle = 'rgba(50, 50, 50, 0.8)';
    ctx.fillRect(barX, barY, barWidth, barHeight);

    // Bar fill (from bottom)
    const fillPercent = (clampedValue - config.min) / (config.max - config.min);
    const fillHeight = barHeight * Math.max(0, Math.min(1, fillPercent));
    const valueColor = getValueColor();
    ctx.fillStyle = tsColorToRgba(valueColor);
    ctx.fillRect(barX, barY + barHeight - fillHeight, barWidth, fillHeight);

    // Bar border
    ctx.strokeStyle = tsColorToRgba(config.trim_color);
    ctx.lineWidth = 1;
    ctx.strokeRect(barX, barY, barWidth, barHeight);

    // Value at bottom
    ctx.fillStyle = tsColorToRgba(config.font_color);
    ctx.font = `bold ${Math.max(10, labelHeight * 0.8)}px monospace`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'bottom';
    ctx.fillText(`${clampedValue.toFixed(config.value_digits)}`, width / 2, height - 2);
  };

  /** Draw analog dial gauge */
  const drawAnalogGauge = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const centerX = width / 2;
    const centerY = height / 2;
    const radius = Math.min(width, height) / 2 - 10;

    // Background
    ctx.fillStyle = tsColorToRgba(config.back_color);
    ctx.fillRect(0, 0, width, height);

    // Bezel
    const bezelWidth = 8;
    const gradient = ctx.createRadialGradient(
      centerX - radius * 0.2, centerY - radius * 0.2, 0,
      centerX, centerY, radius + bezelWidth
    );
    gradient.addColorStop(0, '#888888');
    gradient.addColorStop(0.5, '#555555');
    gradient.addColorStop(1, '#333333');
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius + bezelWidth, 0, Math.PI * 2);
    ctx.arc(centerX, centerY, radius - 2, 0, Math.PI * 2, true);
    ctx.fillStyle = gradient;
    ctx.fill();

    // Face background
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius - 2, 0, Math.PI * 2);
    ctx.fillStyle = tsColorToRgba(config.back_color);
    ctx.fill();

    // Calculate angles (TunerStudio uses degrees, canvas uses radians)
    // Use defaults if values are 0 or undefined (typical analog gauge: 225° start, 270° sweep)
    const startDeg = config.start_angle || 225;
    const sweepDeg = config.sweep_angle || 270;
    const startAngle = (startDeg - 90) * Math.PI / 180;
    const sweepAngle = sweepDeg * Math.PI / 180;
    const endAngle = startAngle + sweepAngle;

    // Draw tick marks - use proportional offsets for better scaling
    const tickRadius = radius - (radius * 0.15);
    const majorTicks = config.major_ticks > 0 ? config.major_ticks : (config.max - config.min) / 10;
    const numMajorTicks = Math.floor((config.max - config.min) / majorTicks) + 1;

    // Calculate if we need to cull labels for small gauges
    const SMALL_GAUGE_THRESHOLD = 80;
    const cullLabels = radius < SMALL_GAUGE_THRESHOLD;

    ctx.strokeStyle = tsColorToRgba(config.trim_color);
    ctx.fillStyle = tsColorToRgba(config.font_color);
    ctx.font = `${Math.max(7, radius * 0.11)}px sans-serif`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';

    for (let i = 0; i < numMajorTicks; i++) {
      const tickValue = config.min + i * majorTicks;
      const tickPercent = (tickValue - config.min) / (config.max - config.min);
      const tickAngle = startAngle + tickPercent * sweepAngle;

      // Major tick line - proportional inner radius
      const innerRadius = tickRadius - (radius * 0.10);
      ctx.beginPath();
      ctx.moveTo(
        centerX + Math.cos(tickAngle) * innerRadius,
        centerY + Math.sin(tickAngle) * innerRadius
      );
      ctx.lineTo(
        centerX + Math.cos(tickAngle) * tickRadius,
        centerY + Math.sin(tickAngle) * tickRadius
      );
      ctx.lineWidth = 2;
      ctx.stroke();

      // Tick label - proportional offset, with culling for small gauges
      // Only draw first and last labels on small gauges
      const shouldDrawLabel = !cullLabels || (i === 0 || i === numMajorTicks - 1);
      if (shouldDrawLabel) {
        const labelRadius = tickRadius - (radius * 0.18);
        ctx.fillText(
          tickValue.toFixed(config.label_digits),
          centerX + Math.cos(tickAngle) * labelRadius,
          centerY + Math.sin(tickAngle) * labelRadius
        );
      }
    }

    // Draw warning/critical zones as arcs
    if (config.high_warning !== null) {
      const warnStartPercent = (config.high_warning - config.min) / (config.max - config.min);
      const warnStartAngle = startAngle + warnStartPercent * sweepAngle;
      ctx.beginPath();
      ctx.arc(centerX, centerY, radius - 5, warnStartAngle, endAngle);
      ctx.strokeStyle = tsColorToRgba(config.warn_color);
      ctx.lineWidth = 4;
      ctx.stroke();
    }

    if (config.high_critical !== null) {
      const critStartPercent = (config.high_critical - config.min) / (config.max - config.min);
      const critStartAngle = startAngle + critStartPercent * sweepAngle;
      ctx.beginPath();
      ctx.arc(centerX, centerY, radius - 5, critStartAngle, endAngle);
      ctx.strokeStyle = tsColorToRgba(config.critical_color);
      ctx.lineWidth = 4;
      ctx.stroke();
    }

    // Draw needle
    const valuePercent = (clampedValue - config.min) / (config.max - config.min);
    const needleAngle = startAngle + valuePercent * sweepAngle;
    const needleLength = radius - 25;

    ctx.save();
    ctx.translate(centerX, centerY);
    ctx.rotate(needleAngle);

    // Needle shadow
    ctx.beginPath();
    ctx.moveTo(-5, 3);
    ctx.lineTo(needleLength, 3);
    ctx.lineTo(needleLength, 5);
    ctx.lineTo(-5, 5);
    ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
    ctx.fill();

    // Needle body
    ctx.beginPath();
    ctx.moveTo(-8, 0);
    ctx.lineTo(needleLength, -2);
    ctx.lineTo(needleLength, 2);
    ctx.lineTo(-8, 0);
    ctx.fillStyle = tsColorToRgba(config.needle_color);
    ctx.fill();

    // Needle center cap
    ctx.beginPath();
    ctx.arc(0, 0, 10, 0, Math.PI * 2);
    ctx.fillStyle = tsColorToRgba(config.trim_color);
    ctx.fill();

    ctx.restore();

    // Title
    ctx.fillStyle = tsColorToRgba(config.font_color);
    ctx.font = `${Math.max(10, radius * 0.15)}px sans-serif`;
    ctx.textAlign = 'center';
    ctx.fillText(config.title, centerX, centerY + radius * 0.4);

    // Value
    ctx.font = `bold ${Math.max(12, radius * 0.2)}px monospace`;
    ctx.fillText(`${clampedValue.toFixed(config.value_digits)} ${config.units}`, centerX, centerY + radius * 0.6);
  };

  /** Draw sweep gauge (tachometer style) */
  const drawSweepGauge = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const centerX = width / 2;
    const centerY = height * 0.6;
    const radius = Math.min(width, height * 1.2) / 2 - 10;

    // Background
    ctx.fillStyle = tsColorToRgba(config.back_color);
    ctx.fillRect(0, 0, width, height);

    // Calculate angles
    const startAngle = (config.start_angle - 90) * Math.PI / 180;
    const sweepAngle = config.sweep_angle * Math.PI / 180;

    // Draw arc background
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius, startAngle, startAngle + sweepAngle);
    ctx.strokeStyle = 'rgba(80, 80, 80, 0.8)';
    ctx.lineWidth = 20;
    ctx.stroke();

    // Draw filled arc
    const valuePercent = (clampedValue - config.min) / (config.max - config.min);
    const valueAngle = startAngle + valuePercent * sweepAngle;
    const valueColor = getValueColor();

    ctx.beginPath();
    ctx.arc(centerX, centerY, radius, startAngle, valueAngle);
    ctx.strokeStyle = tsColorToRgba(valueColor);
    ctx.lineWidth = 20;
    ctx.stroke();

    // Value in center
    ctx.fillStyle = tsColorToRgba(config.font_color);
    ctx.font = `bold ${Math.max(16, radius * 0.3)}px monospace`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(clampedValue.toFixed(config.value_digits), centerX, centerY);

    // Title below
    ctx.font = `${Math.max(10, radius * 0.12)}px sans-serif`;
    ctx.fillText(config.title, centerX, centerY + radius * 0.4);

    // Units
    ctx.fillStyle = tsColorToRgba(config.trim_color);
    ctx.font = `${Math.max(8, radius * 0.1)}px sans-serif`;
    ctx.fillText(config.units, centerX, centerY + radius * 0.55);
  };

  /** Draw horizontal line gauge */
  const drawHorizontalLine = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = 4;
    const lineY = height / 2;
    const lineWidth = width - padding * 2;

    // Background
    ctx.fillStyle = tsColorToRgba(config.back_color);
    ctx.fillRect(0, 0, width, height);

    // Line background
    ctx.strokeStyle = 'rgba(80, 80, 80, 0.8)';
    ctx.lineWidth = 3;
    ctx.beginPath();
    ctx.moveTo(padding, lineY);
    ctx.lineTo(padding + lineWidth, lineY);
    ctx.stroke();

    // Position indicator
    const fillPercent = (clampedValue - config.min) / (config.max - config.min);
    const indicatorX = padding + lineWidth * Math.max(0, Math.min(1, fillPercent));
    const valueColor = getValueColor();

    ctx.fillStyle = tsColorToRgba(valueColor);
    ctx.beginPath();
    ctx.arc(indicatorX, lineY, 6, 0, Math.PI * 2);
    ctx.fill();

    // Value
    ctx.fillStyle = tsColorToRgba(config.font_color);
    ctx.font = `bold ${Math.max(10, height * 0.25)}px monospace`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'bottom';
    ctx.fillText(`${clampedValue.toFixed(config.value_digits)}`, width / 2, lineY - 8);
  };

  /** Draw vertical dashed bar gauge */
  const drawVerticalDashedBar = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = 4;
    const barWidth = width * 0.6;
    const barHeight = height - padding * 2;
    const barX = (width - barWidth) / 2;
    const numSegments = 10;
    const segmentHeight = barHeight / numSegments;
    const segmentGap = 2;

    // Background
    ctx.fillStyle = tsColorToRgba(config.back_color);
    ctx.fillRect(0, 0, width, height);

    // Draw segments
    const fillPercent = (clampedValue - config.min) / (config.max - config.min);
    const filledSegments = Math.floor(fillPercent * numSegments);
    const valueColor = getValueColor();

    for (let i = 0; i < numSegments; i++) {
      const segmentY = padding + (numSegments - 1 - i) * segmentHeight;
      const isFilled = i < filledSegments;

      ctx.fillStyle = isFilled ? tsColorToRgba(valueColor) : 'rgba(50, 50, 50, 0.5)';
      ctx.fillRect(barX, segmentY + segmentGap / 2, barWidth, segmentHeight - segmentGap);
    }

    // Border
    ctx.strokeStyle = tsColorToRgba(config.trim_color);
    ctx.lineWidth = 1;
    ctx.strokeRect(barX, padding, barWidth, barHeight);
  };

  return (
    <canvas
      ref={canvasRef}
      style={{
        width: '100%',
        height: '100%',
        display: 'block',
      }}
    />
  );
}
