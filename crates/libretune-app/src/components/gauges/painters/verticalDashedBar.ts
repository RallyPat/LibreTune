/** VerticalDashedBar — segmented vertical bar with per-segment zone coloring. */

import { tsColorToHex } from '../../dashboards/dashTypes';
import { segmentZoneColor, drawDashedSegment, lightenColor, darkenColor } from '../drawUtils';
import type { Painter } from './types';

export const verticalDashedBarPainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getFontSpec } = pctx;

  const padding = 6;
  const labelHeight = height * 0.1;
  const barWidth = width * 0.5;
  const barHeight = height - padding * 2 - labelHeight * 2;
  const barX = (width - barWidth) / 2;
  const barY = labelHeight + padding;
  const numSegments = 12;
  const segmentHeight = barHeight / numSegments;
  const segmentGap = 3;

  // Background with gradient
  const bgGradient = ctx.createLinearGradient(0, 0, width, 0);
  const bgHex = tsColorToHex(config.back_color);
  bgGradient.addColorStop(0, darkenColor(bgHex, 5));
  bgGradient.addColorStop(0.5, lightenColor(bgHex, 5));
  bgGradient.addColorStop(1, darkenColor(bgHex, 5));
  ctx.fillStyle = bgGradient;
  ctx.fillRect(0, 0, width, height);

  // Title
  ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
  ctx.shadowBlur = 2;
  ctx.fillStyle = tsColorToHex(config.trim_color);
  ctx.font = getFontSpec(Math.max(9, labelHeight * 0.8), { bold: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'top';
  ctx.fillText(config.title, width / 2, 2);
  ctx.shadowColor = 'transparent';

  // Draw segments
  const fillPercent = (value - config.min) / (config.max - config.min);
  const filledSegments = Math.ceil(fillPercent * numSegments);

  for (let i = 0; i < numSegments; i++) {
    const segmentY = barY + (numSegments - 1 - i) * segmentHeight;
    const isFilled = i < filledSegments;
    const segmentPercent = i / numSegments;

    const segmentValue = config.min + segmentPercent * (config.max - config.min);
    const color = segmentZoneColor(config, segmentValue, isFilled);

    drawDashedSegment(
      ctx,
      { x: barX, y: segmentY + segmentGap / 2, w: barWidth, h: segmentHeight - segmentGap },
      barX,
      barX + barWidth,
      color,
      isFilled,
      i === filledSegments - 1,
    );
  }

  // Value at bottom
  ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
  ctx.shadowBlur = 2;
  ctx.fillStyle = tsColorToHex(config.font_color);
  ctx.font = getFontSpec(Math.max(11, labelHeight * 0.9), { bold: true, monospace: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'bottom';
  ctx.fillText(`${value.toFixed(config.value_digits)}`, width / 2, height - 2);
  ctx.shadowColor = 'transparent';
};
