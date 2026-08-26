import { describe, it, expect, vi, afterEach } from 'vitest';
import { askNumber } from '../askNumber';

const answer = (value: string | null) =>
  vi.spyOn(window, 'prompt').mockReturnValue(value);

afterEach(() => {
  vi.restoreAllMocks();
});

describe('askNumber', () => {
  it('returns the number when one is typed', () => {
    answer(' 12.5 ');
    expect(askNumber('x')).toBe(12.5);
  });

  it('returns null for cancel, blank and non-numeric input', () => {
    for (const input of [null, '', '   ', 'abc', '12abc', 'NaN', '1e', 'Infinity']) {
      answer(input);
      expect(askNumber('x')).toBeNull();
    }
  });

  it('accepts a comma decimal separator', () => {
    for (const [input, want] of [['1,5', 1.5], ['12,25', 12.25], ['-3,5', -3.5]] as const) {
      answer(input);
      expect(askNumber('x')).toBe(want);
    }
  });

  it('still rejects an ambiguous thousands group', () => {
    // '1,234' could be 1.234 or 1234 depending on locale. Guessing either way
    // would put a wrong number in a fuel table.
    for (const input of ['1,234', '1,2,3', '1.2,3', '1,0000']) {
      answer(input);
      expect(askNumber('x')).toBeNull();
    }
  });

  it('passes the initial value through as the prompt default', () => {
    const spy = answer('3');
    askNumber('x', 2.5);
    expect(spy).toHaveBeenCalledWith('x', '2.5');
  });
});
