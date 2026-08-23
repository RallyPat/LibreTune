/**
 * Tests for the time-based EMA, spike detection, stagger slots (issue #82)
 * and the fixed-width value formatting used by dial-centered painters.
 */
import { describe, it, expect } from 'vitest';
import {
  emaStep,
  isSpike,
  staggerSlotForChannel,
  STAGGER_SLOTS,
  EMA_TAU_MS,
  SPIKE_RANGE_FRACTION,
} from '../ema';
import { steadyValueText } from '../drawUtils';

describe('emaStep', () => {
  it('returns prev when no time has elapsed', () => {
    expect(emaStep(5, 10, 0)).toBe(5);
    expect(emaStep(5, 10, -16)).toBe(5);
  });

  it('covers ~63% of a step after one time constant', () => {
    // dt=100ms stays under the hidden-tab clamp; use a matching tau.
    const next = emaStep(0, 100, 100, 100);
    expect(next).toBeCloseTo(100 * (1 - Math.exp(-1)), 5);
  });

  it('converges identically regardless of frame rate (time-based, not frame-based)', () => {
    // Same 100 ms of simulated time: 100 fps (10×10ms) vs 10 fps (1×100ms).
    let fast = 0;
    for (let i = 0; i < 10; i++) fast = emaStep(fast, 100, 10);
    const slow = emaStep(0, 100, 100);
    const expected = 100 * (1 - Math.exp(-100 / EMA_TAU_MS));
    expect(fast).toBeCloseTo(expected, 5);
    expect(slow).toBeCloseTo(expected, 5);
  });

  it('clamps huge deltas (tab was hidden) instead of snapping', () => {
    // 10 s hidden should behave like a 100 ms step, not a full snap to target.
    const next = emaStep(0, 100, 10_000);
    expect(next).toBeCloseTo(emaStep(0, 100, 100), 10);
    expect(next).toBeLessThan(90);
  });

  it('honours a custom tau', () => {
    const fastTau = emaStep(0, 100, 100, 50);
    const slowTau = emaStep(0, 100, 100, 500);
    expect(fastTau).toBeGreaterThan(slowTau);
  });
});

describe('isSpike', () => {
  // Range 0..100 → default threshold is 15.
  it('flags deviations beyond the range fraction', () => {
    expect(isSpike(120, 100, 0, 100)).toBe(true); // 20 > 15
    expect(isSpike(80, 100, 0, 100)).toBe(true); // 20 > 15
    expect(isSpike(110, 100, 0, 100)).toBe(false); // 10 < 15
  });

  it('boundary: exactly at the threshold is not a spike', () => {
    const threshold = (100 - 0) * SPIKE_RANGE_FRACTION;
    expect(isSpike(100 + threshold, 100, 0, 100)).toBe(false);
  });

  it('never spikes for a zero/negative range', () => {
    expect(isSpike(100, 0, 50, 50)).toBe(false);
    expect(isSpike(100, 0, 100, 0)).toBe(false);
  });

  it('respects a custom fraction', () => {
    expect(isSpike(105, 100, 0, 100, 0.01)).toBe(true);
    expect(isSpike(105, 100, 0, 100, 0.5)).toBe(false);
  });
});

describe('staggerSlotForChannel', () => {
  it('is deterministic for the same channel', () => {
    expect(staggerSlotForChannel('rpm')).toBe(staggerSlotForChannel('rpm'));
    expect(staggerSlotForChannel('lambda')).toBe(staggerSlotForChannel('lambda'));
  });

  it('stays within the slot count', () => {
    for (const ch of ['rpm', 'map', 'tps', 'lambda', 'clt', 'iat', 'afr', 'advance']) {
      const slot = staggerSlotForChannel(ch);
      expect(slot).toBeGreaterThanOrEqual(0);
      expect(slot).toBeLessThan(STAGGER_SLOTS);
    }
  });

  it('spreads distinct channels across more than one slot', () => {
    const slots = new Set(
      ['rpm', 'map', 'tps', 'lambda', 'clt', 'iat', 'afr', 'advance'].map(staggerSlotForChannel),
    );
    expect(slots.size).toBeGreaterThan(1);
  });

  it('falls back to a rotating counter for channel-less (demo) gauges', () => {
    const a = staggerSlotForChannel('');
    const b = staggerSlotForChannel('');
    expect(a).not.toBe(b);
  });
});

describe('steadyValueText', () => {
  it('pads to the worst-case width of the range', () => {
    // Range -40..220 with 1 digit: worst is "-40.0" (5 chars).
    expect(steadyValueText(100, 1, -40, 220)).toBe('100.0');
    expect(steadyValueText(5, 1, -40, 220)).toBe('  5.0');
  });

  it('keeps constant width across sign and digit-count changes', () => {
    const a = steadyValueText(-12.3, 1, -40, 220);
    const b = steadyValueText(7.8, 1, -40, 220);
    const c = steadyValueText(220, 1, -40, 220);
    expect(a.length).toBe(b.length);
    expect(b.length).toBe(c.length);
  });

  it('never truncates values wider than the worst case', () => {
    // Out-of-range values render in full (padStart never truncates).
    expect(steadyValueText(1234, 1, 0, 100)).toBe('1234.0');
  });
});
