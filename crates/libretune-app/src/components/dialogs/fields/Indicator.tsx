import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { DialogComponent } from '../types';

/// Renders a single boolean indicator (light + label) by evaluating
/// `comp.expression` against the supplied channel-value `context`.
export function Indicator({
  comp,
  context,
}: {
  comp: DialogComponent;
  context: Record<string, number>;
}) {
  const [isOn, setIsOn] = useState(false);

  useEffect(() => {
    if (comp.expression) {
      invoke<boolean>('evaluate_expression', { expression: comp.expression, context })
        .then(setIsOn)
        .catch(console.error);
    }
  }, [comp.expression, context]);

  // label_on/label_off can themselves be braced expressions (same pattern
  // as IndicatorPanelRenderer's tiles) — evaluate the raw label rather than
  // showing e.g. "{ bitStringValue(stftStateList, 2) }" verbatim.
  const rawLabel = isOn ? comp.label_on : comp.label_off;
  const [evaluatedLabel, setEvaluatedLabel] = useState(rawLabel);

  useEffect(() => {
    if (!rawLabel?.trim().startsWith('{')) {
      setEvaluatedLabel(rawLabel);
      return;
    }
    let cancelled = false;
    invoke<string>('evaluate_string_expression', { expression: rawLabel, context })
      .then((value) => { if (!cancelled) setEvaluatedLabel(value); })
      .catch(() => { if (!cancelled) setEvaluatedLabel(rawLabel); });
    return () => {
      cancelled = true;
    };
  }, [rawLabel, context]);

  return (
    <div className="indicator-field">
      <div className={`indicator-light ${isOn ? 'on' : 'off'}`} />
      <span className="indicator-label">{evaluatedLabel}</span>
    </div>
  );
}
