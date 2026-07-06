/**
 * MultiChannelTrend — LibreTune-native multi-channel overlay trend chart.
 *
 * Plots the gauge's primary `output_channel` plus up to 3 additional
 * channels on one time-series graph, each independently normalized to its
 * own min/max (so e.g. coolant °C, boost PSI, and speed km/h can share one
 * chart meaningfully) with a color-coded legend showing live values.
 *
 * Additional series are configured via `config.extra_attrs` using a
 * `lt_seriesN_*` naming convention (N = 2..12):
 *   lt_seriesN_channel  — ECU output channel name (required to enable slot N)
 *   lt_seriesN_label    — legend label (defaults to the channel name)
 *   lt_seriesN_color    — CSS hex color for the line/legend swatch
 *   lt_seriesN_min      — series-local minimum for normalization
 *   lt_seriesN_max      — series-local maximum for normalization
 */

import { tsColorToHex } from '../../dashboards/dashTypes';
import type { TsGaugeConfig } from '../../dashboards/dashTypes';
import { roundRect, lightenColor, darkenColor } from '../drawUtils';
import { getChannelHistoryBuffer, useRealtimeStore } from '../../../stores/realtimeStore';
import type { Painter } from './types';

interface TrendSeries {
  channel: string;
  label: string;
  color: string;
  min: number;
  max: number;
}

const MAX_EXTRA_SERIES = 11;

function readLatestChannelValue(channel: string): number | undefined {
  const channels = useRealtimeStore.getState().channels;
  if (channels[channel] !== undefined) return channels[channel];
  const lower = channel.toLowerCase();
  for (const key of Object.keys(channels)) {
    if (key.toLowerCase() === lower) return channels[key];
  }
  return undefined;
}

/** Parse the primary series plus any `lt_seriesN_*` extra series from config. */
function resolveSeries(config: TsGaugeConfig, primaryColor: string): TrendSeries[] {
  const series: TrendSeries[] = [
    {
      channel: config.output_channel,
      label: config.title || config.output_channel || 'Series 1',
      color: primaryColor,
      min: config.min,
      max: config.max,
    },
  ];

  const attrs = config.extra_attrs || {};
  for (let n = 2; n <= 1 + MAX_EXTRA_SERIES; n++) {
    const channel = attrs[`lt_series${n}_channel`];
    if (!channel) continue;
    series.push({
      channel,
      label: attrs[`lt_series${n}_label`] || channel,
      color: attrs[`lt_series${n}_color`] || '#94a3b8',
      min: attrs[`lt_series${n}_min`] !== undefined ? parseFloat(attrs[`lt_series${n}_min`]) : 0,
      max: attrs[`lt_series${n}_max`] !== undefined ? parseFloat(attrs[`lt_series${n}_max`]) : 100,
    });
  }
  return series;
}

/** Build normalized (0-1) points for one series from history, or a flat demo line if no history yet. */
function buildNormalizedPoints(s: TrendSeries, fallbackValue: number): number[] {
  const history = getChannelHistoryBuffer(s.channel);
  const range = s.max - s.min || 1;
  if (history && history.length > 1) {
    return history.map((v) => Math.max(0, Math.min(1, (v - s.min) / range)));
  }
  const demoPercent = Math.max(0, Math.min(1, (fallbackValue - s.min) / range));
  return new Array(50).fill(demoPercent);
}

