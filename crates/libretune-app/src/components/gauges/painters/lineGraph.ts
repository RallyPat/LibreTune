/** LineGraph — time-series line chart with filled gradient area and current value dot. */

import { tsColorToHex } from '../../dashboards/dashTypes';
import { roundRect, lightenColor, darkenColor } from '../drawUtils';
import { getChannelHistoryBuffer } from '../../../stores/realtimeStore';
import type { Painter } from './types';

export const lineGraphPainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getValueColor, getFontSpec } = pctx;

  const padding = 8;
  const titleHeight = height * 0.12;
  const graphWidth = width - padding * 2;
  const graphHeight = height - titleHeight - padding * 2;
  const graphY = titleHeight + padding;

  // Background with gradient
  const bgGradient = ctx.createLinearGradient(0, 0, 0, height);
  const bgHex = tsColorToHex(config.back_color);
  bgGradient.addColorStop(0, lightenColor(bgHex, 5));
  bgGradient.addColorStop(1, darkenColor(bgHex, 10));
  ctx.fillStyle = bgGradient;
  ctx.fillRect(0, 0, width, height);

  // Title and value
  ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
  ctx.shadowBlur = 2;
  ctx.fillStyle = tsColorToHex(config.trim_color);
  ctx.font = getFontSpec(Math.max(9, titleHeight * 0.8), { bold: true });
  ctx.textAlign = 'left';
  ctx.textBaseline = 'top';
  ctx.fillText(config.title, padding, 3);

  ctx.fillStyle = tsColorToHex(config.font_color);
  ctx.font = getFontSpec(Math.max(10, titleHeight * 0.9), { bold: true, monospace: true });
  ctx.textAlign = 'right';
  ctx.fillText(`${value.toFixed(config.value_digits)} ${config.units}`, width - padding, 3);
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

  // Build points from history (or generate sample data if no history)
  // Read history imperatively from the non-reactive buffer — no React re-renders needed.
  const history = getChannelHistoryBuffer(config.output_channel);
  const points: { x: number; y: number }[] = [];

  if (history && history.length > 0) {
    // Use actual history data
    const dataRange = config.max - config.min;
    for (let i = 0; i < history.length; i++) {
      const t = i / (history.length - 1);
      const historicalValue = history[i];
      const historicalPercent = (historicalValue - config.min) / dataRange;
      const clampedPercent = Math.max(0, Math.min(1, historicalPercent));

      points.push({
        x: padding + t * graphWidth,
        y: graphY + graphHeight - clampedPercent * graphHeight,
      });
    }
  } else {
    // No history available - show simulated data for demo
    const numPoints = 50;
    const valuePercent = (value - config.min) / (config.max - config.min);

    for (let i = 0; i < numPoints; i++) {
      const t = i / (numPoints - 1);
      // Simulate some variation leading up to current value
      const noise = Math.sin(t * 20) * 0.05 + Math.sin(t * 7) * 0.03;
      const historicalPercent = valuePercent + (1 - t) * (Math.random() * 0.2 - 0.1) + noise * (1 - t);
      const clampedPercent = Math.max(0, Math.min(1, historicalPercent));

      points.push({
        x: padding + t * graphWidth,
        y: graphY + graphHeight - clampedPercent * graphHeight,
      });
    }
  }

  if (points.length === 0) return; // Nothing to draw

  // Draw filled area under the line
  const lineColor = tsColorToHex(getValueColor());
  const fillGradient = ctx.createLinearGradient(0, graphY, 0, graphY + graphHeight);
  fillGradient.addColorStop(0, lineColor + '60');
  fillGradient.addColorStop(1, lineColor + '10');

  ctx.beginPath();
  ctx.moveTo(points[0].x, graphY + graphHeight);
  for (const point of points) {
    ctx.lineTo(point.x, point.y);
  }
  ctx.lineTo(points[points.length - 1].x, graphY + graphHeight);
  ctx.closePath();
  ctx.fillStyle = fillGradient;
  ctx.fill();

  // Draw the line
  ctx.beginPath();
  ctx.moveTo(points[0].x, points[0].y);
  for (let i = 1; i < points.length; i++) {
    ctx.lineTo(points[i].x, points[i].y);
  }
  ctx.strokeStyle = lineColor;
  ctx.lineWidth = 2;
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  ctx.stroke();

  // Draw current value dot with glow
  const lastPoint = points[points.length - 1];
  ctx.shadowColor = lineColor;
  ctx.shadowBlur = 8;
  ctx.beginPath();
  ctx.arc(lastPoint.x, lastPoint.y, 4, 0, Math.PI * 2);
  ctx.fillStyle = lightenColor(lineColor, 30);
  ctx.fill();
  ctx.shadowColor = 'transparent';

  // Min/max labels on Y axis
  ctx.fillStyle = tsColorToHex(config.trim_color);
  ctx.font = getFontSpec(Math.max(7, graphHeight * 0.08));
  ctx.textAlign = 'left';
  ctx.textBaseline = 'top';
  ctx.fillText(config.max.toFixed(0), padding + 2, graphY + 2);
  ctx.textBaseline = 'bottom';
  ctx.fillText(config.min.toFixed(0), padding + 2, graphY + graphHeight - 2);
};
