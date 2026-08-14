/**
 * Designer "hidden" flag for dashboard components.
 *
 * Stored as `extra_attrs['lt_designer_hidden'] = "true"` — a LibreTune-only
 * extension attribute the XML writer round-trips verbatim. The LayerPanel
 * hide/show toggle used to overwrite `enabled_condition` with the literal
 * string "false", which collided with real user-authored conditions of that
 * value and forced stash/restore gymnastics; a dedicated key leaves the
 * runtime-evaluated condition field untouched.
 */

import { DashComponent, isGauge, isIndicator } from '../dashTypes';

export const DESIGNER_HIDDEN_KEY = 'lt_designer_hidden';

function getExtraAttrs(c: DashComponent): Record<string, string> {
  if (isGauge(c)) return c.Gauge.extra_attrs ?? {};
  if (isIndicator(c)) return c.Indicator.extra_attrs ?? {};
  return {};
}

export function isDesignerHidden(c: DashComponent): boolean {
  return getExtraAttrs(c)[DESIGNER_HIDDEN_KEY] === 'true';
}

export function withDesignerHidden(c: DashComponent, hidden: boolean): DashComponent {
  const attrs = { ...getExtraAttrs(c) };
  if (hidden) {
    attrs[DESIGNER_HIDDEN_KEY] = 'true';
  } else {
    delete attrs[DESIGNER_HIDDEN_KEY];
  }

  if (isGauge(c)) return { Gauge: { ...c.Gauge, extra_attrs: attrs } };
  if (isIndicator(c)) return { Indicator: { ...c.Indicator, extra_attrs: attrs } };
  return c;
}
