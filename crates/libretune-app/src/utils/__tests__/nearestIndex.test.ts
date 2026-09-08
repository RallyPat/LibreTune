import { describe, expect, it } from 'vitest';
import { nearestIndex } from '../nearestIndex';

describe('nearestIndex', () => {
  const data = [
    { t: 0, v: 'a' },
    { t: 10, v: 'b' },
    { t: 20, v: 'c' },
    { t: 30, v: 'd' },
    { t: 100, v: 'e' },
  ];
  const getTime = (item: { t: number }) => item.t;

  it('returns the exact index when t matches a sample exactly', () => {
    expect(nearestIndex(data, 20, getTime)).toBe(2);
  });

  it('rounds to the nearer neighbor when t falls between samples', () => {
    expect(nearestIndex(data, 24, getTime)).toBe(2); // closer to 20 than 30
    expect(nearestIndex(data, 26, getTime)).toBe(3); // closer to 30 than 20
  });

  it('clamps to the first index when t is before the first sample', () => {
    expect(nearestIndex(data, -50, getTime)).toBe(0);
  });

  it('clamps to the last index when t is after the last sample', () => {
    expect(nearestIndex(data, 1000, getTime)).toBe(4);
  });

  it('returns 0 for a single-element array', () => {
    expect(nearestIndex([{ t: 5, v: 'x' }], 999, getTime)).toBe(0);
  });

  it('matches a brute-force linear scan across many random queries', () => {
    const big = Array.from({ length: 2000 }, (_, i) => ({ t: i * 3, v: i }));
    const bruteForce = (t: number) => {
      let closest = 0;
      for (let i = 1; i < big.length; i++) {
        if (Math.abs(big[i].t - t) < Math.abs(big[closest].t - t)) closest = i;
      }
      return closest;
    };
    for (const t of [-10, 0, 1, 2, 4, 3001, 5999, 6001, 3000000]) {
      expect(nearestIndex(big, t, getTime)).toBe(bruteForce(t));
    }
  });
});
