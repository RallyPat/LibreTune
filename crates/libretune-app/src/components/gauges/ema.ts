/**
 * `ema` — time-based exponential moving average + transient-spike detection
 * for gauge rendering (issue #82).
 *
 * Replaces the old frame-count lerp (`value += diff * 0.25` per rAF tick),
 * whose smoothing constant varied with frame rate: at 60 fps it converged in
 * ~280 ms, at 10 fps it would have taken ~1.7 s. A time-constant EMA
 * (`alpha = 1 - exp(-dt/tau)`) converges identically at any refresh rate,
 * which is what makes the configurable 10–30 Hz redraw cap feel smooth.
 *
 * Spike detection answers the issue's "peak detect is important to figure out
 * problems if short pulses can happen": when the raw channel value jumps away
 * from the smoothed display value by more than a fraction of the gauge range,
 * the gauge flashes (critical color) briefly so a transient is visible even
 * though the EMA itself filters it out of the needle position.
 *
 * Pure functions, no canvas/timers — same testability pattern as
 * `peakTracking.ts`.
 */

/** EMA time constant: ~63% of a step is covered per 120 ms of elapsed time. */
export const EMA_TAU_MS = 120;

/** Fraction of gauge range that counts as a transient spike. */
export const SPIKE_RANGE_FRACTION = 0.15;

/** How long the spike flash stays on after the last spiking sample (ms). */
export const SPIKE_FLASH_MS = 400;

/**
 * Advance the EMA one step. `dtMs` is the elapsed time since the previous
 * step; pass the rAF timestamp delta. `dtMs <= 0` returns `prev` unchanged;
 * very large deltas (tab was hidden) are clamped to 100 ms so a backgrounded
 * tab doesn't snap the needle on return.
 */
export function emaStep(
  prev: number,
  target: number,
  dtMs: number,
  tauMs: number = EMA_TAU_MS,
): number {
  if (dtMs <= 0) return prev;
  if (dtMs > 100) dtMs = 100;
  const alpha = 1 - Math.exp(-dtMs / tauMs);
  return prev + (target - prev) * alpha;
}

/** True when `raw` deviates from the smoothed value by more than `frac` of range. */
export function isSpike(
  raw: number,
  smoothed: number,
  min: number,
  max: number,
  frac: number = SPIKE_RANGE_FRACTION,
): boolean {
  const range = max - min;
  if (range <= 0) return false;
  return Math.abs(raw - smoothed) > range * frac;
}

/** Number of phase slots redraw timers are spread across (issue #82 staggering). */
export const STAGGER_SLOTS = 16;

let mountCounter = 0;

/**
 * Deterministic stagger slot for a gauge. Gauges bound to a channel hash to a
 * stable slot (same dashboard → same phasing every launch); channel-less demo
 * gauges fall back to a per-mount counter. The render loop multiplies the
 * slot by `drawInterval / STAGGER_SLOTS` to obtain its phase offset, so 16+
 * gauges spread their redraws evenly instead of all painting on one frame.
 */
export function staggerSlotForChannel(channel: string): number {
  if (!channel) {
    const slot = mountCounter % STAGGER_SLOTS;
    mountCounter += 1;
    return slot;
  }
  // djb2 hash — tiny, stable across runs.
  let h = 5381;
  for (let i = 0; i < channel.length; i++) {
    h = ((h << 5) + h + channel.charCodeAt(i)) | 0;
  }
  return Math.abs(h) % STAGGER_SLOTS;
}
