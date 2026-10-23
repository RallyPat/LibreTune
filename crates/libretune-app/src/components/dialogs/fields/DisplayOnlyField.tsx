import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { HelpCircle } from 'lucide-react';
import type { Constant, FieldInfo } from '../types';
import { useDialogValueSource } from '../DialogValueSource';

/**
 * Renders a TunerStudio `displayOnlyField = [!|#]"Label" [, name], [{cond}]`
 * dialog directive.
 *
 * - With a constant name: a read-only row showing the constant's current
 *   value — the option TEXT for `bits` constants (e.g. the configured pin),
 *   number+units for scalars, the string for strings. Never editable here.
 * - Without a name (`blankfield` or omitted): a static text line. `#`/`!`
 *   label prefixes carry the usual section-header / note styling, matching
 *   the Label renderer.
 *
 * Visibility/enable conditions are evaluated by DialogFieldWrapper; for a
 * read-only row the "enable" state only dims the value.
 */
export default function DisplayOnlyField({
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
  name?: string;
  onUpdate?: () => void;
  context: Record<string, number>;
  fieldEnabledCondition?: boolean;
  onOptimisticUpdate?: (name: string, value: number) => void;
  onFieldFocus?: (info: FieldInfo) => void;
  showAllHelpIcons?: boolean;
}) {
  const [constant, setConstant] = useState<Constant | null>(null);
  const [numValue, setNumValue] = useState<number | null>(null);
  const [strValue, setStrValue] = useState<string>('');
  const valueSource = useDialogValueSource();
  const isChanged = !!name && !!valueSource?.changedNames.has(name);

  useEffect(() => {
    if (!name) return;
    let cancelled = false;
    invoke<Constant>('get_constant', { name }).then((c) => {
      if (cancelled) return;
      setConstant(c);
      if (c.value_type === 'string') {
        if (valueSource) {
          setStrValue(valueSource.strings[name] ?? '');
          return;
        }
        invoke<string>('get_constant_string_value', { name })
          .then((v) => !cancelled && setStrValue(v))
          .catch(() => !cancelled && setStrValue(''));
      } else {
        if (valueSource) {
          setNumValue(valueSource.numbers[name] ?? 0);
          return;
        }
        invoke<number>('get_constant_value', { name })
          .then((v) => !cancelled && setNumValue(v))
          .catch(() => !cancelled && setNumValue(0));
      }
    }).catch((e) => {
      console.error(`[DisplayOnlyField] Failed to fetch constant '${name}':`, e);
    });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [name, valueSource]);

  // ── Text-only line: mirror the Label renderer's styling semantics ──
  if (!name) {
    const text = label.trim();
    const isSection = text.startsWith('#');
    const isNote = text.startsWith('!');
    const display = isSection
      ? text.replace(/^#+\s*/, '')
      : isNote
        ? text.replace(/^!+\s*/, '')
        : label;
    return (
      <div
        className={`dialog-label${isSection ? ' dialog-section-header' : ''}${isNote ? ' dialog-note' : ''}`}
      >
        {display}
      </div>
    );
  }

  if (!constant) return <div className="field-loading">Loading {label}...</div>;

  const isEnabled = fieldEnabledCondition !== false;
  const bitOptions = constant.bit_options || [];
  const displayLabel = label.replace(/^[#!]+\s*/, '') || constant.label || constant.name;
  const displayUnits = constant.units || '';

  const handleFocus = () => {
    onFieldFocus?.({
      label: displayLabel,
      name: constant.name,
      help: constant.help,
    });
  };

  let valueText: string;
  if (constant.value_type === 'bits') {
    const idx = Math.round(numValue ?? 0);
    valueText = bitOptions[idx] ?? `Index: ${idx}`;
  } else if (constant.value_type === 'string') {
    valueText = strValue;
  } else {
    valueText = `${(numValue ?? 0).toFixed(constant.digits)}${displayUnits ? ` ${displayUnits}` : ''}`;
  }

  const rowClass = [
    'settings-field',
    'settings-field--display-only',
    !isEnabled ? 'is-disabled' : '',
    isChanged ? 'tune-diff-changed' : '',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div className={rowClass}>
      <label className="field-label">
        <span className="field-label-text">{displayLabel}</span>
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
        <div className="field-display-value">{valueText}</div>
      </div>
    </div>
  );
}
