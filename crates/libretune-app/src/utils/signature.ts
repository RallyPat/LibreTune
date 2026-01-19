export type MatchType = 'exact' | 'partial' | 'mismatch';

/**
 * Should the UI block further automatic actions (sync) for the given match type?
 * - exact: never block
 * - partial: advisory (do not block, show warning)
 * - mismatch: block (require user decision)
 */
export function shouldBlockOnSignature(matchType: MatchType | null | undefined): boolean {
  if (!matchType) return false;
  return matchType === 'mismatch';
}
