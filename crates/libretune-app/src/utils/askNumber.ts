/**
 * prompt() for a number, rejecting anything that isn't one.
 *
 * Returns null if the user cancelled, left it blank, or typed something that
 * doesn't parse cleanly - callers must treat null as "do nothing". Uses
 * Number() rather than parseFloat() on purpose: parseFloat('12abc') is 12,
 * which silently accepts a typo as a tune value.
 */
export function askNumber(label: string, initial?: number): number | null {
  const raw = window.prompt(label, initial === undefined ? '' : String(initial));
  if (raw === null) return null;
  let trimmed = raw.trim();
  if (trimmed === '') return null;
  // A comma decimal separator is what most of Europe types, and prompt() gives
  // no locale hint. Number('1,5') is NaN, so without this the entry is
  // rejected silently and looks identical to pressing Cancel.
  //
  // Accepted only where it cannot be a thousands group: one comma, followed by
  // one or two digits, to the end. '1,234' stays rejected because 1.234 and
  // 1234 are both plausible readings of it and picking wrong puts a value out
  // by a factor of a thousand into a fuel table.
  if (/^-?\d+,\d{1,2}$/.test(trimmed)) {
    trimmed = trimmed.replace(',', '.');
  }
  const value = Number(trimmed);
  return Number.isFinite(value) ? value : null;
}
