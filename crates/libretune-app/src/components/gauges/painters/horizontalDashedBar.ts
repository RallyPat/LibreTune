/** HorizontalDashedBar — segmented horizontal bar with per-segment zone coloring. */

import { tsColorToHex } from '../../dashboards/dashTypes';
import { roundRect, lightenColor, darkenColor } from '../drawUtils';
import type { Painter } from './types';

export const horizontalDashedBarPainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getFontSpec } = pctx;

  const padding = 6;
  const titleHeight = height * 0.15;
  const valueHeight = height * 0.15;
  const barHeight = height * 0.35;
  const barWidth = width - padding * 2;
  const barX = padding;
  const barY = titleHeight + (height - titleHeight - valueHeight - barHeight) / 2;
  const numSegments = 14;
  const segmentWidth = barWidth / numSegments;
  const segmentGap = 3;

  // Background
  const bgGradient = ctx.createLinearGradient(0, 0, 0, height);
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
  ctx.font = getFontSpec(Math.max(9, titleHeight * 0.6), { bold: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'top';
  ctx.fillText(config.title, width / 2, 2);
  ctx.shadowColor = 'transparent';

  const fillPercent = (value - config.min) / (config.max - config.min);
  const filledSegments = Math.ceil(fillPercent * numSegments);

  for (let i = 0; i < numSegments; i++) {
    const segmentX = barX + i * segmentWidth;
    const isFilled = i < filledSegments;
    const segmentPercent = i / numSegments;
    const segmentValue = config.min + segmentPercent * (config.max - config.min);

    let segmentColor: string;
    if (config.high_critical !== null && segmentValue >= config.high_critical) {
      segmentColor = isFilled ? tsColorToHex(config.critical_color) : '#401010';
    } else if (config.high_warning !== null && segmentValue >= config.high_warning) {
      segmentColor = isFilled ? tsColorToHex(config.warn_color) : '#403010';
    } else {
      segmentColor = isFilled ? tsColorToHex(config.needle_color) : '#303030';
    }

    if (isFilled) {
      const segGradient = ctx.createLinearGradient(segmentX, 0, segmentX + segmentWidth, 0);
      segGradient.addColorStop(0, darkenColor(segmentColor, 15));
      segGradient.addColorStop(0.3, lightenColor(segmentColor, 20));
      segGradient.addColorStop(0.7, lightenColor(segmentColor, 15));
      segGradient.addColorStop(1, darkenColor(segmentColor, 10));
      ctx.fillStyle = segGradient;

      if (i === filledSegments - 1) {
        ctx.shadowColor = segmentColor;
        ctx.shadowBlur = 6;
      }
    } else {
      ctx.fillStyle = segmentColor;
    }

    roundRect(
      ctx,
      segmentX + segmentGap / 2,
      barY,
      segmentWidth - segmentGap,
      barHeight,
      2,
    );
    ctx.fill();
    ctx.shadowColor = 'transparent';
  }

  // Value
  ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
  ctx.shadowBlur = 2;
  ctx.fillStyle = tsColorToHex(config.font_color);
  ctx.font = getFontSpec(Math.max(11, valueHeight * 0.6), { bold: true, monospace: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'bottom';
  ctx.fillText(`${value.toFixed(config.value_digits)}`, width / 2, height - 2);
  ctx.shadowColor = 'transparent';
};
