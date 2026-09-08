import { describe, expect, it, vi } from 'vitest';
import { incrementalMap, EMPTY_INCREMENTAL_MAP_CACHE, type IncrementalMapCache } from '../incrementalMap';

describe('incrementalMap', () => {
  it('fully maps on the first call from an empty cache', () => {
    const source = [1, 2, 3];
    const mapFn = vi.fn((n: number) => n * 2);

    const result = incrementalMap(source, EMPTY_INCREMENTAL_MAP_CACHE, mapFn);

    expect(result.mapped).toEqual([2, 4, 6]);
    expect(mapFn).toHaveBeenCalledTimes(3);
  });

  it('reuses the cached mapped array when the source is unchanged', () => {
    const source = [1, 2, 3];
    const mapFn = vi.fn((n: number) => n * 2);
    const first = incrementalMap(source, EMPTY_INCREMENTAL_MAP_CACHE, mapFn);

    mapFn.mockClear();
    const second = incrementalMap(source, first, mapFn);

    expect(second.mapped).toBe(first.mapped); // same reference, not recomputed
    expect(mapFn).not.toHaveBeenCalled();
  });

  it('maps only the newly appended tail when source grows by appending', () => {
    const source1 = [1, 2, 3];
    const mapFn = vi.fn((n: number) => n * 2);
    const cache1 = incrementalMap(source1, EMPTY_INCREMENTAL_MAP_CACHE, mapFn);

    // Simulate `[...prev, ...fresh]`: same leading element references, plus new ones.
    const source2 = [...source1, 4, 5];
    mapFn.mockClear();
    const cache2 = incrementalMap(source2, cache1, mapFn);

    expect(cache2.mapped).toEqual([2, 4, 6, 8, 10]);
    // Only the two new elements should have been mapped, not the whole array.
    expect(mapFn).toHaveBeenCalledTimes(2);
    expect(mapFn).toHaveBeenCalledWith(4);
    expect(mapFn).toHaveBeenCalledWith(5);
    // The unchanged prefix objects are reused, not recreated.
    expect(cache2.mapped[0]).toBe(cache1.mapped[0]);
    expect(cache2.mapped[1]).toBe(cache1.mapped[1]);
    expect(cache2.mapped[2]).toBe(cache1.mapped[2]);
  });

  it('falls back to a full remap when the front of the array changes (e.g. a trimming cap)', () => {
    const source1 = [1, 2, 3, 4, 5];
    const mapFn = vi.fn((n: number) => n * 2);
    const cache1 = incrementalMap(source1, EMPTY_INCREMENTAL_MAP_CACHE, mapFn);

    // Simulate MAX_FRONTEND_SAMPLES trimming the oldest entry off the front.
    const source2 = [2, 3, 4, 5, 6];
    mapFn.mockClear();
    const cache2 = incrementalMap(source2, cache1, mapFn);

    expect(cache2.mapped).toEqual([4, 6, 8, 10, 12]);
    expect(mapFn).toHaveBeenCalledTimes(5); // full remap, not incremental
  });

  it('falls back to a full remap when the source array is replaced entirely', () => {
    const source1 = [1, 2, 3];
    const mapFn = vi.fn((n: number) => n * 2);
    const cache1 = incrementalMap(source1, EMPTY_INCREMENTAL_MAP_CACHE, mapFn);

    const source2 = [9, 8, 7]; // e.g. loading a different log file
    mapFn.mockClear();
    const cache2 = incrementalMap(source2, cache1, mapFn);

    expect(cache2.mapped).toEqual([18, 16, 14]);
    expect(mapFn).toHaveBeenCalledTimes(3);
  });

  it('handles shrinking to empty (e.g. Clear) without throwing', () => {
    const source1 = [1, 2, 3];
    const mapFn = (n: number) => n * 2;
    const cache1 = incrementalMap(source1, EMPTY_INCREMENTAL_MAP_CACHE, mapFn);

    const cache2 = incrementalMap([] as number[], cache1 as IncrementalMapCache<number, number>, mapFn);

    expect(cache2.mapped).toEqual([]);
  });
});
