/**
 * `useGaugeRenderer` — the rendering host shared by every `<TsGauge>`.
 *
 * Owns:
 *  - the canvas element ref
 *  - DPR-aware backing-store sizing via `ResizeObserver`
 *  - the requestAnimationFrame loop and its lerp-based smoothing
 *  - the imperative Zustand-store read each frame (no React re-render)
 *  - the 100 ms idle-watchdog that catches new store values once the
 *    rAF loop has converged
 *
 * The caller supplies a `paint` callback that draws ONE frame given
 * the current display value. The callback is stored in a ref so the
 * caller can change it across renders without restarting the rAF loop.
 *
 * This module does NOT know about `GaugePainter` types or any
 * specific gauge style — it is a pure renderer host.
 */

import { useEffect, useRef } from 'react';
import type { TsGaugeConfig } from '../dashboards/dashTypes';
import { useRealtimeStore } from '../../stores/realtimeStore';
import { getGaugeOverride } from '../../stores/gaugeOverride';

/** Lerp factor per animation frame (~280ms to converge at 60fps). */
const ANIMATION_LERP = 0.25;

/**
 * Frame limiter: cap drawing to ~30fps per gauge.
 *
 * With 10+ gauges each running at 60fps, the browser can't keep up
 * with 600+ canvas draws per second (each AnalogGauge draw creates
 * gradients, arcs, text = 2-5ms). 30fps per gauge → ~300/sec total,
 * well within budget.
 */
const DRAW_INTERVAL_MS = 33;

/** Function the host calls to draw one frame. */
export type GaugePaintFn = (
  ctx: CanvasRenderingContext2D,
  cssW: number,
  cssH: number,
  displayValue: number,
  peakValue: number,
) => void;

export interface UseGaugeRendererOptions {
  config: TsGaugeConfig;
  /** Block the rAF loop from starting until embedded fonts/images have loaded. */
  enabled: boolean;
  /** Per-frame painter — called with the current display value. */
  paint: GaugePaintFn;
  /**
   * When true, the rAF loop keeps running at a throttled rate even after the
   * animated value has converged. Required for time-series painters (LineGraph,
   * Histogram, MultiChannelTrend) so the trace keeps scrolling when the channel
   * value is steady — the underlying history buffer receives a new sample every
   * tick, but the visual only updates if we keep repainting.
   */
  continuousRender?: boolean;
}

export interface UseGaugeRendererResult {
  /** Attach this to the `<canvas>` element. */
  canvasRef: React.RefObject<HTMLCanvasElement>;
  /**
   * Live ref to the smoothly-animated display value. Helpers that need the
   * current value outside the paint callback (e.g. threshold-zone coloring)
   * read `displayValueRef.current` directly.
   */
  displayValueRef: React.MutableRefObject<number>;
  /**
   * Persistent peak (maximum) of the display value since this gauge
   * mounted. Painters consult this when `config.peak_hold === true` to
   * draw a TS-style peak marker. Resets when the gauge is reseated
   * (component remount).
   */
  peakValueRef: React.MutableRefObject<number>;
}

