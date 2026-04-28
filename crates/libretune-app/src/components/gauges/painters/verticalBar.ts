/** VerticalBarGauge — vertical progress bar with tick marks and 3D gradient fill. */

import { tsColorToRgba, tsColorToHex } from '../../dashboards/dashTypes';
import { roundRect, lightenColor, darkenColor } from '../drawUtils';
import type { Painter } from './types';

export const verticalBarPainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getValueColor, getFontSpec } = pctx;

  const padding = 6;
  const labelHeight = height * 0.12;
  const barWidth = width * 0.45;
  const barHeight = height - labelHeight * 2 - padding * 3;
  const barX = (width - barWidth) / 2;
  const barY = labelHeight + padding * 1.5;
  const cornerRadius = Math.min(4, barWidth * 0.15);

  // Background with gradient
  const bgGradient = ctx.createLinearGradient(0, 0, width, 0);
  const bgHex = tsColorToHex(config.back_color);
  bgGradient.addColorStop(0, lightenColor(bgHex, 5));
  bgGradient.addColorStop(0.5, bgHex);
  bgGradient.addColorStop(1, darkenColor(bgHex, 10));
  ctx.fillStyle = bgGradient;
  ctx.fillRect(0, 0, width, height);

  // Title at top with shadow
  ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
  ctx.shadowBlur = 2;
  ctx.fillStyle = tsColorToRgba(config.trim_color);
  ctx.font = getFontSpec(Math.max(9, labelHeight * 0.75), { bold: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'top';
  ctx.fillText(config.title, width / 2, 3);
  ctx.shadowColor = 'transparent';

  // Bar background with inset effect
  ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
  ctx.shadowBlur = 4;
  ctx.shadowOffsetX = 2;
  ctx.shadowOffsetY = 2;
  const barBgGradient = ctx.createLinearGradient(barX, 0, barX + barWidth, 0);
  barBgGradient.addColorStop(0, '#202020');
  barBgGradient.addColorStop(0.3, '#383838');
  barBgGradient.addColorStop(0.7, '#383838');
  barBgGradient.addColorStop(1, '#282828');
  ctx.fillStyle = barBgGradient;
  roundRect(ctx, barX, barY, barWidth, barHeight, cornerRadius);
  ctx.fill();
  ctx.shadowColor = 'transparent';

  // Bar fill (from bottom) with gradient
  const fillPercent = (value - config.min) / (config.max - config.min);
  const fillHeight = barHeight * Math.max(0, Math.min(1, fillPercent));
  if (fillHeight > 0) {
    const valueColor = getValueColor();
    const valueHex = tsColorToHex(valueColor);
    const fillGradient = ctx.createLinearGradient(barX, 0, barX + barWidth, 0);
    fillGradient.addColorStop(0, darkenColor(valueHex, 15));
    fillGradient.addColorStop(0.3, lightenColor(valueHex, 15));
    fillGradient.addColorStop(0.7, lightenColor(valueHex, 10));
    fillGradient.addColorStop(1, darkenColor(valueHex, 10));
    ctx.fillStyle = fillGradient;
    roundRect(ctx, barX, barY + barHeight - fillHeight, barWidth, fillHeight, cornerRadius);
    ctx.fill();

    // Highlight stripe
    ctx.fillStyle = 'rgba(255, 255, 255, 0.12)';
    ctx.fillRect(barX + 3, barY + barHeight - fillHeight + 2, barWidth * 0.35, fillHeight - 4);
  }

  // Bar border
  ctx.strokeStyle = tsColorToRgba(config.trim_color);
  ctx.lineWidth = 1;
  roundRect(ctx, barX, barY, barWidth, barHeight, cornerRadius);
  ctx.stroke();

  // Tick marks on right side
  const tickCount = 5;
  ctx.strokeStyle = tsColorToRgba(config.trim_color);
  ctx.lineWidth = 1;
  for (let i = 0; i <= tickCount; i++) {
    const tickY = barY + barHeight - (barHeight * i / tickCount);
    ctx.beginPath();
    ctx.moveTo(barX + barWidth + 2, tickY);
    ctx.lineTo(barX + barWidth + 6, tickY);
    ctx.stroke();
  }

  // Value at bottom with glow
  ctx.shadowColor = 'rgba(0, 0, 0, 0.6)';
  ctx.shadowBlur = 3;
  ctx.fillStyle = tsColorToRgba(config.font_color);
  ctx.font = getFontSpec(Math.max(11, labelHeight * 0.9), { bold: true, monospace: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'bottom';
  ctx.fillText(`${value.toFixed(config.value_digits)}`, width / 2, height - 2);
  ctx.shadowColor = 'transparent';
};
