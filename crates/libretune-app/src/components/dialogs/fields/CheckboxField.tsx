import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { HelpCircle } from 'lucide-react';
import type { Constant, FieldInfo } from '../types';
import { useDialogValueSource } from '../DialogValueSource';
import { PowerCycleBadge } from './PowerCycleBadge';

/**
 * Renders a TunerStudio `checkbox = "Label", name, [flipValue], [{cond}]`
 * dialog directive as a real checkbox bound to a constant.
 *
 * - Without `flipValue`: checked ⇄ value != 0, toggling writes 1/0.
 * - With `flipValue`: the sense is inverted — checked ⇄ value == 0, toggling
 *   writes 0/1. Used by the dual-checkbox pattern where two checkboxes on the
 *   same constant present the two states of an On/Off setting (issue #338).
 *
 * The checked state prefers the shared `context` (which the tab shell updates
 * optimistically after every write, so sibling checkboxes bound to the same
 * constant stay in step), with a local pending override covering the window
 * before the context catches up. A locally fetched value is the fallback for
 * embedded panels that don't provide live context.
 *
 * Directive visibility/enable conditions are evaluated by DialogFieldWrapper;
 * `fieldEnabledCondition` here only controls the disabled attribute.
 */
export default function CheckboxField({
  label,
  name,
  flipValue = false,
  onUpdate,
  context,
  fieldEnabledCondition,
  onOptimisticUpdate,
  onFieldFocus,
  showAllHelpIcons = true,
}: {
  label: string;
  name: string;
  flipValue?: boolean;
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
  // Local value written by this checkbox but not yet reflected in `context`.
  // Cleared once the shared context catches up, so later context changes
  // (sibling toggles, full refetches) keep flowing through.
  const pendingLocalRef = useRef<number | null>(null);
  const valueSource = useDialogValueSource();
  const readOnly = !!valueSource?.readOnly;
  const isChanged = !!valueSource?.changedNames.has(name);

  // Fetch the constant metadata + current raw value. Honor the value source
  // (tune-diff preview) the same way DialogField's bits branch does.
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
      console.error(`[CheckboxField] Failed to fetch constant '${name}':`, e);
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

  // Enable precedence mirrors DialogField: directive-level condition first,
  // then the constant's own visibility_condition as a disable condition.
  const isEnabled =
    fieldEnabledCondition !== undefined ? fieldEnabledCondition : constEnabled;

  // Prefer the pending local write (covers the window before the optimistic
  // context update lands), then the shared context (live, keeps siblings of
  // the dual-checkbox pattern in step), then the locally fetched value.
  const effectiveValue =
    pendingLocalRef.current ?? context[name] ?? value;
  const isChecked = flipValue ? effectiveValue === 0 : effectiveValue !== 0;

  const handleFocus = () => {
    onFieldFocus?.({
      label,
      name: constant.name,
      help: constant.help,
    });
  };

  const toggle = (nextChecked: boolean) => {
    if (readOnly) return;
    // Normal: checked ⇄ 1. flipValue: checked ⇄ 0.
    const nextValue = flipValue ? (nextChecked ? 0 : 1) : nextChecked ? 1 : 0;
    pendingLocalRef.current = nextValue;
    setValue(nextValue);
    invoke('update_constant', { name, value: nextValue })
      .then(() => {
        onOptimisticUpdate?.(name, nextValue);
        onUpdate?.();
      })
      .catch((err) => {
        console.error(`[CheckboxField] Failed to update '${name}':`, err);
        // Revert local state on failure
        pendingLocalRef.current = null;
        invoke<number>('get_constant_value', { name })
          .then(setValue)
          .catch(() => {});
      });
  };

  const rowClass = [
    'settings-field',
    'settings-field--checkbox',
    !isEnabled || readOnly ? 'is-disabled' : '',
    isChanged ? 'tune-diff-changed' : '',
  ]
    .filter(Boolean)
    .join(' ');

  const displayLabel = label || constant.label || constant.name;

  // A checkbox bound to a constant holding values other than 0/1 can't
  // faithfully represent the state — surface the raw value so INI authors
  // notice (flipValue boxes legitimately sit at 0/1, so only flag others).
  const showValueHint = !flipValue && effectiveValue !== 0 && effectiveValue !== 1;

  return (
    <div className={rowClass}>
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
      <div className="field-input-wrap">
        <label className="field-checkbox-control">
          <input
            type="checkbox"
            aria-label={displayLabel}
            checked={isChecked}
            disabled={!isEnabled || readOnly}
            onChange={(e) => toggle(e.target.checked)}
            onFocus={handleFocus}
          />
          {showValueHint && (
            <span className="field-checkbox-hint">value: {effectiveValue}</span>
          )}
        </label>
      </div>
    </div>
  );
}
