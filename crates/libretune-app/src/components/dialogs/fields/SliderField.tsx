import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { HelpCircle } from 'lucide-react';
import type { Constant, FieldInfo } from '../types';
import { useDialogValueSource } from '../DialogValueSource';
import { PowerCycleBadge } from './PowerCycleBadge';

/**
 * Renders a TunerStudio `slider = "Label", name, [horizontal|vertical],
 * [{cond}]` dialog directive as a range control bound to a constant.
 *
 * The slider operates in engineering units using the constant's own
 * min/max, with a step of one unit in the constant's `digits` precision.
 * Dragging updates the display only; the write is committed on release
 * (pointer up / key up / blur) so a drag doesn't spam the ECU with writes.
 *
 * The INI orientation token is captured by the parser for fidelity but the
 * control is rendered horizontally regardless — vertical sliders don't fit
 * the dialog row layout.
 *
 * The displayed value prefers the shared `context` (updated optimistically
 * after every write so sibling controls stay in step), with a local pending
 * override covering the window before the context catches up — the same
 * strategy as CheckboxField.
 */
export default function SliderField({
  label,
  name,
  onUpdate,
  context,
  fieldEnabledCondition,
  onOptimisticUpdate,
  onFieldFocus,
  showAllHelpIcons = true,
}: {
  label: string;
  name: string;
  onUpdate?: () => void;
  context: Record<string, number>;
  fieldEnabledCondition?: boolean;
  onOptimisticUpdate?: (name: string, value: number) => void;
  onFieldFocus?: (info: FieldInfo) => void;
  showAllHelpIcons?: boolean;
}) {
  const [constant, setConstant] = useState<Constant | null>(null);
  const [value, setValue] = useState<number>(0);
  const [constEnabled, setConstEnabled] = useState<boolean>(true);
  // Local value from an in-progress drag, not yet committed/written.
  const [dragValue, setDragValue] = useState<number | null>(null);
  // Local value written by this slider but not yet reflected in `context`.
  const pendingLocalRef = useRef<number | null>(null);
  const valueSource = useDialogValueSource();
  const readOnly = !!valueSource?.readOnly;
  const isChanged = !!valueSource?.changedNames.has(name);

  // Fetch the constant metadata + current value. Honor the value source
  // (tune-diff preview) the same way DialogField/CheckboxField do.
  useEffect(() => {
    invoke<Constant>('get_constant', { name }).then((c) => {
      setConstant(c);
      if (valueSource) {
        setValue(valueSource.numbers[name] ?? 0);
        return;
      }
      invoke<number>('get_constant_value', { name })
        .then(setValue)
        .catch(() => setValue(0));
    }).catch((e) => {
      console.error(`[SliderField] Failed to fetch constant '${name}':`, e);
    });
  }, [name, valueSource]);

  // Fall back to the constant's own visibility_condition as an enable
  // condition when the directive carries none (same as DialogField).
  useEffect(() => {
    if (fieldEnabledCondition !== undefined) return;
    if (!constant?.visibility_condition) {
      setConstEnabled(true);
      return;
    }
    const fieldContext = { ...context, [name]: value };
    invoke<boolean>('evaluate_expression', {
      expression: constant.visibility_condition,
      context: fieldContext,
    })
      .then(setConstEnabled)
      .catch(() => setConstEnabled(true));
  }, [
    fieldEnabledCondition,
    constant?.visibility_condition,
    context,
    name,
    value,
  ]);

  // Hand off to the shared context once it reflects our pending local write.
  useEffect(() => {
    if (
      pendingLocalRef.current !== null &&
      context[name] === pendingLocalRef.current
    ) {
      pendingLocalRef.current = null;
    }
  }, [context, name]);

  if (!constant) return <div className="field-loading">Loading {label}...</div>;

  const isEnabled =
    fieldEnabledCondition !== undefined ? fieldEnabledCondition : constEnabled;

  // Prefer an in-progress drag, then the pending local write, then the shared
  // context, then the locally fetched value.
  const effectiveValue =
    dragValue ?? pendingLocalRef.current ?? context[name] ?? value;

  // Clamp the slider domain to something sane if the INI declares no usable
  // bounds (min >= max happens on placeholder constants).
  const rawMin = constant.min;
  const rawMax = constant.max;
  const hasBounds = Number.isFinite(rawMin) && Number.isFinite(rawMax) && rawMax > rawMin;
  const min = hasBounds ? rawMin : 0;
  const max = hasBounds ? rawMax : 100;
  const step = Math.pow(10, -constant.digits);
  const displayLabel = label || constant.label || constant.name;
  const displayUnits = constant.units || '';

  const handleFocus = () => {
    onFieldFocus?.({
      label: displayLabel,
      name: constant.name,
      help: constant.help,
    });
  };

  const commit = (nextValue: number) => {
    if (readOnly) return;
    const clamped = Math.min(max, Math.max(min, nextValue));
    setDragValue(null);
    // Compare against the last COMMITTED value, not the displayed one —
    // during a drag the display already shows the released position.
    const committedValue = pendingLocalRef.current ?? context[name] ?? value;
    if (clamped === committedValue) return;
    pendingLocalRef.current = clamped;
    setValue(clamped);
    invoke('update_constant', { name, value: clamped })
      .then(() => {
        onOptimisticUpdate?.(name, clamped);
        onUpdate?.();
      })
      .catch((err) => {
        console.error(`[SliderField] Failed to update '${name}':`, err);
        // Revert local state on failure
        pendingLocalRef.current = null;
        invoke<number>('get_constant_value', { name })
          .then(setValue)
          .catch(() => {});
      });
  };

  const rowClass = [
    'settings-field',
    'settings-field--slider',
    !isEnabled || readOnly ? 'is-disabled' : '',
    isChanged ? 'tune-diff-changed' : '',
  ]
    .filter(Boolean)
    .join(' ');

  const renderLabel = () => (
    <label className="field-label">
      <span className="field-label-text">{displayLabel}</span>
      {constant.requires_power_cycle && <PowerCycleBadge />}
      {(showAllHelpIcons || constant.help) && (
        <span
          className="help-icon"
          title={constant.help || 'Click for info'}
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            handleFocus();
          }}
          tabIndex={0}
          onKeyDown={(e) => e.key === 'Enter' && handleFocus()}
        >
          <HelpCircle size={16} />
        </span>
      )}
    </label>
  );

  return (
    <div className={rowClass}>
      {renderLabel()}
      <div className="field-input-wrap">
        <div className="field-slider-control">
          <input
            type="range"
            aria-label={displayLabel}
            min={min}
            max={max}
            step={step}
            value={effectiveValue}
            disabled={!isEnabled || readOnly}
            onChange={(e) => {
              // Dragging: update the display only; commit on release.
              setDragValue(parseFloat(e.target.value));
            }}
            onPointerUp={(e) => commit(parseFloat(e.currentTarget.value))}
            onKeyUp={(e) => commit(parseFloat(e.currentTarget.value))}
            onBlur={(e) => commit(parseFloat(e.currentTarget.value))}
            onFocus={handleFocus}
          />
          <span className="field-slider-value">
            {effectiveValue.toFixed(constant.digits)}
            {displayUnits ? ` ${displayUnits}` : ''}
          </span>
        </div>
      </div>
    </div>
  );
}
