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

function DialogField({ label, name, onUpdate }: { label: string; name: string; onUpdate?: () => void }) {
  const [constant, setConstant] = useState<Constant | null>(null);
  const [numValue, setNumValue] = useState<number | null>(null);
  const [strValue, setStrValue] = useState<string>('');
  const [selectedBit, setSelectedBit] = useState<number>(0);

  useEffect(() => {
    invoke<Constant>('get_constant', { name }).then((c) => {
      setConstant(c);
      // Fetch value based on type
      if (c.value_type === 'string') {
        invoke<string>('get_constant_string_value', { name })
          .then(setStrValue)
          .catch(() => setStrValue(''));
      } else if (c.value_type === 'bits') {
        invoke<number>('get_constant_value', { name })
          .then((v) => setSelectedBit(Math.round(v)))
          .catch(() => setSelectedBit(0));
      } else {
        invoke<number>('get_constant_value', { name })
          .then(setNumValue)
          .catch(() => setNumValue(0));
      }
    }).catch(console.error);
  }, [name]);

  if (!constant) return <div className="field-loading">Loading {label}...</div>;

  const displayLabel = label || constant.label || constant.name;

  // String field
  if (constant.value_type === 'string') {
    return (
      <div className="settings-field">
        <label>{displayLabel}</label>
        <div className="field-input-wrap">
          <input
            type="text"
            value={strValue}
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
    // If only 2 options, render as checkbox
    if (constant.bit_options.length === 2) {
      return (
        <div className="settings-field">
          <label>
            <input
              type="checkbox"
              checked={selectedBit === 1}
              onChange={(e) => {
                const newVal = e.target.checked ? 1 : 0;
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
    return (
      <div className="settings-field">
        <label>{displayLabel}</label>
        <div className="field-input-wrap">
          <select
            value={selectedBit}
            onChange={(e) => {
              const newVal = parseInt(e.target.value, 10);
              setSelectedBit(newVal);
              invoke('update_constant', { name, value: newVal })
                .then(() => onUpdate?.())
                .catch((err) => alert('Update failed: ' + err));
            }}
          >
            {constant.bit_options.map((opt, i) => (
              <option key={i} value={i}>{opt}</option>
            ))}
          </select>
        </div>
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
  const [tableInfo, setTableInfo] = useState<TableInfo | null>(null);
  const [curveData, setCurveData] = useState<CurveData | null>(null);
  const [panelType, setPanelType] = useState<'loading' | 'dialog' | 'table' | 'curve' | 'unknown'>('loading');

  useEffect(() => {
    // First try as dialog
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
                // Neither dialog nor table nor curve
                setPanelType('unknown');
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
    return <DialogField label={comp.label || ''} name={comp.name} onUpdate={onUpdate} />;
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
    return <RecursivePanel name={comp.name} openTable={openTable} context={context} onUpdate={onUpdate} />;
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
