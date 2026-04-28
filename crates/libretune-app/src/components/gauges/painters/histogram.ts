/** Histogram — bar chart distribution visualization centered on current value. */

import { tsColorToHex } from '../../dashboards/dashTypes';
import { roundRect, lightenColor, darkenColor } from '../drawUtils';
import type { Painter } from './types';

export const histogramPainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getFontSpec } = pctx;

  const padding = 8;
  const titleHeight = height * 0.12;
  const valueHeight = height * 0.1;
  const graphWidth = width - padding * 2;
  const graphHeight = height - titleHeight - valueHeight - padding * 2;
  const graphY = titleHeight + padding;
  const numBars = 20;
  const barWidth = (graphWidth - (numBars - 1) * 2) / numBars;

  // Background with gradient
  const bgGradient = ctx.createLinearGradient(0, 0, 0, height);
  const bgHex = tsColorToHex(config.back_color);
  bgGradient.addColorStop(0, lightenColor(bgHex, 5));
  bgGradient.addColorStop(1, darkenColor(bgHex, 10));
  ctx.fillStyle = bgGradient;
  ctx.fillRect(0, 0, width, height);

  // Title
  ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
  ctx.shadowBlur = 2;
  ctx.fillStyle = tsColorToHex(config.trim_color);
  ctx.font = getFontSpec(Math.max(9, titleHeight * 0.8), { bold: true });
  ctx.textAlign = 'left';
  ctx.textBaseline = 'top';
  ctx.fillText(config.title, padding, 3);
  ctx.shadowColor = 'transparent';

  // Graph background with inset
  ctx.shadowColor = 'rgba(0, 0, 0, 0.3)';
  ctx.shadowBlur = 3;
  ctx.shadowOffsetY = 1;
  ctx.fillStyle = '#1a1a1a';
  roundRect(ctx, padding - 2, graphY - 2, graphWidth + 4, graphHeight + 4, 4);
  ctx.fill();
  ctx.shadowColor = 'transparent';

  // Grid lines
  ctx.strokeStyle = 'rgba(80, 80, 80, 0.3)';
  ctx.lineWidth = 1;
  for (let i = 1; i < 4; i++) {
    const gridY = graphY + graphHeight * (i / 4);
    ctx.beginPath();
    ctx.moveTo(padding, gridY);
    ctx.lineTo(padding + graphWidth, gridY);
    ctx.stroke();
  }

  // Generate histogram data based on current value position
  // This simulates a distribution centered around the current value
  const valuePercent = (value - config.min) / (config.max - config.min);
  const centerBar = Math.floor(valuePercent * numBars);
  const normalColor = tsColorToHex(config.needle_color); // Use needle_color for normal range
  const warnColor = tsColorToHex(config.warn_color);

  for (let i = 0; i < numBars; i++) {
    const barX = padding + i * (barWidth + 2);

    // Create a bell-curve distribution around the current value
    const distFromCenter = Math.abs(i - centerBar);
    const barHeightPercent = Math.max(0.05, Math.exp(-distFromCenter * distFromCenter / 20));
    const barHeight = graphHeight * barHeightPercent;
    const barY = graphY + graphHeight - barHeight;

    // Color based on position in range
    const barPercent = i / numBars;
    let barColor: string;
    if (config.high_critical !== null && barPercent >= (config.high_critical - config.min) / (config.max - config.min)) {
      barColor = tsColorToHex(config.critical_color);
    } else if (config.high_warning !== null && barPercent >= (config.high_warning - config.min) / (config.max - config.min)) {
      barColor = warnColor;
    } else {
      barColor = normalColor;
    }

    // Bar gradient
    const barGradient = ctx.createLinearGradient(barX, barY, barX, barY + barHeight);
    barGradient.addColorStop(0, lightenColor(barColor, 25));
    barGradient.addColorStop(0.5, barColor);
    barGradient.addColorStop(1, darkenColor(barColor, 15));
    ctx.fillStyle = barGradient;
    roundRect(ctx, barX, barY, barWidth, barHeight, 2);
    ctx.fill();
  }

  // Current value marker
  const markerX = padding + valuePercent * graphWidth;
  ctx.strokeStyle = tsColorToHex(config.needle_color);
  ctx.lineWidth = 2;
  ctx.setLineDash([4, 2]);
  ctx.beginPath();
  ctx.moveTo(markerX, graphY);
  ctx.lineTo(markerX, graphY + graphHeight);
  ctx.stroke();
  ctx.setLineDash([]);

  // Value display
  ctx.fillStyle = tsColorToHex(config.font_color);
  ctx.font = getFontSpec(Math.max(10, valueHeight * 0.9), { bold: true, monospace: true });
  ctx.textAlign = 'right';
  ctx.textBaseline = 'top';
  ctx.fillText(`${value.toFixed(config.value_digits)} ${config.units}`, width - padding, 3);
};
