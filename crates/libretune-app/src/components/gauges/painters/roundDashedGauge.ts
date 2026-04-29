/** RoundDashedGauge — circular gauge with segmented arc (~270°). */

import { tsColorToHex, tsColorToRgba } from '../../dashboards/dashTypes';
import { lightenColor, createMetallicGradient } from '../drawUtils';
import type { Painter } from './types';

export const roundDashedGaugePainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getValueColor, getFontSpec } = pctx;

  const padding = Math.min(width, height) * 0.08;
  const centerX = width / 2;
  const centerY = height / 2;
  const radius = Math.min(width, height) / 2 - padding;

  // Metallic outer bezel
  ctx.shadowColor = 'rgba(0,0,0,0.4)';
  ctx.shadowBlur = 6;
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
  const bezelGradient = createMetallicGradient(ctx, centerX, centerY, 0, radius, config.trim_color);
  ctx.fillStyle = bezelGradient;
  ctx.fill();
  ctx.shadowColor = 'transparent';

  // Inner background
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius * 0.9, 0, Math.PI * 2);
  ctx.fillStyle = tsColorToRgba(config.back_color);
  ctx.fill();

  // Draw dashed segments around 270 degrees (like a speedometer)
  const startAngle = Math.PI * 0.75; // 135 degrees
  const endAngle = Math.PI * 2.25; // 405 degrees
  const totalSweep = endAngle - startAngle;
  const segments = 30;
  const segmentWidth = radius * 0.08;
  const innerRadius = radius * 0.65;
  const outerRadius = radius * 0.85;

  for (let i = 0; i < segments; i++) {
    const angle = startAngle + (i / (segments - 1)) * totalSweep;
    const segmentValue = config.min + (i / (segments - 1)) * (config.max - config.min);

    const x1 = centerX + Math.cos(angle) * innerRadius;
    const y1 = centerY + Math.sin(angle) * innerRadius;
    const x2 = centerX + Math.cos(angle) * outerRadius;
    const y2 = centerY + Math.sin(angle) * outerRadius;

    // Determine color
    let segmentColor = tsColorToHex(config.trim_color);
    if (segmentValue >= config.max - (config.max - config.min) * 0.1) {
      segmentColor = tsColorToHex(config.critical_color);
    } else if (segmentValue >= config.max - (config.max - config.min) * 0.25) {
      segmentColor = tsColorToHex(config.warn_color);
    }

    // Draw segment
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
    ctx.lineWidth = segmentWidth;
    ctx.lineCap = 'round';

    if (segmentValue <= value) {
      ctx.strokeStyle = segmentColor;
      ctx.shadowColor = segmentColor;
      ctx.shadowBlur = 4;
    } else {
      ctx.strokeStyle = lightenColor(segmentColor, -60);
      ctx.shadowColor = 'transparent';
    }
    ctx.stroke();
    ctx.shadowColor = 'transparent';
  }

  // Value in center
  const valueTextColorTs = getValueColor();
  const fontSize = Math.max(12, radius * 0.28);
  ctx.fillStyle = tsColorToHex(valueTextColorTs);
  ctx.font = getFontSpec(fontSize, { bold: true, monospace: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(value.toFixed(config.value_digits), centerX, centerY);

  // Units
  if (config.units) {
    ctx.fillStyle = tsColorToHex(config.font_color);
    ctx.font = getFontSpec(fontSize * 0.45);
    ctx.fillText(config.units, centerX, centerY + fontSize * 0.7);
  }

  // Title
  if (config.title) {
    ctx.fillStyle = tsColorToHex(config.font_color);
    ctx.font = getFontSpec(fontSize * 0.35);
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, centerX, 4);
  }
};
