/**
 * `useGaugeRenderer` — the rendering host shared by every `<TsGauge>`.
 *
 * Owns:
 *  - the canvas element ref
 *  - DPR-aware backing-store sizing via `ResizeObserver`
 *  - the requestAnimationFrame loop and its time-based EMA smoothing
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
import { seedPeak, nextPeakState, type PeakState } from './peakTracking';
import { emaStep, isSpike, staggerSlotForChannel, STAGGER_SLOTS, SPIKE_FLASH_MS } from './ema';
import { getDrawIntervalMs } from './renderSettings';

/**
 * Frame limiter: per-gauge drawing is capped at the user-configured
 * dashboard refresh rate (10–30 Hz — see `renderSettings`), and the redraw
 * timers are phase-staggered so 10+ gauges don't all paint on the same frame
 * (issue #82: extreme CPU load). Value smoothing is a time-constant EMA
 * (`ema.ts`), so needle motion looks identical at any refresh rate; the old
 * per-frame lerp converged slower at low frame rates.
 */

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
  /** Prop-supplied value; only consulted when `overrideStore` is true. */
  value: number;
  /** When true, the value prop drives the gauge instead of the store (sweep/demo). */
  overrideStore: boolean;
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
  /**
   * Optional output ref: the loop writes `true` while a transient-spike
   * flash is active (issue #82 peak detection). TsGauge passes a ref its
   * paint callback closes over, so painters can tint the value without the
   * loop ever restarting.
   */
  spikeActiveRef?: React.MutableRefObject<boolean>;
}

export interface UseGaugeRendererResult {
  /** Attach this to the `<canvas>` element. */
  canvasRef: React.RefObject<HTMLCanvasElement>;
  /**
   * Live ref to the smoothly-animated display value. Painters that
   * still live as nested closures inside the host component can read
   * `displayValueRef.current` directly instead of using the value
   * passed into the `paint` callback. Equivalent for now; will go
   * away once every painter is a top-level pure function.
   */
  displayValueRef: React.MutableRefObject<number>;
  /**
   * Persistent peak (maximum) of the display value. Painters consult this
   * when `config.peak_hold === true` to draw a TS-style peak marker.
   * Seeded from the gauge's persisted `history_value` (issue #129) and,
   * when `history_delay > 0`, decayed back to the present value once the
   * hold expires (`peakTracking.ts`). Resets when the gauge is reseated
   * (component remount).
   */
  peakValueRef: React.MutableRefObject<number>;
}

