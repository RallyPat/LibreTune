/**
 * TS Gauge Renderer
 * 
 * Renders gauges based on TS GaugePainter types.
 * Uses canvas for all gauge rendering with high-quality visual effects.
 * Wrapped in React.memo with custom comparator for performance optimization.
 */

import React, { useEffect, useRef, useState, useCallback } from 'react';
import { TsGaugeConfig, TsColor, tsColorToRgba, tsColorToHex } from '../dashboards/dashTypes';

interface TsGaugeProps {
  config: TsGaugeConfig;
  value: number;
  embeddedImages?: Map<string, string>;
}

// Cache for loaded fonts
const loadedFonts = new Set<string>();

// Cache for loaded HTMLImageElement objects (for canvas drawImage)
const loadedImages = new Map<string, HTMLImageElement>();

// Threshold for value change to trigger redraw (0.5% of gauge range)
const VALUE_CHANGE_THRESHOLD_PERCENT = 0.5;

/**
 * Internal TsGauge component - wrapped in React.memo below
 */
function TsGaugeInner({ config, value, embeddedImages }: TsGaugeProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [fontsReady, setFontsReady] = useState(false);
  const [imagesReady, setImagesReady] = useState(false);

  // Clamp value to min/max if peg_limits is true
  const clampedValue = config.peg_limits 
    ? Math.max(config.min, Math.min(config.max, value))
    : value;

  // Load embedded fonts and images
  useEffect(() => {
    if (!embeddedImages) {
      setFontsReady(true);
      setImagesReady(true);
      return;
    }

    const loadAssets = async () => {
      const loadPromises: Promise<void>[] = [];
      
      for (const [id, dataUrl] of embeddedImages.entries()) {
        // Load fonts
        if (dataUrl.startsWith('data:font/ttf') && !loadedFonts.has(id)) {
          loadPromises.push(
            (async () => {
              try {
                const fontFace = new FontFace(id, `url(${dataUrl})`);
                await fontFace.load();
                document.fonts.add(fontFace);
                loadedFonts.add(id);
              } catch (e) {
                console.warn(`Failed to load embedded font ${id}:`, e);
              }
            })()
          );
        }
        
        // Load images (PNG/GIF)
        if ((dataUrl.startsWith('data:image/png') || dataUrl.startsWith('data:image/gif')) 
            && !loadedImages.has(id)) {
          loadPromises.push(
            new Promise<void>((resolve) => {
              const img = new Image();
              img.onload = () => {
                loadedImages.set(id, img);
                resolve();
              };
              img.onerror = () => {
                console.warn(`Failed to load embedded image ${id}`);
                resolve();
              };
              img.src = dataUrl;
            })
          );
        }
      }
      
      await Promise.all(loadPromises);
      setFontsReady(true);
      setImagesReady(true);
    };

    loadAssets();
  }, [embeddedImages]);

  /** Get a loaded image by name or id */
  const getEmbeddedImage = useCallback((name: string | null | undefined): HTMLImageElement | null => {
    if (!name) return null;
    return loadedImages.get(name) || null;
  }, []);
  
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
    if (loadedFonts.has(customFont)) {
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
    if (config.high_critical != null && clampedValue >= config.high_critical) {
      return config.critical_color;
    }
    if (config.low_critical != null && clampedValue <= config.low_critical) {
      return config.critical_color;
    }
    if (config.high_warning != null && clampedValue >= config.high_warning) {
      return config.warn_color;
    }
    if (config.low_warning != null && clampedValue <= config.low_warning) {
      return config.warn_color;
    }
    return config.font_color;
  }, [config, clampedValue]);

  /** Create a metallic gradient for bezels */
  const createMetallicGradient = useCallback((
    ctx: CanvasRenderingContext2D,
    x: number, y: number, r1: number, r2: number,
    baseColor: TsColor
  ): CanvasGradient => {
    const gradient = ctx.createRadialGradient(x - r2 * 0.3, y - r2 * 0.3, r1, x, y, r2);
    const hex = tsColorToHex(baseColor);
    gradient.addColorStop(0, lightenColor(hex, 60));
    gradient.addColorStop(0.3, lightenColor(hex, 30));
    gradient.addColorStop(0.5, hex);
    gradient.addColorStop(0.7, darkenColor(hex, 20));
    gradient.addColorStop(1, darkenColor(hex, 40));
    return gradient;
  }, []);

  /** Lighten a hex color */
  const lightenColor = (hex: string, percent: number): string => {
    const num = parseInt(hex.replace('#', ''), 16);
    const amt = Math.round(2.55 * percent);
    const R = Math.min(255, (num >> 16) + amt);
    const G = Math.min(255, ((num >> 8) & 0x00FF) + amt);
    const B = Math.min(255, (num & 0x0000FF) + amt);
    return `#${(0x1000000 + R * 0x10000 + G * 0x100 + B).toString(16).slice(1)}`;
  };

  /** Darken a hex color */
  const darkenColor = (hex: string, percent: number): string => {
    const num = parseInt(hex.replace('#', ''), 16);
    const amt = Math.round(2.55 * percent);
    const R = Math.max(0, (num >> 16) - amt);
    const G = Math.max(0, ((num >> 8) & 0x00FF) - amt);
    const B = Math.max(0, (num & 0x0000FF) - amt);
    return `#${(0x1000000 + R * 0x10000 + G * 0x100 + B).toString(16).slice(1)}`;
  };

  // Ref to track pending rAF and last drawn value
  const rafIdRef = useRef<number | null>(null);
  const lastDrawnValueRef = useRef<number | null>(null);

  useEffect(() => {
    if (!fontsReady || !imagesReady) return;
    
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    // Skip redraw if value hasn't changed significantly (within threshold)
    const range = config.max - config.min;
    const threshold = range * (VALUE_CHANGE_THRESHOLD_PERCENT / 100);
    if (lastDrawnValueRef.current !== null && 
        Math.abs(clampedValue - lastDrawnValueRef.current) < threshold) {
      return;
    }

    // Cancel any pending rAF to avoid redundant redraws
    if (rafIdRef.current !== null) {
      cancelAnimationFrame(rafIdRef.current);
    }

    // Schedule draw on next animation frame (throttles to 60fps max)
    rafIdRef.current = requestAnimationFrame(() => {
      rafIdRef.current = null;
      lastDrawnValueRef.current = clampedValue;

      // Set canvas size to match container
      const rect = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
      
      // Reset transform before scaling to prevent accumulation
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.scale(dpr, dpr);

      // Clear canvas
      ctx.clearRect(0, 0, rect.width, rect.height);

      // Enable anti-aliasing for smoother rendering
      if (config.antialiasing_on === false) {
        ctx.imageSmoothingEnabled = false;
      } else {
        ctx.imageSmoothingEnabled = true;
        ctx.imageSmoothingQuality = 'high';
      }
    
      // Get needle image if configured
      const needleImage = getEmbeddedImage(config.needle_image_file_name);
      
      // Get background image if configured
      const bgImage = getEmbeddedImage(config.background_image_file_name);

      // Render based on gauge painter type
      switch (config.gauge_painter) {
        case 'BasicReadout':
          drawBasicReadout(ctx, rect.width, rect.height);
          break;
        case 'HorizontalBarGauge':
          drawHorizontalBar(ctx, rect.width, rect.height);
          break;
        case 'VerticalBarGauge':
          drawVerticalBar(ctx, rect.width, rect.height);
          break;
        case 'AnalogGauge':
        case 'BasicAnalogGauge':
        case 'CircleAnalogGauge':
          drawAnalogGauge(ctx, rect.width, rect.height, needleImage, bgImage);
          break;
        case 'AsymmetricSweepGauge':
          drawSweepGauge(ctx, rect.width, rect.height);
          break;
        case 'HorizontalLineGauge':
          drawHorizontalLine(ctx, rect.width, rect.height);
          break;
        case 'HorizontalDashedBar':
          drawHorizontalDashedBar(ctx, rect.width, rect.height);
          break;
        case 'VerticalDashedBar':
          drawVerticalDashedBar(ctx, rect.width, rect.height);
          break;
        case 'Histogram':
          drawHistogram(ctx, rect.width, rect.height);
          break;
        case 'LineGraph':
          drawLineGraph(ctx, rect.width, rect.height);
          break;
        case 'AnalogBarGauge':
          drawAnalogBarGauge(ctx, rect.width, rect.height);
          break;
        case 'AnalogMovingBarGauge':
          drawAnalogMovingBarGauge(ctx, rect.width, rect.height);
          break;
        case 'RoundGauge':
          drawRoundGauge(ctx, rect.width, rect.height);
          break;
        case 'RoundDashedGauge':
          drawRoundDashedGauge(ctx, rect.width, rect.height);
          break;
        case 'FuelMeter':
          drawFuelMeter(ctx, rect.width, rect.height);
          break;
        case 'Tachometer':
          drawTachometer(ctx, rect.width, rect.height);
          break;
        default:
          // Fallback to basic readout for unimplemented types
          drawBasicReadout(ctx, rect.width, rect.height);
      }
    }); // End of requestAnimationFrame callback

    // Cleanup: cancel pending rAF on unmount or re-render
    return () => {
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current);
        rafIdRef.current = null;
      }
    };
  }, [config, clampedValue, value, embeddedImages, fontsReady, imagesReady, getValueColor, createMetallicGradient, getEmbeddedImage, getFontFamily]);

  /** Draw digital readout (LCD style) with improved visuals */
  const drawBasicReadout = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = 6;
    const innerWidth = width - padding * 2;
    const innerHeight = height - padding * 2;
    const cornerRadius = Math.min(8, width * 0.05);
    
    // Use smaller dimension for balanced font scaling (prevents text clipping)
    const minDim = Math.min(width, height);
    // Apply font_size_adjustment as a multiplier (typically -2 to +2, we scale by ~10% per unit)
    const fontScale = 1 + (config.font_size_adjustment ?? 0) * 0.1;

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
    const valueText = clampedValue.toFixed(config.value_digits);
    
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

  /** Helper to draw rounded rectangles */
  const roundRect = (ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) => {
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
  };

  /** Draw horizontal bar gauge with improved 3D effect */
  const drawHorizontalBar = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = 6;
    const barHeight = height * 0.35;
    const barY = (height - barHeight) / 2 + height * 0.08;
    const barWidth = width - padding * 2;
    const cornerRadius = Math.min(4, barHeight * 0.3);

    // Background with subtle gradient
    const bgGradient = ctx.createLinearGradient(0, 0, 0, height);
    const bgHex = tsColorToHex(config.back_color);
    bgGradient.addColorStop(0, lightenColor(bgHex, 10));
    bgGradient.addColorStop(1, darkenColor(bgHex, 15));
    ctx.fillStyle = bgGradient;
    ctx.fillRect(0, 0, width, height);

    // Title
    ctx.fillStyle = tsColorToRgba(config.trim_color);
    ctx.font = getFontSpec(Math.max(9, height * 0.14));
    ctx.textAlign = 'left';
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, padding, 3);

    // Bar background with inset effect
    ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
    ctx.shadowBlur = 3;
    ctx.shadowOffsetX = 1;
    ctx.shadowOffsetY = 1;
    const barBgGradient = ctx.createLinearGradient(0, barY, 0, barY + barHeight);
    barBgGradient.addColorStop(0, '#252525');
    barBgGradient.addColorStop(0.5, '#404040');
    barBgGradient.addColorStop(1, '#303030');
    ctx.fillStyle = barBgGradient;
    roundRect(ctx, padding, barY, barWidth, barHeight, cornerRadius);
    ctx.fill();
    ctx.shadowColor = 'transparent';

    // Bar fill with gradient
    const fillPercent = (clampedValue - config.min) / (config.max - config.min);
    const fillWidth = barWidth * Math.max(0, Math.min(1, fillPercent));
    if (fillWidth > 0) {
      const valueColor = getValueColor();
      const valueHex = tsColorToHex(valueColor);
      const fillGradient = ctx.createLinearGradient(0, barY, 0, barY + barHeight);
      fillGradient.addColorStop(0, lightenColor(valueHex, 30));
      fillGradient.addColorStop(0.3, lightenColor(valueHex, 10));
      fillGradient.addColorStop(0.7, valueHex);
      fillGradient.addColorStop(1, darkenColor(valueHex, 20));
      ctx.fillStyle = fillGradient;
      roundRect(ctx, padding, barY, fillWidth, barHeight, cornerRadius);
      ctx.fill();
      
      // Highlight stripe
      ctx.fillStyle = 'rgba(255, 255, 255, 0.15)';
      ctx.fillRect(padding + 2, barY + 2, fillWidth - 4, barHeight * 0.3);
    }

    // Bar border
    ctx.strokeStyle = tsColorToRgba(config.trim_color);
    ctx.lineWidth = 1;
    roundRect(ctx, padding, barY, barWidth, barHeight, cornerRadius);
    ctx.stroke();

    // Value text with shadow
    ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
    ctx.shadowBlur = 2;
    ctx.fillStyle = tsColorToRgba(config.font_color);
    ctx.font = getFontSpec(Math.max(11, height * 0.18), { bold: true, monospace: true });
    ctx.textAlign = 'right';
    ctx.textBaseline = 'top';
    ctx.fillText(`${clampedValue.toFixed(config.value_digits)} ${config.units}`, width - padding, 3);
    ctx.shadowColor = 'transparent';
  };

  /** Draw vertical bar gauge with improved 3D effect */
  const drawVerticalBar = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = 6;
    const labelHeight = height * 0.12;
    const barWidth = width * 0.45;
    const barHeight = height - labelHeight * 2 - padding * 3;
    const barX = (width - barWidth) / 2;
    const barY = labelHeight + padding * 1.5;
    const cornerRadius = Math.min(4, barWidth * 0.15);

    // Background with gradient
    const bgGradient = ctx.createLinearGradient(0, 0, width, 0);
    const bgHex = tsColorToHex(config.back_color);
    bgGradient.addColorStop(0, lightenColor(bgHex, 5));
    bgGradient.addColorStop(0.5, bgHex);
    bgGradient.addColorStop(1, darkenColor(bgHex, 10));
    ctx.fillStyle = bgGradient;
    ctx.fillRect(0, 0, width, height);

    // Title at top with shadow
    ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
    ctx.shadowBlur = 2;
    ctx.fillStyle = tsColorToRgba(config.trim_color);
    ctx.font = getFontSpec(Math.max(9, labelHeight * 0.75), { bold: true });
    ctx.textAlign = 'center';
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, width / 2, 3);
    ctx.shadowColor = 'transparent';

    // Bar background with inset effect
    ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
    ctx.shadowBlur = 4;
    ctx.shadowOffsetX = 2;
    ctx.shadowOffsetY = 2;
    const barBgGradient = ctx.createLinearGradient(barX, 0, barX + barWidth, 0);
    barBgGradient.addColorStop(0, '#202020');
    barBgGradient.addColorStop(0.3, '#383838');
    barBgGradient.addColorStop(0.7, '#383838');
    barBgGradient.addColorStop(1, '#282828');
    ctx.fillStyle = barBgGradient;
    roundRect(ctx, barX, barY, barWidth, barHeight, cornerRadius);
    ctx.fill();
    ctx.shadowColor = 'transparent';

    // Bar fill (from bottom) with gradient
    const fillPercent = (clampedValue - config.min) / (config.max - config.min);
    const fillHeight = barHeight * Math.max(0, Math.min(1, fillPercent));
    if (fillHeight > 0) {
      const valueColor = getValueColor();
      const valueHex = tsColorToHex(valueColor);
      const fillGradient = ctx.createLinearGradient(barX, 0, barX + barWidth, 0);
      fillGradient.addColorStop(0, darkenColor(valueHex, 15));
      fillGradient.addColorStop(0.3, lightenColor(valueHex, 15));
      fillGradient.addColorStop(0.7, lightenColor(valueHex, 10));
      fillGradient.addColorStop(1, darkenColor(valueHex, 10));
      ctx.fillStyle = fillGradient;
      roundRect(ctx, barX, barY + barHeight - fillHeight, barWidth, fillHeight, cornerRadius);
      ctx.fill();
      
      // Highlight stripe
      ctx.fillStyle = 'rgba(255, 255, 255, 0.12)';
      ctx.fillRect(barX + 3, barY + barHeight - fillHeight + 2, barWidth * 0.35, fillHeight - 4);
    }

    // Bar border
    ctx.strokeStyle = tsColorToRgba(config.trim_color);
    ctx.lineWidth = 1;
    roundRect(ctx, barX, barY, barWidth, barHeight, cornerRadius);
    ctx.stroke();

    // Tick marks on right side
    const tickCount = 5;
    ctx.strokeStyle = tsColorToRgba(config.trim_color);
    ctx.lineWidth = 1;
    for (let i = 0; i <= tickCount; i++) {
      const tickY = barY + barHeight - (barHeight * i / tickCount);
      ctx.beginPath();
      ctx.moveTo(barX + barWidth + 2, tickY);
      ctx.lineTo(barX + barWidth + 6, tickY);
      ctx.stroke();
    }

    // Value at bottom with glow
    ctx.shadowColor = 'rgba(0, 0, 0, 0.6)';
    ctx.shadowBlur = 3;
    ctx.fillStyle = tsColorToRgba(config.font_color);
    ctx.font = getFontSpec(Math.max(11, labelHeight * 0.9), { bold: true, monospace: true });
    ctx.textAlign = 'center';
    ctx.textBaseline = 'bottom';
    ctx.fillText(`${clampedValue.toFixed(config.value_digits)}`, width / 2, height - 2);
    ctx.shadowColor = 'transparent';
  };

  /** Draw analog dial gauge with metallic bezel and improved visuals */
  const drawAnalogGauge = (
    ctx: CanvasRenderingContext2D, 
    width: number, 
    height: number,
    needleImage?: HTMLImageElement | null,
    bgImage?: HTMLImageElement | null
  ) => {
    const centerX = width / 2;
    const centerY = height / 2;
    const radius = Math.min(width, height) / 2 - 8;

    // Background - use image if available, otherwise use color
    if (bgImage) {
      ctx.drawImage(bgImage, 0, 0, width, height);
    } else {
      ctx.fillStyle = tsColorToRgba(config.back_color);
      ctx.fillRect(0, 0, width, height);
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
    const startAngle = (startDeg - 90) * Math.PI / 180;
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
    const valuePercent = (clampedValue - config.min) / (config.max - config.min);
    const needleAngle = angleAt(valuePercent);
    const needleLength = faceRadius - Math.max(20, faceRadius * 0.25);
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
      // Draw custom needle image - centered on rotation point, pointing right
      const imgWidth = needleImage.width;
      const imgHeight = needleImage.height;
      const scale = needleLength / imgWidth;
      ctx.drawImage(
        needleImage,
        0, -imgHeight * scale / 2,
        imgWidth * scale, imgHeight * scale
      );
    } else {
      // Needle body with gradient
      const needleGradient = ctx.createLinearGradient(0, -needleWidth, 0, needleWidth);
      const needleHex = tsColorToHex(config.needle_color);
      needleGradient.addColorStop(0, lightenColor(needleHex, 30));
      needleGradient.addColorStop(0.5, needleHex);
      needleGradient.addColorStop(1, darkenColor(needleHex, 20));

      ctx.beginPath();
      ctx.moveTo(-faceRadius * 0.12, 0);
      ctx.lineTo(needleLength * 0.7, -needleWidth);
      ctx.lineTo(needleLength, 0);
      ctx.lineTo(needleLength * 0.7, needleWidth);
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

    // Title with shadow
    ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
    ctx.shadowBlur = 2;
    ctx.fillStyle = fontHex;
    ctx.font = getFontSpec(Math.max(9, faceRadius * 0.13), { bold: true });
    ctx.textAlign = 'center';
    ctx.fillText(config.title, centerX, centerY + faceRadius * 0.35);
    ctx.shadowColor = 'transparent';

    // Value display with background
    const valueFontSize = Math.max(11, faceRadius * 0.16);
    const valueText = `${clampedValue.toFixed(config.value_digits)} ${config.units}`;
    ctx.font = getFontSpec(valueFontSize, { bold: true, monospace: true });
    const valueWidth = ctx.measureText(valueText).width;
    
    // Value background
    const valueY = config.display_value_at_180
      ? centerY + faceRadius * 0.7
      : centerY + faceRadius * 0.55;
    ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
    roundRect(ctx, centerX - valueWidth / 2 - 4, valueY - valueFontSize / 2 - 2, valueWidth + 8, valueFontSize + 4, 3);
    ctx.fill();
    
    // Value text
    ctx.fillStyle = fontHex;
    ctx.textBaseline = 'middle';
    ctx.fillText(valueText, centerX, valueY);
  };

  /** Draw sweep gauge (tachometer style) with improved visuals */
  const drawSweepGauge = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const centerX = width / 2;
    const centerY = height * 0.58;
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
    
    const startAngle = (startDeg - 90) * Math.PI / 180;
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
    const valuePercent = Math.max(0, Math.min(1, (clampedValue - config.min) / (config.max - config.min)));
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
    ctx.fillText(clampedValue.toFixed(config.value_digits), centerX, sweepValueY);
    ctx.shadowColor = 'transparent';

    // Title below value
    ctx.font = getFontSpec(Math.max(10, radius * 0.11), { bold: true });
    ctx.fillText(config.title, centerX, centerY + radius * 0.35);

    // Units
    ctx.fillStyle = tsColorToHex(config.trim_color);
    ctx.font = getFontSpec(Math.max(9, radius * 0.09));
    ctx.fillText(config.units, centerX, centerY + radius * 0.5);
  };

  /** Draw horizontal line gauge with improved visuals */
  const drawHorizontalLine = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
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
    const fillPercent = Math.max(0, Math.min(1, (clampedValue - config.min) / (config.max - config.min)));
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
    ctx.fillText(`${clampedValue.toFixed(config.value_digits)} ${config.units}`, width - padding, 3);
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

  /** Draw vertical dashed bar gauge with improved visuals */
  const drawVerticalDashedBar = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = 6;
    const labelHeight = height * 0.1;
    const barWidth = width * 0.5;
    const barHeight = height - padding * 2 - labelHeight * 2;
    const barX = (width - barWidth) / 2;
    const barY = labelHeight + padding;
    const numSegments = 12;
    const segmentHeight = barHeight / numSegments;
    const segmentGap = 3;

    // Background with gradient
    const bgGradient = ctx.createLinearGradient(0, 0, width, 0);
    const bgHex = tsColorToHex(config.back_color);
    bgGradient.addColorStop(0, darkenColor(bgHex, 5));
    bgGradient.addColorStop(0.5, lightenColor(bgHex, 5));
    bgGradient.addColorStop(1, darkenColor(bgHex, 5));
    ctx.fillStyle = bgGradient;
    ctx.fillRect(0, 0, width, height);

    // Title
    ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
    ctx.shadowBlur = 2;
    ctx.fillStyle = tsColorToHex(config.trim_color);
    ctx.font = getFontSpec(Math.max(9, labelHeight * 0.8), { bold: true });
    ctx.textAlign = 'center';
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, width / 2, 2);
    ctx.shadowColor = 'transparent';

    // Draw segments
    const fillPercent = (clampedValue - config.min) / (config.max - config.min);
    const filledSegments = Math.ceil(fillPercent * numSegments);

    for (let i = 0; i < numSegments; i++) {
      const segmentY = barY + (numSegments - 1 - i) * segmentHeight;
      const isFilled = i < filledSegments;
      const segmentPercent = i / numSegments;
      
      // Determine segment color based on value zones
      let segmentColor: string;
      const segmentValue = config.min + segmentPercent * (config.max - config.min);
      if (config.high_critical !== null && segmentValue >= config.high_critical) {
        segmentColor = isFilled ? tsColorToHex(config.critical_color) : '#401010';
      } else if (config.high_warning !== null && segmentValue >= config.high_warning) {
        segmentColor = isFilled ? tsColorToHex(config.warn_color) : '#403010';
      } else {
        // Use needle_color for normal range (typically green) 
        segmentColor = isFilled ? tsColorToHex(config.needle_color) : '#303030';
      }

      // Draw segment with gradient
      if (isFilled) {
        const segGradient = ctx.createLinearGradient(barX, 0, barX + barWidth, 0);
        segGradient.addColorStop(0, darkenColor(segmentColor, 15));
        segGradient.addColorStop(0.3, lightenColor(segmentColor, 20));
        segGradient.addColorStop(0.7, lightenColor(segmentColor, 15));
        segGradient.addColorStop(1, darkenColor(segmentColor, 10));
        ctx.fillStyle = segGradient;
        
        // Glow on top segment
        if (i === filledSegments - 1) {
          ctx.shadowColor = segmentColor;
          ctx.shadowBlur = 6;
        }
      } else {
        ctx.fillStyle = segmentColor;
      }
      
      roundRect(ctx, barX, segmentY + segmentGap / 2, barWidth, segmentHeight - segmentGap, 2);
      ctx.fill();
      ctx.shadowColor = 'transparent';
    }

    // Value at bottom
    ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
    ctx.shadowBlur = 2;
    ctx.fillStyle = tsColorToHex(config.font_color);
    ctx.font = getFontSpec(Math.max(11, labelHeight * 0.9), { bold: true, monospace: true });
    ctx.textAlign = 'center';
    ctx.textBaseline = 'bottom';
    ctx.fillText(`${clampedValue.toFixed(config.value_digits)}`, width / 2, height - 2);
    ctx.shadowColor = 'transparent';
  };

  /** Draw horizontal dashed bar gauge */
  const drawHorizontalDashedBar = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    const padding = 6;
    const titleHeight = height * 0.15;
    const valueHeight = height * 0.15;
    const barHeight = height * 0.35;
    const barWidth = width - padding * 2;
    const barX = padding;
    const barY = titleHeight + (height - titleHeight - valueHeight - barHeight) / 2;
    const numSegments = 14;
    const segmentWidth = barWidth / numSegments;
    const segmentGap = 3;

    // Background
    const bgGradient = ctx.createLinearGradient(0, 0, 0, height);
    const bgHex = tsColorToHex(config.back_color);
    bgGradient.addColorStop(0, darkenColor(bgHex, 5));
    bgGradient.addColorStop(0.5, lightenColor(bgHex, 5));
    bgGradient.addColorStop(1, darkenColor(bgHex, 5));
    ctx.fillStyle = bgGradient;
    ctx.fillRect(0, 0, width, height);

    // Title
    ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
    ctx.shadowBlur = 2;
    ctx.fillStyle = tsColorToHex(config.trim_color);
    ctx.font = getFontSpec(Math.max(9, titleHeight * 0.6), { bold: true });
    ctx.textAlign = 'center';
    ctx.textBaseline = 'top';
    ctx.fillText(config.title, width / 2, 2);
    ctx.shadowColor = 'transparent';

    const fillPercent = (clampedValue - config.min) / (config.max - config.min);
    const filledSegments = Math.ceil(fillPercent * numSegments);

    for (let i = 0; i < numSegments; i++) {
      const segmentX = barX + i * segmentWidth;
      const isFilled = i < filledSegments;
      const segmentPercent = i / numSegments;
      const segmentValue = config.min + segmentPercent * (config.max - config.min);

      let segmentColor: string;
      if (config.high_critical !== null && segmentValue >= config.high_critical) {
        segmentColor = isFilled ? tsColorToHex(config.critical_color) : '#401010';
      } else if (config.high_warning !== null && segmentValue >= config.high_warning) {
        segmentColor = isFilled ? tsColorToHex(config.warn_color) : '#403010';
      } else {
        segmentColor = isFilled ? tsColorToHex(config.needle_color) : '#303030';
      }

      if (isFilled) {
        const segGradient = ctx.createLinearGradient(segmentX, 0, segmentX + segmentWidth, 0);
        segGradient.addColorStop(0, darkenColor(segmentColor, 15));
        segGradient.addColorStop(0.3, lightenColor(segmentColor, 20));
        segGradient.addColorStop(0.7, lightenColor(segmentColor, 15));
        segGradient.addColorStop(1, darkenColor(segmentColor, 10));
        ctx.fillStyle = segGradient;

        if (i === filledSegments - 1) {
          ctx.shadowColor = segmentColor;
          ctx.shadowBlur = 6;
        }
      } else {
        ctx.fillStyle = segmentColor;
      }

      roundRect(
        ctx,
        segmentX + segmentGap / 2,
        barY,
        segmentWidth - segmentGap,
        barHeight,
        2
      );
      ctx.fill();
      ctx.shadowColor = 'transparent';
    }

    // Value
    ctx.shadowColor = 'rgba(0, 0, 0, 0.4)';
    ctx.shadowBlur = 2;
    ctx.fillStyle = tsColorToHex(config.font_color);
    ctx.font = getFontSpec(Math.max(11, valueHeight * 0.6), { bold: true, monospace: true });
    ctx.textAlign = 'center';
    ctx.textBaseline = 'bottom';
    ctx.fillText(`${clampedValue.toFixed(config.value_digits)}`, width / 2, height - 2);
    ctx.shadowColor = 'transparent';
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
    const valuePercent = (clampedValue - config.min) / (config.max - config.min);
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
    ctx.fillText(`${clampedValue.toFixed(config.value_digits)} ${config.units}`, width - padding, 3);
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
    ctx.fillText(`${clampedValue.toFixed(config.value_digits)} ${config.units}`, width - padding, 3);
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

    // Generate sample data showing current value with some history
    // In a real implementation, this would use a history buffer
    const numPoints = 50;
    const points: { x: number; y: number }[] = [];
    const valuePercent = (clampedValue - config.min) / (config.max - config.min);
    
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
    const normalizedValue = Math.max(0, Math.min(1, (clampedValue - config.min) / range));
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
    ctx.fillText(clampedValue.toFixed(config.value_digits), centerX, centerY - radius * 0.3);
    
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
    const normalizedValue = Math.max(0, Math.min(1, (clampedValue - config.min) / range));
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
    ctx.fillText(clampedValue.toFixed(config.value_digits), centerX, centerY - radius * 0.35);
    
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
      if (segmentValue > clampedValue) {
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
    ctx.fillText(clampedValue.toFixed(config.value_digits), centerX, centerY);
    
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
      
      if (segmentValue <= clampedValue) {
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
    ctx.fillText(clampedValue.toFixed(config.value_digits), centerX, centerY);
    
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
    const normalizedValue = (clampedValue - config.min) / (config.max - config.min);
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
    ctx.fillText(`${clampedValue.toFixed(config.value_digits)}${config.units || '%'}`, centerX, centerY + radius * 0.35);
    
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
    const normalizedValue = (clampedValue - config.min) / (config.max - config.min);
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
    ctx.fillText(clampedValue.toFixed(0), centerX, centerY + radius * 0.35);
    
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
 * - Value change is below threshold (0.5% of gauge range)
 * 
 * This prevents unnecessary canvas redraws during high-frequency realtime updates.
 */
const TsGauge = React.memo(TsGaugeInner, (prevProps, nextProps) => {
  // Always re-render if config changes
  if (prevProps.config !== nextProps.config) {
    return false;
  }
  
  // Always re-render if embeddedImages change
  if (prevProps.embeddedImages !== nextProps.embeddedImages) {
    return false;
  }
  
  // Skip re-render if value change is below threshold
  const range = prevProps.config.max - prevProps.config.min;
  const threshold = range * (VALUE_CHANGE_THRESHOLD_PERCENT / 100);
  if (Math.abs(prevProps.value - nextProps.value) < threshold) {
    return true; // Props are equal, skip re-render
  }
  
  return false; // Props changed, re-render
});

export default TsGauge;
