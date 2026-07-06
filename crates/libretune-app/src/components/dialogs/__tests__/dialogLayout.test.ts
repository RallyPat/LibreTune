import { describe, expect, it } from 'vitest';
import { applyTableSettingsLayout } from '../dialogLayout';

describe('applyTableSettingsLayout', () => {
  it('splits user-table dialog (table component + settings panel)', () => {
    const components = [
      { type: 'Table' as const, name: 'userTable2Tbl' },
      { type: 'Panel' as const, name: 'userTable2Settings' },
    ];
    const { components: out, hasTableSplit } = applyTableSettingsLayout(
      'userTable2TblSettings',
      components,
    );
    expect(hasTableSplit).toBe(true);
    expect(out[0].position).toBe('East');
    expect(out[1].position).toBe('West');
  });

  it('does not split Generic PWM dialog', () => {
    const components = [
      { type: 'Panel' as const, name: 'gppwm1left' },
      { type: 'Panel' as const, name: 'gppwm1Tbl' },
    ];
    const { components: out, hasTableSplit } = applyTableSettingsLayout('gppwm1', components);
    expect(hasTableSplit).toBe(false);
    expect(out.every((c) => !c.position)).toBe(true);
  });

  it('does not split script-table dialog', () => {
    const components = [
      { type: 'Field' as const, name: 'scriptTableName1', label: 'Name' },
      { type: 'Panel' as const, name: 'scriptTable1Tbl' },
    ];
    const { hasTableSplit } = applyTableSettingsLayout('scriptTable1TblSettings', components);
    expect(hasTableSplit).toBe(false);
  });
});
