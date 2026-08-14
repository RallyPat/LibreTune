import { describe, expect, it } from 'vitest';
import { isDesignerHidden, withDesignerHidden } from '../../shared/designerHidden';
import type { DashComponent } from '../../dashTypes';

function makeGauge(overrides: Record<string, unknown> = {}): DashComponent {
  return {
    Gauge: {
      id: 'g1',
      ...overrides,
    },
  } as unknown as DashComponent;
}

function makeIndicator(overrides: Record<string, unknown> = {}): DashComponent {
  return {
    Indicator: {
      id: 'i1',
      ...overrides,
    },
  } as unknown as DashComponent;
}

describe('designer hidden flag', () => {
  it('hides a gauge and shows it again, clearing the flag', () => {
    const gauge = makeGauge();

    const hidden = withDesignerHidden(gauge, true);
    expect(isDesignerHidden(hidden)).toBe(true);
    expect((hidden as { Gauge: { extra_attrs: Record<string, string> } }).Gauge.extra_attrs.lt_designer_hidden).toBe('true');

    const shown = withDesignerHidden(hidden, false);
    expect(isDesignerHidden(shown)).toBe(false);
    expect((shown as { Gauge: { extra_attrs: Record<string, string> } }).Gauge.extra_attrs.lt_designer_hidden).toBeUndefined();
  });

  it('never touches enabled_condition, even one with the literal value "false"', () => {
    // Regression: the old implementation overwrote enabled_condition with
    // "false" to hide, so a user-authored condition of exactly "false" was
    // indistinguishable from designer-hidden.
    const gauge = makeGauge({ enabled_condition: 'false' });

    const hidden = withDesignerHidden(gauge, true);
    const hiddenGauge = (hidden as { Gauge: { enabled_condition: string | null } }).Gauge;
    expect(isDesignerHidden(hidden)).toBe(true);
    expect(hiddenGauge.enabled_condition).toBe('false');

    const shown = withDesignerHidden(hidden, false);
    expect((shown as { Gauge: { enabled_condition: string | null } }).Gauge.enabled_condition).toBe('false');
    expect(isDesignerHidden(shown)).toBe(false);
  });

  it('preserves a real enabled_condition expression untouched', () => {
    const gauge = makeGauge({ enabled_condition: 'rpm > 0' });

    const hidden = withDesignerHidden(gauge, true);
    expect((hidden as { Gauge: { enabled_condition: string | null } }).Gauge.enabled_condition).toBe('rpm > 0');

    const shown = withDesignerHidden(hidden, false);
    expect((shown as { Gauge: { enabled_condition: string | null } }).Gauge.enabled_condition).toBe('rpm > 0');
  });

  it('does the same for indicators', () => {
    const indicator = makeIndicator({ enabled_condition: 'hasLambdaSensor' });

    const hidden = withDesignerHidden(indicator, true);
    expect(isDesignerHidden(hidden)).toBe(true);
    expect((hidden as { Indicator: { enabled_condition: string | null } }).Indicator.enabled_condition).toBe('hasLambdaSensor');

    const shown = withDesignerHidden(hidden, false);
    expect(isDesignerHidden(shown)).toBe(false);
    expect((shown as { Indicator: { enabled_condition: string | null } }).Indicator.enabled_condition).toBe('hasLambdaSensor');
  });

  it('preserves other extra_attrs entries untouched (e.g. trend series config)', () => {
    const gauge = makeGauge({
      enabled_condition: 'rpm > 0',
      extra_attrs: { lt_series2_channel: 'boost' },
    });

    const hidden = withDesignerHidden(gauge, true);
    const shown = withDesignerHidden(hidden, false);
    const attrs = (shown as { Gauge: { extra_attrs: Record<string, string> } }).Gauge.extra_attrs;

    expect(attrs.lt_series2_channel).toBe('boost');
  });

  it('hiding an already-hidden component is idempotent', () => {
    const gauge = makeGauge();
    const hiddenOnce = withDesignerHidden(gauge, true);
    const hiddenTwice = withDesignerHidden(hiddenOnce, true);
    expect(isDesignerHidden(hiddenTwice)).toBe(true);
  });
});
