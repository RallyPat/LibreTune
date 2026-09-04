/**
 * Cache produced by `incrementalMap`, threaded back in on the next call
 * (typically held in a `useRef`) so an unchanged/append-only source array
 * doesn't have to be remapped from scratch.
 */
export interface IncrementalMapCache<S, T> {
  source: readonly S[];
  mapped: T[];
}

export const EMPTY_INCREMENTAL_MAP_CACHE: IncrementalMapCache<never, never> = {
  source: [],
  mapped: [],
};

/**
 * Maps `source` to a new array via `mapFn`, reusing `cache.mapped` instead of
 * remapping every element whenever possible:
 *   - source unchanged (same length, same tail identity) -> returns the
 *     cached mapped array as-is.
 *   - source is `cache.source` with only new items appended (common for
 *     growing session logs) -> maps just the new tail and concatenates it
 *     onto the cached mapped prefix.
 *   - anything else (array replaced, front trimmed, etc.) -> falls back to a
 *     full remap.
 *
 * `source` must supply the same object references for elements it shares
 * with the previous call for the append-only fast path to be detected —
 * true of any array built via `[...prev, ...fresh]`.
 */
export function incrementalMap<S, T>(
  source: readonly S[],
  cache: IncrementalMapCache<S, T>,
  mapFn: (item: S) => T,
): IncrementalMapCache<S, T> {
  const prevLen = cache.source.length;
  const isAppendOnly =
    prevLen > 0 && source.length >= prevLen && source[prevLen - 1] === cache.source[prevLen - 1];

  let mapped: T[];
  if (isAppendOnly && source.length === prevLen) {
    mapped = cache.mapped;
  } else if (isAppendOnly) {
    mapped = [...cache.mapped, ...source.slice(prevLen).map((item) => mapFn(item))];
  } else {
    mapped = source.map((item) => mapFn(item));
  }

  return { source, mapped };
}
