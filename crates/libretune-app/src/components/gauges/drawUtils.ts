/**
 * Pure canvas-drawing helpers shared by gauge painters.
 *
 * These are intentionally free of React, store subscriptions, and
 * gauge-config knowledge so they can be unit-tested and reused by any
 * future per-painter module without dragging in component state.
 */

import type { TsColor, TsGaugeConfig } from '../dashboards/dashTypes';
import { tsColorToHex } from '../dashboards/dashTypes';
import { isFontLoaded } from './assetCache';

/** Stroke/fill helper to define a rounded-rectangle path. */
export function roundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
): void {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + w - r, y);
  ctx.quadraticCurveTo(x + w, y, x + w, y + r);
  ctx.lineTo(x + w, y + h - r);
  ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
  ctx.lineTo(x + r, y + h);
  ctx.quadraticCurveTo(x, y + h, x, y + h - r);
  ctx.lineTo(x, y + r);
  ctx.quadraticCurveTo(x, y, x + r, y);
  ctx.closePath();
}

/** Lighten a #rrggbb hex color by `percent` (0-100). */
export function lightenColor(hex: string, percent: number): string {
  const num = parseInt(hex.replace('#', ''), 16);
  const amt = Math.round(2.55 * percent);
  const R = Math.min(255, (num >> 16) + amt);
  const G = Math.min(255, ((num >> 8) & 0x00ff) + amt);
  const B = Math.min(255, (num & 0x0000ff) + amt);
  return `#${(0x1000000 + R * 0x10000 + G * 0x100 + B).toString(16).slice(1)}`;
}

/** Darken a #rrggbb hex color by `percent` (0-100). */
export function darkenColor(hex: string, percent: number): string {
  const num = parseInt(hex.replace('#', ''), 16);
  const amt = Math.round(2.55 * percent);
  const R = Math.max(0, (num >> 16) - amt);
  const G = Math.max(0, ((num >> 8) & 0x00ff) - amt);
  const B = Math.max(0, (num & 0x0000ff) - amt);
  return `#${(0x1000000 + R * 0x10000 + G * 0x100 + B).toString(16).slice(1)}`;
}

/**
 * Build a radial gradient that gives a metallic-bezel look around a
 * circular gauge.
 */
export function createMetallicGradient(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  r1: number,
  r2: number,
  baseColor: TsColor,
): CanvasGradient {
  const gradient = ctx.createRadialGradient(x - r2 * 0.3, y - r2 * 0.3, r1, x, y, r2);
  const hex = tsColorToHex(baseColor);
  gradient.addColorStop(0, lightenColor(hex, 60));
  gradient.addColorStop(0.3, lightenColor(hex, 30));
  gradient.addColorStop(0.5, hex);
  gradient.addColorStop(0.7, darkenColor(hex, 20));
  gradient.addColorStop(1, darkenColor(hex, 40));
  return gradient;
}

/**
 * The gray inset gradient used for bar/track backgrounds. `endStop` varies
 * slightly between painters (`#303030` vs `#353535`) — pass it through so
 * consolidating the factory doesn't change any painter's appearance.
 */
export function insetTrackGradient(
  ctx: CanvasRenderingContext2D,
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  endStop = '#303030',
): CanvasGradient {
  const gradient = ctx.createLinearGradient(x0, y0, x1, y1);
  gradient.addColorStop(0, '#252525');
  gradient.addColorStop(0.5, '#404040');
  gradient.addColorStop(1, endStop);
  return gradient;
}

/**
 * Angle (radians) at `percent` (0-1) along a sweep starting at
 * `startAngle`, honoring counter-clockwise sweeps.
 */
export function angleAtPercent(
  startAngle: number,
  sweepAngle: number,
  ccw: boolean,
  percent: number,
): number {
  return ccw
    ? startAngle - percent * sweepAngle
    : startAngle + percent * sweepAngle;
}