export function useGaugeRenderer(opts: UseGaugeRendererOptions): UseGaugeRendererResult {
  const { config, value, overrideStore, enabled, paint, continuousRender } = opts;

  // Spike-flash output: caller-supplied ref wins; internal fallback otherwise.
  const internalSpikeRef = useRef(false);
  const spikeRef = opts.spikeActiveRef ?? internalSpikeRef;

  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Initial clamped value used to seed both display and target on first render.
  const initialClamped = config.peg_limits
    ? Math.max(config.min, Math.min(config.max, value))
    : value;

  // displayValueRef holds the CURRENTLY DISPLAYED (smoothly animated) value.
  const displayValueRef = useRef(initialClamped);
  // peakValueRef holds the peak-hold marker value — seeded from the .dash
  // file's persisted HistoryValue, ratcheted upward by the animation loop,
  // and decayed per HistoryDelay (see peakTracking.ts).
  const peakValueRef = useRef(seedPeak(config, initialClamped));
  const peakStateRef = useRef<PeakState>({ peak: peakValueRef.current, lastNewPeakAt: null });
  // targetRef holds the ANIMATION TARGET — updated by store reads or the
  // sweep/demo prop-sync effect below.
  const targetRef = useRef(initialClamped);

  // Track overrideStore in a ref so the animation loop closure always
  // sees the current value without forcing the effect to restart.
  const overrideStoreRef = useRef(overrideStore);
  overrideStoreRef.current = overrideStore;

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
  /** Next timestamp at which this gauge may draw (phase-staggered gate). */
  const nextDrawAtRef = useRef(0);
  /** rAF timestamp of the previous animate() tick — drives the time-based EMA. */
  const lastFrameTimeRef = useRef(0);
  /** Spike-flash expiry timestamp on the rAF clock (0 = inactive). */
  const spikeUntilRef = useRef(0);
  /** Phase slot for redraw staggering, claimed once per mount. */
  const staggerSlotRef = useRef<number | null>(null);

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

  // Sync targetRef when overrideStore is true (sweep/demo mode).
  useEffect(() => {
    if (!overrideStore) return;
    const clamped = config.peg_limits
      ? Math.max(config.min, Math.min(config.max, value))
      : value;
    targetRef.current = clamped;
    if (startAnimationRef.current) startAnimationRef.current();
  }, [config.peg_limits, config.min, config.max, value, overrideStore]);

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
  // Self-contained: the loop reads the store value imperatively each frame
  // (via `getState()`, NOT via subscribe). This eliminates fragile cross-
  // effect ref sharing that previously caused gauges to freeze when the
  // animation effect re-ran while the subscription still held a stale
  // `startAnimationRef`.
  //
  // When `overrideStore` is true (sweep/demo), `targetRef` is driven by
  // the prop-sync effect above; the loop still runs but skips the store read.
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

    // Claim a phase slot once so this gauge's redraws stay evenly offset
    // from the others (issue #82 staggered redraw timers).
    if (staggerSlotRef.current === null) {
      staggerSlotRef.current = staggerSlotForChannel(channel);
    }

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

      // Normal mode: read the store each frame to pick up new data.
      if (!overrideStoreRef.current && channel) {
        const raw = readStoreValue();
        if (raw !== undefined) {
          const peg = config.peg_limits;
          const clamped = peg ? Math.max(config.min, Math.min(config.max, raw)) : raw;
          targetRef.current = clamped;
        }
      }

      const target = targetRef.current;
      // Peak-hold: ratchet upward, and once `history_delay` has elapsed
      // without a new peak, let the marker fall back to the present value.
      peakStateRef.current = nextPeakState(
        peakStateRef.current,
        target,
        timestamp,
        config.history_delay,
      );
      peakValueRef.current = peakStateRef.current.peak;

      // Elapsed time since the previous tick — drives the time-based EMA.
      // First tick assumes one 60fps frame so the EMA starts moving.
      const dt =
        lastFrameTimeRef.current === 0 ? 1000 / 60 : timestamp - lastFrameTimeRef.current;
      lastFrameTimeRef.current = timestamp;

      // Transient spike detection (issue #82): if the raw value jumped far
      // from the smoothed display value, flash briefly so short pulses stay
      // visible even though the EMA filters them out of the needle position.
      if (isSpike(target, displayValueRef.current, config.min, config.max)) {
        spikeUntilRef.current = timestamp + SPIKE_FLASH_MS;
      }
      spikeRef.current = timestamp < spikeUntilRef.current;

      const diff = target - displayValueRef.current;
      const keepAlive = continuousRender && !overrideStoreRef.current && channel;

      /**
       * Phase-staggered draw gate: draws at most once per configured
       * refresh interval, offset by this gauge's stagger slot so gauges
       * spread their redraws evenly across the interval.
       */
      const drawDue = () => {
        if (timestamp < nextDrawAtRef.current) return;
        drawFrame();
        lastDrawTimeRef.current = timestamp;
        const interval = getDrawIntervalMs();
        if (nextDrawAtRef.current === 0) {
          // First draw: apply the phase offset.
          nextDrawAtRef.current =
            timestamp + ((staggerSlotRef.current ?? 0) * interval) / STAGGER_SLOTS;
        } else {
          nextDrawAtRef.current = timestamp + interval;
        }
      };

      if (Math.abs(diff) > epsilon) {
        // Time-constant EMA: converges identically at any refresh rate.
        displayValueRef.current = emaStep(displayValueRef.current, target, dt);
        drawDue();
        rafIdRef.current = requestAnimationFrame(animate);
      } else if (keepAlive) {
        // Time-series painters need continuous redraws so the trace scrolls
        // even when the channel value is constant. Snap to target and throttle
        // drawing to the configured frame interval to avoid burning CPU.
        displayValueRef.current = target;
        drawDue();
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

    // Watchdog: when the rAF loop is idle (converged), poll the store
    // every 100ms. Costs ~one hash lookup per gauge per tick.
    const watchdog = setInterval(() => {
      if (!loopActive || overrideStoreRef.current || !channel) return;
      if (rafIdRef.current === null) {
        // Peak-hold decay must advance even while the loop is idle
        // (steady value) — otherwise the marker would hold forever.
        // performance.now() shares the rAF clock.
        const next = nextPeakState(
          peakStateRef.current,
          targetRef.current,
          performance.now(),
          config.history_delay,
        );
        if (next !== peakStateRef.current) {
          peakStateRef.current = next;
          peakValueRef.current = next.peak;
          drawFrame();
        }
      }
      // Spike-flash expiry must repaint even while the loop is idle —
      // performance.now() shares the rAF clock used for spikeUntilRef.
      const spikeOn = performance.now() < spikeUntilRef.current;
      if (spikeOn !== spikeRef.current) {
        spikeRef.current = spikeOn;
        drawFrame();
      }
      const raw = readStoreValue();
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
    // Note: `value` and `paint` are intentionally omitted — they're
    // consumed via refs so the effect only restarts when channel/range
    // semantics or the readiness gate change.
  }, [
    enabled,
    config.output_channel,
    config.min,
    config.max,
    config.peg_limits,
    config.antialiasing_on,
    config.history_delay,
    continuousRender,
  ]);

  return { canvasRef, displayValueRef, peakValueRef };
}
