/** HorizontalLineGauge — horizontal line with a glowing position dot. */

import { tsColorToHex } from '../../dashboards/dashTypes';
import { lightenColor, darkenColor } from '../drawUtils';
import type { Painter } from './types';

export const horizontalLinePainter: Painter = (pctx) => {
  const { ctx, width, height, value, config, getValueColor, getFontSpec } = pctx;

  const padding = 8;
  const lineY = height * 0.55;
  const lineWidth = width - padding * 2;

  // Background with gradient
  const bgGradient = ctx.createLinearGradient(0, 0, 0, height);
  const bgHex = tsColorToHex(config.back_color);
  bgGradient.addColorStop(0, lightenColor(bgHex, 8));
  bgGradient.addColorStop(1, darkenColor(bgHex, 8));
  ctx.fillStyle = bgGradient;
  ctx.fillRect(0, 0, width, height);

  // Title
  ctx.fillStyle = tsColorToHex(config.trim_color);
  ctx.font = getFontSpec(Math.max(9, height * 0.16), { bold: true });
  ctx.textAlign = 'left';
  ctx.textBaseline = 'top';
  ctx.fillText(config.title, padding, 3);

  // Track with inset effect
  ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
  ctx.shadowBlur = 3;
  ctx.shadowOffsetY = 1;
  const trackGradient = ctx.createLinearGradient(0, lineY - 4, 0, lineY + 4);
  trackGradient.addColorStop(0, '#252525');
  trackGradient.addColorStop(0.5, '#404040');
  trackGradient.addColorStop(1, '#353535');
  ctx.strokeStyle = trackGradient;
  ctx.lineWidth = 6;
  ctx.lineCap = 'round';
  ctx.beginPath();
  ctx.moveTo(padding, lineY);
  ctx.lineTo(padding + lineWidth, lineY);
  ctx.stroke();
  ctx.shadowColor = 'transparent';

  // Filled portion
  const fillPercent = Math.max(0, Math.min(1, (value - config.min) / (config.max - config.min)));
  const fillLength = lineWidth * fillPercent;
  const valueColor = getValueColor();
  const valueHex = tsColorToHex(valueColor);

  if (fillLength > 0) {
    const fillGradient = ctx.createLinearGradient(padding, 0, padding + fillLength, 0);
    fillGradient.addColorStop(0, darkenColor(valueHex, 10));
    fillGradient.addColorStop(1, lightenColor(valueHex, 10));
    ctx.strokeStyle = fillGradient;
    ctx.lineWidth = 4;
    ctx.beginPath();
    ctx.moveTo(padding, lineY);
    ctx.lineTo(padding + fillLength, lineY);
    ctx.stroke();
  }

  // Position indicator with glow
  const indicatorX = padding + fillLength;
  ctx.shadowColor = valueHex;
  ctx.shadowBlur = 8;
  const indicatorGradient = ctx.createRadialGradient(indicatorX - 2, lineY - 2, 0, indicatorX, lineY, 8);
  indicatorGradient.addColorStop(0, lightenColor(valueHex, 40));
  indicatorGradient.addColorStop(0.5, valueHex);
  indicatorGradient.addColorStop(1, darkenColor(valueHex, 20));
  ctx.fillStyle = indicatorGradient;
  ctx.beginPath();
  ctx.arc(indicatorX, lineY, 7, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.3)';
  ctx.lineWidth = 1;
  ctx.stroke();
  ctx.shadowColor = 'transparent';

  // Value display
  ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
  ctx.shadowBlur = 2;
  ctx.fillStyle = tsColorToHex(config.font_color);
  ctx.font = getFontSpec(Math.max(11, height * 0.22), { bold: true, monospace: true });
  ctx.textAlign = 'right';
  ctx.textBaseline = 'top';
  ctx.fillText(`${value.toFixed(config.value_digits)} ${config.units}`, width - padding, 3);
  ctx.shadowColor = 'transparent';

  // Min/max labels
  ctx.fillStyle = tsColorToHex(config.trim_color);
  ctx.font = getFontSpec(Math.max(8, height * 0.12));
  ctx.textAlign = 'left';
  ctx.textBaseline = 'top';
  ctx.fillText(config.min.toFixed(0), padding, lineY + 8);
  ctx.textAlign = 'right';
  ctx.fillText(config.max.toFixed(0), width - padding, lineY + 8);
};
