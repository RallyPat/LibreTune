import { parseNumber } from './askNumber';
import { STOICH_AFR } from './unitConversions';
import type { GeneratableTableKind } from './tableGenerator';

export type TableAdjustKind = 'add' | 'sub' | 'mul';

const LAMBDA_MIN = 0.55;
const LAMBDA_MAX = 1.55;

export function parseTableAdjustInput(
  raw: string,
  kind: TableAdjustKind,
): { ok: true; value: number } | { ok: false; error: string } {
  const n = parseNumber(raw);
  if (n === null) return { ok: false, error: 'Enter a number' };
  if (kind === 'mul' && n <= 0) return { ok: false, error: 'Multiplier must be greater than 0' };
  if ((kind === 'add' || kind === 'sub') && n <= 0) {
    return { ok: false, error: 'Amount must be greater than 0' };
  }
  return { ok: true, value: n };
}

export function tableAdjustTransform(kind: TableAdjustKind, amount: number): (v: number) => number {
  if (kind === 'add') return (v) => v + amount;
  if (kind === 'sub') return (v) => v - amount;
  return (v) => v * amount;
}

/** Reject results that are not a number, outside INI min/max, or off stoich for AFR/λ. */
export function tableAdjustResultError(
  cells: number[],
  next: (v: number) => number,
  opts: { tableKind: GeneratableTableKind | null; min?: number; max?: number },
): string | null {
  const out = cells.map(next);
  if (out.some((v) => !Number.isFinite(v))) return 'Result is not a number';
  if (opts.min !== undefined && out.some((v) => v < opts.min!)) {
    return `Below table minimum (${opts.min})`;
  }
  if (opts.max !== undefined && out.some((v) => v > opts.max!)) {
    return `Above table maximum (${opts.max})`;
  }
  if (opts.tableKind === 'afr') {
    const looksAfr = cells.some((v) => Math.abs(v) > 4);
    const stoich = STOICH_AFR.gasoline;
    if (out.some((v) => {
      const lambda = looksAfr ? v / stoich : v;
      return lambda < LAMBDA_MIN || lambda > LAMBDA_MAX;
    })) {
      return `Outside stoichiometric range (λ ${LAMBDA_MIN}–${LAMBDA_MAX})`;
    }
  }
  if (opts.tableKind === 've' && out.some((v) => v <= 0 || v > 255)) {
    return 'VE result must be between 0 and 255';
  }
  return null;
}
