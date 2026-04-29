/** AnalogMovingBarGauge — sweeping needle with bar trail across a 140° arc. */

import { tsColorToHex } from '../../dashboards/dashTypes';
import { lightenColor, darkenColor, createMetallicGradient } from '../drawUtils';
import type { Painter } from './types';

export const analogMovingBarGaugePainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getValueColor, getFontSpec } = pctx;

  const centerX = width / 2;
  const centerY = height * 0.9;
  const radius = Math.min(width, height) * 0.8;
  const barWidth = radius * 0.08;

  // Angle range: 140° arc
  const startAngle = Math.PI + Math.PI * 0.2;
  const endAngle = -Math.PI * 0.2;
  const totalSweep = startAngle - endAngle;

  // Metallic bezel
  const bezelWidth = 6;
  const bezelGradient = createMetallicGradient(ctx, centerX, centerY, 0, radius + bezelWidth * 2, config.trim_color);
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius + bezelWidth, startAngle, endAngle, false);
  ctx.lineWidth = bezelWidth * 2;
  ctx.strokeStyle = bezelGradient;
  ctx.stroke();

  // Track background with inset shadow
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius - barWidth, startAngle, endAngle, false);
  ctx.arc(centerX, centerY, radius - barWidth * 3, endAngle, startAngle, true);
  ctx.closePath();
  ctx.fillStyle = darkenColor(tsColorToHex(config.back_color), 40);
  ctx.fill();

  // Warning/danger zones
  const warnStart = config.high_warning ?? (config.min + (config.max - config.min) * 0.7);
  const dangerStart = config.high_critical ?? (config.min + (config.max - config.min) * 0.9);
  const range = config.max - config.min;

  // Draw zone arcs
  const drawZone = (startVal: number, endVal: number, color: string) => {
    const s = Math.max(0, Math.min(1, (startVal - config.min) / range));
    const e = Math.max(0, Math.min(1, (endVal - config.min) / range));
    const sAngle = startAngle - s * totalSweep;
    const eAngle = startAngle - e * totalSweep;

    ctx.beginPath();
    ctx.arc(centerX, centerY, radius - barWidth * 2, sAngle, eAngle, true);
    ctx.lineWidth = barWidth;
    ctx.strokeStyle = color;
    ctx.lineCap = 'butt';
    ctx.stroke();
  };

  drawZone(config.min, warnStart, tsColorToHex(config.font_color));
  drawZone(warnStart, dangerStart, tsColorToHex(config.warn_color));
  drawZone(dangerStart, config.max, tsColorToHex(config.critical_color));

  // Calculate value angle
  const normalizedValue = Math.max(0, Math.min(1, (value - config.min) / range));
  const valueAngle = startAngle - normalizedValue * totalSweep;

  // Moving bar (filled from start to current value)
  const barColorTs = getValueColor();
  const barColorHex = tsColorToHex(barColorTs);
  const barGradient = ctx.createRadialGradient(centerX, centerY, radius * 0.5, centerX, centerY, radius);
  barGradient.addColorStop(0, lightenColor(barColorHex, 30));
  barGradient.addColorStop(1, barColorHex);

  ctx.beginPath();
  ctx.arc(centerX, centerY, radius - barWidth * 0.5, startAngle, valueAngle, true);
  ctx.lineWidth = barWidth * 0.8;
  ctx.strokeStyle = barGradient;
  ctx.lineCap = 'round';
  ctx.stroke();

  // Needle at current position
  const needleLength = radius * 0.85;
  const needleWidth = 4;

  ctx.save();
  ctx.translate(centerX, centerY);
  ctx.rotate(valueAngle);

  // Needle shadow
  ctx.shadowColor = 'rgba(0,0,0,0.5)';
  ctx.shadowBlur = 6;
  ctx.shadowOffsetY = 2;

  // Needle body
  ctx.beginPath();
  ctx.moveTo(needleLength, 0);
  ctx.lineTo(0, -needleWidth);
  ctx.lineTo(-needleLength * 0.1, 0);
  ctx.lineTo(0, needleWidth);
  ctx.closePath();

  const needleGradient = ctx.createLinearGradient(0, -needleWidth, 0, needleWidth);
  needleGradient.addColorStop(0, '#ff4444');
  needleGradient.addColorStop(0.5, '#ff0000');
  needleGradient.addColorStop(1, '#aa0000');
  ctx.fillStyle = needleGradient;
  ctx.fill();

  ctx.restore();

  // Center cap
  ctx.shadowColor = 'rgba(0,0,0,0.3)';
  ctx.shadowBlur = 4;
  const capGradient = ctx.createRadialGradient(centerX - 2, centerY - 2, 0, centerX, centerY, 12);
  capGradient.addColorStop(0, '#666666');
  capGradient.addColorStop(0.5, '#444444');
  capGradient.addColorStop(1, '#222222');
  ctx.beginPath();
  ctx.arc(centerX, centerY, 10, 0, Math.PI * 2);
  ctx.fillStyle = capGradient;
  ctx.fill();
  ctx.shadowColor = 'transparent';

  // Value text
  const valueTextColorTs = getValueColor();
  ctx.fillStyle = tsColorToHex(valueTextColorTs);
  const fontSize = Math.max(14, radius * 0.18);
  ctx.font = getFontSpec(fontSize, { bold: true, monospace: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(value.toFixed(config.value_digits), centerX, centerY - radius * 0.35);

  // Units
  if (config.units) {
    ctx.fillStyle = tsColorToHex(config.font_color);
    ctx.font = getFontSpec(fontSize * 0.6);
    ctx.fillText(config.units, centerX, centerY - radius * 0.18);
  }

  // Title
  if (config.title) {
    ctx.fillStyle = tsColorToHex(config.font_color);
    ctx.font = getFontSpec(fontSize * 0.5);
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, centerX, 4);
  }
};