/** Zone color for a filled/unfilled dashed-bar segment. */
export function segmentZoneColor(
  config: TsGaugeConfig,
  segmentValue: number,
  isFilled: boolean,
): string {
  if (config.high_critical !== null && segmentValue >= config.high_critical) {
    return isFilled ? tsColorToHex(config.critical_color) : '#401010';
  }
  if (config.high_warning !== null && segmentValue >= config.high_warning) {
    return isFilled ? tsColorToHex(config.warn_color) : '#403010';
  }
  // needle_color for the normal range (typically green)
  return isFilled ? tsColorToHex(config.needle_color) : '#303030';
}

/** Paint one dashed-bar segment: gradient fill + top-segment glow.
 *  `gradX0`/`gradX1` give the gradient span (across the bar for vertical
 *  bars, across the segment for horizontal ones). */
export function drawDashedSegment(
  ctx: CanvasRenderingContext2D,
  rect: { x: number; y: number; w: number; h: number },
  gradX0: number,
  gradX1: number,
  color: string,
  isFilled: boolean,
  isTopSegment: boolean,
): void {
  if (isFilled) {
    const segGradient = ctx.createLinearGradient(gradX0, 0, gradX1, 0);
    segGradient.addColorStop(0, darkenColor(color, 15));
    segGradient.addColorStop(0.3, lightenColor(color, 20));
    segGradient.addColorStop(0.7, lightenColor(color, 15));
    segGradient.addColorStop(1, darkenColor(color, 10));
    ctx.fillStyle = segGradient;
    if (isTopSegment) {
      ctx.shadowColor = color;
      ctx.shadowBlur = 6;
    }
  } else {
    ctx.fillStyle = color;
  }
  roundRect(ctx, rect.x, rect.y, rect.w, rect.h, 2);
  ctx.fill();
  ctx.shadowColor = 'transparent';
}

/**
 * Zone color for a round gauge's segment/ring marks. Honors the gauge's
 * configured high_warning/high_critical thresholds, falling back to the
 * top-10%/top-25% heuristic when the INI/dashboard provides none.
 */
export function roundZoneColor(config: TsGaugeConfig, segmentValue: number): string {
  const range = config.max - config.min;
  const criticalAt = config.high_critical ?? config.max - range * 0.1;
  const warnAt = config.high_warning ?? config.max - range * 0.25;
  if (segmentValue >= criticalAt) return tsColorToHex(config.critical_color);
  if (segmentValue >= warnAt) return tsColorToHex(config.warn_color);
  return tsColorToHex(config.trim_color);
}

/**
 * Resolve a configured font name to a CSS font stack with web-safe
 * fallbacks. Embedded fonts (loaded via assetCache) win first, then the
 * well-known stack table, then the raw name with fallbacks appended.
 */
export function getFontStack(customFont: string | undefined, preferMonospace = false): string {
  const webSafeStacks: Record<string, string> = {
    'Arial': 'Arial, Helvetica, sans-serif',
    'Arial Black': '"Arial Black", Gadget, sans-serif',
    'Verdana': 'Verdana, Geneva, sans-serif',
    'Tahoma': 'Tahoma, Geneva, sans-serif',
    'Trebuchet MS': '"Trebuchet MS", Helvetica, sans-serif',
    'Georgia': 'Georgia, serif',
    'Times New Roman': '"Times New Roman", Times, serif',
    'Courier New': '"Courier New", Courier, monospace',
    'Consolas': 'Consolas, Monaco, "Lucida Console", monospace',
    'Monaco': 'Monaco, Consolas, monospace',
  };

  const defaultStack = preferMonospace
    ? '"Courier New", Consolas, Monaco, monospace'
    : 'Arial, Helvetica, sans-serif';

  if (!customFont) {
    return defaultStack;
  }
  if (webSafeStacks[customFont]) {
    return webSafeStacks[customFont];
  }
  if (isFontLoaded(customFont)) {
    return preferMonospace
      ? `"${customFont}", "Courier New", monospace`
      : `"${customFont}", Arial, sans-serif`;
  }
  return preferMonospace
    ? `"${customFont}", "Courier New", Consolas, monospace`
    : `"${customFont}", Arial, Helvetica, sans-serif`;
}
