/**
 * Painter context + registry — Phase B3 scaffolding for migrating
 * the inline `drawXxx` closures inside `TsGauge.tsx` into per-painter
 * top-level pure functions.
 *
 * Migration is incremental: painters listed in `painterRegistry`
 * win; any `GaugePainter` not in the registry falls through to the
 * existing inline closures in `TsGauge.tsx`. This keeps every
 * commit small and bounded in visual-regression risk.
 */

import type { TsGaugeConfig, TsColor, GaugePainter } from '../../dashboards/dashTypes';

/**
 * Everything a painter needs to render a single frame.
 *
 * The `value` field is the smoothly-animated display value (already
 * lerped by `useGaugeRenderer` — painters must NOT pull from any
 * other source).
 */
export interface PainterContext {
  ctx: CanvasRenderingContext2D;
  width: number;
  height: number;
  /** Smoothly-animated display value (lerp'd toward target). */
  value: number;
  /**
   * Persistent peak (maximum) the gauge has observed since mount,
   * threaded through by `useGaugeRenderer`. Painters are free to ignore
   * it; analog/sweep painters render a small marker at this position
   * when `config.peak_hold === true` (Plan D-5).
   */
  peakValue: number;
  config: TsGaugeConfig;
  /** True while displaying a TunerStudio dashboard in legacy bitmap mode. */
  legacyMode: boolean;
  /** Pre-resolved embedded background image (or null). */
  bgImage: HTMLImageElement | null;
  /** Pre-resolved embedded needle image (or null). */
  needleImage: HTMLImageElement | null;
  /** Threshold-aware text/needle color. */
  getValueColor: () => TsColor;
  getFontSpec: (size: number, options?: { bold?: boolean; monospace?: boolean }) => string;
  getFontFamily: (preferMonospace?: boolean) => string;
  getEmbeddedImage: (name: string | null | undefined) => HTMLImageElement | null;
}

/** A self-contained painter: draws one frame from a `PainterContext`. */
export type Painter = (pctx: PainterContext) => void;

/**
 * Registry of migrated painters.
 *
 * `TsGauge.tsx`'s `paint` callback consults this map first; if the
 * configured `gauge_painter` is not present, it falls back to the
 * legacy inline closure for that painter. This lets us migrate
 * painters one or two at a time with zero behavior change for the
 * rest.
 */
export const painterRegistry: Partial<Record<GaugePainter, Painter>> = {};

/**
 * Register a painter under one or more `GaugePainter` keys.
 *
 * Some painters (e.g. `AnalogGauge` / `BasicAnalogGauge` /
 * `CircleAnalogGauge`) intentionally share an implementation — pass
 * an array to register all of them at once.
 */
export function registerPainter(keys: GaugePainter | GaugePainter[], painter: Painter): void {
  const arr = Array.isArray(keys) ? keys : [keys];
  for (const key of arr) {
    painterRegistry[key] = painter;
  }
}
