/** AnalogBarGauge — semicircular bar indicator with metallic bezel. */

import { tsColorToHex } from '../../dashboards/dashTypes';
import { lightenColor, darkenColor, createMetallicGradient } from '../drawUtils';
import type { Painter } from './types';

export const analogBarGaugePainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getValueColor, getFontSpec } = pctx;

  const centerX = width / 2;
  const centerY = height * 0.85;
  const radius = Math.min(width, height) * 0.75;
  const barWidth = radius * 0.15;

  // Angle range: 180° arc from left to right
  const startAngle = Math.PI;
  const endAngle = 0;
  const totalSweep = Math.PI;

  // Background arc with metallic bezel
  const bezelGradient = createMetallicGradient(ctx, centerX, centerY, 0, radius + barWidth / 2 + 8, config.trim_color);
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius + barWidth / 2 + 4, startAngle, endAngle, false);
  ctx.arc(centerX, centerY, radius - barWidth / 2 - 4, endAngle, startAngle, true);
  ctx.closePath();
  ctx.fillStyle = bezelGradient;
  ctx.fill();

  // Track background
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius, startAngle, endAngle, false);
  ctx.lineWidth = barWidth;
  ctx.strokeStyle = darkenColor(tsColorToHex(config.back_color), 30);
  ctx.lineCap = 'butt';
  ctx.stroke();

  // Calculate value angle
  const range = config.max - config.min;
  const normalizedValue = Math.max(0, Math.min(1, (value - config.min) / range));
  const valueAngle = startAngle - (normalizedValue * totalSweep);

  // Value bar with gradient
  if (normalizedValue > 0) {
    const valueColorTs = getValueColor();
    const valueColorHex = tsColorToHex(valueColorTs);
    const barGradient = ctx.createLinearGradient(0, centerY - radius, width, centerY);
    barGradient.addColorStop(0, darkenColor(valueColorHex, 20));
    barGradient.addColorStop(0.5, valueColorHex);
    barGradient.addColorStop(1, lightenColor(valueColorHex, 20));

    ctx.beginPath();
    ctx.arc(centerX, centerY, radius, startAngle, valueAngle, true);
    ctx.lineWidth = barWidth - 4;
    ctx.strokeStyle = barGradient;
    ctx.lineCap = 'round';
    ctx.stroke();
  }

  // Tick marks
  const tickCount = 10;
  ctx.strokeStyle = tsColorToHex(config.trim_color);
  ctx.lineWidth = 1;
  for (let i = 0; i <= tickCount; i++) {
    const tickAngle = startAngle - (i / tickCount) * totalSweep;
    const innerRadius = radius - barWidth / 2 - 8;
    const outerRadius = radius - barWidth / 2 - 15;
    const isMajor = i % 2 === 0;

    ctx.beginPath();
    ctx.moveTo(
      centerX + Math.cos(tickAngle) * (isMajor ? outerRadius : innerRadius + 3),
      centerY + Math.sin(tickAngle) * (isMajor ? outerRadius : innerRadius + 3),
    );
    ctx.lineTo(
      centerX + Math.cos(tickAngle) * innerRadius,
      centerY + Math.sin(tickAngle) * innerRadius,
    );
    ctx.stroke();
  }

  // Value text in center
  const valueColorTs = getValueColor();
  ctx.fillStyle = tsColorToHex(valueColorTs);
  const fontSize = Math.max(12, radius * 0.2);
  ctx.font = getFontSpec(fontSize, { bold: true, monospace: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(value.toFixed(config.value_digits), centerX, centerY - radius * 0.3);

  // Units below value
  if (config.units) {
    ctx.fillStyle = tsColorToHex(config.font_color);
    ctx.font = getFontSpec(fontSize * 0.5);
    ctx.fillText(config.units, centerX, centerY - radius * 0.1);
  }

  // Title at top
  if (config.title) {
    ctx.fillStyle = tsColorToHex(config.font_color);
    ctx.font = getFontSpec(fontSize * 0.5);
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, centerX, 4);
  }
};
