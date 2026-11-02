import { describe, it, expect } from 'vitest';
import { trailDisplayRow } from '../tableTrail';

describe('trailDisplayRow', () => {
  it('passes rows through unchanged when the axis is not flipped', () => {
    expect(trailDisplayRow(0, 4, false)).toBe(0);
    expect(trailDisplayRow(3, 4, false)).toBe(3);
  });

  it('mirrors data rows when yAxisBottom renders rows reversed', () => {
    expect(trailDisplayRow(0, 4, true)).toBe(3);
    expect(trailDisplayRow(3, 4, true)).toBe(0);
    expect(trailDisplayRow(1, 4, true)).toBe(2);
  });

  it('is its own inverse — geometry built from a flipped row flips back', () => {
    // A data row mapped to display space and back must land where it started,
    // whatever the row count.
    for (let row = 0; row < 7; row++) {
      const display = trailDisplayRow(row, 7, true);
      expect(trailDisplayRow(display, 7, true)).toBe(row);
    }
  });
});
