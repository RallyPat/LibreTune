/**
 * BasicReadout painter — LCD-style digital numeric display.
 *
 * Migrated from the inline `drawBasicReadout` closure in
 * `TsGauge.tsx` as the first proof-of-concept for the per-painter
 * module pattern. Behavior is byte-for-byte identical to the original
 * closure.
 */

import { tsColorToRgba, tsColorToHex } from '../../dashboards/dashTypes';
import { roundRect, lightenColor, darkenColor } from '../drawUtils';
import type { Painter } from './types';

export const basicReadoutPainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, legacyMode, bgImage, getValueColor, getFontSpec } = pctx;

  const padding = 6;
  const innerWidth = width - padding * 2;
  const innerHeight = height - padding * 2;
  const cornerRadius = Math.min(8, width * 0.05);

  // Use smaller dimension for balanced font scaling (prevents text clipping)
  const minDim = Math.min(width, height);
  // Apply font_size_adjustment as a multiplier (typically -2 to +2, we scale by ~10% per unit)
  const fontScale = 1 + (config.font_size_adjustment ?? 0) * 0.1;

  const useLegacyBackground = legacyMode && !!bgImage;
  if (useLegacyBackground && bgImage) {
    ctx.drawImage(bgImage, 0, 0, width, height);
  } else {
    // Outer frame with gradient (metallic look)
    const frameGradient = ctx.createLinearGradient(0, 0, width, height);
    frameGradient.addColorStop(0, '#555555');
    frameGradient.addColorStop(0.5, '#333333');
    frameGradient.addColorStop(1, '#222222');
    ctx.fillStyle = frameGradient;
    roundRect(ctx, 0, 0, width, height, cornerRadius);
    ctx.fill();

    // Inner LCD panel with subtle inset effect
    const innerX = padding - 2;
    const innerY = padding - 2;
    const innerW = innerWidth + 4;
    const innerH = innerHeight + 4;

    // Inset shadow
    ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
    ctx.shadowBlur = 4;
    ctx.shadowOffsetX = 2;
    ctx.shadowOffsetY = 2;
    ctx.fillStyle = tsColorToRgba(config.back_color);
    roundRect(ctx, innerX, innerY, innerW, innerH, cornerRadius - 2);
    ctx.fill();
    ctx.shadowColor = 'transparent';

    // LCD background with slight gradient for depth
    const lcdGradient = ctx.createLinearGradient(padding, padding, padding, height - padding);
    const bgHex = tsColorToHex(config.back_color);
    lcdGradient.addColorStop(0, lightenColor(bgHex, 5));
    lcdGradient.addColorStop(1, darkenColor(bgHex, 10));
    ctx.fillStyle = lcdGradient;
    roundRect(ctx, padding, padding, innerWidth, innerHeight, cornerRadius - 2);
    ctx.fill();
  }

  // Calculate font sizes based on the smaller dimension for balanced scaling
  const titleFontSize = Math.max(9, minDim * 0.12 * fontScale);
  const valueFontSize = Math.max(14, minDim * 0.35 * fontScale);
  const unitsFontSize = Math.max(8, minDim * 0.10 * fontScale);

  // Title at top
  ctx.fillStyle = tsColorToRgba(config.trim_color);
  ctx.font = getFontSpec(titleFontSize);
  ctx.textAlign = 'center';
  ctx.textBaseline = 'top';
  ctx.fillText(config.title, width / 2, padding + 2);

  // Value in center (large, LCD-style)
  const valueColor = getValueColor();
  const valueText = value.toFixed(config.value_digits);

  // Value glow effect for active values
  if (valueColor !== config.font_color) {
    ctx.shadowColor = tsColorToRgba(valueColor);
    ctx.shadowBlur = 8;
  }

  ctx.fillStyle = tsColorToRgba(valueColor);
  // Use monospace or LCD-style font for values
  ctx.font = getFontSpec(valueFontSize, { bold: true, monospace: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(valueText, width / 2, height / 2 + titleFontSize * 0.3);
  ctx.shadowColor = 'transparent';

  // Units at bottom
  ctx.fillStyle = tsColorToRgba(config.trim_color);
  ctx.font = getFontSpec(unitsFontSize);
  ctx.textAlign = 'center';
  ctx.textBaseline = 'bottom';
  ctx.fillText(config.units, width / 2, height - padding - 2);
};
