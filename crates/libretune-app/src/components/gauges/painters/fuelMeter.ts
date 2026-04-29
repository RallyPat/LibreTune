/** FuelMeter — stylized fuel-level gauge with E/F labels and pump icon. */

import { tsColorToHex } from '../../dashboards/dashTypes';
import { createMetallicGradient } from '../drawUtils';
import type { Painter } from './types';

export const fuelMeterPainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getValueColor, getFontSpec } = pctx;

  const padding = Math.min(width, height) * 0.1;
  const centerX = width / 2;
  const centerY = height / 2;
  const radius = Math.min(width, height) / 2 - padding;

  // Outer bezel
  ctx.shadowColor = 'rgba(0,0,0,0.4)';
  ctx.shadowBlur = 6;
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
  const bezelGradient = createMetallicGradient(ctx, centerX, centerY, 0, radius, config.trim_color);
  ctx.fillStyle = bezelGradient;
  ctx.fill();
  ctx.shadowColor = 'transparent';

  // Inner black background
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius * 0.88, 0, Math.PI * 2);
  ctx.fillStyle = '#0a0a0a';
  ctx.fill();

  // Draw fuel gauge arc (half circle, bottom portion)
  const arcStartAngle = Math.PI * 0.8;
  const arcSweep = Math.PI * 1.4;
  const arcRadius = radius * 0.7;

  // Background arc
  ctx.beginPath();
  ctx.arc(centerX, centerY, arcRadius, arcStartAngle, arcStartAngle + arcSweep);
  ctx.lineWidth = radius * 0.12;
  ctx.lineCap = 'round';
  ctx.strokeStyle = '#333333';
  ctx.stroke();

  // Filled arc based on value (0-100% as typical fuel gauge)
  const normalizedValue = (value - config.min) / (config.max - config.min);
  const fillAngle = arcStartAngle + arcSweep * normalizedValue;

  // Color gradient for fuel level
  const fuelGradient = ctx.createLinearGradient(
    centerX - arcRadius, centerY,
    centerX + arcRadius, centerY,
  );
  fuelGradient.addColorStop(0, tsColorToHex(config.critical_color)); // Empty = red
  fuelGradient.addColorStop(0.25, tsColorToHex(config.warn_color)); // Low = orange
  fuelGradient.addColorStop(0.5, tsColorToHex(config.trim_color)); // Normal
  fuelGradient.addColorStop(1, tsColorToHex(config.trim_color)); // Full

  ctx.beginPath();
  ctx.arc(centerX, centerY, arcRadius, arcStartAngle, fillAngle);
  ctx.lineWidth = radius * 0.12;
  ctx.lineCap = 'round';
  ctx.strokeStyle = fuelGradient;
  ctx.stroke();

  // E and F labels
  const labelRadius = radius * 0.55;
  const eAngle = arcStartAngle + Math.PI * 0.05;
  const fAngle = arcStartAngle + arcSweep - Math.PI * 0.05;

  ctx.fillStyle = '#ffffff';
  ctx.font = getFontSpec(radius * 0.18, { bold: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';

  ctx.fillText('E', centerX + Math.cos(eAngle) * labelRadius, centerY + Math.sin(eAngle) * labelRadius);
  ctx.fillText('F', centerX + Math.cos(fAngle) * labelRadius, centerY + Math.sin(fAngle) * labelRadius);

  // Fuel pump icon (simple representation)
  const iconY = centerY - radius * 0.15;
  ctx.fillStyle = '#aaaaaa';
  ctx.font = getFontSpec(radius * 0.25);
  ctx.fillText('⛽', centerX, iconY);

  // Value below
  const valueTextColorTs = getValueColor();
  const fontSize = Math.max(10, radius * 0.2);
  ctx.fillStyle = tsColorToHex(valueTextColorTs);
  ctx.font = getFontSpec(fontSize, { bold: true, monospace: true });
  ctx.fillText(`${value.toFixed(config.value_digits)}${config.units || '%'}`, centerX, centerY + radius * 0.35);

  // Title
  if (config.title) {
    ctx.fillStyle = tsColorToHex(config.font_color);
    ctx.font = getFontSpec(fontSize * 0.5);
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, centerX, 4);
  }
};
