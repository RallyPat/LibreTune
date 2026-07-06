import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { HelpCircle } from 'lucide-react';
import type { Constant, FieldInfo } from '../types';
import { isIncompleteNumericInput } from '../types';

export default function DialogField({ 
  label, 
  name, 
  onUpdate, 
  context,
  fieldEnabledCondition,
  onOptimisticUpdate,
  onFieldFocus,
  showAllHelpIcons = true
}: { 
  label: string; 
  name: string; 
  onUpdate?: () => void; 
  context: Record<string, number>;
  fieldEnabledCondition?: boolean; // Enable condition from DialogComponent::Field
  onOptimisticUpdate?: (name: string, value: number) => void;
  onFieldFocus?: (info: FieldInfo) => void;
  showAllHelpIcons?: boolean; // Show help icons on all fields (true) or only fields with help (false)
}) {
  const [constant, setConstant] = useState<Constant | null>(null);
  const [numValue, setNumValue] = useState<number | null>(null);
  const [numInputStr, setNumInputStr] = useState<string>(''); // Raw string value during editing
  const [strValue, setStrValue] = useState<string>('');
  const [selectedBit, setSelectedBit] = useState<number>(0);
  const [isEnabled, setIsEnabled] = useState<boolean>(true);

  useEffect(() => {
    invoke<Constant>('get_constant', { name }).then((c) => {
      console.log(`[DialogField] Fetched constant '${name}':`, {
        value_type: c.value_type,
        bit_options_count: c.bit_options?.length || 0,
        bit_options: c.bit_options?.slice(0, 5) || [],
      });
      setConstant(c);
      // Fetch value based on type
      if (c.value_type === 'string') {
        invoke<string>('get_constant_string_value', { name })
          .then(setStrValue)
          .catch(() => setStrValue(''));
      } else if (c.value_type === 'bits') {
        invoke<number>('get_constant_value', { name })
          .then((v) => {
            console.log(`[DialogField] Got value for '${name}':`, v);
            setSelectedBit(Math.round(v));
          })
          .catch((e) => {
            console.error(`[DialogField] Failed to get value for '${name}':`, e);
            setSelectedBit(0);
          });
      } else {
        invoke<number>('get_constant_value', { name })
          .then((v) => {
            setNumValue(v);
            setNumInputStr(v.toString());
          })
          .catch(() => {
            setNumValue(0);
            setNumInputStr('0');
          });
      }
    }).catch((e) => {
      console.error(`[DialogField] Failed to fetch constant '${name}':`, e);
    });
  }, [name]);

  // Visibility is now handled by DialogFieldWrapper, not here

  // Evaluate enable condition - combine field-level condition with constant visibility_condition
  // This allows fields to be visible but disabled (per EFI Analytics spec and closed-source program suggestion)
  useEffect(() => {
    // Field-level enable condition (from DialogComponent::Field) takes precedence
    if (fieldEnabledCondition !== undefined) {
      setIsEnabled(fieldEnabledCondition);
      return;
    }
    
    // Fall back to constant's visibility_condition as enable condition
    if (constant?.visibility_condition) {
      // Build context with current field value included
      const fieldContext = { ...context };
      if (constant.value_type === 'bits') {
        fieldContext[name] = selectedBit;
      } else if (constant.value_type === 'scalar' && numValue !== null) {
        fieldContext[name] = numValue;
      }
      
      invoke<boolean>('evaluate_expression', { 
        expression: constant.visibility_condition, 
        context: fieldContext
      })
        .then(setIsEnabled)
        .catch(() => setIsEnabled(true)); // Enable on error
    } else {
      setIsEnabled(true); // Enabled by default if no condition
    }
  }, [fieldEnabledCondition, constant?.visibility_condition, context, name, selectedBit, numValue, constant?.value_type]);

  if (!constant) return <div className="field-loading">Loading {label}...</div>;

  // Always show field (don't hide based on condition) - condition controls enable/disable instead
  // This matches the closed-source program's behavior: "all 12 channels should be visible but disabled"

  const displayLabel = label || constant.label || constant.name;

  const fieldRowClass = (extra?: string) =>
    ['settings-field', 'dialog-field', !isEnabled ? 'is-disabled' : '', extra]
      .filter(Boolean)
      .join(' ');

  const renderLabel = () => (
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
  );
  
  // Handle field focus to show help in description panel
  const handleFocus = () => {
    onFieldFocus?.({
      label: displayLabel,
      name: constant.name,
      help: constant.help
    });
  };
  
  // Filter out "INVALID" from bit_options and build index mapping
  const validBitOptions: string[] = [];
  const originalToFilteredMap = new Map<number, number>();
  const filteredToOriginalMap = new Map<number, number>();
  
  // Ensure bit_options exists and is an array
  const bitOptions = constant.bit_options || [];
  
  if (constant.value_type === 'bits') {
    if (bitOptions.length === 0) {
      console.warn(`[DialogField] Constant '${name}' has no bit_options!`);
    }
    let filteredIndex = 0;
    for (let i = 0; i < bitOptions.length; i++) {
      const isInvalid = bitOptions[i]?.trim().toUpperCase() === 'INVALID';
      if (!isInvalid) {
        validBitOptions.push(bitOptions[i]);
        originalToFilteredMap.set(i, filteredIndex);
        filteredToOriginalMap.set(filteredIndex, i);
        filteredIndex++;
      }
    }
    // If all options were filtered out but we have options, keep at least the first one
    if (validBitOptions.length === 0 && bitOptions.length > 0) {
      console.warn(`[DialogField] All options filtered for '${name}', keeping first option`);
      validBitOptions.push(bitOptions[0]);
      originalToFilteredMap.set(0, 0);
      filteredToOriginalMap.set(0, 0);
    }
    console.log(`[DialogField] '${name}': ${bitOptions.length} total options, ${validBitOptions.length} valid options, selectedBit=${selectedBit}`);
  } else {
    // Not bits type, use all options
    validBitOptions.push(...bitOptions);
    for (let i = 0; i < bitOptions.length; i++) {
      originalToFilteredMap.set(i, i);
      filteredToOriginalMap.set(i, i);
    }
  }
  
  // Find the filtered index for the current selectedBit
  // If selectedBit is INVALID or not in the map, find the first valid option
  let filteredSelectedBit = originalToFilteredMap.get(selectedBit);
  if (filteredSelectedBit === undefined && validBitOptions.length > 0) {
    // Current selection is INVALID or not mapped, use first valid option for display
    // Find the first valid original index
    const firstValidOriginal = Array.from(filteredToOriginalMap.values())[0] ?? 0;
    filteredSelectedBit = originalToFilteredMap.get(firstValidOriginal) ?? 0;
  } else if (filteredSelectedBit === undefined) {
    // No valid options at all, default to 0
    filteredSelectedBit = 0;
  }

  // String field
  if (constant.value_type === 'string') {
    return (
      <div className={fieldRowClass()}>
        {renderLabel()}
        <div className="field-input-wrap">
          <input
            type="text"
            value={strValue}
            disabled={!isEnabled}
            onChange={(e) => setStrValue(e.target.value)}
            onFocus={handleFocus}
            onBlur={async () => {
              try {
                await invoke('update_constant_string', { name: constant.name, value: strValue });
              } catch (err) {
                console.error('Failed to update string constant:', err);
              }
              onUpdate?.();
            }}
            placeholder={constant.help || ''}
          />
        </div>
      </div>
    );
  }

  // Bits field — always a dropdown (enum-style INI bits constants)
  if (constant.value_type === 'bits') {
    // If no bit_options at all in INI, show read-only display
    if (bitOptions.length === 0) {
      return (
        <div className={fieldRowClass()}>
          <label className="field-label">
            <span className="field-label-text">{displayLabel}</span>
          </label>
          <div className="field-input-wrap">
            <input
              type="text"
              value={`Index: ${selectedBit} (no bit_options in INI)`}
              disabled={true}
              style={{ opacity: 0.7 }}
            />
            <span className="field-unit">{constant.units}</span>
          </div>
          <div style={{ color: 'orange', padding: '4px', fontSize: '0.85em' }}>
            Warning: No bit_options defined in INI for this constant
          </div>
        </div>
      );
    }
    
    // If all options were filtered out as INVALID, show all options anyway (including INVALID)
    // This ensures dropdowns always render when bit_options exist
    if (validBitOptions.length === 0) {
      // Rebuild maps to include all options (no filtering)
      validBitOptions.length = 0;
      originalToFilteredMap.clear();
      filteredToOriginalMap.clear();
      for (let i = 0; i < bitOptions.length; i++) {
        validBitOptions.push(bitOptions[i]);
        originalToFilteredMap.set(i, i);
        filteredToOriginalMap.set(i, i);
      }
      filteredSelectedBit = selectedBit;
    }
    
    // Always render bits fields as dropdowns (TunerStudio-style), including
    // binary choices like "Four Stroke" / "Two Stroke" or false / true.
    const safeSelectedBit = (filteredSelectedBit !== undefined && filteredSelectedBit >= 0 && filteredSelectedBit < validBitOptions.length)
      ? filteredSelectedBit
      : (selectedBit >= 0 && selectedBit < bitOptions.length && originalToFilteredMap.has(selectedBit))
        ? originalToFilteredMap.get(selectedBit) ?? 0
        : 0;
    
    return (
      <div className={fieldRowClass()}>
        {renderLabel()}
        <div className="field-input-wrap">
          <select
            value={safeSelectedBit}
            disabled={!isEnabled}
            onFocus={handleFocus}
            onChange={(e) => {
              const filteredVal = parseInt(e.target.value, 10);
              // Convert filtered index back to original index using the map
              const originalVal = filteredToOriginalMap.get(filteredVal);
              if (originalVal !== undefined) {
                setSelectedBit(originalVal);
                invoke('update_constant', { name, value: originalVal })
                  .then(() => {
                    onOptimisticUpdate?.(name, originalVal);
                    onUpdate?.();
                  })
                  .catch((err) => alert('Update failed: ' + err));
              } else {
                // Fallback: use the filtered value directly if not in map
                console.warn(`[DialogField] No original index found for filtered index ${filteredVal}, using directly`);
                setSelectedBit(filteredVal);
                invoke('update_constant', { name, value: filteredVal })
                  .then(() => {
                    onOptimisticUpdate?.(name, filteredVal);
                    onUpdate?.();
                  })
                  .catch((err) => alert('Update failed: ' + err));
              }
            }}
          >
            {validBitOptions.length === 0 ? (
              <option value={0}>No options available</option>
            ) : (
              validBitOptions.map((opt, i) => {
                // Show only the option label (e.g. 'NONE'), not index or '0="NONE"'
                // If option is a quoted string like 'NONE', show just 'NONE'
                // If option is empty, show as 'Not Assigned'
                let displayText = opt?.trim() || '';
                if (displayText === '') displayText = 'Not Assigned';
                // Remove any index prefix (e.g. '0="NONE"' -> 'NONE')
                const eqIdx = displayText.indexOf('=');
                if (eqIdx !== -1) {
                  displayText = displayText.substring(eqIdx + 1).replace(/^"|"$/g, '').trim();
                }
                // Remove any surrounding quotes
                displayText = displayText.replace(/^"|"$/g, '');
                return <option key={i} value={i}>{displayText}</option>;
              })
            )}
          </select>
        </div>
        {validBitOptions.length === 0 && bitOptions.length > 0 && (
          <div style={{ color: 'orange', padding: '4px', fontSize: '0.85em' }}>
            Warning: All options filtered out as INVALID
          </div>
        )}
      </div>
    );
  }

  // Default: numeric scalar field
  return (
    <div className={fieldRowClass()}>
      {renderLabel()}
      <div className="field-input-wrap">
        <input
          type="text"
          inputMode="decimal"
          value={numInputStr}
          disabled={!isEnabled}
          onFocus={handleFocus}
          onChange={(e) => {
            // Store raw string value to preserve partial input like "1." or ""
            // Allow numbers, decimal point, minus sign, and empty string using regex
            const value = e.target.value;
            if (/^-?\d*\.?\d*$/.test(value) || value === '') {
              setNumInputStr(value);
            }
          }}
          onBlur={() => {
            // Parse and validate on blur
            const parsed = parseFloat(numInputStr);
            if (!isNaN(parsed)) {
              // Clamp to min/max
              const clamped = Math.max(constant.min, Math.min(constant.max, parsed));
              setNumValue(clamped);
              setNumInputStr(clamped.toString());
              invoke('update_constant', { name, value: clamped })
                .then(() => {
                  onOptimisticUpdate?.(name, clamped);
                  onUpdate?.();
                })
                .catch((e) => alert('Update failed: ' + e));
            } else if (isIncompleteNumericInput(numInputStr)) {
              // Incomplete input (empty, just minus, or just decimal) - treat as 0
              setNumValue(0);
              setNumInputStr('0');
              invoke('update_constant', { name, value: 0 })
                .then(() => {
                  onOptimisticUpdate?.(name, 0);
                  onUpdate?.();
                })
                .catch((e) => alert('Update failed: ' + e));
            } else {
              // Invalid input - restore previous valid value
              setNumInputStr(numValue?.toString() ?? '0');
            }
          }}
          onKeyDown={(e) => {
            // Submit on Enter
            if (e.key === 'Enter') {
              e.currentTarget.blur();
            }
          }}
        />
        <span className="field-unit">{constant.units}</span>
      </div>
    </div>
  );
}
