/**
 * HorizontalBarGauge painter — horizontal progress bar with rounded
 * corners, gradient fill, and a value readout in the corner.
 *
 * Migrated from the inline `drawHorizontalBar` closure in
 * `TsGauge.tsx`. Behavior is byte-for-byte identical to the original.
 */

import { tsColorToRgba, tsColorToHex } from '../../dashboards/dashTypes';
import { roundRect, lightenColor, darkenColor } from '../drawUtils';
import type { Painter } from './types';

export const horizontalBarPainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getValueColor, getFontSpec } = pctx;

  const padding = 6;
  const barHeight = height * 0.35;
  const barY = (height - barHeight) / 2 + height * 0.08;
  const barWidth = width - padding * 2;
  const cornerRadius = Math.min(4, barHeight * 0.3);

  // Background with subtle gradient
  const bgGradient = ctx.createLinearGradient(0, 0, 0, height);
  const bgHex = tsColorToHex(config.back_color);
  bgGradient.addColorStop(0, lightenColor(bgHex, 10));
  bgGradient.addColorStop(1, darkenColor(bgHex, 15));
  ctx.fillStyle = bgGradient;
  ctx.fillRect(0, 0, width, height);

  // Title
  ctx.fillStyle = tsColorToRgba(config.trim_color);
  ctx.font = getFontSpec(Math.max(9, height * 0.14));
  ctx.textAlign = 'left';
  ctx.textBaseline = 'top';
  ctx.fillText(config.title, padding, 3);

  // Bar background with inset effect
  ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
  ctx.shadowBlur = 3;
  ctx.shadowOffsetX = 1;
  ctx.shadowOffsetY = 1;
  const barBgGradient = ctx.createLinearGradient(0, barY, 0, barY + barHeight);
  barBgGradient.addColorStop(0, '#252525');
  barBgGradient.addColorStop(0.5, '#404040');
  barBgGradient.addColorStop(1, '#303030');
  ctx.fillStyle = barBgGradient;
  roundRect(ctx, padding, barY, barWidth, barHeight, cornerRadius);
  ctx.fill();
  ctx.shadowColor = 'transparent';

  // Bar fill with gradient
  const fillPercent = (value - config.min) / (config.max - config.min);
  const fillWidth = barWidth * Math.max(0, Math.min(1, fillPercent));
  if (fillWidth > 0) {
    const valueColor = getValueColor();
    const valueHex = tsColorToHex(valueColor);
    const fillGradient = ctx.createLinearGradient(0, barY, 0, barY + barHeight);
    fillGradient.addColorStop(0, lightenColor(valueHex, 30));
    fillGradient.addColorStop(0.3, lightenColor(valueHex, 10));
    fillGradient.addColorStop(0.7, valueHex);
    fillGradient.addColorStop(1, darkenColor(valueHex, 20));
    ctx.fillStyle = fillGradient;
    roundRect(ctx, padding, barY, fillWidth, barHeight, cornerRadius);
    ctx.fill();

    // Highlight stripe
    ctx.fillStyle = 'rgba(255, 255, 255, 0.15)';
    ctx.fillRect(padding + 2, barY + 2, fillWidth - 4, barHeight * 0.3);
  }

  // Bar border
  ctx.strokeStyle = tsColorToRgba(config.trim_color);
  ctx.lineWidth = 1;
  roundRect(ctx, padding, barY, barWidth, barHeight, cornerRadius);
  ctx.stroke();

  // Value text with shadow
  ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
  ctx.shadowBlur = 2;
  ctx.fillStyle = tsColorToRgba(config.font_color);
  ctx.font = getFontSpec(Math.max(11, height * 0.18), { bold: true, monospace: true });
  ctx.textAlign = 'right';
  ctx.textBaseline = 'top';
  ctx.fillText(`${value.toFixed(config.value_digits)} ${config.units}`, width - padding, 3);
  ctx.shadowColor = 'transparent';
};
