import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ArrowLeft, Activity, Grid3X3, HelpCircle } from 'lucide-react';
import CurveEditor, { SimpleGaugeInfo } from '../curves/CurveEditor';
import TableEditor2D from '../tables/TableEditor2D';
import './DialogRenderer.css';

interface DialogComponent {
  type: 'Panel' | 'Field' | 'LiveGraph' | 'Table' | 'Label' | 'Indicator' | 'CommandButton';
  name?: string;
  label?: string;
  text?: string;
  title?: string;
  position?: string;
  channels?: string[];
  expression?: string;
  label_off?: string;
  label_on?: string;
  visibility_condition?: string;  // Visibility condition (hides field if false)
  enabled_condition?: string;     // Enable condition (disables field if false)
  condition?: string;             // Legacy: single condition (treated as enabled_condition)
  // CommandButton specific fields
  command?: string;               // Command name from [ControllerCommands]
  on_close_behavior?: 'ClickOnCloseIfEnabled' | 'ClickOnCloseIfDisabled' | 'ClickOnClose';
}

interface DialogDefinition {
  name: string;
  title: string;
  components: DialogComponent[];
}

interface Constant {
  name: string;
  label?: string;
  units: string;
  digits: number;
  min: number;
  max: number;
  value_type: 'scalar' | 'string' | 'bits' | 'array';
  bit_options: string[];
  help?: string;
  visibility_condition?: string;  // Expression for when field should be visible
  display_offset?: number;  // For bits type: offset to add to displayed value (e.g., +1 for [4:7+1])
}

interface TableInfo {
  name: string;
  title: string;
}

interface BackendTableData {
  name: string;
  title: string;
  x_axis_name?: string;
  y_axis_name?: string;
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  x_output_channel?: string | null;
  y_output_channel?: string | null;
}

interface CurveData {
  name: string;
  title: string;
  x_bins: number[];
  y_bins: number[];
  x_label: string;
  y_label: string;
  x_axis?: [number, number, number] | null;
  y_axis?: [number, number, number] | null;
  x_output_channel?: string | null;
  gauge?: string | null;
}

