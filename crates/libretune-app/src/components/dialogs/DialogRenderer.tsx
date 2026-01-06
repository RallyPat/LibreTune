import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ArrowLeft, Activity, Grid3X3 } from 'lucide-react';
import './DialogRenderer.css';

interface DialogComponent {
  type: 'Panel' | 'Field' | 'LiveGraph' | 'Table' | 'Label' | 'Indicator';
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
}

interface TableInfo {
  name: string;
  title: string;
}

interface CurveData {
  name: string;
  title: string;
  x_bins: number[];
  y_bins: number[];
  x_label: string;
  y_label: string;
}

// Simple inline curve chart renderer using SVG
function CurveChart({ data }: { data: CurveData }) {
  const width = 300;
  const height = 200;
  const padding = 40;
  
  const xMin = Math.min(...data.x_bins);
  const xMax = Math.max(...data.x_bins);
  const yMin = Math.min(...data.y_bins);
  const yMax = Math.max(...data.y_bins);
  
  const xRange = xMax - xMin || 1;
  const yRange = yMax - yMin || 1;
  
  const scaleX = (x: number) => padding + ((x - xMin) / xRange) * (width - 2 * padding);
  const scaleY = (y: number) => height - padding - ((y - yMin) / yRange) * (height - 2 * padding);
  
  const points = data.x_bins.map((x, i) => `${scaleX(x)},${scaleY(data.y_bins[i])}`).join(' ');
  
  return (
    <svg width={width} height={height} className="curve-svg">
      {/* Grid lines */}
      <line x1={padding} y1={height - padding} x2={width - padding} y2={height - padding} stroke="#444" />
      <line x1={padding} y1={height - padding} x2={padding} y2={padding} stroke="#444" />
      
      {/* Axis labels */}
      <text x={width / 2} y={height - 5} textAnchor="middle" fill="#aaa" fontSize="10">{data.x_label}</text>
      <text x={10} y={height / 2} textAnchor="middle" fill="#aaa" fontSize="10" transform={`rotate(-90, 10, ${height / 2})`}>{data.y_label}</text>
      
      {/* Data line */}
      <polyline points={points} fill="none" stroke="#4a9eff" strokeWidth="2" />
      
      {/* Data points */}
      {data.x_bins.map((x, i) => (
        <circle key={i} cx={scaleX(x)} cy={scaleY(data.y_bins[i])} r="4" fill="#4a9eff" />
      ))}
    </svg>
  );
}

function DialogField({ 
  label, 
  name, 
  onUpdate, 
  context,
  fieldEnabledCondition 
}: { 
  label: string; 
  name: string; 
  onUpdate?: () => void; 
  context: Record<string, number>;
  fieldEnabledCondition?: boolean; // Enable condition from DialogComponent::Field
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
        <label>{displayLabel}</label>
        <div className="field-input-wrap">
          <input
            type="text"
            value={strValue}
            disabled={!isEnabled}
            onChange={(e) => setStrValue(e.target.value)}
            onBlur={() => {
              // TODO: Add update_constant_string command when implemented
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
                  .then(() => onUpdate?.())
                  .catch((e) => alert('Update failed: ' + e));
              }}
            />
            {displayLabel}
          </label>
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
        <label>{displayLabel}</label>
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
                  .then(() => onUpdate?.())
                  .catch((err) => alert('Update failed: ' + err));
              } else {
                // Fallback: use the filtered value directly if not in map
                console.warn(`[DialogField] No original index found for filtered index ${filteredVal}, using directly`);
                setSelectedBit(filteredVal);
                invoke('update_constant', { name, value: filteredVal })
                  .then(() => onUpdate?.())
                  .catch((err) => alert('Update failed: ' + err));
              }
            }}
          >
            {validBitOptions.length === 0 ? (
              <option value={0}>No options available</option>
            ) : (
              validBitOptions.map((opt, i) => (
                <option key={i} value={i}>{opt}</option>
              ))
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
    <div className="settings-field">
      <label>{displayLabel}</label>
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
                .then(() => onUpdate?.())
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
  const [curveData, setCurveData] = useState<CurveData | null>(null);
  const [panelType, setPanelType] = useState<'loading' | 'dialog' | 'indicatorPanel' | 'table' | 'curve' | 'unknown'>('loading');

  useEffect(() => {
    // First try as indicatorPanel
    invoke<IndicatorPanel>('get_indicator_panel', { name })
      .then((panel) => {
        setIndicatorPanel(panel);
        setPanelType('indicatorPanel');
      })
      .catch(() => {
        // Not an indicatorPanel, try as dialog
        invoke<DialogDefinition>('get_dialog_definition', { name })
          .then((def) => {
            setDefinition(def);
            setPanelType('dialog');
          })
          .catch(() => {
            // Not a dialog, try as table (lightweight check)
            invoke<TableInfo>('get_table_info', { tableName: name })
              .then((info) => {
                setTableInfo(info);
                setPanelType('table');
              })
              .catch((err) => {
                console.debug(`Panel '${name}' is not a table:`, err);
                // Not a table, try as curve
                invoke<CurveData>('get_curve_data', { curveName: name })
                  .then((data) => {
                    setCurveData(data);
                    setPanelType('curve');
                  })
                  .catch((err2) => {
                    console.debug(`Panel '${name}' is not a curve:`, err2);
                    // Neither indicatorPanel nor dialog nor table nor curve
                    setPanelType('unknown');
                  });
              });
          });
      });
  }, [name]);

  if (panelType === 'loading') {
    return <div className="panel-loading">Loading {name}...</div>;
  }

  // Render as clickable table link if it's a table
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
      <div className="embedded-curve">
        <div className="curve-title">{curveData.title || name}</div>
        <div className="curve-chart">
          <CurveChart data={curveData} />
        </div>
      </div>
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
  onUpdate 
}: { 
  comp: DialogComponent; 
  context: Record<string, number>; 
  onUpdate?: () => void;
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
}: {
  comp: DialogComponent;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
}) {
  if (comp.type === 'Field' && comp.name) {
    return <DialogFieldWrapper comp={comp} context={context} onUpdate={onUpdate} />;
  }
  if (comp.type === 'Label' && comp.text) {
    return <div className="dialog-label">{comp.text}</div>;
  }
  if (comp.type === 'Table' && comp.name) {
    return (
      <div className="embedded-table-link" onClick={() => openTable(comp.name!)}>
        <Grid3X3 size={20} />
        <span>Open Table: {comp.name}</span>
      </div>
    );
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
  return null;
}

export interface DialogRendererProps {
  definition: DialogDefinition;
  onBack: () => void;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
}

export default function DialogRenderer({ definition, onBack, openTable, context, onUpdate }: DialogRendererProps) {
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
          {definition.title}
        </h2>
      </div>

      <div className="glass-card dialog-container">
        {definition.components.map((comp, i) => (
          <DialogComponentRenderer key={i} comp={comp} openTable={openTable} context={context} onUpdate={onUpdate} />
        ))}
      </div>
    </div>
  );
}

// Export types for use in App.tsx
export type { DialogDefinition, DialogComponent };
