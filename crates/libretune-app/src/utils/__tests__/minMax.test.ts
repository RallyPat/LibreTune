import { describe, it, expect } from 'vitest';
import { minMax, arrayMin, arrayMax } from '../minMax';

describe('minMax', () => {
  it('returns {min: Infinity, max: -Infinity} for an empty array', () => {
    expect(minMax([])).toEqual({ min: Infinity, max: -Infinity });
  });

  it('computes min and max for a normal array', () => {
    expect(minMax([3, 1, 4, 1, 5, 9, 2, 6])).toEqual({ min: 1, max: 9 });
  });

  it('handles negative and single-element arrays', () => {
    expect(minMax([-5])).toEqual({ min: -5, max: -5 });
    expect(minMax([-3, -1, -7, -2])).toEqual({ min: -7, max: -1 });
  });

  it('skips NaN values', () => {
    expect(minMax([1, NaN, 5, NaN, 3])).toEqual({ min: 1, max: 5 });
    expect(minMax([NaN, NaN])).toEqual({ min: Infinity, max: -Infinity });
  });

  it('does not throw for a very large array and returns the correct min/max', () => {
    const size = 200_000;
    const arr = new Float64Array(size);
    for (let i = 0; i < size; i++) {
      arr[i] = i;
    }
    // Plant a known min/max away from the ends.
    arr[100] = -42;
    arr[size - 100] = 999_999;

    expect(() => minMax(arr)).not.toThrow();
    expect(minMax(arr)).toEqual({ min: -42, max: 999_999 });
  });

  it('does not throw for a very large plain array', () => {
    const size = 200_000;
    const arr = Array.from({ length: size }, (_, i) => i);

    expect(() => minMax(arr)).not.toThrow();
    expect(minMax(arr)).toEqual({ min: 0, max: size - 1 });
  });
});

describe('arrayMin / arrayMax', () => {
  it('return the min and max respectively', () => {
    const values = [10, -2, 33, 4];
    expect(arrayMin(values)).toBe(-2);
    expect(arrayMax(values)).toBe(33);
  });

  it('return Infinity/-Infinity for empty input', () => {
    expect(arrayMin([])).toBe(Infinity);
    expect(arrayMax([])).toBe(-Infinity);
  });
});
