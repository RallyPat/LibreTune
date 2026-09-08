/**
 * Binary search for the index of the item in a time-sorted array whose
 * timestamp is nearest to `t`.
 *
 * `data` must be sorted ascending by the value `getTime` returns (as time-series
 * samples naturally are). Runs in O(log n) instead of the O(n) linear scan a
 * naive "closest point" search would need — this matters for callers invoked
 * at high frequency (e.g. realtime/playback ticks) over large sample arrays.
 *
 * Returns 0 for an empty array; callers should guard `data.length === 0`
 * before indexing with the result.
 */
export function nearestIndex<T>(data: readonly T[], t: number, getTime: (item: T) => number): number {
  let lo = 0;
  let hi = data.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (getTime(data[mid]) < t) lo = mid + 1;
    else hi = mid;
  }
  if (lo > 0 && Math.abs(getTime(data[lo - 1]) - t) < Math.abs(getTime(data[lo]) - t)) return lo - 1;
  return lo;
}
