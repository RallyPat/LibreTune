/** AsymmetricSweepGauge — curved sweep gauge with glowing tip and warning zones. */

import { tsColorToHex } from '../../dashboards/dashTypes';
import { lightenColor, darkenColor } from '../drawUtils';
import type { Painter } from './types';

export const sweepGaugePainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getValueColor, getFontSpec } = pctx;

  // Allow configurable pivot offset (for TunerStudio compatibility), default to center
  const pivotOffsetX = config.needle_pivot_offset_x ?? 0;
  const pivotOffsetY = config.needle_pivot_offset_y ?? 0;
  const centerX = width / 2 + pivotOffsetX;
  const centerY = height * 0.58 + pivotOffsetY;
  const radius = Math.min(width, height * 1.15) / 2 - 8;
  const arcWidth = Math.max(16, radius * 0.18);

  // Background with gradient
  const bgGradient = ctx.createLinearGradient(0, 0, 0, height);
  const bgHex = tsColorToHex(config.back_color);
  bgGradient.addColorStop(0, lightenColor(bgHex, 8));
  bgGradient.addColorStop(1, darkenColor(bgHex, 12));
  ctx.fillStyle = bgGradient;
  ctx.fillRect(0, 0, width, height);

  // Calculate angles - use actual values, only fallback if truly undefined
  const startDeg = config.sweep_begin_degree ?? config.start_angle ?? 210;
  const sweepDeg = config.sweep_angle ?? 120;
  const ccw = config.counter_clockwise ?? false;

  const startAngle = startDeg * Math.PI / 180;
  const sweepAngle = sweepDeg * Math.PI / 180;
  const endAngle = ccw ? startAngle - sweepAngle : startAngle + sweepAngle;

  // Helper to calculate angle at a given percentage
  const angleAt = (percent: number) => ccw
    ? startAngle - percent * sweepAngle
    : startAngle + percent * sweepAngle;

  // Arc track background with inset effect
  ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
  ctx.shadowBlur = 4;
  ctx.shadowOffsetX = 2;
  ctx.shadowOffsetY = 2;

  const trackGradient = ctx.createLinearGradient(0, centerY - radius, 0, centerY + radius);
  trackGradient.addColorStop(0, '#252525');
  trackGradient.addColorStop(0.5, '#404040');
  trackGradient.addColorStop(1, '#303030');
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius, startAngle, endAngle, ccw);
  ctx.strokeStyle = trackGradient;
  ctx.lineWidth = arcWidth;
  ctx.lineCap = 'round';
  ctx.stroke();
  ctx.shadowColor = 'transparent';

  // Warning/critical zones
  if (config.high_warning !== null) {
    const warnPercent = (config.high_warning - config.min) / (config.max - config.min);
    const warnAngle = angleAt(warnPercent);
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius, warnAngle, endAngle, ccw);
    ctx.strokeStyle = tsColorToHex(config.warn_color);
    ctx.lineWidth = arcWidth - 4;
    ctx.lineCap = 'butt';
    ctx.stroke();
  }

  if (config.high_critical !== null) {
    const critPercent = (config.high_critical - config.min) / (config.max - config.min);
    const critAngle = angleAt(critPercent);
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius, critAngle, endAngle, ccw);
    ctx.strokeStyle = tsColorToHex(config.critical_color);
    ctx.lineWidth = arcWidth - 4;
    ctx.lineCap = 'butt';
    ctx.stroke();
  }

  // Draw filled arc with gradient
  const valuePercent = Math.max(0, Math.min(1, (value - config.min) / (config.max - config.min)));
  const valueAngle = angleAt(valuePercent);
  const valueColor = getValueColor();
  const valueHex = tsColorToHex(valueColor);

  if (valuePercent > 0.01) {
    const fillGradient = ctx.createLinearGradient(
      centerX + Math.cos(startAngle) * radius,
      centerY + Math.sin(startAngle) * radius,
      centerX + Math.cos(valueAngle) * radius,
      centerY + Math.sin(valueAngle) * radius,
    );
    fillGradient.addColorStop(0, darkenColor(valueHex, 10));
    fillGradient.addColorStop(0.5, lightenColor(valueHex, 15));
    fillGradient.addColorStop(1, valueHex);

    ctx.beginPath();
    ctx.arc(centerX, centerY, radius, startAngle, valueAngle, ccw);
    ctx.strokeStyle = fillGradient;
    ctx.lineWidth = arcWidth - 4;
    ctx.lineCap = 'round';
    ctx.stroke();

    // Glow effect at tip
    ctx.shadowColor = valueHex;
    ctx.shadowBlur = 8;
    ctx.beginPath();
    ctx.arc(
      centerX + Math.cos(valueAngle) * radius,
      centerY + Math.sin(valueAngle) * radius,
      arcWidth / 4,
      0,
      Math.PI * 2,
    );
    ctx.fillStyle = lightenColor(valueHex, 20);
    ctx.fill();
    ctx.shadowColor = 'transparent';
  }

  // Tick marks
  const majorTicks = config.major_ticks > 0 ? config.major_ticks : (config.max - config.min) / 5;
  const numTicks = Math.floor((config.max - config.min) / majorTicks) + 1;
  const tickOuterRadius = radius + arcWidth / 2 + 4;
  const tickInnerRadius = radius + arcWidth / 2;

  ctx.strokeStyle = tsColorToHex(config.trim_color);
  ctx.fillStyle = tsColorToHex(config.font_color);
  ctx.font = getFontSpec(Math.max(8, radius * 0.1));
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';

  for (let i = 0; i < numTicks; i++) {
    const tickPercent = i / (numTicks - 1);
    const tickAngle = angleAt(tickPercent);

    ctx.beginPath();
    ctx.moveTo(centerX + Math.cos(tickAngle) * tickInnerRadius, centerY + Math.sin(tickAngle) * tickInnerRadius);
    ctx.lineTo(centerX + Math.cos(tickAngle) * tickOuterRadius, centerY + Math.sin(tickAngle) * tickOuterRadius);
    ctx.lineWidth = 2;
    ctx.stroke();

    // Labels at ends and middle only for small gauges
    const shouldLabel = radius > 60 || i === 0 || i === numTicks - 1;
    if (shouldLabel) {
      const labelRadius = tickOuterRadius + 10;
      const tickValue = config.min + i * majorTicks;
      ctx.fillText(
        tickValue.toFixed(config.label_digits),
        centerX + Math.cos(tickAngle) * labelRadius,
        centerY + Math.sin(tickAngle) * labelRadius,
      );
    }
  }

  const minorTicksPerMajor = config.minor_ticks > 0 ? config.minor_ticks : 0;
  if (minorTicksPerMajor > 0 && numTicks > 1) {
    const totalMinorTicks = (numTicks - 1) * minorTicksPerMajor;
    ctx.strokeStyle = tsColorToHex(config.trim_color);
    ctx.lineWidth = 1;
    for (let i = 0; i < totalMinorTicks; i++) {
      if (i % minorTicksPerMajor === 0) continue;
      const tickPercent = i / totalMinorTicks;
      const tickAngle = angleAt(tickPercent);
      const minorInner = tickInnerRadius + 2;
      const minorOuter = tickOuterRadius - 2;
      ctx.beginPath();
      ctx.moveTo(centerX + Math.cos(tickAngle) * minorInner, centerY + Math.sin(tickAngle) * minorInner);
      ctx.lineTo(centerX + Math.cos(tickAngle) * minorOuter, centerY + Math.sin(tickAngle) * minorOuter);
      ctx.stroke();
    }
  }

  // Value display in center with glow
  const fontHex = tsColorToHex(config.font_color);
  const valueFontSize = Math.max(18, radius * 0.28);
  ctx.shadowColor = valueHex;
  ctx.shadowBlur = 6;
  ctx.fillStyle = fontHex;
  ctx.font = getFontSpec(valueFontSize, { bold: true, monospace: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  const sweepValueY = config.display_value_at_180
    ? centerY + radius * 0.25
    : centerY;
  ctx.fillText(value.toFixed(config.value_digits), centerX, sweepValueY);
  ctx.shadowColor = 'transparent';

  // Title below value
  ctx.font = getFontSpec(Math.max(10, radius * 0.11), { bold: true });
  ctx.fillText(config.title, centerX, centerY + radius * 0.35);

  // Units
  ctx.fillStyle = tsColorToHex(config.trim_color);
  ctx.font = getFontSpec(Math.max(9, radius * 0.09));
  ctx.fillText(config.units, centerX, centerY + radius * 0.5);
};
