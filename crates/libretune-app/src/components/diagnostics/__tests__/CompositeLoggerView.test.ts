import { describe, it, expect } from 'vitest';
import { toEntries } from '../CompositeLoggerView';

const rec = (index: number, fields: Record<string, number>) => ({ index, fields });

describe('composite record mapping', () => {
  it('reads levels and the declared timestamp by INI field name', () => {
    const out = toEntries([
      rec(0, { priLevel: 1, secLevel: 0, sync: 1, time: 1.5 }),
      rec(1, { priLevel: 0, secLevel: 1, sync: 0, time: 2.0 }),
    ]);

    expect(out).toEqual([
      { time_us: 1500, primary: true, secondary: false, sync: true, voltage: undefined },
      { time_us: 2000, primary: false, secondary: true, sync: false, voltage: undefined },
    ]);
  });

  it('does not invent a timebase: a record with no timestamp is dropped', () => {
    // The old path stamped every sample at a hardcoded 10 kHz regardless of
    // what the ECU reported, so the waveform's time axis was fabricated.
    expect(toEntries([rec(0, { priLevel: 1 })])).toEqual([]);
  });

  it('converts each declared timestamp field in its own units', () => {
    // time and refTime are declared scale 0.001 in ms; toothTime is scale 1.0
    // already in microseconds. Treating them alike was a 1000x error.
    expect(toEntries([rec(0, { time: 3 })])[0].time_us).toBe(3000);
    expect(toEntries([rec(0, { refTime: 3 })])[0].time_us).toBe(3000);
    expect(toEntries([rec(0, { toothTime: 4000 })])[0].time_us).toBe(4000);
  });

  it('treats a missing level flag as low rather than failing the record', () => {
    const [only] = toEntries([rec(0, { time: 1 })]);
    expect(only).toMatchObject({ primary: false, secondary: false, sync: false });
  });
});
