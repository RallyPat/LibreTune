/**
 * TelemetryStat — flat, modern stat tile (LibreTune-native painter).
 *
 * Designed for compact "racing telemetry" style dashboards: a solid dark
 * panel, a colored accent stripe down the left edge (repurposing
 * `needle_color`, since this painter has no needle), a bold uppercase
 * label, a large monospace value, and a thin range bar along the bottom
 * showing where the value sits between min/max (with warning/critical tick
 * marks). Deliberately flat — no metallic bezel/gradient — to read as
 * "modern HUD" rather than a skeuomorphic gauge.
 */

import { tsColorToHex } from '../../dashboards/dashTypes';
import { roundRect, darkenColor } from '../drawUtils';
import type { Painter } from './types';

export const telemetryStatPainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getValueColor, getFontSpec } = pctx;

  const compact = height < 42 || height < width * 0.45;
  const cornerRadius = compact ? 2 : Math.min(6, width * 0.04, height * 0.08);
  const accentWidth = compact ? 0 : Math.max(3, width * 0.025);
  const padding = compact ? Math.max(4, width * 0.04) : Math.max(8, width * 0.06);

  // Panel background (flat, subtle vertical darken toward the bottom).
  const bgHex = tsColorToHex(config.back_color);
  const bgGradient = ctx.createLinearGradient(0, 0, 0, height);
  bgGradient.addColorStop(0, bgHex);
  bgGradient.addColorStop(1, darkenColor(bgHex, 6));
  ctx.fillStyle = bgGradient;
  roundRect(ctx, 0, 0, width, height, cornerRadius);
  ctx.fill();

  if (!compact) {
    // Accent stripe down the left edge.
    const accentHex = tsColorToHex(config.needle_color);
    ctx.save();
    roundRect(ctx, 0, 0, width, height, cornerRadius);
    ctx.clip();
    ctx.fillStyle = accentHex;
    ctx.fillRect(0, 0, accentWidth, height);
    ctx.restore();
  }

  const contentX = accentWidth + padding * (compact ? 0.4 : 0.6);
  const minDim = Math.min(width, height);
  const fontScale = 1 + (config.font_size_adjustment ?? 0) * 0.1;

  const valueColor = getValueColor();
  const valueHex = tsColorToHex(valueColor);
  const valueText = value.toFixed(config.value_digits);

  if (compact) {
    // Dense Grafana-style: label left, value right on one row.
    const labelSize = Math.max(7, minDim * 0.22 * fontScale);
    ctx.fillStyle = tsColorToHex(config.trim_color);
    ctx.font = getFontSpec(labelSize, { bold: true });
    ctx.textAlign = 'left';
    ctx.textBaseline = 'middle';
    ctx.fillText(config.title.toUpperCase(), contentX, height / 2);

    const valueSize = Math.max(9, minDim * 0.34 * fontScale);
    if (valueHex !== tsColorToHex(config.font_color)) {
      ctx.shadowColor = valueHex;
      ctx.shadowBlur = 6;
    }
    ctx.fillStyle = valueHex;
    ctx.font = getFontSpec(valueSize, { bold: true, monospace: true });
    ctx.textAlign = 'right';
    const valueLine = config.units ? `${valueText} ${config.units}` : valueText;
    ctx.fillText(valueLine, width - padding, height / 2);
    ctx.shadowColor = 'transparent';
    return;
  }

  // Uppercase label, letter-spaced.
  const labelSize = Math.max(9, minDim * 0.13 * fontScale);
  ctx.fillStyle = tsColorToHex(config.trim_color);
  ctx.font = getFontSpec(labelSize, { bold: true });
  ctx.textAlign = 'left';
  ctx.textBaseline = 'top';
  drawLetterSpaced(ctx, config.title.toUpperCase(), contentX, padding * 0.55, Math.max(1, labelSize * 0.12));

  // Big value + units on the baseline.
  const valueSize = Math.max(16, minDim * 0.40 * fontScale);
  const isAlert = valueHex !== tsColorToHex(config.font_color);
  if (isAlert) {
    ctx.shadowColor = valueHex;
    ctx.shadowBlur = 10;
  }
  ctx.fillStyle = valueHex;
  ctx.font = getFontSpec(valueSize, { bold: true, monospace: true });
  ctx.textAlign = 'left';
  ctx.textBaseline = 'alphabetic';
  const valueBaselineY = height * 0.68;
  ctx.fillText(valueText, contentX, valueBaselineY);
  ctx.shadowColor = 'transparent';

  if (config.units) {
    const unitsSize = Math.max(8, minDim * 0.12 * fontScale);
    const valueWidth = ctx.measureText(valueText).width;
    ctx.fillStyle = tsColorToHex(config.trim_color);
    ctx.font = getFontSpec(unitsSize, { bold: true });
    ctx.textAlign = 'left';
    ctx.textBaseline = 'alphabetic';
    ctx.fillText(config.units, contentX + valueWidth + unitsSize * 0.4, valueBaselineY);
  }

  // Thin range bar along the bottom, with warning/critical tick marks.
  const barHeight = Math.max(3, height * 0.06);
  const barY = height - padding * 0.5 - barHeight;
  const barX = contentX;
  const barW = width - contentX - padding * 0.5;
  if (barW > 4) {
    ctx.fillStyle = 'rgba(255, 255, 255, 0.08)';
    roundRect(ctx, barX, barY, barW, barHeight, barHeight / 2);
    ctx.fill();

    const range = config.max - config.min;
    const pct = range !== 0 ? Math.max(0, Math.min(1, (value - config.min) / range)) : 0;
    if (pct > 0) {
      ctx.fillStyle = valueHex;
      roundRect(ctx, barX, barY, Math.max(barHeight, barW * pct), barHeight, barHeight / 2);
      ctx.fill();
    }

    const tick = (v: number | null | undefined, color: string) => {
      if (v == null || range === 0) return;
      const t = Math.max(0, Math.min(1, (v - config.min) / range));
      const tx = barX + barW * t;
      ctx.fillStyle = color;
      ctx.fillRect(tx - 0.5, barY - 2, 1, barHeight + 4);
    };
    tick(config.low_warning, tsColorToHex(config.warn_color));
    tick(config.high_warning, tsColorToHex(config.warn_color));
    tick(config.low_critical, tsColorToHex(config.critical_color));
    tick(config.high_critical, tsColorToHex(config.critical_color));
  }
};

/** Draw text with manual letter-spacing (canvas text APIs have no native support). */
function drawLetterSpaced(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  spacing: number,
): void {
  let cursorX = x;
  for (const char of text) {
    ctx.fillText(char, cursorX, y);
    cursorX += ctx.measureText(char).width + spacing;
  }
}
