/** LineGraph — time-series line chart with filled gradient area and current value dot. */

import { tsColorToHex } from '../../dashboards/dashTypes';
import { roundRect, lightenColor, darkenColor } from '../drawUtils';
import { getChannelHistoryBuffer } from '../../../stores/realtimeStore';
import { emaStep, isSpike } from '../ema';
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
  const spikePoints: { x: number; y: number }[] = [];
  if (history && history.length > 0) {
    // EMA-smooth the trace (issue #82: raw samples at high stream rates look
    // like an audio waveform). Samples far from the smoothed value are
    // transient spikes — highlighted as dots below so short pulses that
    // indicate problems stay visible (the issue's "peak detect" ask).
    const dataRange = config.max - config.min;
    const SAMPLE_DT_MS = 100; // matches the default stream cadence
    const TRACE_TAU_MS = 250; // ~3-sample smoothing horizon
    const smoothed = new Array<number>(history.length);
    let ema = history[0];
    for (let i = 0; i < history.length; i++) {
      ema = i === 0 ? history[0] : emaStep(ema, history[i], SAMPLE_DT_MS, TRACE_TAU_MS);
      smoothed[i] = ema;
      if (i > 0 && isSpike(history[i], ema, config.min, config.max)) {
        const t = i / (history.length - 1);
        const rawPercent = Math.max(0, Math.min(1, (history[i] - config.min) / dataRange));
        spikePoints.push({
          x: padding + t * graphWidth,
          y: graphY + graphHeight - rawPercent * graphHeight,
        });
      }
    }
    for (let i = 0; i < smoothed.length; i++) {
      const t = i / (smoothed.length - 1);
      const smoothedPercent = Math.max(0, Math.min(1, (smoothed[i] - config.min) / dataRange));

      points.push({
        x: padding + t * graphWidth,
        y: graphY + graphHeight - smoothedPercent * graphHeight,
      });
    }
  } else {
    // No history available — render a flat line at the current/default value
    // so disconnected gauges are steady instead of showing a random waving trace
    // (Issue #83). The deterministic shape gives immediate visual feedback while
    // still resembling a line graph.
    const numPoints = 50;
    const valuePercent = (value - config.min) / (config.max - config.min);
    const clampedPercent = Math.max(0, Math.min(1, valuePercent));

    for (let i = 0; i < numPoints; i++) {
      const t = i / (numPoints - 1);
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

  // Transient-spike dots (issue #82): raw samples that deviated sharply from
  // the EMA-smoothed trace, drawn in the critical color at the raw position.
  if (spikePoints.length > 0) {
    const spikeColor = tsColorToHex(config.critical_color);
    ctx.fillStyle = spikeColor;
    for (const sp of spikePoints) {
      ctx.beginPath();
      ctx.arc(sp.x, sp.y, 2.5, 0, Math.PI * 2);
      ctx.fill();
    }
  }

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