function DialogField({ 
  label, 
  name, 
  onUpdate, 
  context,
  fieldEnabledCondition,
  onOptimisticUpdate 
}: { 
  label: string; 
  name: string; 
  onUpdate?: () => void; 
  context: Record<string, number>;
  fieldEnabledCondition?: boolean; // Enable condition from DialogComponent::Field
  onOptimisticUpdate?: (name: string, value: number) => void;
}) {
  const [constant, setConstant] = useState<Constant | null>(null);
  const [numValue, setNumValue] = useState<number | null>(null);
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
          .then(setNumValue)
          .catch(() => setNumValue(0));
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
      <div className="settings-field">
        <label>
          {displayLabel}
          {constant.help && (
            <span className="help-icon" title={constant.help}>
              <HelpCircle size={14} />
            </span>
          )}
        </label>
        <div className="field-input-wrap">
          <input
            type="text"
            value={strValue}
            disabled={!isEnabled}
            onChange={(e) => setStrValue(e.target.value)}
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
        {constant.help && <div className="field-help">{constant.help}</div>}
      </div>
    );
  }

  // Bits field (dropdown or checkbox)
  if (constant.value_type === 'bits') {
    // If no bit_options at all in INI, show read-only display
    if (bitOptions.length === 0) {
      return (
        <div className="settings-field">
          <label>{displayLabel}</label>
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
    
    // If only 2 valid options, render as checkbox
    if (validBitOptions.length === 2) {
      // Find original indices for the two valid options
      const validIndices = bitOptions
        .map((opt, i) => ({ opt, i }))
        .filter(({ opt }) => opt?.trim().toUpperCase() !== 'INVALID')
        .map(({ i }) => i);
      
      const checkedIndex = validIndices[1] ?? validIndices[0];
      const uncheckedIndex = validIndices[0];
      
      // Get the option labels for display
      const uncheckedLabel = bitOptions[uncheckedIndex]?.trim() || 'Off';
      const checkedLabel = bitOptions[checkedIndex]?.trim() || 'On';
      
      return (
        <div className="settings-field">
          <label>
            <input
              type="checkbox"
              checked={selectedBit === checkedIndex}
              disabled={!isEnabled}
              onChange={(e) => {
                const newVal = e.target.checked ? checkedIndex : uncheckedIndex;
                setSelectedBit(newVal);
                invoke('update_constant', { name, value: newVal })
                  .then(() => {
                    // Optimistically update context so sibling fields re-evaluate immediately
                    onOptimisticUpdate?.(name, newVal);
                    onUpdate?.();
                  })
                  .catch((e) => alert('Update failed: ' + e));
              }}
            />
            {displayLabel}: {uncheckedLabel} / {checkedLabel}
            {constant.help && (
              <span className="help-icon" title={constant.help}>
                <HelpCircle size={14} />
              </span>
            )}
          </label>
          {constant.help && <div className="field-help">{constant.help}</div>}
        </div>
      );
    }
    // Otherwise render as dropdown
    // Ensure filteredSelectedBit is valid
    const safeSelectedBit = (filteredSelectedBit !== undefined && filteredSelectedBit >= 0 && filteredSelectedBit < validBitOptions.length)
      ? filteredSelectedBit
      : (selectedBit >= 0 && selectedBit < bitOptions.length && originalToFilteredMap.has(selectedBit))
        ? originalToFilteredMap.get(selectedBit) ?? 0
        : 0;
    
    return (
      <div className="settings-field">
        <label>
          {displayLabel}
          {constant.help && (
            <span className="help-icon" title={constant.help}>
              <HelpCircle size={14} />
            </span>
          )}
        </label>
        <div className="field-input-wrap">
          <select
            value={safeSelectedBit}
            disabled={!isEnabled}
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
                // Apply display_offset to the label if it's a numeric option
                // For [4:7+1], raw 0 should display as "1", raw 1 as "2", etc.
                const offset = constant.display_offset ?? 0;
                const originalIdx = filteredToOriginalMap.get(i) ?? i;
                const displayVal = originalIdx + offset;
                // If the option is already a descriptive string, use it; 
                // otherwise show the offset-adjusted value
                const displayText = opt && opt.trim() !== '' && isNaN(Number(opt)) 
                  ? opt 
                  : `${displayVal}`;
                return <option key={i} value={i}>{displayText}</option>;
              })
            )}
          </select>
        </div>
        {constant.help && <div className="field-help">{constant.help}</div>}
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
    <div className="settings-field">
      <label>
        {displayLabel}
        {constant.help && (
          <span className="help-icon" title={constant.help}>
            <HelpCircle size={14} />
          </span>
        )}
      </label>
      <div className="field-input-wrap">
        <input
          type="number"
          step={1 / Math.pow(10, constant.digits)}
          value={numValue ?? 0}
          disabled={!isEnabled}
          onChange={(e) => setNumValue(parseFloat(e.target.value))}
          onBlur={() => {
            if (numValue !== null) {
              invoke('update_constant', { name, value: numValue })
                .then(() => {
                  onOptimisticUpdate?.(name, numValue);
                  onUpdate?.();
                })
                .catch((e) => alert('Update failed: ' + e));
            }
          }}
        />
        <span className="field-unit">{constant.units}</span>
      </div>
      {constant.help && <div className="field-help">{constant.help}</div>}
    </div>
  );
}

function Indicator({ comp, context }: { comp: DialogComponent; context: Record<string, number> }) {
  const [isOn, setIsOn] = useState(false);

  useEffect(() => {
    if (comp.expression) {
      invoke<boolean>('evaluate_expression', { expression: comp.expression, context })
        .then(setIsOn)
        .catch(console.error);
    }
  }, [comp.expression, context]);

  return (
    <div className="indicator-field">
      <div className={`indicator-light ${isOn ? 'on' : 'off'}`} />
      <span className="indicator-label">{isOn ? comp.label_on : comp.label_off}</span>
    </div>
  );
}

// Settings key for command warning preference
const COMMAND_WARNINGS_DISABLED_KEY = 'libretune_command_warnings_disabled';

