/**
 * Pure canvas-drawing helpers shared by gauge painters.
 *
 * These are intentionally free of React, store subscriptions, and
 * gauge-config knowledge so they can be unit-tested and reused by any
 * future per-painter module without dragging in component state.
 */

import type { TsColor } from '../dashboards/dashTypes';
import { tsColorToHex } from '../dashboards/dashTypes';

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
