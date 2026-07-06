import { describe, expect, it } from 'vitest';
import { applyTableSettingsLayout } from '../dialogLayout';

describe('applyTableSettingsLayout', () => {
  it('stacks user-table dialog with settings above table (no side split)', () => {
    const components = [
      { type: 'Panel' as const, name: 'userTable2Tbl' },
      { type: 'Panel' as const, name: 'userTable2Settings' },
    ];
    const { components: out, hasTableSplit } = applyTableSettingsLayout(
      'userTable2TblSettings',
      components,
    );
    expect(hasTableSplit).toBe(false);
    expect(out.map((c) => c.name)).toEqual(['userTable2Settings', 'userTable2Tbl']);
    expect(out.every((c) => !c.position)).toBe(true);
  });

  it('strips West/Center positions and orders settings before table (epicEFI)', () => {
    const components = [
      { type: 'Panel' as const, name: 'userTable1left', position: 'West' },
      { type: 'Panel' as const, name: 'userTable1Tbl', position: 'Center' },
    ];
    const { components: out, hasTableSplit } = applyTableSettingsLayout(
      'userTable1TblSettings',
      components,
    );
    expect(hasTableSplit).toBe(false);
    expect(out.map((c) => c.name)).toEqual(['userTable1left', 'userTable1Tbl']);
    expect(out.every((c) => !c.position)).toBe(true);
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