function CommandButton({ comp, context }: { comp: DialogComponent; context: Record<string, number> }) {
  const [isEnabled, setIsEnabled] = useState(true);
  const [isExecuting, setIsExecuting] = useState(false);
  const [showWarning, setShowWarning] = useState(false);
  const [warningsDisabled, setWarningsDisabled] = useState(false);

  // Load warning preference from localStorage
  useEffect(() => {
    const saved = localStorage.getItem(COMMAND_WARNINGS_DISABLED_KEY);
    if (saved === 'true') {
      setWarningsDisabled(true);
    }
  }, []);

  // Evaluate enable condition
  useEffect(() => {
    if (comp.enabled_condition) {
      invoke<boolean>('evaluate_expression', { expression: comp.enabled_condition, context })
        .then(setIsEnabled)
        .catch((err) => {
          console.error('Error evaluating command button condition:', err);
          setIsEnabled(true); // Default to enabled on error
        });
    }
  }, [comp.enabled_condition, context]);

  const executeCommand = async () => {
    if (!comp.command || isExecuting) return;
    
    setIsExecuting(true);
    try {
      await invoke('execute_controller_command', { commandName: comp.command });
    } catch (err) {
      console.error('Command execution failed:', err);
      alert(`Command failed: ${err}`);
    } finally {
      setIsExecuting(false);
    }
  };

  const handleClick = () => {
    if (!isEnabled || isExecuting) return;
    
    // Show warning on first use if not disabled
    if (!warningsDisabled) {
      setShowWarning(true);
    } else {
      executeCommand();
    }
  };

  const handleWarningConfirm = (disableWarnings: boolean) => {
    setShowWarning(false);
    if (disableWarnings) {
      setWarningsDisabled(true);
      localStorage.setItem(COMMAND_WARNINGS_DISABLED_KEY, 'true');
    }
    executeCommand();
  };

  return (
    <>
      <div className="command-button-field">
        <button
          className={`command-button ${isExecuting ? 'executing' : ''}`}
          onClick={handleClick}
          disabled={!isEnabled || isExecuting}
        >
          {isExecuting ? 'Executing...' : comp.label}
        </button>
      </div>
      
      {showWarning && (
        <div className="command-warning-overlay" onClick={() => setShowWarning(false)}>
          <div className="command-warning-dialog" onClick={(e) => e.stopPropagation()}>
            <h3>⚠️ Controller Command Warning</h3>
            <p>
              This button sends raw commands directly to the ECU.
              These commands bypass normal memory synchronization and may:
            </p>
            <ul>
              <li>Cause the ECU tune to become out of sync</li>
              <li>Activate outputs (injectors, coils, etc.)</li>
              <li>Alter ECU behavior unexpectedly</li>
            </ul>
            <p>Only proceed if you understand what this command does.</p>
            <div className="command-warning-buttons">
              <button onClick={() => setShowWarning(false)}>Cancel</button>
              <button onClick={() => handleWarningConfirm(false)}>Execute Once</button>
              <button onClick={() => handleWarningConfirm(true)} className="danger">
                Always Allow
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

interface IndicatorPanel {
  name: string;
  columns: number;
  visibility_condition?: string;
  indicators: Array<{
    expression: string;
    label_off: string;
    label_on: string;
    color_off_fg?: string;
    color_off_bg?: string;
    color_on_fg?: string;
    color_on_bg?: string;
  }>;
}

interface PortEditorConfig {
  name: string;
  label: string;
  enable_condition?: string;
}

function RecursivePanel({
  name,
  openTable,
  context,
  onUpdate,
}: {
  name: string;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
}) {
  const [definition, setDefinition] = useState<DialogDefinition | null>(null);
  const [indicatorPanel, setIndicatorPanel] = useState<IndicatorPanel | null>(null);
  const [tableInfo, setTableInfo] = useState<TableInfo | null>(null);
  const [tableData, setTableData] = useState<BackendTableData | null>(null);
  const [curveData, setCurveData] = useState<CurveData | null>(null);
  const [gaugeConfig, setGaugeConfig] = useState<SimpleGaugeInfo | null>(null);
  const [portEditor, setPortEditor] = useState<PortEditorConfig | null>(null);
  const [panelType, setPanelType] = useState<'loading' | 'dialog' | 'indicatorPanel' | 'table' | 'curve' | 'portEditor' | 'unknown'>('loading');

  useEffect(() => {
    // Reset state when name changes and track cancellation
    let cancelled = false;
    
    setPanelType('loading');
    setDefinition(null);
    setIndicatorPanel(null);
    setTableInfo(null);
    setTableData(null);
    setCurveData(null);
    setGaugeConfig(null);
    setPortEditor(null);

    // First try as indicatorPanel
    invoke<IndicatorPanel>('get_indicator_panel', { name })
      .then((panel) => {
        if (cancelled) return;
        setIndicatorPanel(panel);
        setPanelType('indicatorPanel');
      })
      .catch(() => {
        if (cancelled) return;
        // Not an indicatorPanel, try as dialog
        invoke<DialogDefinition>('get_dialog_definition', { name })
          .then((def) => {
            if (cancelled) return;
            setDefinition(def);
            setPanelType('dialog');
          })
          .catch(() => {
            if (cancelled) return;
            // Not a dialog, try as table (lightweight check first, then full data)
            invoke<TableInfo>('get_table_info', { tableName: name })
              .then((info) => {
                if (cancelled) return;
                setTableInfo(info);
                // Now fetch full table data for embedded rendering
                invoke<BackendTableData>('get_table_data', { tableName: name })
                  .then((data) => {
                    if (cancelled) return;
                    setTableData(data);
                    setPanelType('table');
                  })
                  .catch((dataErr) => {
                    if (cancelled) return;
                    console.debug(`Could not load table data for '${name}':`, dataErr);
                    // Still show as table but without embedded view
                    setPanelType('table');
                  });
              })
              .catch((err) => {
                if (cancelled) return;
                console.debug(`Panel '${name}' is not a table:`, err);
                // Not a table, try as curve
                invoke<CurveData>('get_curve_data', { curveName: name })
                  .then((data) => {
                    if (cancelled) return;
                    setCurveData(data);
                    setPanelType('curve');
                    // Fetch gauge config if curve has a gauge reference
                    if (data.gauge) {
                      invoke<SimpleGaugeInfo>('get_gauge_config', { gaugeName: data.gauge })
                        .then((gc) => { if (!cancelled) setGaugeConfig(gc); })
                        .catch((gaugeErr) => console.debug(`Could not load gauge ${data.gauge}:`, gaugeErr));
                    }
                  })
                  .catch((err2) => {
                    if (cancelled) return;
                    console.debug(`Panel '${name}' is not a curve:`, err2);
                    // Not a curve, try as portEditor
                    invoke<PortEditorConfig>('get_port_editor', { name })
                      .then((editor) => {
                        if (cancelled) return;
                        setPortEditor(editor);
                        setPanelType('portEditor');
                      })
                      .catch((err3) => {
                        if (cancelled) return;
                        console.debug(`Panel '${name}' is not a portEditor:`, err3);
                        // None of the known types
                        setPanelType('unknown');
                      });
                  });
              });
          });
      });

    // Cleanup function to prevent state updates after unmount
    return () => {
      cancelled = true;
    };
  }, [name]);

  if (panelType === 'loading') {
    return <div className="panel-loading">Loading {name}...</div>;
  }

  // Render as embedded table editor if we have full table data
  if (panelType === 'table' && tableInfo && tableData) {
    return (
      <TableEditor2D
        title={tableInfo.title || name}
        table_name={tableData.name}
        x_axis_name={tableData.x_axis_name || 'X'}
        y_axis_name={tableData.y_axis_name || 'Y'}
        x_bins={tableData.x_bins}
        y_bins={tableData.y_bins}
        z_values={tableData.z_values}
        embedded={true}
        realtimeData={context}
        onOpenInTab={() => openTable(name)}
        onValuesChange={(values) => {
          // Save changes to backend
          invoke('update_table_data', {
            table_name: tableData.name,
            z_values: values,
          }).then(() => {
            onUpdate?.();
          }).catch((err) => {
            console.error('Failed to update table:', err);
          });
        }}
      />
    );
  }

  // Fallback to clickable table link if we only have table info (no data)
  if (panelType === 'table' && tableInfo) {
    return (
      <div className="embedded-table-link" onClick={() => openTable(name)}>
        <Grid3X3 size={20} />
        <span>Open Table: {tableInfo.title || name}</span>
      </div>
    );
  }

  // Render as curve editor if it's a curve
  if (panelType === 'curve' && curveData) {
    return (
      <CurveEditor
        data={curveData}
        embedded={true}
        realtimeData={context}
        simpleGaugeInfo={gaugeConfig}
        onValuesChange={(yBins) => {
          console.log('Curve values changed:', yBins);
          onUpdate?.();
        }}
      />
    );
  }

  // Render as indicatorPanel
  if (panelType === 'indicatorPanel' && indicatorPanel) {
    return <IndicatorPanelRenderer panel={indicatorPanel} context={context} />;
  }

  // Render as dialog
  if (panelType === 'dialog' && definition) {
    return (
      <div className="nested-panel">
        {definition.title && definition.title !== name && <div className="panel-title">{definition.title}</div>}
        <div className="panel-content">
          {definition.components.map((comp, i) => (
            <DialogComponentRenderer key={i} comp={comp} openTable={openTable} context={context} onUpdate={onUpdate} />
          ))}
        </div>
      </div>
    );
  }

  // Render as portEditor - placeholder for programmable outputs configuration
  if (panelType === 'portEditor' && portEditor) {
    return (
      <div className="embedded-port-editor">
        <div className="port-editor-title">{portEditor.label || name}</div>
        <div className="port-editor-placeholder">
          Programmable Output Configuration: {portEditor.name}
        </div>
      </div>
    );
  }

  // Unknown panel type - show nothing or a placeholder
  return null;
}

function IndicatorPanelRenderer({
  panel,
  context,
}: {
  panel: IndicatorPanel;
  context: Record<string, number>;
}) {
  const [indicatorValues, setIndicatorValues] = useState<Record<string, boolean>>({});

  useEffect(() => {
    // Evaluate all indicator expressions
    const evaluations = panel.indicators.map((ind) =>
      invoke<boolean>('evaluate_expression', {
        expression: ind.expression,
        context,
      })
        .then((value) => ({ expression: ind.expression, value }))
        .catch(() => ({ expression: ind.expression, value: false }))
    );

    Promise.all(evaluations).then((results) => {
      const values: Record<string, boolean> = {};
      results.forEach(({ expression, value }) => {
        values[expression] = value;
      });
      setIndicatorValues(values);
    });
  }, [panel.indicators, context]);

  const columns = panel.columns || 2;
  const gridStyle: React.CSSProperties = {
    display: 'grid',
    gridTemplateColumns: `repeat(${columns}, 1fr)`,
    gap: '8px',
  };

  return (
    <div className="indicator-panel">
      <div style={gridStyle}>
        {panel.indicators.map((ind, i) => {
          const isOn = indicatorValues[ind.expression] || false;
          const fgColor = isOn ? (ind.color_on_fg || 'red') : (ind.color_off_fg || 'white');
          const bgColor = isOn ? (ind.color_on_bg || 'black') : (ind.color_off_bg || 'black');
          
          return (
            <div key={i} className="indicator-field">
              <div
                className={`indicator-light ${isOn ? 'on' : 'off'}`}
                style={{
                  background: isOn ? fgColor : bgColor,
                  boxShadow: isOn ? `0 0 8px ${fgColor}` : 'none',
                }}
              />
              <span className="indicator-label" style={{ color: fgColor }}>
                {isOn ? ind.label_on : ind.label_off}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function DialogFieldWrapper({ 
  comp, 
  context, 
  onUpdate,
  onOptimisticUpdate 
}: { 
  comp: DialogComponent; 
  context: Record<string, number>; 
  onUpdate?: () => void;
  onOptimisticUpdate?: (name: string, value: number) => void;
}) {
  const [fieldVisible, setFieldVisible] = useState<boolean>(true);
  const [fieldEnabled, setFieldEnabled] = useState<boolean>(true);
  
  // Evaluate visibility condition (hides field if false)
  useEffect(() => {
    const visCondition = comp.visibility_condition || (comp.condition && comp.enabled_condition ? undefined : comp.condition);
    if (visCondition) {
      invoke<boolean>('evaluate_expression', { 
        expression: visCondition, 
        context 
      })
        .then((result) => {
          console.log(`[DialogFieldWrapper] Visibility condition '${visCondition}' for '${comp.name}' evaluated to:`, result);
          setFieldVisible(result);
        })
        .catch((err) => {
          console.warn(`[DialogFieldWrapper] Failed to evaluate visibility condition '${visCondition}' for '${comp.name}':`, err);
          setFieldVisible(true); // Show on error
        });
    } else {
      setFieldVisible(true);
    }
  }, [comp.visibility_condition, comp.condition, comp.enabled_condition, context, comp.name]);
  
  // Evaluate enable condition (disables field if false)
  // Per closed-source program suggestion: "all 12 channels should be visible but disabled"
  useEffect(() => {
    const enCondition = comp.enabled_condition || (comp.condition && !comp.visibility_condition ? comp.condition : undefined);
    if (enCondition) {
      invoke<boolean>('evaluate_expression', { 
        expression: enCondition, 
        context 
      })
        .then((result) => {
          console.log(`[DialogFieldWrapper] Enable condition '${enCondition}' for '${comp.name}' evaluated to:`, result);
          setFieldEnabled(result);
        })
        .catch((err) => {
          console.warn(`[DialogFieldWrapper] Failed to evaluate enable condition '${enCondition}' for '${comp.name}':`, err);
          setFieldEnabled(true); // Enable on error
        });
    } else {
      setFieldEnabled(true);
    }
  }, [comp.enabled_condition, comp.condition, comp.visibility_condition, context, comp.name]);
  
  // Hide field if visibility condition is false
  if (!fieldVisible || !comp.name) return null;
  
  return <DialogField 
    label={comp.label || ''} 
    name={comp.name} 
    onUpdate={onUpdate} 
    context={context}
    fieldEnabledCondition={fieldEnabled}
    onOptimisticUpdate={onOptimisticUpdate}
  />;
}

function PanelVisibilityWrapper({
  comp,
  openTable,
  context,
  onUpdate,
}: {
  comp: DialogComponent;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
}) {
  const [panelVisible, setPanelVisible] = useState<boolean>(true);
  
  useEffect(() => {
    if (comp.visibility_condition) {
      invoke<boolean>('evaluate_expression', { 
        expression: comp.visibility_condition, 
        context 
      })
        .then(setPanelVisible)
        .catch((err) => {
          console.warn(`[PanelVisibilityWrapper] Failed to evaluate panel visibility condition '${comp.visibility_condition}':`, err);
          setPanelVisible(true); // Show on error
        });
    } else {
      setPanelVisible(true);
    }
  }, [comp.visibility_condition, context]);
  
  if (!panelVisible || !comp.name) return null;
  
  return <RecursivePanel name={comp.name} openTable={openTable} context={context} onUpdate={onUpdate} />;
}

function DialogComponentRenderer({
  comp,
  openTable,
  context,
  onUpdate,
  onOptimisticUpdate,
}: {
  comp: DialogComponent;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
  onOptimisticUpdate?: (name: string, value: number) => void;
}) {
  if (comp.type === 'Field' && comp.name) {
    return <DialogFieldWrapper comp={comp} context={context} onUpdate={onUpdate} onOptimisticUpdate={onOptimisticUpdate} />;
  }
  if (comp.type === 'Label' && comp.text) {
    return <div className="dialog-label">{comp.text}</div>;
  }
  if (comp.type === 'Table' && comp.name) {
    // Use RecursivePanel to handle table rendering (embedded or link fallback)
    return <RecursivePanel name={comp.name} openTable={openTable} context={context} onUpdate={onUpdate} />;
  }
  if (comp.type === 'LiveGraph') {
    return (
      <div className="embedded-graph-placeholder">
        <Activity size={20} />
        <span>Live Graph: {comp.title || comp.name}</span>
      </div>
    );
  }
  if (comp.type === 'Panel' && comp.name) {
    return <PanelVisibilityWrapper comp={comp} openTable={openTable} context={context} onUpdate={onUpdate} />;
  }
  if (comp.type === 'Indicator') {
    return <Indicator comp={comp} context={context} />;
  }
  if (comp.type === 'CommandButton' && comp.command) {
    return <CommandButton comp={comp} context={context} />;
  }
  return null;
}

export interface DialogRendererProps {
  definition: DialogDefinition;
  onBack: () => void;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
  onOptimisticUpdate?: (name: string, value: number) => void;
  /** Override title for display (formatted as "Menu Label (ini_name)") */
  displayTitle?: string;
}

export default function DialogRenderer({ definition, onBack, openTable, context, onUpdate, onOptimisticUpdate, displayTitle }: DialogRendererProps) {
  // The context is already dynamic - it contains the current values of all constants
  // Conditions like {cylindersCount > 5} will automatically evaluate based on the current cylindersCount value
  // This works for any cylinder count: 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, etc.
  
  return (
    <div className="dialog-view view-transition">
      <div className="editor-header">
        <button onClick={onBack} className="icon-btn" title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 className="content-title" style={{ margin: 0 }}>
          {displayTitle || definition.title}
        </h2>
      </div>

      <div className="glass-card dialog-container">
        {definition.components.map((comp, i) => (
          <DialogComponentRenderer key={i} comp={comp} openTable={openTable} context={context} onUpdate={onUpdate} onOptimisticUpdate={onOptimisticUpdate} />
        ))}
      </div>
    </div>
  );
}

// Export types for use in App.tsx
export type { DialogDefinition, DialogComponent };
