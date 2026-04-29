/** AnalogGauge — classic circular dial with metallic bezel, ticks, gradient needle, center cap. */

import { tsColorToHex, tsColorToRgba } from '../../dashboards/dashTypes';
import { roundRect, lightenColor, darkenColor, createMetallicGradient } from '../drawUtils';
import type { Painter } from './types';

export const analogGaugePainter: Painter = (pctx) => {
  const { ctx, width, height, value, peakValue, config, bgImage, needleImage, getValueColor: _gv, getFontSpec } = pctx;
  void _gv; // unused — analog gauge uses face color from config

  // Enforce perfect circle: use the smaller of width/height, center in canvas
  const size = Math.min(width, height);
  const pivotOffsetX = 0;
  const pivotOffsetY = 0;
  const centerX = width / 2 + pivotOffsetX;
  const centerY = height / 2 + pivotOffsetY;
  const radius = size / 2 - 8;

  // Background - use image if available, otherwise use color
  if (bgImage) {
    // Center the image in the square area
    ctx.drawImage(bgImage, centerX - size / 2, centerY - size / 2, size, size);
  } else {
    ctx.fillStyle = tsColorToRgba(config.back_color);
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
    ctx.fill();
  }

  // Outer shadow
  ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
  ctx.shadowBlur = 8;
  ctx.shadowOffsetX = 3;
  ctx.shadowOffsetY = 3;
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
  ctx.fillStyle = '#333';
  ctx.fill();
  ctx.shadowColor = 'transparent';

  // Metallic bezel - outer ring
  const bezelWidth = config.border_width > 0
    ? Math.min(radius * 0.3, config.border_width)
    : Math.max(6, radius * 0.08);
  const bezelGradient = createMetallicGradient(ctx, centerX, centerY, radius + 2, radius - bezelWidth, config.trim_color);
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
  ctx.arc(centerX, centerY, radius - bezelWidth, 0, Math.PI * 2, true);
  ctx.fillStyle = bezelGradient;
  ctx.fill();

  // Inner bezel highlight
  ctx.beginPath();
  ctx.arc(centerX, centerY, radius - bezelWidth + 1, 0, Math.PI * 2);
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.15)';
  ctx.lineWidth = 1;
  ctx.stroke();

  // Face background with subtle radial gradient
  const faceRadius = radius - bezelWidth - 2;
  const faceGradient = ctx.createRadialGradient(
    centerX - faceRadius * 0.3, centerY - faceRadius * 0.3, 0,
    centerX, centerY, faceRadius,
  );
  const backHex = tsColorToHex(config.back_color);
  faceGradient.addColorStop(0, lightenColor(backHex, 15));
  faceGradient.addColorStop(0.7, backHex);
  faceGradient.addColorStop(1, darkenColor(backHex, 10));
  ctx.beginPath();
  ctx.arc(centerX, centerY, faceRadius, 0, Math.PI * 2);
  ctx.fillStyle = faceGradient;
  ctx.fill();

  // Calculate angles (TS uses degrees, canvas uses radians)
  const startDeg = config.sweep_begin_degree ?? config.start_angle ?? 225;
  const sweepDeg = config.sweep_angle ?? 270;
  const ccw = config.counter_clockwise ?? false;

  const startAngle = startDeg * Math.PI / 180;
  const sweepAngle = sweepDeg * Math.PI / 180;
  const endAngle = ccw ? startAngle - sweepAngle : startAngle + sweepAngle;

  // Helper to calculate angle at a given percentage (0-1) along the sweep
  const angleAt = (percent: number) => ccw
    ? startAngle - percent * sweepAngle
    : startAngle + percent * sweepAngle;

  // Draw warning/critical zones as arcs (behind tick marks)
  const zoneRadius = faceRadius - 4;
  const zoneWidth = Math.max(4, faceRadius * 0.06);

  if (config.high_warning !== null) {
    const warnStartPercent = (config.high_warning - config.min) / (config.max - config.min);
    const warnStartAngle = angleAt(warnStartPercent);
    ctx.beginPath();
    ctx.arc(centerX, centerY, zoneRadius, warnStartAngle, endAngle, ccw);
    const warnHex = tsColorToHex(config.warn_color);
    ctx.strokeStyle = warnHex;
    ctx.lineWidth = zoneWidth;
    ctx.lineCap = 'butt';
    ctx.stroke();
  }

  if (config.high_critical !== null) {
    const critStartPercent = (config.high_critical - config.min) / (config.max - config.min);
    const critStartAngle = angleAt(critStartPercent);
    ctx.beginPath();
    ctx.arc(centerX, centerY, zoneRadius, critStartAngle, endAngle, ccw);
    const critHex = tsColorToHex(config.critical_color);
    ctx.strokeStyle = critHex;
    ctx.lineWidth = zoneWidth;
    ctx.lineCap = 'butt';
    ctx.stroke();
  }

  // Draw tick marks
  const tickRadius = faceRadius - Math.max(10, faceRadius * 0.12);
  const majorTicks = config.major_ticks > 0 ? config.major_ticks : (config.max - config.min) / 10;
  const numMajorTicks = Math.floor((config.max - config.min) / majorTicks) + 1;
  const minorTicksPerMajor = config.minor_ticks > 0 ? config.minor_ticks : 0;

  const cullLabels = radius < 70;
  const trimHex = tsColorToHex(config.trim_color);
  const fontHex = tsColorToHex(config.font_color);

  // Minor ticks
  ctx.strokeStyle = darkenColor(trimHex, 30);
  ctx.lineWidth = 1;
  if (minorTicksPerMajor > 0) {
    const totalMinorTicks = (numMajorTicks - 1) * minorTicksPerMajor;
    for (let i = 0; i < totalMinorTicks; i++) {
      if (i % minorTicksPerMajor === 0) continue;
      const tickPercent = i / totalMinorTicks;
      const tickAngle = angleAt(tickPercent);
      const innerRadius = tickRadius - (faceRadius * 0.05);
      ctx.beginPath();
      ctx.moveTo(centerX + Math.cos(tickAngle) * innerRadius, centerY + Math.sin(tickAngle) * innerRadius);
      ctx.lineTo(centerX + Math.cos(tickAngle) * tickRadius, centerY + Math.sin(tickAngle) * tickRadius);
      ctx.stroke();
    }
  }

  // Major ticks and labels
  ctx.strokeStyle = trimHex;
  ctx.fillStyle = fontHex;
  const fontSize = Math.max(8, faceRadius * 0.14);
  ctx.font = getFontSpec(fontSize);
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';

  for (let i = 0; i < numMajorTicks; i++) {
    const tickValue = config.min + i * majorTicks;
    const tickPercent = (tickValue - config.min) / (config.max - config.min);
    const tickAngle = angleAt(tickPercent);

    const innerRadius = tickRadius - (faceRadius * 0.10);
    ctx.beginPath();
    ctx.moveTo(centerX + Math.cos(tickAngle) * innerRadius, centerY + Math.sin(tickAngle) * innerRadius);
    ctx.lineTo(centerX + Math.cos(tickAngle) * tickRadius, centerY + Math.sin(tickAngle) * tickRadius);
    ctx.lineWidth = 2;
    ctx.stroke();

    const shouldDrawLabel = !cullLabels || (i === 0 || i === numMajorTicks - 1);
    if (shouldDrawLabel) {
      const labelRadius = tickRadius - (faceRadius * 0.22);
      ctx.fillText(
        tickValue.toFixed(config.label_digits),
        centerX + Math.cos(tickAngle) * labelRadius,
        centerY + Math.sin(tickAngle) * labelRadius,
      );
    }
  }

  // Draw needle with shadow
  const valuePercent = (value - config.min) / (config.max - config.min);
  const needleAngle = angleAt(valuePercent);
  // Use config.needle_length if present, otherwise use a visually correct default
  let needleLength: number;
  if (typeof config.needle_length === 'number' && config.needle_length > 0 && config.needle_length <= 1.5) {
    // If needle_length is a fraction (<=1.5), treat as percent of faceRadius
    needleLength = faceRadius * config.needle_length;
  } else if (typeof config.needle_length === 'number' && config.needle_length > 1.5) {
    // If needle_length is a pixel value
    needleLength = Math.min(faceRadius, config.needle_length);
  } else {
    // Default: 35% of faceRadius (shrink by 50%)
    needleLength = faceRadius * 0.35;
  }
  const needleWidth = Math.max(3, faceRadius * 0.04);

  ctx.save();
  ctx.translate(centerX, centerY);
  ctx.rotate(needleAngle);

  // Needle shadow
  ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
  ctx.shadowBlur = 4;
  ctx.shadowOffsetX = 2;
  ctx.shadowOffsetY = 2;

  if (needleImage) {
    // Draw custom needle image - center image at pivot, allow for config offset
    const imgWidth = needleImage.width;
    const imgHeight = needleImage.height;
    const scale = needleLength / imgWidth;
    // Optionally allow config offsets for image alignment
    const imgOffsetX = config.needle_image_offset_x ?? 0;
    const imgOffsetY = config.needle_image_offset_y ?? 0;
    ctx.drawImage(
      needleImage,
      -imgWidth * scale / 2 + imgOffsetX,
      -imgHeight * scale / 2 + imgOffsetY,
      imgWidth * scale,
      imgHeight * scale,
    );
  } else {
    // Needle body with gradient, symmetric about pivot
    const needleGradient = ctx.createLinearGradient(0, -needleWidth, 0, needleWidth);
    const needleHex = tsColorToHex(config.needle_color);
    needleGradient.addColorStop(0, lightenColor(needleHex, 30));
    needleGradient.addColorStop(0.5, needleHex);
    needleGradient.addColorStop(1, darkenColor(needleHex, 20));

    ctx.beginPath();
    // Needle base at pivot (0,0), symmetric left/right
    ctx.moveTo(-needleLength * 0.08, -needleWidth);
    ctx.lineTo(needleLength, 0);
    ctx.lineTo(-needleLength * 0.08, needleWidth);
    ctx.closePath();
    ctx.fillStyle = needleGradient;
    ctx.fill();
  }
  ctx.shadowColor = 'transparent';

  // Needle center cap with metallic finish
  const capRadius = Math.max(6, faceRadius * 0.1);
  const capGradient = ctx.createRadialGradient(-capRadius * 0.3, -capRadius * 0.3, 0, 0, 0, capRadius);
  capGradient.addColorStop(0, '#aaaaaa');
  capGradient.addColorStop(0.5, '#666666');
  capGradient.addColorStop(1, '#444444');
  ctx.beginPath();
  ctx.arc(0, 0, capRadius, 0, Math.PI * 2);
  ctx.fillStyle = capGradient;
  ctx.fill();
  ctx.strokeStyle = '#333';
  ctx.lineWidth = 1;
  ctx.stroke();

  ctx.restore();

  // Optional peak-hold marker (Plan D-5).
  // Renders as a small wedge on the dial face at the gauge's all-time-high
  // position. Uses the warning color if the peak crossed a warn/critical
  // threshold, else the trim color.
  if (config.peak_hold && peakValue > config.min) {
    const peakClamped = Math.max(config.min, Math.min(config.max, peakValue));
    const peakPct = (peakClamped - config.min) / (config.max - config.min);
    const peakAngle = angleAt(peakPct);
    const inner = faceRadius * 0.78;
    const outer = faceRadius * 0.94;
    const peakColor =
      (config.high_critical != null && peakClamped >= config.high_critical) ||
      (config.low_critical != null && peakClamped <= config.low_critical)
        ? tsColorToHex(config.critical_color)
        : (config.high_warning != null && peakClamped >= config.high_warning) ||
          (config.low_warning != null && peakClamped <= config.low_warning)
          ? tsColorToHex(config.warn_color)
          : tsColorToHex(config.trim_color);
    ctx.save();
    ctx.translate(centerX, centerY);
    ctx.rotate(peakAngle);
    ctx.beginPath();
    ctx.moveTo(inner, 0);
    ctx.lineTo(outer, -outer * 0.04);
    ctx.lineTo(outer, outer * 0.04);
    ctx.closePath();
    ctx.fillStyle = peakColor;
    ctx.fill();
    ctx.restore();
  }

  // Title with shadow (move up to avoid overlap)
  ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
  ctx.shadowBlur = 2;
  ctx.fillStyle = fontHex;
  ctx.font = getFontSpec(Math.max(9, faceRadius * 0.13), { bold: true });
  ctx.textAlign = 'center';
  ctx.textBaseline = 'top';
  ctx.fillText(config.title, centerX, centerY + faceRadius * 0.25);
  ctx.shadowColor = 'transparent';

  // Value display with background (move down to avoid overlap)
  const valueFontSize = Math.max(11, faceRadius * 0.16);
  const valueText = `${value.toFixed(config.value_digits)} ${config.units}`;
  ctx.font = getFontSpec(valueFontSize, { bold: true, monospace: true });
  const valueWidth = ctx.measureText(valueText).width;
  const valueY = centerY + faceRadius * 0.55;
  ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
  roundRect(ctx, centerX - valueWidth / 2 - 4, valueY - valueFontSize / 2 - 2, valueWidth + 8, valueFontSize + 4, 3);
  ctx.fill();
  ctx.fillStyle = fontHex;
  ctx.textBaseline = 'middle';
  ctx.fillText(valueText, centerX, valueY);
};
