/**
 * TS Gauge Renderer
 * 
 * Renders gauges based on TS GaugePainter types.
 * Uses canvas for all gauge rendering with high-quality visual effects.
 * Wrapped in React.memo with custom comparator for performance optimization.
 */

import React, { useEffect, useState, useCallback } from 'react';
import { TsGaugeConfig, TsColor } from '../dashboards/dashTypes';
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

  /** Get color based on value thresholds. Honours `config.hysteresis` (Plan D-5):
   * once the gauge has entered a warning/critical zone, it stays there until
   * the displayed value backs off the threshold by at least `hysteresis` units.
   * Prevents flicker when a channel hovers right at a threshold.
   */
  const lastZoneRef = React.useRef<'normal' | 'warn' | 'critical'>('normal');
  const getValueColor = useCallback((): TsColor => {
    const v = displayValueRef.current;
    const h = config.hysteresis ?? 0;
    const prev = lastZoneRef.current;
    // Compute base zone with widened bounds when leaving a hot zone.
    const inCritical = (
      (config.high_critical != null && v >= config.high_critical - (prev === 'critical' ? h : 0)) ||
      (config.low_critical != null && v <= config.low_critical + (prev === 'critical' ? h : 0))
    );
    const inWarn = !inCritical && (
      (config.high_warning != null && v >= config.high_warning - (prev === 'warn' || prev === 'critical' ? h : 0)) ||
      (config.low_warning != null && v <= config.low_warning + (prev === 'warn' || prev === 'critical' ? h : 0))
    );
    let zone: 'normal' | 'warn' | 'critical' = 'normal';
    if (inCritical) zone = 'critical';
    else if (inWarn) zone = 'warn';
    lastZoneRef.current = zone;
    if (zone === 'critical') return config.critical_color;
    if (zone === 'warn') return config.warn_color;
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
    (ctx: CanvasRenderingContext2D, cssW: number, cssH: number, displayValue: number, peakValue: number) => {
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
          peakValue,
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

      // 2. Fall back to migrated BasicReadout for any unknown painter.
      painterRegistry.BasicReadout?.({
        ctx,
        width: cssW,
        height: cssH,
        value: displayValue,
        peakValue,
        config,
        legacyMode,
        bgImage,
        needleImage,
        getValueColor,
        getFontSpec,
        getFontFamily,
        getEmbeddedImage,
      });
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
