/**
 * TS Gauge Renderer
 * 
 * Renders gauges based on TS GaugePainter types.
 * Uses canvas for all gauge rendering with high-quality visual effects.
 * Wrapped in React.memo with custom comparator for performance optimization.
 */

import React, { useEffect, useState, useCallback } from 'react';
import { TsGaugeConfig, TsColor, tsColorToRgba, tsColorToHex } from '../dashboards/dashTypes';
import { getChannelHistoryBuffer } from '../../stores/realtimeStore';
import {
  roundRect,
  lightenColor,
  darkenColor,
  createMetallicGradient,
} from './drawUtils';
import {
  getEmbeddedImage as getCachedEmbeddedImage,
  isFontLoaded,
  loadEmbeddedAssets,
} from './assetCache';
import { useGaugeRenderer } from './useGaugeRenderer';
import {
  ensurePaintersRegistered,
  painterRegistry,
  type PainterContext,
} from './painters';

ensurePaintersRegistered();

interface TsGaugeProps {
  config: TsGaugeConfig;
  value: number;
  embeddedImages?: Map<string, string>;
  legacyMode?: boolean;
  /** When true, the value prop takes priority over the store subscription (sweep/demo mode) */
  overrideStore?: boolean;
}

/**
 * Internal TsGauge component - wrapped in React.memo below
 */
