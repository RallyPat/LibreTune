import { shouldBlockOnSignature } from '../signature';

describe('signature policy helper', () => {
  it('blocks on mismatch only', () => {
    expect(shouldBlockOnSignature('mismatch')).toBe(true);
    expect(shouldBlockOnSignature('partial')).toBe(false);
    expect(shouldBlockOnSignature('exact')).toBe(false);
    expect(shouldBlockOnSignature(null)).toBe(false);
    expect(shouldBlockOnSignature(undefined)).toBe(false);
  });
});
