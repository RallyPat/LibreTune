/**
 * Single-pass min/max over an array-like of numbers.
 *
 * Spreading a large array into `Math.min(...arr)` / `Math.max(...arr)`
 * blows the call stack once the array is roughly 100k+ elements (each
 * spread argument becomes a function-call argument). Datalog sample
 * arrays and channel data routinely exceed that, so any min/max over
 * data of unbounded size must use a plain loop instead.
 *
 * NaN values are skipped so a single bad sample can't poison the result.
 */
export function minMax(values: ArrayLike<number>): { min: number; max: number } {
  let min = Infinity;
  let max = -Infinity;

  for (let i = 0; i < values.length; i++) {
    const value = values[i];
    if (Number.isNaN(value)) continue;
    if (value < min) min = value;
    if (value > max) max = value;
  }

  return { min, max };
}

/** Convenience wrapper returning only the minimum. */
export function arrayMin(values: ArrayLike<number>): number {
  return minMax(values).min;
}

/** Convenience wrapper returning only the maximum. */
export function arrayMax(values: ArrayLike<number>): number {
  return minMax(values).max;
}
