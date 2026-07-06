import { describe, expect, it } from 'vitest';
import {
  applyTableSettingsLayout,
  applyConfigLiveStateLayout,
  inferLiveStateGateExpression,
  isConfigLiveStateDialog,
  isGppwmLiveChannel,
  isReferenceGaugePanel,
  isUserTableLiveChannel,
  partitionHardwareTestComponents,
} from '../dialogLayout';

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

describe('config + live-state dialog layout', () => {
  it('detects WMI and launch control shell dialogs', () => {
    expect(isConfigLiveStateDialog('WmiControlDialog')).toBe(true);
    expect(isConfigLiveStateDialog('smLaunchControl')).toBe(true);
    expect(isConfigLiveStateDialog('ioTest')).toBe(false);
  });

  it('preserves west/east split for shell dialogs', () => {
    const components = [
      { type: 'Panel' as const, name: 'WmiSettingsDialog', position: 'West' },
      { type: 'Panel' as const, name: 'wmiLiveStateDialog', position: 'East' },
    ];
    const { useConfigLiveSplit } = applyConfigLiveStateLayout('WmiControlDialog', components);
    expect(useConfigLiveSplit).toBe(true);
  });

  it('infers live-state gate expressions', () => {
    expect(inferLiveStateGateExpression('wmiLiveStateDialog')).toBe('isWmiEnabled');
    expect(inferLiveStateGateExpression('launch_control_stateDialog')).toBe(
      'launchControlEnabled',
    );
    expect(inferLiveStateGateExpression('testSpark')).toBeNull();
  });
});

describe('embedded table live channel helpers', () => {
  it('detects user-table and gppwm live channels', () => {
    expect(isUserTableLiveChannel('userTableXAxis1')).toBe(true);
    expect(isUserTableLiveChannel('gppwmXAxis1')).toBe(false);
    expect(isGppwmLiveChannel('gppwmXAxis1')).toBe(true);
    expect(isGppwmLiveChannel('gppwmSwitch1')).toBe(true);
    expect(isGppwmLiveChannel('gppwmOutput4')).toBe(true);
    expect(isGppwmLiveChannel('rpm')).toBe(false);
  });
});

describe('hardware test layout helpers', () => {
  it('identifies reference gauge panel', () => {
    expect(isReferenceGaugePanel('injTest_r')).toBe(true);
    expect(isReferenceGaugePanel('testSpark')).toBe(false);
  });

  it('partitions ioTest panels into compact and auxiliary rows', () => {
    const components = [
      { type: 'Panel' as const, name: 'testSpark' },
      { type: 'Panel' as const, name: 'injTest_r' },
      { type: 'Panel' as const, name: 'testOther' },
    ];
    const { compact, auxiliary } = partitionHardwareTestComponents(components);
    expect(compact.map((c) => c.name)).toEqual(['testSpark']);
    expect(auxiliary.map((c) => c.name)).toEqual(['testOther', 'injTest_r']);
  });
});
