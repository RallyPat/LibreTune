/**
 * Parse a typed number, rejecting anything that isn't one.
 *
 * Uses Number() rather than parseFloat(): parseFloat('12abc') is 12.
 * Accepts a comma decimal only when it cannot be a thousands group
 * (`1,5` → 1.5; `1,234` stays rejected).
 */
export function parseNumber(raw: string): number | null {
  let trimmed = raw.trim();
  if (trimmed === '') return null;
  if (/^-?\d+,\d{1,2}$/.test(trimmed)) {
    trimmed = trimmed.replace(',', '.');
  }
  const value = Number(trimmed);
  return Number.isFinite(value) ? value : null;
}

/**
 * prompt() for a number. Returns null if the user cancelled, left it blank,
 * or typed something that doesn't parse cleanly.
 */
export function askNumber(label: string, initial?: number): number | null {
  const raw = window.prompt(label, initial === undefined ? '' : String(initial));
  if (raw === null) return null;
  return parseNumber(raw);
}