export function useGaugeRenderer(opts: UseGaugeRendererOptions): UseGaugeRendererResult {
  const { config, enabled, paint, continuousRender } = opts;

  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Initial clamped value used to seed both display and target on first render.
  const initialClamped = config.peg_limits
    ? Math.max(config.min, Math.min(config.max, config.min))
    : config.min;

  // displayValueRef holds the CURRENTLY DISPLAYED (smoothly animated) value.
  const displayValueRef = useRef(initialClamped);
  // peakValueRef holds the maximum value ever observed by the gauge —
  // used for the optional `peak_hold` marker.
  const peakValueRef = useRef(initialClamped);
  // targetRef holds the ANIMATION TARGET — updated by store/override reads.
  const targetRef = useRef(initialClamped);

  // Stash the latest paint callback in a ref so we can swap it across
  // renders without tearing down the animation loop.
  const paintRef = useRef<GaugePaintFn>(paint);
  paintRef.current = paint;

  // "Kick" the animation loop. Set inside the main render effect; called
  // by the prop-sync and ResizeObserver effects to wake an idle loop.
  const startAnimationRef = useRef<(() => void) | null>(null);

  // Pending rAF ID — `null` when the loop is idle.
  const rafIdRef = useRef<number | null>(null);
  const lastDrawTimeRef = useRef(0);

  /**
   * Cached canvas dimensions — updated only by ResizeObserver, NOT every
   * frame. Setting `canvas.width/height` destroys and reallocates the GPU
   * buffer; doing it 600-1200×/sec across 10-20 gauges freezes the browser.
   */
  const canvasSizeRef = useRef<{ w: number; h: number; cssW: number; cssH: number }>({
    w: 0,
    h: 0,
    cssW: 0,
    cssH: 0,
  });

  // ResizeObserver — keeps the backing-store size in sync with CSS size.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const syncSize = () => {
      const rect = canvas.getBoundingClientRect();
      if (rect.width === 0 || rect.height === 0) return;
      const dpr = window.devicePixelRatio || 1;
      const newW = Math.round(rect.width * dpr);
      const newH = Math.round(rect.height * dpr);
      const cur = canvasSizeRef.current;
      if (cur.w !== newW || cur.h !== newH) {
        canvas.width = newW;
        canvas.height = newH;
        canvasSizeRef.current = { w: newW, h: newH, cssW: rect.width, cssH: rect.height };
        if (startAnimationRef.current) startAnimationRef.current();
      }
    };

    syncSize();
    const ro = new ResizeObserver(() => syncSize());
    ro.observe(canvas);
    return () => ro.disconnect();
  }, []);

  // Main animation/render effect.
  //
  // Self-contained: the loop reads the current target value imperatively each
  // frame (via `getState()`, NOT via subscribe) — first from the non-reactive
  // sweep/demo override map, then from the realtime store. This eliminates
  // fragile cross-effect ref sharing and keeps live data entirely out of
  // React's render path.
  useEffect(() => {
    if (!enabled) return;

    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    if (rafIdRef.current !== null) {
      cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = null;
    }

    // Channel lookup variables — cached after first successful resolution.
    const channel = config.output_channel || '';
    const channelLower = channel.toLowerCase();
    let resolvedKey: string | null = null;

    // Stop animating when within 0.1% of the gauge range of the target.
    const epsilon = Math.max((config.max - config.min) * 0.001, 0.01);

    /** Look up the channel value in the store (case-insensitive with caching). */
    const readStoreValue = (): number | undefined => {
      const channels = useRealtimeStore.getState().channels;
      if (resolvedKey !== null) {
        return channels[resolvedKey];
      }
      let val = channels[channel];
      if (val !== undefined) { resolvedKey = channel; return val; }
      val = channels[channelLower];
      if (val !== undefined) { resolvedKey = channelLower; return val; }
      // One-time full scan (O(n) keys, happens only once per gauge instance).
      for (const key of Object.keys(channels)) {
        if (key.toLowerCase() === channelLower) {
          resolvedKey = key;
          return channels[key];
        }
      }
      return undefined;
    };

    /** Current animation target: sweep/demo override first, then the store. */
    const readTargetValue = (): number | undefined =>
      getGaugeOverride(channel) ?? readStoreValue();

    /** Draw one frame using `displayValueRef.current` as the gauge value. */
    const drawFrame = () => {
      const { w, h, cssW, cssH } = canvasSizeRef.current;
      if (w === 0 || h === 0) return;
      const dpr = w / cssW;
      // DO NOT set canvas.width/height here — that destroys the GPU buffer.
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.scale(dpr, dpr);
      ctx.clearRect(0, 0, cssW, cssH);
      if (config.antialiasing_on === false) {
        ctx.imageSmoothingEnabled = false;
      } else {
        ctx.imageSmoothingEnabled = true;
        ctx.imageSmoothingQuality = 'high';
      }
      paintRef.current(ctx, cssW, cssH, displayValueRef.current, peakValueRef.current);
    };

    let loopActive = true;

    const animate = (timestamp: number) => {
      if (!loopActive) return;

      // Pick up new data each frame: override (sweep/demo) first, then store.
      if (channel) {
        const raw = readTargetValue();
        if (raw !== undefined) {
          const peg = config.peg_limits;
          const clamped = peg ? Math.max(config.min, Math.min(config.max, raw)) : raw;
          targetRef.current = clamped;
        }
      }

      const target = targetRef.current;
      if (target > peakValueRef.current) {
        peakValueRef.current = target;
      }
      const diff = target - displayValueRef.current;
      const keepAlive = continuousRender && channel;
      if (Math.abs(diff) > epsilon) {
        displayValueRef.current = displayValueRef.current + diff * ANIMATION_LERP;
        if (timestamp - lastDrawTimeRef.current >= DRAW_INTERVAL_MS) {
          drawFrame();
          lastDrawTimeRef.current = timestamp;
        }
        rafIdRef.current = requestAnimationFrame(animate);
      } else if (keepAlive) {
        // Time-series painters need continuous redraws so the trace scrolls
        // even when the channel value is constant. Snap to target and throttle
        // drawing to the configured frame interval to avoid burning CPU.
        displayValueRef.current = target;
        if (timestamp - lastDrawTimeRef.current >= DRAW_INTERVAL_MS) {
          drawFrame();
          lastDrawTimeRef.current = timestamp;
        }
        rafIdRef.current = requestAnimationFrame(animate);
      } else {
        // Snap to target and always draw final frame.
        displayValueRef.current = target;
        drawFrame();
        lastDrawTimeRef.current = timestamp;
        // Loop goes idle — the watchdog below will restart it if needed.
        rafIdRef.current = null;
      }
    };

    /** Kick the animation loop if it is not already running. */
    const kickAnimation = () => {
      if (loopActive && rafIdRef.current === null) {
        rafIdRef.current = requestAnimationFrame(animate);
      }
    };

    startAnimationRef.current = kickAnimation;

    // Initial kick.
    rafIdRef.current = requestAnimationFrame(animate);

    // Watchdog: when the rAF loop is idle (converged), poll the target
    // (override, then store) every 100ms. Costs ~one hash lookup per gauge
    // per tick.
    const watchdog = setInterval(() => {
      if (!loopActive || !channel) return;
      if (rafIdRef.current !== null) return; // Already animating.
      const raw = readTargetValue();
      if (raw !== undefined) {
        const peg = config.peg_limits;
        const clamped = peg ? Math.max(config.min, Math.min(config.max, raw)) : raw;
        if (Math.abs(clamped - displayValueRef.current) > epsilon) {
          targetRef.current = clamped;
          kickAnimation();
        }
      }
    }, 100);

    return () => {
      loopActive = false;
      startAnimationRef.current = null;
      clearInterval(watchdog);
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current);
        rafIdRef.current = null;
      }
    };
    // Note: `paint` is intentionally omitted — it's consumed via a ref so
    // the effect only restarts when channel/range semantics or the
    // readiness gate change.
  }, [
    enabled,
    config.output_channel,
    config.min,
    config.max,
    config.peg_limits,
    config.antialiasing_on,
    continuousRender,
  ]);

  return { canvasRef, displayValueRef, peakValueRef };
}