export const multiChannelTrendPainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getFontSpec } = pctx;

  const padding = 6;
  const titleHeight = Math.max(14, height * 0.08);
  const series = resolveSeries(config, tsColorToHex(config.font_color));

  // Legend grows with series count — Grafana-style multi-row when dense.
  const legendRowHeight = Math.max(11, height * 0.045);
  const legendCols = Math.max(2, Math.floor((width - padding * 2) / 110));
  const legendRows = Math.max(1, Math.ceil(series.length / legendCols));
  const legendHeight = legendRows * legendRowHeight + 4;

  const graphWidth = width - padding * 2;
  const graphY = titleHeight + legendHeight + padding * 0.5;
  const graphHeight = height - graphY - padding;

  // Background.
  const bgHex = tsColorToHex(config.back_color);
  const bgGradient = ctx.createLinearGradient(0, 0, 0, height);
  bgGradient.addColorStop(0, lightenColor(bgHex, 4));
  bgGradient.addColorStop(1, darkenColor(bgHex, 8));
  ctx.fillStyle = bgGradient;
  ctx.fillRect(0, 0, width, height);

  // Title.
  ctx.fillStyle = tsColorToHex(config.trim_color);
  ctx.font = getFontSpec(Math.max(9, titleHeight * 0.85), { bold: true });
  ctx.textAlign = 'left';
  ctx.textBaseline = 'top';
  ctx.fillText(config.title.toUpperCase(), padding, 2);

  const legendFontSize = Math.max(7, legendRowHeight * 0.65);
  ctx.font = getFontSpec(legendFontSize, { bold: true, monospace: true });
  ctx.textBaseline = 'middle';
  const colWidth = (width - padding * 2) / legendCols;

  for (let i = 0; i < series.length; i++) {
    const s = series[i];
    const live = i === 0 ? value : readLatestChannelValue(s.channel) ?? s.min;
    const col = i % legendCols;
    const row = Math.floor(i / legendCols);
    const legendX = padding + col * colWidth;
    const legendY = titleHeight + row * legendRowHeight + legendRowHeight / 2 + 2;
    const dotR = Math.max(2, legendFontSize * 0.22);

    ctx.fillStyle = s.color;
    ctx.beginPath();
    ctx.arc(legendX + dotR, legendY, dotR, 0, Math.PI * 2);
    ctx.fill();

    ctx.fillStyle = s.color;
    ctx.textAlign = 'left';
    const text = `${s.label} ${live.toFixed(1)}`;
    ctx.fillText(text, legendX + dotR * 2 + 3, legendY);
  }

  // Graph panel (inset).
  ctx.shadowColor = 'rgba(0, 0, 0, 0.3)';
  ctx.shadowBlur = 3;
  ctx.shadowOffsetY = 1;
  ctx.fillStyle = '#141414';
  roundRect(ctx, padding - 2, graphY - 2, graphWidth + 4, graphHeight + 4, 4);
  ctx.fill();
  ctx.shadowColor = 'transparent';

  // Grid lines.
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.08)';
  ctx.lineWidth = 1;
  for (let i = 1; i < 4; i++) {
    const gridY = graphY + graphHeight * (i / 4);
    ctx.beginPath();
    ctx.moveTo(padding, gridY);
    ctx.lineTo(padding + graphWidth, gridY);
    ctx.stroke();
  }

  // Plot each series (primary drawn last so it's on top).
  for (let i = series.length - 1; i >= 0; i--) {
    const s = series[i];
    const normalized = buildNormalizedPoints(s, i === 0 ? value : readLatestChannelValue(s.channel) ?? s.min);
    if (normalized.length < 2) continue;

    const isPrimary = i === 0;
    ctx.beginPath();
    for (let p = 0; p < normalized.length; p++) {
      const t = p / (normalized.length - 1);
      const x = padding + t * graphWidth;
      const y = graphY + graphHeight - normalized[p] * graphHeight;
      if (p === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    const lineW = series.length > 6 ? 1.1 : series.length > 3 ? 1.4 : isPrimary ? 2 : 1.6;
    ctx.strokeStyle = s.color;
    ctx.lineWidth = isPrimary && series.length <= 3 ? 2.25 : lineW;
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    ctx.globalAlpha = isPrimary ? 1 : 0.85;
    if (isPrimary) {
      ctx.shadowColor = s.color;
      ctx.shadowBlur = 4;
    }
    ctx.stroke();
    ctx.shadowColor = 'transparent';
    ctx.globalAlpha = 1;

    // Glowing dot at the latest point for the primary series only (avoid clutter).
    if (isPrimary) {
      const lastT = 1;
      const lastX = padding + lastT * graphWidth;
      const lastY = graphY + graphHeight - normalized[normalized.length - 1] * graphHeight;
      ctx.shadowColor = s.color;
      ctx.shadowBlur = 8;
      ctx.beginPath();
      ctx.arc(lastX, lastY, 3.5, 0, Math.PI * 2);
      ctx.fillStyle = lightenColor(s.color, 30);
      ctx.fill();
      ctx.shadowColor = 'transparent';
    }
  }
};
