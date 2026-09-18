import { describe, expect, it } from 'vitest';
import {
  parseTableAdjustInput,
  tableAdjustResultError,
  tableAdjustTransform,
} from '../tableAdjustInput';

describe('parseTableAdjustInput', () => {
  it('accepts a number', () => {
    expect(parseTableAdjustInput('10', 'sub')).toEqual({ ok: true, value: 10 });
    expect(parseTableAdjustInput('0.9', 'mul')).toEqual({ ok: true, value: 0.9 });
    expect(parseTableAdjustInput('1,5', 'add')).toEqual({ ok: true, value: 1.5 });
  });

  it('rejects non-numbers', () => {
    for (const raw of ['', 'abc', '12abc', 'NaN', 'Infinity']) {
      expect(parseTableAdjustInput(raw, 'add').ok).toBe(false);
    }
  });

  it('rejects a non-positive multiplier', () => {
    expect(parseTableAdjustInput('0', 'mul').ok).toBe(false);
    expect(parseTableAdjustInput('-1', 'mul').ok).toBe(false);
  });

  it('rejects a non-positive add/subtract amount', () => {
    expect(parseTableAdjustInput('0', 'sub').ok).toBe(false);
    expect(parseTableAdjustInput('-10', 'add').ok).toBe(false);
  });
});

describe('tableAdjustResultError', () => {
  it('rejects AFR results off stoich', () => {
    const next = tableAdjustTransform('mul', 10);
    expect(tableAdjustResultError([14.7], next, { tableKind: 'afr' })).toMatch(/stoichiometric/);
  });

  it('allows a 10% VE drop', () => {
    const next = tableAdjustTransform('sub', 10);
    expect(tableAdjustResultError([80, 90], next, { tableKind: 've' })).toBeNull();
  });

  it('rejects a 10× VE jump', () => {
    const next = tableAdjustTransform('mul', 10);
    expect(tableAdjustResultError([80], next, { tableKind: 've' })).toMatch(/VE/);
  });
});
