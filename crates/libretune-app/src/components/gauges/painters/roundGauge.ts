/** RoundGauge — 360° circular gauge with segments. */

import { tsColorToHex, tsColorToRgba } from '../../dashboards/dashTypes';
import { lightenColor, createMetallicGradient } from '../drawUtils';
import type { Painter } from './types';

export const roundGaugePainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getValueColor, getFontSpec } = pctx;

  const padding = Math.min(width, height) * 0.08;
  const centerX = width / 2;
  const centerY = height / 2;
  const radius = Math.min(width, height) / 2 - padding;

  // Metallic outer ring
  const ringWidth = radius * 0.12;
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
  ctx.arc(centerX, centerY, radius - ringWidth, 0, Math.PI * 2, true);
  ctx.closePath();
  const ringGradient = createMetallicGradient(ctx, centerX, centerY, 0, radius, config.trim_color);
  ctx.fillStyle = ringGradient;
  ctx.fill();

  // Inner background
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius - ringWidth, 0, Math.PI * 2);
  ctx.fillStyle = tsColorToRgba(config.back_color);
  ctx.fill();

  // Draw segments around the full 360 degrees
  const innerRadius = radius * 0.55;
  const outerRadius = radius * 0.85;
  const segments = 60;
  const gapAngle = Math.PI / 180; // 1 degree gap

  for (let i = 0; i < segments; i++) {
    const startAngle = (i / segments) * Math.PI * 2 - Math.PI / 2;
    const endAngle = ((i + 1) / segments) * Math.PI * 2 - Math.PI / 2 - gapAngle;
    const segmentValue = config.min + (i / segments) * (config.max - config.min);

    ctx.beginPath();
    ctx.arc(centerX, centerY, outerRadius, startAngle, endAngle);
    ctx.arc(centerX, centerY, innerRadius, endAngle, startAngle, true);
    ctx.closePath();

    // Color based on value and warning zones
    let segmentColor = tsColorToHex(config.trim_color);
    if (segmentValue >= config.max - (config.max - config.min) * 0.1) {
      segmentColor = tsColorToHex(config.critical_color);
    } else if (segmentValue >= config.max - (config.max - config.min) * 0.25) {
      segmentColor = tsColorToHex(config.warn_color);
    }

    // Dim segments beyond current value
    if (segmentValue > value) {
      ctx.fillStyle = lightenColor(segmentColor, -60);
    } else {
      ctx.fillStyle = segmentColor;
    }
    ctx.fill();
  }

  // Value display in center
  const valueTextColorTs = getValueColor();
  const fontSize = Math.max(12, radius * 0.25);
  ctx.fillStyle = tsColorToHex(valueTextColorTs);
  ctx.font = getFontSpec(fontSize, { bold: true, monospace: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(value.toFixed(config.value_digits), centerX, centerY);

  // Units below value
  if (config.units) {
    ctx.fillStyle = tsColorToHex(config.font_color);
    ctx.font = getFontSpec(fontSize * 0.5);
    ctx.fillText(config.units, centerX, centerY + fontSize * 0.6);
  }

  // Title at top
  if (config.title) {
    ctx.fillStyle = tsColorToHex(config.font_color);
    ctx.font = getFontSpec(fontSize * 0.4);
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, centerX, 4);
  }
};
