/** Tachometer — specialized RPM gauge with redline zone and chrome bezel. */

import { tsColorToHex } from '../../dashboards/dashTypes';
import { createMetallicGradient } from '../drawUtils';
import type { Painter } from './types';

export const tachometerPainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getValueColor, getFontSpec } = pctx;

  const padding = Math.min(width, height) * 0.08;
  const centerX = width / 2;
  const centerY = height / 2;
  const radius = Math.min(width, height) / 2 - padding;

  // Outer chrome bezel
  ctx.shadowColor = 'rgba(0,0,0,0.5)';
  ctx.shadowBlur = 8;

  // Double ring effect
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
  const outerBezel = createMetallicGradient(ctx, centerX, centerY, 0, radius, config.trim_color);
  ctx.fillStyle = outerBezel;
  ctx.fill();

  ctx.beginPath();
  ctx.arc(centerX, centerY, radius * 0.93, 0, Math.PI * 2);
  ctx.fillStyle = '#1a1a1a';
  ctx.fill();
  ctx.shadowColor = 'transparent';

  // Tachometer typically spans 270 degrees
  const startAngle = Math.PI * 0.75;
  const totalSweep = Math.PI * 1.5;

  // Draw major tick marks with numbers
  const tickInnerRadius = radius * 0.72;
  const tickOuterRadius = radius * 0.85;
  const majorTicks = Math.ceil(config.max / 1000); // One tick per 1000 RPM

  ctx.strokeStyle = '#ffffff';
  ctx.fillStyle = '#ffffff';
  ctx.lineWidth = 2;

  for (let i = 0; i <= majorTicks; i++) {
    const tickValue = i * 1000;
    if (tickValue > config.max) continue;

    const normalizedTick = (tickValue - config.min) / (config.max - config.min);
    const angle = startAngle + totalSweep * normalizedTick;

    const x1 = centerX + Math.cos(angle) * tickInnerRadius;
    const y1 = centerY + Math.sin(angle) * tickInnerRadius;
    const x2 = centerX + Math.cos(angle) * tickOuterRadius;
    const y2 = centerY + Math.sin(angle) * tickOuterRadius;

    // Color red for redline zone
    if (tickValue >= config.max * 0.85) {
      ctx.strokeStyle = tsColorToHex(config.critical_color);
      ctx.fillStyle = tsColorToHex(config.critical_color);
    } else {
      ctx.strokeStyle = '#ffffff';
      ctx.fillStyle = '#ffffff';
    }

    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
    ctx.stroke();

    // Number labels
    const labelRadius = radius * 0.58;
    const labelX = centerX + Math.cos(angle) * labelRadius;
    const labelY = centerY + Math.sin(angle) * labelRadius;

    ctx.font = getFontSpec(radius * 0.12, { bold: true });
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(String(i), labelX, labelY);
  }

  // Minor ticks
  ctx.strokeStyle = '#666666';
  ctx.lineWidth = 1;
  const minorTicks = majorTicks * 5;
  for (let i = 0; i < minorTicks; i++) {
    const tickValue = config.min + (i / minorTicks) * (config.max - config.min);
    if (tickValue % 1000 === 0) continue; // Skip major tick positions

    const normalizedTick = (tickValue - config.min) / (config.max - config.min);
    const angle = startAngle + totalSweep * normalizedTick;

    const x1 = centerX + Math.cos(angle) * (tickInnerRadius + (tickOuterRadius - tickInnerRadius) * 0.5);
    const y1 = centerY + Math.sin(angle) * (tickInnerRadius + (tickOuterRadius - tickInnerRadius) * 0.5);
    const x2 = centerX + Math.cos(angle) * tickOuterRadius;
    const y2 = centerY + Math.sin(angle) * tickOuterRadius;

    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
    ctx.stroke();
  }

  // Redline zone arc
  const redlineStart = config.max * 0.85;
  const redlineNormalized = (redlineStart - config.min) / (config.max - config.min);
  const redlineAngle = startAngle + totalSweep * redlineNormalized;

  ctx.beginPath();
  ctx.arc(centerX, centerY, radius * 0.88, redlineAngle, startAngle + totalSweep);
  ctx.lineWidth = radius * 0.05;
  ctx.strokeStyle = tsColorToHex(config.critical_color);
  ctx.stroke();

  // Needle
  const normalizedValue = (value - config.min) / (config.max - config.min);
  const needleAngle = startAngle + totalSweep * normalizedValue;
  const needleLength = radius * 0.65;
  const needleWidth = radius * 0.03;

  ctx.save();
  ctx.translate(centerX, centerY);
  ctx.rotate(needleAngle);

  // Needle shadow
  ctx.shadowColor = 'rgba(0,0,0,0.5)';
  ctx.shadowBlur = 8;
  ctx.shadowOffsetX = 4;
  ctx.shadowOffsetY = 4;

  // Needle body - red with gradient
  ctx.beginPath();
  ctx.moveTo(-needleLength * 0.15, 0);
  ctx.lineTo(needleLength, 0);
  ctx.lineTo(-needleLength * 0.15, -needleWidth);
  ctx.lineTo(-needleLength * 0.15, needleWidth);
  ctx.closePath();

  const needleGradient = ctx.createLinearGradient(0, -needleWidth, 0, needleWidth);
  needleGradient.addColorStop(0, '#ff4444');
  needleGradient.addColorStop(0.5, '#ff0000');
  needleGradient.addColorStop(1, '#aa0000');
  ctx.fillStyle = needleGradient;
  ctx.fill();

  ctx.restore();
  ctx.shadowColor = 'transparent';

  // Center hub
  const hubGradient = ctx.createRadialGradient(centerX - 3, centerY - 3, 0, centerX, centerY, radius * 0.1);
  hubGradient.addColorStop(0, '#888888');
  hubGradient.addColorStop(0.5, '#444444');
  hubGradient.addColorStop(1, '#222222');
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius * 0.08, 0, Math.PI * 2);
  ctx.fillStyle = hubGradient;
  ctx.fill();

  // Digital RPM display at bottom
  const valueTextColorTs = getValueColor();
  const fontSize = Math.max(10, radius * 0.16);
  ctx.fillStyle = tsColorToHex(valueTextColorTs);
  ctx.font = getFontSpec(fontSize, { bold: true, monospace: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(value.toFixed(0), centerX, centerY + radius * 0.35);

  // RPM label
  ctx.fillStyle = '#888888';
  ctx.font = getFontSpec(fontSize * 0.6);
  ctx.fillText('RPM × 1000', centerX, centerY + radius * 0.52);

  // Title at top
  if (config.title) {
    ctx.fillStyle = tsColorToHex(config.font_color);
    ctx.font = getFontSpec(fontSize * 0.5);
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, centerX, 4);
  }
};