function TsGaugeInner({ config, value, embeddedImages, legacyMode = false, overrideStore = false }: TsGaugeProps) {
  const [fontsReady, setFontsReady] = useState(false);
  const [imagesReady, setImagesReady] = useState(false);

  // Load embedded fonts and images
  useEffect(() => {
    if (!embeddedImages) {
      setFontsReady(true);
      setImagesReady(true);
      return;
    }

    let cancelled = false;
    loadEmbeddedAssets(embeddedImages).then(() => {
      if (cancelled) return;
      setFontsReady(true);
      setImagesReady(true);
    });
    return () => {
      cancelled = true;
    };
  }, [embeddedImages]);

  /** Get a loaded image by name or id */
  const getEmbeddedImage = useCallback(
    (name: string | null | undefined): HTMLImageElement | null => getCachedEmbeddedImage(name),
    [],
  );
  
  /** 
   * Get font family with web-safe fallbacks.
   * If the configured font is an embedded font ID, it will be used first,
   * followed by similar web-safe alternatives.
   */
  const getFontFamily = useCallback((preferMonospace = false): string => {
    const customFont = config.font_family;
    
    // Map common font names to web-safe stacks
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
    
    // Check if it's a well-known font with a web-safe stack
    if (webSafeStacks[customFont]) {
      return webSafeStacks[customFont];
    }
    
    // If it's an embedded font (should be loaded), use it with fallbacks
    if (isFontLoaded(customFont)) {
      return preferMonospace 
        ? `"${customFont}", "Courier New", monospace`
        : `"${customFont}", Arial, sans-serif`;
    }
    
    // Unknown font - try it but add fallbacks
    return preferMonospace 
      ? `"${customFont}", "Courier New", Consolas, monospace`
      : `"${customFont}", Arial, Helvetica, sans-serif`;
  }, [config.font_family]);

  const getFontSpec = useCallback((
    size: number,
    options?: { bold?: boolean; monospace?: boolean }
  ): string => {
    const italic = config.italic_font ? 'italic ' : '';
    const bold = options?.bold ? 'bold ' : '';
    const monospace = options?.monospace ?? false;
    const adjustedSize = Math.max(1, size + (config.font_size_adjustment ?? 0));
    return `${italic}${bold}${adjustedSize}px ${getFontFamily(monospace)}`;
  }, [config.font_size_adjustment, config.italic_font, getFontFamily]);

  /** Get color based on value thresholds */
  const getValueColor = useCallback((): TsColor => {
    // Use != null to catch both null and undefined
    if (config.high_critical != null && displayValueRef.current >= config.high_critical) {
      return config.critical_color;
    }
    if (config.low_critical != null && displayValueRef.current <= config.low_critical) {
      return config.critical_color;
    }
    if (config.high_warning != null && displayValueRef.current >= config.high_warning) {
      return config.warn_color;
    }
    if (config.low_warning != null && displayValueRef.current <= config.low_warning) {
      return config.warn_color;
    }
    return config.font_color;
  }, [config]);

  /**
   * Per-frame painter dispatcher.
   *
   * Migrated painters live as pure top-level functions in
   * `gauges/painters/` and are looked up via `painterRegistry`.
   * Painters not yet migrated still live as inline `drawXxx`
   * closures in this file and are dispatched by the `switch` below.
   * The hook stores this callback in a ref, so swapping it across
   * renders does not restart the rAF loop.
   */
  const paint = useCallback(
    (ctx: CanvasRenderingContext2D, cssW: number, cssH: number, displayValue: number) => {
      const needleImage = getEmbeddedImage(config.needle_image_file_name);
      const bgImage = getEmbeddedImage(config.background_image_file_name);

      // 1. Try the registry first.
      const migrated = painterRegistry[config.gauge_painter];
      if (migrated) {
        const pctx: PainterContext = {
          ctx,
          width: cssW,
          height: cssH,
          value: displayValue,
          config,
          legacyMode,
          bgImage,
          needleImage,
          getValueColor,
          getFontSpec,
          getFontFamily,
          getEmbeddedImage,
        };
        migrated(pctx);
        return;
      }

      // 2. Fall back to inline closures for painters not yet migrated.
      switch (config.gauge_painter) {
        case 'AnalogGauge':
        case 'BasicAnalogGauge':
        case 'CircleAnalogGauge':
          drawAnalogGauge(ctx, cssW, cssH, needleImage, bgImage);
          break;
        case 'AsymmetricSweepGauge':
          drawSweepGauge(ctx, cssW, cssH);
          break;
        case 'Histogram':
          drawHistogram(ctx, cssW, cssH);
          break;
        case 'LineGraph':
          drawLineGraph(ctx, cssW, cssH);
          break;
        case 'AnalogBarGauge':
          drawAnalogBarGauge(ctx, cssW, cssH);
          break;
        case 'AnalogMovingBarGauge':
          drawAnalogMovingBarGauge(ctx, cssW, cssH);
          break;
        case 'RoundGauge':
          drawRoundGauge(ctx, cssW, cssH);
          break;
        case 'RoundDashedGauge':
          drawRoundDashedGauge(ctx, cssW, cssH);
          break;
        case 'FuelMeter':
          drawFuelMeter(ctx, cssW, cssH);
          break;
        case 'Tachometer':
          drawTachometer(ctx, cssW, cssH);
          break;
        default:
          // Unknown painter — fall back to the migrated BasicReadout.
          painterRegistry.BasicReadout?.({
            ctx,
            width: cssW,
            height: cssH,
            value: displayValue,
            config,
            legacyMode,
            bgImage,
            needleImage,
            getValueColor,
            getFontSpec,
            getFontFamily,
            getEmbeddedImage,
          });
      }
    },
    // The legacy painter closures close over `config`, helpers, and
    // `displayValueRef`; we only need to refresh `paint` when the
    // dispatch key or config-derived inputs change. The hook stores
    // the callback in a ref, so we don't pay an effect-restart cost.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [config, legacyMode, getEmbeddedImage, getValueColor, getFontSpec, getFontFamily],
  );

  const { canvasRef, displayValueRef } = useGaugeRenderer({
    config,
    value,
    overrideStore,
    enabled: fontsReady && imagesReady,
    paint,
  });

  /** Draw analog dial gauge with metallic bezel and improved visuals */
  const drawAnalogGauge = (
    ctx: CanvasRenderingContext2D, 
    width: number, 
    height: number,
    needleImage?: HTMLImageElement | null,
    bgImage?: HTMLImageElement | null
  ) => {
    // Enforce perfect circle: use the smaller of width/height, center in canvas
    const size = Math.min(width, height);
    // const pivotOffsetX = config.needle_pivot_offset_x ?? 0;
    // const pivotOffsetY = config.needle_pivot_offset_y ?? 0;
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
      centerX, centerY, faceRadius
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
    // Use actual values from config, only fallback if truly undefined
    const startDeg = config.sweep_begin_degree ?? config.start_angle ?? 225;
    const sweepDeg = config.sweep_angle ?? 270;
    const ccw = config.counter_clockwise ?? false;
    
    // Convert to radians: TS angles are measured from 3 o'clock position,
    // canvas arc() measures from the positive x-axis (also 3 o'clock)
    // So we just need to convert degrees to radians
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
          centerY + Math.sin(tickAngle) * labelRadius
        );
      }
    }

    // Draw needle with shadow
    const valuePercent = (displayValueRef.current - config.min) / (config.max - config.min);
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
        imgHeight * scale
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
    const valueText = `${displayValueRef.current.toFixed(config.value_digits)} ${config.units}`;
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

  /** Draw sweep gauge (tachometer style) with improved visuals */
  const drawSweepGauge = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
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
    const valuePercent = Math.max(0, Math.min(1, (displayValueRef.current - config.min) / (config.max - config.min)));
    const valueAngle = angleAt(valuePercent);
    const valueColor = getValueColor();
    const valueHex = tsColorToHex(valueColor);

    if (valuePercent > 0.01) {
      const fillGradient = ctx.createLinearGradient(
        centerX + Math.cos(startAngle) * radius,
        centerY + Math.sin(startAngle) * radius,
        centerX + Math.cos(valueAngle) * radius,
        centerY + Math.sin(valueAngle) * radius
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
        Math.PI * 2
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
          centerY + Math.sin(tickAngle) * labelRadius
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
    ctx.fillText(displayValueRef.current.toFixed(config.value_digits), centerX, sweepValueY);
    ctx.shadowColor = 'transparent';

    // Title below value
    ctx.font = getFontSpec(Math.max(10, radius * 0.11), { bold: true });
    ctx.fillText(config.title, centerX, centerY + radius * 0.35);

    // Units
    ctx.fillStyle = tsColorToHex(config.trim_color);
    ctx.font = getFontSpec(Math.max(9, radius * 0.09));
    ctx.fillText(config.units, centerX, centerY + radius * 0.5);
  };

  /** Draw histogram gauge - shows distribution of values */
  const drawHistogram = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = 8;
    const titleHeight = height * 0.12;
    const valueHeight = height * 0.1;
    const graphWidth = width - padding * 2;
    const graphHeight = height - titleHeight - valueHeight - padding * 2;
    const graphY = titleHeight + padding;
    const numBars = 20;
    const barWidth = (graphWidth - (numBars - 1) * 2) / numBars;

    // Background with gradient
    const bgGradient = ctx.createLinearGradient(0, 0, 0, height);
    const bgHex = tsColorToHex(config.back_color);
    bgGradient.addColorStop(0, lightenColor(bgHex, 5));
    bgGradient.addColorStop(1, darkenColor(bgHex, 10));
    ctx.fillStyle = bgGradient;
    ctx.fillRect(0, 0, width, height);

    // Title
    ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
    ctx.shadowBlur = 2;
    ctx.fillStyle = tsColorToHex(config.trim_color);
    ctx.font = getFontSpec(Math.max(9, titleHeight * 0.8), { bold: true });
    ctx.textAlign = 'left';
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, padding, 3);
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

    // Generate histogram data based on current value position
    // This simulates a distribution centered around the current value
    const valuePercent = (displayValueRef.current - config.min) / (config.max - config.min);
    const centerBar = Math.floor(valuePercent * numBars);
    const normalColor = tsColorToHex(config.needle_color); // Use needle_color for normal range
    const warnColor = tsColorToHex(config.warn_color);

    for (let i = 0; i < numBars; i++) {
      const barX = padding + i * (barWidth + 2);
      
      // Create a bell-curve distribution around the current value
      const distFromCenter = Math.abs(i - centerBar);
      const barHeightPercent = Math.max(0.05, Math.exp(-distFromCenter * distFromCenter / 20));
      const barHeight = graphHeight * barHeightPercent;
      const barY = graphY + graphHeight - barHeight;

      // Color based on position in range
      const barPercent = i / numBars;
      let barColor: string;
      if (config.high_critical !== null && barPercent >= (config.high_critical - config.min) / (config.max - config.min)) {
        barColor = tsColorToHex(config.critical_color);
      } else if (config.high_warning !== null && barPercent >= (config.high_warning - config.min) / (config.max - config.min)) {
        barColor = warnColor;
      } else {
        barColor = normalColor;
      }

      // Bar gradient
      const barGradient = ctx.createLinearGradient(barX, barY, barX, barY + barHeight);
      barGradient.addColorStop(0, lightenColor(barColor, 25));
      barGradient.addColorStop(0.5, barColor);
      barGradient.addColorStop(1, darkenColor(barColor, 15));
      ctx.fillStyle = barGradient;
      roundRect(ctx, barX, barY, barWidth, barHeight, 2);
      ctx.fill();
    }

    // Current value marker
    const markerX = padding + valuePercent * graphWidth;
    ctx.strokeStyle = tsColorToHex(config.needle_color);
    ctx.lineWidth = 2;
    ctx.setLineDash([4, 2]);
    ctx.beginPath();
    ctx.moveTo(markerX, graphY);
    ctx.lineTo(markerX, graphY + graphHeight);
    ctx.stroke();
    ctx.setLineDash([]);

    // Value display
    ctx.fillStyle = tsColorToHex(config.font_color);
    ctx.font = getFontSpec(Math.max(10, valueHeight * 0.9), { bold: true, monospace: true });
    ctx.textAlign = 'right';
    ctx.textBaseline = 'top';
    ctx.fillText(`${displayValueRef.current.toFixed(config.value_digits)} ${config.units}`, width - padding, 3);
  };

  /** Draw line graph gauge - shows value over time */
  const drawLineGraph = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
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
    ctx.fillText(`${displayValueRef.current.toFixed(config.value_digits)} ${config.units}`, width - padding, 3);
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
    
    if (history && history.length > 0) {
      // Use actual history data
      const dataRange = config.max - config.min;
      for (let i = 0; i < history.length; i++) {
        const t = i / (history.length - 1);
        const historicalValue = history[i];
        const historicalPercent = (historicalValue - config.min) / dataRange;
        const clampedPercent = Math.max(0, Math.min(1, historicalPercent));
        
        points.push({
          x: padding + t * graphWidth,
          y: graphY + graphHeight - clampedPercent * graphHeight
        });
      }
    } else {
      // No history available - show simulated data for demo
      const numPoints = 50;
      const valuePercent = (displayValueRef.current - config.min) / (config.max - config.min);
      
      for (let i = 0; i < numPoints; i++) {
        const t = i / (numPoints - 1);
        // Simulate some variation leading up to current value
        const noise = Math.sin(t * 20) * 0.05 + Math.sin(t * 7) * 0.03;
        const historicalPercent = valuePercent + (1 - t) * (Math.random() * 0.2 - 0.1) + noise * (1 - t);
        const clampedPercent = Math.max(0, Math.min(1, historicalPercent));
        
        points.push({
          x: padding + t * graphWidth,
          y: graphY + graphHeight - clampedPercent * graphHeight
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

  /** Draw analog bar gauge - semicircular bar indicator */
  const drawAnalogBarGauge = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const centerX = width / 2;
    const centerY = height * 0.85;
    const radius = Math.min(width, height) * 0.75;
    const barWidth = radius * 0.15;
    
    // Angle range: 180° arc from left to right
    const startAngle = Math.PI;
    const endAngle = 0;
    const totalSweep = Math.PI;
    
    // Background arc with metallic bezel
    const bezelGradient = createMetallicGradient(ctx, centerX, centerY, 0, radius + barWidth/2 + 8, config.trim_color);
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius + barWidth/2 + 4, startAngle, endAngle, false);
    ctx.arc(centerX, centerY, radius - barWidth/2 - 4, endAngle, startAngle, true);
    ctx.closePath();
    ctx.fillStyle = bezelGradient;
    ctx.fill();
    
    // Track background
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius, startAngle, endAngle, false);
    ctx.lineWidth = barWidth;
    ctx.strokeStyle = darkenColor(tsColorToHex(config.back_color), 30);
    ctx.lineCap = 'butt';
    ctx.stroke();
    
    // Calculate value angle
    const range = config.max - config.min;
    const normalizedValue = Math.max(0, Math.min(1, (displayValueRef.current - config.min) / range));
    const valueAngle = startAngle - (normalizedValue * totalSweep);
    
    // Value bar with gradient
    if (normalizedValue > 0) {
      const valueColorTs = getValueColor();
      const valueColorHex = tsColorToHex(valueColorTs);
      const barGradient = ctx.createLinearGradient(0, centerY - radius, width, centerY);
      barGradient.addColorStop(0, darkenColor(valueColorHex, 20));
      barGradient.addColorStop(0.5, valueColorHex);
      barGradient.addColorStop(1, lightenColor(valueColorHex, 20));
      
      ctx.beginPath();
      ctx.arc(centerX, centerY, radius, startAngle, valueAngle, true);
      ctx.lineWidth = barWidth - 4;
      ctx.strokeStyle = barGradient;
      ctx.lineCap = 'round';
      ctx.stroke();
    }
    
    // Tick marks
    const tickCount = 10;
    ctx.strokeStyle = tsColorToHex(config.trim_color);
    ctx.lineWidth = 1;
    for (let i = 0; i <= tickCount; i++) {
      const tickAngle = startAngle - (i / tickCount) * totalSweep;
      const innerRadius = radius - barWidth/2 - 8;
      const outerRadius = radius - barWidth/2 - 15;
      const isMajor = i % 2 === 0;
      
      ctx.beginPath();
      ctx.moveTo(
        centerX + Math.cos(tickAngle) * (isMajor ? outerRadius : innerRadius + 3),
        centerY + Math.sin(tickAngle) * (isMajor ? outerRadius : innerRadius + 3)
      );
      ctx.lineTo(
        centerX + Math.cos(tickAngle) * innerRadius,
        centerY + Math.sin(tickAngle) * innerRadius
      );
      ctx.stroke();
    }
    
    // Value text in center
    const valueColorTs = getValueColor();
    ctx.fillStyle = tsColorToHex(valueColorTs);
    const fontSize = Math.max(12, radius * 0.2);
    ctx.font = getFontSpec(fontSize, { bold: true, monospace: true });
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(displayValueRef.current.toFixed(config.value_digits), centerX, centerY - radius * 0.3);
    
    // Units below value
    if (config.units) {
      ctx.fillStyle = tsColorToHex(config.font_color);
      ctx.font = getFontSpec(fontSize * 0.5);
      ctx.fillText(config.units, centerX, centerY - radius * 0.1);
    }
    
    // Title at top
    if (config.title) {
      ctx.fillStyle = tsColorToHex(config.font_color);
      ctx.font = getFontSpec(fontSize * 0.5);
      ctx.textBaseline = 'top';
      ctx.fillText(config.title, centerX, 4);
    }
  };

  /** Draw analog moving bar gauge - sweeping needle with bar trail */
  const drawAnalogMovingBarGauge = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const centerX = width / 2;
    const centerY = height * 0.9;
    const radius = Math.min(width, height) * 0.8;
    const barWidth = radius * 0.08;
    
    // Angle range: 140° arc
    const startAngle = Math.PI + Math.PI * 0.2;
    const endAngle = -Math.PI * 0.2;
    const totalSweep = startAngle - endAngle;
    
    // Metallic bezel
    const bezelWidth = 6;
    const bezelGradient = createMetallicGradient(ctx, centerX, centerY, 0, radius + bezelWidth * 2, config.trim_color);
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius + bezelWidth, startAngle, endAngle, false);
    ctx.lineWidth = bezelWidth * 2;
    ctx.strokeStyle = bezelGradient;
    ctx.stroke();
    
    // Track background with inset shadow
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius - barWidth, startAngle, endAngle, false);
    ctx.arc(centerX, centerY, radius - barWidth * 3, endAngle, startAngle, true);
    ctx.closePath();
    ctx.fillStyle = darkenColor(tsColorToHex(config.back_color), 40);
    ctx.fill();
    
    // Warning/danger zones
    const warnStart = config.high_warning ?? (config.min + (config.max - config.min) * 0.7);
    const dangerStart = config.high_critical ?? (config.min + (config.max - config.min) * 0.9);
    const range = config.max - config.min;
    
    // Draw zone arcs
    const drawZone = (startVal: number, endVal: number, color: string) => {
      const s = Math.max(0, Math.min(1, (startVal - config.min) / range));
      const e = Math.max(0, Math.min(1, (endVal - config.min) / range));
      const sAngle = startAngle - s * totalSweep;
      const eAngle = startAngle - e * totalSweep;
      
      ctx.beginPath();
      ctx.arc(centerX, centerY, radius - barWidth * 2, sAngle, eAngle, true);
      ctx.lineWidth = barWidth;
      ctx.strokeStyle = color;
      ctx.lineCap = 'butt';
      ctx.stroke();
    };
    
    drawZone(config.min, warnStart, tsColorToHex(config.font_color));
    drawZone(warnStart, dangerStart, tsColorToHex(config.warn_color));
    drawZone(dangerStart, config.max, tsColorToHex(config.critical_color));
    
    // Calculate value angle
    const normalizedValue = Math.max(0, Math.min(1, (displayValueRef.current - config.min) / range));
    const valueAngle = startAngle - normalizedValue * totalSweep;
    
    // Moving bar (filled from start to current value)
    const barColorTs = getValueColor();
    const barColorHex = tsColorToHex(barColorTs);
    const barGradient = ctx.createRadialGradient(centerX, centerY, radius * 0.5, centerX, centerY, radius);
    barGradient.addColorStop(0, lightenColor(barColorHex, 30));
    barGradient.addColorStop(1, barColorHex);
    
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius - barWidth * 0.5, startAngle, valueAngle, true);
    ctx.lineWidth = barWidth * 0.8;
    ctx.strokeStyle = barGradient;
    ctx.lineCap = 'round';
    ctx.stroke();
    
    // Needle at current position
    const needleLength = radius * 0.85;
    const needleWidth = 4;
    
    ctx.save();
    ctx.translate(centerX, centerY);
    ctx.rotate(valueAngle);
    
    // Needle shadow
    ctx.shadowColor = 'rgba(0,0,0,0.5)';
    ctx.shadowBlur = 6;
    ctx.shadowOffsetY = 2;
    
    // Needle body
    ctx.beginPath();
    ctx.moveTo(needleLength, 0);
    ctx.lineTo(0, -needleWidth);
    ctx.lineTo(-needleLength * 0.1, 0);
    ctx.lineTo(0, needleWidth);
    ctx.closePath();
    
    const needleGradient = ctx.createLinearGradient(0, -needleWidth, 0, needleWidth);
    needleGradient.addColorStop(0, '#ff4444');
    needleGradient.addColorStop(0.5, '#ff0000');
    needleGradient.addColorStop(1, '#aa0000');
    ctx.fillStyle = needleGradient;
    ctx.fill();
    
    ctx.restore();
    
    // Center cap
    ctx.shadowColor = 'rgba(0,0,0,0.3)';
    ctx.shadowBlur = 4;
    const capGradient = ctx.createRadialGradient(centerX - 2, centerY - 2, 0, centerX, centerY, 12);
    capGradient.addColorStop(0, '#666666');
    capGradient.addColorStop(0.5, '#444444');
    capGradient.addColorStop(1, '#222222');
    ctx.beginPath();
    ctx.arc(centerX, centerY, 10, 0, Math.PI * 2);
    ctx.fillStyle = capGradient;
    ctx.fill();
    ctx.shadowColor = 'transparent';
    
    // Value text
    const valueTextColorTs = getValueColor();
    ctx.fillStyle = tsColorToHex(valueTextColorTs);
    const fontSize = Math.max(14, radius * 0.18);
    ctx.font = getFontSpec(fontSize, { bold: true, monospace: true });
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(displayValueRef.current.toFixed(config.value_digits), centerX, centerY - radius * 0.35);
    
    // Units
    if (config.units) {
      ctx.fillStyle = tsColorToHex(config.font_color);
      ctx.font = getFontSpec(fontSize * 0.6);
      ctx.fillText(config.units, centerX, centerY - radius * 0.18);
    }
    
    // Title
    if (config.title) {
      ctx.fillStyle = tsColorToHex(config.font_color);
      ctx.font = getFontSpec(fontSize * 0.5);
      ctx.textBaseline = 'top';
      ctx.fillText(config.title, centerX, 4);
    }
  };

  /** Draw round gauge - 360 degree circular gauge with segments */
  const drawRoundGauge = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = Math.min(width, height) * 0.08;
    const centerX = width / 2;
    const centerY = height / 2;
    const radius = Math.min(width, height) / 2 - padding;
    
    // Metallic outer ring
    const ringWidth = radius * 0.12;
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
    ctx.arc(centerX, centerY, radius - ringWidth, 0, Math.PI * 2, true);
    ctx.closePath();
    const ringGradient = createMetallicGradient(ctx, centerX, centerY, 0, radius, config.trim_color);
    ctx.fillStyle = ringGradient;
    ctx.fill();
    
    // Inner background
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius - ringWidth, 0, Math.PI * 2);
    ctx.fillStyle = tsColorToRgba(config.back_color);
    ctx.fill();
    
    // Draw segments around the full 360 degrees
    const innerRadius = radius * 0.55;
    const outerRadius = radius * 0.85;
    const segments = 60;
    const gapAngle = Math.PI / 180; // 1 degree gap
    
    for (let i = 0; i < segments; i++) {
      const startAngle = (i / segments) * Math.PI * 2 - Math.PI / 2;
      const endAngle = ((i + 1) / segments) * Math.PI * 2 - Math.PI / 2 - gapAngle;
      const segmentValue = config.min + (i / segments) * (config.max - config.min);
      
      ctx.beginPath();
      ctx.arc(centerX, centerY, outerRadius, startAngle, endAngle);
      ctx.arc(centerX, centerY, innerRadius, endAngle, startAngle, true);
      ctx.closePath();
      
      // Color based on value and warning zones
      let segmentColor = tsColorToHex(config.trim_color);
      if (segmentValue >= config.max - (config.max - config.min) * 0.1) {
        segmentColor = tsColorToHex(config.critical_color);
      } else if (segmentValue >= config.max - (config.max - config.min) * 0.25) {
        segmentColor = tsColorToHex(config.warn_color);
      }
      
      // Dim segments beyond current value
      if (segmentValue > displayValueRef.current) {
        ctx.fillStyle = lightenColor(segmentColor, -60);
      } else {
        ctx.fillStyle = segmentColor;
      }
      ctx.fill();
    }
    
    // Value display in center
    const valueTextColorTs = getValueColor();
    const fontSize = Math.max(12, radius * 0.25);
    ctx.fillStyle = tsColorToHex(valueTextColorTs);
    ctx.font = getFontSpec(fontSize, { bold: true, monospace: true });
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(displayValueRef.current.toFixed(config.value_digits), centerX, centerY);
    
    // Units below value
    if (config.units) {
      ctx.fillStyle = tsColorToHex(config.font_color);
      ctx.font = getFontSpec(fontSize * 0.5);
      ctx.fillText(config.units, centerX, centerY + fontSize * 0.6);
    }
    
    // Title at top
    if (config.title) {
      ctx.fillStyle = tsColorToHex(config.font_color);
      ctx.font = getFontSpec(fontSize * 0.4);
      ctx.textBaseline = 'top';
      ctx.fillText(config.title, centerX, 4);
    }
  };

  /** Draw round dashed gauge - circular gauge with dashed segments */
  const drawRoundDashedGauge = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = Math.min(width, height) * 0.08;
    const centerX = width / 2;
    const centerY = height / 2;
    const radius = Math.min(width, height) / 2 - padding;
    
    // Metallic outer bezel
    ctx.shadowColor = 'rgba(0,0,0,0.4)';
    ctx.shadowBlur = 6;
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
    const bezelGradient = createMetallicGradient(ctx, centerX, centerY, 0, radius, config.trim_color);
    ctx.fillStyle = bezelGradient;
    ctx.fill();
    ctx.shadowColor = 'transparent';
    
    // Inner background
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius * 0.9, 0, Math.PI * 2);
    ctx.fillStyle = tsColorToRgba(config.back_color);
    ctx.fill();
    
    // Draw dashed segments around 270 degrees (like a speedometer)
    const startAngle = Math.PI * 0.75; // 135 degrees
    const endAngle = Math.PI * 2.25; // 405 degrees
    const totalSweep = endAngle - startAngle;
    const segments = 30;
    const segmentWidth = radius * 0.08;
    const innerRadius = radius * 0.65;
    const outerRadius = radius * 0.85;
    
    for (let i = 0; i < segments; i++) {
      const angle = startAngle + (i / (segments - 1)) * totalSweep;
      const segmentValue = config.min + (i / (segments - 1)) * (config.max - config.min);
      
      const x1 = centerX + Math.cos(angle) * innerRadius;
      const y1 = centerY + Math.sin(angle) * innerRadius;
      const x2 = centerX + Math.cos(angle) * outerRadius;
      const y2 = centerY + Math.sin(angle) * outerRadius;
      
      // Determine color
      let segmentColor = tsColorToHex(config.trim_color);
      if (segmentValue >= config.max - (config.max - config.min) * 0.1) {
        segmentColor = tsColorToHex(config.critical_color);
      } else if (segmentValue >= config.max - (config.max - config.min) * 0.25) {
        segmentColor = tsColorToHex(config.warn_color);
      }
      
      // Draw segment
      ctx.beginPath();
      ctx.moveTo(x1, y1);
      ctx.lineTo(x2, y2);
      ctx.lineWidth = segmentWidth;
      ctx.lineCap = 'round';
      
      if (segmentValue <= displayValueRef.current) {
        ctx.strokeStyle = segmentColor;
        ctx.shadowColor = segmentColor;
        ctx.shadowBlur = 4;
      } else {
        ctx.strokeStyle = lightenColor(segmentColor, -60);
        ctx.shadowColor = 'transparent';
      }
      ctx.stroke();
      ctx.shadowColor = 'transparent';
    }
    
    // Value in center
    const valueTextColorTs = getValueColor();
    const fontSize = Math.max(12, radius * 0.28);
    ctx.fillStyle = tsColorToHex(valueTextColorTs);
    ctx.font = getFontSpec(fontSize, { bold: true, monospace: true });
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(displayValueRef.current.toFixed(config.value_digits), centerX, centerY);
    
    // Units
    if (config.units) {
      ctx.fillStyle = tsColorToHex(config.font_color);
      ctx.font = getFontSpec(fontSize * 0.45);
      ctx.fillText(config.units, centerX, centerY + fontSize * 0.7);
    }
    
    // Title
    if (config.title) {
      ctx.fillStyle = tsColorToHex(config.font_color);
      ctx.font = getFontSpec(fontSize * 0.35);
      ctx.textBaseline = 'top';
      ctx.fillText(config.title, centerX, 4);
    }
  };

  /** Draw fuel meter - stylized fuel gauge with tank icon */
  const drawFuelMeter = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
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
    const normalizedValue = (displayValueRef.current - config.min) / (config.max - config.min);
    const fillAngle = arcStartAngle + arcSweep * normalizedValue;
    
    // Color gradient for fuel level
    const fuelGradient = ctx.createLinearGradient(
      centerX - arcRadius, centerY,
      centerX + arcRadius, centerY
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
    ctx.fillText(`${displayValueRef.current.toFixed(config.value_digits)}${config.units || '%'}`, centerX, centerY + radius * 0.35);
    
    // Title
    if (config.title) {
      ctx.fillStyle = tsColorToHex(config.font_color);
      ctx.font = getFontSpec(fontSize * 0.5);
      ctx.textBaseline = 'top';
      ctx.fillText(config.title, centerX, 4);
    }
  };

  /** Draw tachometer - specialized RPM gauge with redline */
  const drawTachometer = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = Math.min(width, height) * 0.08;
    const centerX = width / 2;
    const centerY = height / 2;
    const radius = Math.min(width, height) / 2 - padding;
    
    // Outer chrome bezel
    ctx.shadowColor = 'rgba(0,0,0,0.5)';
    ctx.shadowBlur = 8;
    
    // Double ring effect
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
    const outerBezel = createMetallicGradient(ctx, centerX, centerY, 0, radius, config.trim_color);
    ctx.fillStyle = outerBezel;
    ctx.fill();
    
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius * 0.93, 0, Math.PI * 2);
    ctx.fillStyle = '#1a1a1a';
    ctx.fill();
    ctx.shadowColor = 'transparent';
    
    // Tachometer typically spans 270 degrees
    const startAngle = Math.PI * 0.75;
    const totalSweep = Math.PI * 1.5;
    
    // Draw major tick marks with numbers
    const tickInnerRadius = radius * 0.72;
    const tickOuterRadius = radius * 0.85;
    const majorTicks = Math.ceil(config.max / 1000); // One tick per 1000 RPM
    
    ctx.strokeStyle = '#ffffff';
    ctx.fillStyle = '#ffffff';
    ctx.lineWidth = 2;
    
    for (let i = 0; i <= majorTicks; i++) {
      const tickValue = i * 1000;
      if (tickValue > config.max) continue;
      
      const normalizedTick = (tickValue - config.min) / (config.max - config.min);
      const angle = startAngle + totalSweep * normalizedTick;
      
      const x1 = centerX + Math.cos(angle) * tickInnerRadius;
      const y1 = centerY + Math.sin(angle) * tickInnerRadius;
      const x2 = centerX + Math.cos(angle) * tickOuterRadius;
      const y2 = centerY + Math.sin(angle) * tickOuterRadius;
      
      // Color red for redline zone
      if (tickValue >= config.max * 0.85) {
        ctx.strokeStyle = tsColorToHex(config.critical_color);
        ctx.fillStyle = tsColorToHex(config.critical_color);
      } else {
        ctx.strokeStyle = '#ffffff';
        ctx.fillStyle = '#ffffff';
      }
      
      ctx.beginPath();
      ctx.moveTo(x1, y1);
      ctx.lineTo(x2, y2);
      ctx.stroke();
      
      // Number labels
      const labelRadius = radius * 0.58;
      const labelX = centerX + Math.cos(angle) * labelRadius;
      const labelY = centerY + Math.sin(angle) * labelRadius;
      
      ctx.font = getFontSpec(radius * 0.12, { bold: true });
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(String(i), labelX, labelY);
    }
    
    // Minor ticks
    ctx.strokeStyle = '#666666';
    ctx.lineWidth = 1;
    const minorTicks = majorTicks * 5;
    for (let i = 0; i < minorTicks; i++) {
      const tickValue = config.min + (i / minorTicks) * (config.max - config.min);
      if (tickValue % 1000 === 0) continue; // Skip major tick positions
      
      const normalizedTick = (tickValue - config.min) / (config.max - config.min);
      const angle = startAngle + totalSweep * normalizedTick;
      
      const x1 = centerX + Math.cos(angle) * (tickInnerRadius + (tickOuterRadius - tickInnerRadius) * 0.5);
      const y1 = centerY + Math.sin(angle) * (tickInnerRadius + (tickOuterRadius - tickInnerRadius) * 0.5);
      const x2 = centerX + Math.cos(angle) * tickOuterRadius;
      const y2 = centerY + Math.sin(angle) * tickOuterRadius;
      
      ctx.beginPath();
      ctx.moveTo(x1, y1);
      ctx.lineTo(x2, y2);
      ctx.stroke();
    }
    
    // Redline zone arc
    const redlineStart = config.max * 0.85;
    const redlineNormalized = (redlineStart - config.min) / (config.max - config.min);
    const redlineAngle = startAngle + totalSweep * redlineNormalized;
    
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius * 0.88, redlineAngle, startAngle + totalSweep);
    ctx.lineWidth = radius * 0.05;
    ctx.strokeStyle = tsColorToHex(config.critical_color);
    ctx.stroke();
    
    // Needle
    const normalizedValue = (displayValueRef.current - config.min) / (config.max - config.min);
    const needleAngle = startAngle + totalSweep * normalizedValue;
    const needleLength = radius * 0.65;
    const needleWidth = radius * 0.03;
    
    ctx.save();
    ctx.translate(centerX, centerY);
    ctx.rotate(needleAngle);
    
    // Needle shadow
    ctx.shadowColor = 'rgba(0,0,0,0.5)';
    ctx.shadowBlur = 8;
    ctx.shadowOffsetX = 4;
    ctx.shadowOffsetY = 4;
    
    // Needle body - red with gradient
    ctx.beginPath();
    ctx.moveTo(-needleLength * 0.15, 0);
    ctx.lineTo(needleLength, 0);
    ctx.lineTo(-needleLength * 0.15, -needleWidth);
    ctx.lineTo(-needleLength * 0.15, needleWidth);
    ctx.closePath();
    
    const needleGradient = ctx.createLinearGradient(0, -needleWidth, 0, needleWidth);
    needleGradient.addColorStop(0, '#ff4444');
    needleGradient.addColorStop(0.5, '#ff0000');
    needleGradient.addColorStop(1, '#aa0000');
    ctx.fillStyle = needleGradient;
    ctx.fill();
    
    ctx.restore();
    ctx.shadowColor = 'transparent';
    
    // Center hub
    const hubGradient = ctx.createRadialGradient(centerX - 3, centerY - 3, 0, centerX, centerY, radius * 0.1);
    hubGradient.addColorStop(0, '#888888');
    hubGradient.addColorStop(0.5, '#444444');
    hubGradient.addColorStop(1, '#222222');
    ctx.beginPath();
    ctx.arc(centerX, centerY, radius * 0.08, 0, Math.PI * 2);
    ctx.fillStyle = hubGradient;
    ctx.fill();
    
    // Digital RPM display at bottom
    const valueTextColorTs = getValueColor();
    const fontSize = Math.max(10, radius * 0.16);
    ctx.fillStyle = tsColorToHex(valueTextColorTs);
    ctx.font = getFontSpec(fontSize, { bold: true, monospace: true });
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(displayValueRef.current.toFixed(0), centerX, centerY + radius * 0.35);
    
    // RPM label
    ctx.fillStyle = '#888888';
    ctx.font = getFontSpec(fontSize * 0.6);
    ctx.fillText('RPM × 1000', centerX, centerY + radius * 0.52);
    
    // Title at top
    if (config.title) {
      ctx.fillStyle = tsColorToHex(config.font_color);
      ctx.font = getFontSpec(fontSize * 0.5);
      ctx.textBaseline = 'top';
      ctx.fillText(config.title, centerX, 4);
    }
  };

  return (
    <canvas
      ref={canvasRef}
      style={{
        width: '100%',
        height: '100%',
        display: 'block',
      }}
    />
  );
}

/**
 * TsGauge - Memoized gauge component.
 * 
 * Uses custom comparator to skip re-renders when:
 * - Config hasn't changed
 * For live data, the internal store subscription drives the animation loop
 * directly (bypassing React rendering). The value prop only matters for
 * sweep/demo mode (when overrideStore is true).
 */
const TsGauge = React.memo(TsGaugeInner, (prevProps, nextProps) => {
  return (
    prevProps.value === nextProps.value &&
    prevProps.config === nextProps.config &&
    prevProps.embeddedImages === nextProps.embeddedImages &&
    prevProps.legacyMode === nextProps.legacyMode &&
    prevProps.overrideStore === nextProps.overrideStore
  );
});

export default TsGauge;
