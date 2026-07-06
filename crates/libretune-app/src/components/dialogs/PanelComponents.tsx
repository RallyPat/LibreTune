/**
 * Internal panel/field/component renderers used by DialogRenderer.
 * These are mutually recursive (RecursivePanel -> DialogComponentRenderer ->
 * RecursivePanel) so they live together in this file.
 */

import { useState, useEffect, useLayoutEffect, useRef, memo, useMemo, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Activity, Grid3X3, AlertTriangle } from 'lucide-react';
import CurveEditor, { SimpleGaugeInfo } from '../curves/CurveEditor';
import TableEditor2D from '../tables/TableEditor2D';
import {
  type DialogComponent,
  type DialogDefinition,
  type TableInfo,
  type BackendTableData,
  type CurveData,
  type FieldInfo,
  type IndicatorPanel,
  type ReadoutPanel,
  type PortEditorConfig,
  buildStdPlaceholderDefinition,
} from './types';
import { Indicator } from './fields/Indicator';
import { IndicatorPanelRenderer } from './fields/IndicatorPanelRenderer';
import { ReadoutPanelRenderer } from './fields/ReadoutPanelRenderer';
import { CommandButton } from './fields/CommandButton';
import DialogField from './fields/DialogField';
import { RuntimeValueReadout } from './fields/RuntimeValueReadout';
import { isUserTableLiveChannel } from './dialogLayout';

export const RecursivePanel = memo(function RecursivePanel({
  name,
  openTable,
  context,
  onUpdate,
  onFieldFocus,
  showAllHelpIcons,
}: {
  name: string;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
  onFieldFocus?: (info: FieldInfo) => void;
  showAllHelpIcons?: boolean;
}) {
  const [definition, setDefinition] = useState<DialogDefinition | null>(null);
  const [indicatorPanel, setIndicatorPanel] = useState<IndicatorPanel | null>(null);
  const [readoutPanel, setReadoutPanel] = useState<ReadoutPanel | null>(null);
  const [tableInfo, setTableInfo] = useState<TableInfo | null>(null);
  const [tableData, setTableData] = useState<BackendTableData | null>(null);
  const [curveData, setCurveData] = useState<CurveData | null>(null);
  const [gaugeConfig, setGaugeConfig] = useState<SimpleGaugeInfo | null>(null);
  const [portEditor, setPortEditor] = useState<PortEditorConfig | null>(null);
  const [panelType, setPanelType] = useState<'loading' | 'dialog' | 'indicatorPanel' | 'readoutPanel' | 'table' | 'curve' | 'portEditor' | 'unknown'>('loading');

  useLayoutEffect(() => {
    let cancelled = false;
    
    setPanelType('loading');
    setDefinition(null);
    setIndicatorPanel(null);
    setReadoutPanel(null);
    setTableInfo(null);
    setTableData(null);
    setCurveData(null);
    setGaugeConfig(null);
    setPortEditor(null);

    const stdPlaceholder = buildStdPlaceholderDefinition(name);
    if (stdPlaceholder) {
      setDefinition(stdPlaceholder);
      setPanelType('dialog');
      return () => {
        cancelled = true;
      };
    }

    // First try as indicatorPanel
    invoke<IndicatorPanel>('get_indicator_panel', { name })
      .then((panel) => {
        if (cancelled) return;
        console.debug(`[RecursivePanel] '${name}' resolved as indicatorPanel`);
        setIndicatorPanel(panel);
        setPanelType('indicatorPanel');
      })
      .catch(() => {
        if (cancelled) return;
        invoke<ReadoutPanel>('get_readout_panel', { name })
          .then((panel) => {
            if (cancelled) return;
            console.debug(`[RecursivePanel] '${name}' resolved as readoutPanel`);
            setReadoutPanel(panel);
            setPanelType('readoutPanel');
          })
          .catch(() => {
            if (cancelled) return;
            // Not a readoutPanel, try as dialog
            invoke<DialogDefinition>('get_dialog_definition', { name })
          .then((def) => {
            if (cancelled) return;
            console.debug(`[RecursivePanel] '${name}' resolved as dialog`);
            setDefinition(def);
            setPanelType('dialog');
          })
          .catch(() => {
            if (cancelled) return;
            // Not a dialog, try as table (lightweight check first, then full data)
            invoke<TableInfo>('get_table_info', { tableName: name })
              .then((info) => {
                if (cancelled) return;
                console.debug(`[RecursivePanel] '${name}' resolved as table: ${info.title}`);
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
                console.log(`[RecursivePanel] Trying to resolve '${name}' as curve...`);
                invoke<CurveData>('get_curve_data', { curveName: name })
                  .then((data) => {
                    if (cancelled) return;
                    console.log(`[RecursivePanel] ✅ '${name}' resolved as curve:`, {
                      name: data.name,
                      title: data.title,
                      x_bins: data.x_bins,
                      y_bins: data.y_bins,
                      x_bins_type: typeof data.x_bins,
                      y_bins_type: typeof data.y_bins,
                      x_bins_isArray: Array.isArray(data.x_bins),
                      rawData: JSON.stringify(data).slice(0, 500),
                    });
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
                    console.warn(`[RecursivePanel] ⚠️ Panel '${name}' is not a curve. Error:`, err2);
                    // Not a curve, try as portEditor
                    invoke<PortEditorConfig>('get_port_editor', { name })
                      .then((editor) => {
                        if (cancelled) return;
                        console.debug(`[RecursivePanel] '${name}' resolved as portEditor`);
                        setPortEditor(editor);
                        setPanelType('portEditor');
                      })
                      .catch((err3) => {
                        if (cancelled) return;
                        console.debug(`Panel '${name}' is not a portEditor:`, err3);
                        // None of the known types - log all errors for debugging
                        console.error(`[RecursivePanel] ❌ Panel '${name}' could not be resolved as any known type:`, {
                          indicatorPanel: 'not an indicatorPanel',
                          readoutPanel: 'not a readoutPanel',
                          dialog: 'not a dialog',
                          table: String(err),
                          curve: String(err2),
                          portEditor: String(err3),
                        });
                        setPanelType('unknown');
                      });
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
        x_output_channel={tableData.x_output_channel}
        y_output_channel={tableData.y_output_channel}
        embedded={true}
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
    const hasValidBins = curveData.x_bins?.length > 0 && curveData.y_bins?.length > 0;
    console.log(`[RecursivePanel] 📈 RENDERING CurveEditor for '${name}' with data:`, {
      name: curveData.name,
      title: curveData.title,
      x_bins_length: curveData.x_bins?.length ?? 0,
      y_bins_length: curveData.y_bins?.length ?? 0,
      hasValidBins,
      x_bins_sample: curveData.x_bins?.slice(0, 3),
      y_bins_sample: curveData.y_bins?.slice(0, 3),
    });
    if (!hasValidBins) {
      console.warn(`[RecursivePanel] ⚠️ Curve '${name}' has empty bins - curve may not render correctly. Check get_curve_data backend logs.`);
    }
    return (
      <CurveEditor
        data={curveData}
        embedded={true}
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

  // Render as readoutPanel (live numeric gauges)
  if (panelType === 'readoutPanel' && readoutPanel) {
    return <ReadoutPanelRenderer panel={readoutPanel} />;
  }

  // Render as dialog
  if (panelType === 'dialog' && definition) {
    const fieldCount = definition.components.filter((c) => c.type === 'Field').length;
    const multiColumn = fieldCount >= 8;
    return (
      <div className={`nested-panel${multiColumn ? ' nested-panel--multi' : ''}`}>
        {definition.title && definition.title !== name && <div className="panel-title">{definition.title}</div>}
        <div className={`panel-content${multiColumn ? ' panel-content--multi' : ''}`}>
          {definition.components.map((comp, i) => (
            <DialogComponentRenderer key={i} comp={comp} openTable={openTable} context={context} onUpdate={onUpdate} onFieldFocus={onFieldFocus} showAllHelpIcons={showAllHelpIcons} />
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

  // Unknown panel type - show error feedback so users can report missing panels
  return (
    <div className="panel-load-error">
      <span className="panel-error-icon"><AlertTriangle size={16} /></span>
      <span>Panel "{name}" could not be loaded</span>
    </div>
  );
});

export function DialogFieldWrapper({ 
  comp, 
  context, 
  onUpdate,
  onOptimisticUpdate,
  onFieldFocus,
  showAllHelpIcons
}: { 
  comp: DialogComponent; 
  context: Record<string, number>; 
  onUpdate?: () => void;
  onOptimisticUpdate?: (name: string, value: number) => void;
  onFieldFocus?: (info: FieldInfo) => void;
  showAllHelpIcons?: boolean;
}) {
  const [fieldVisible, setFieldVisible] = useState<boolean>(true);
  const [fieldEnabled, setFieldEnabled] = useState<boolean>(true);
  
  // Evaluate visibility condition (hides field if false)
  useEffect(() => {
    const visCondition =
      comp.visibility_condition ||
      (comp.condition && comp.enabled_condition ? comp.condition : undefined);
    const enCondition =
      comp.enabled_condition ||
      (comp.condition && !comp.visibility_condition ? comp.condition : undefined);

    const evaluate = (expression: string) =>
      invoke<boolean>('evaluate_expression', {
        expression,
        context,
      });

    if (visCondition) {
      evaluate(visCondition)
        .then((result) => setFieldVisible(result))
        .catch((err) => {
          console.warn(
            `[DialogFieldWrapper] Failed to evaluate visibility condition '${visCondition}' for '${comp.name}':`,
            err,
          );
          setFieldVisible(true);
        });
      return;
    }

    if (enCondition) {
      evaluate(enCondition)
        .then((result) => setFieldVisible(result))
        .catch((err) => {
          console.warn(
            `[DialogFieldWrapper] Failed to evaluate enable condition '${enCondition}' for '${comp.name}':`,
            err,
          );
          setFieldVisible(true);
        });
      return;
    }

    setFieldVisible(true);
  }, [comp.visibility_condition, comp.condition, comp.enabled_condition, context, comp.name]);

  // Evaluate enable condition (disables field if false when a separate visibility condition exists)
  useEffect(() => {
    if (!comp.visibility_condition) {
      setFieldEnabled(true);
      return;
    }

    const enCondition =
      comp.enabled_condition ||
      (comp.condition && !comp.visibility_condition ? comp.condition : undefined);
    if (enCondition) {
      invoke<boolean>('evaluate_expression', {
        expression: enCondition,
        context,
      })
        .then((result) => {
          setFieldEnabled(result);
        })
        .catch((err) => {
          console.warn(
            `[DialogFieldWrapper] Failed to evaluate enable condition '${enCondition}' for '${comp.name}':`,
            err,
          );
          setFieldEnabled(true);
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
    onFieldFocus={onFieldFocus}
    showAllHelpIcons={showAllHelpIcons}
  />;
}

export function PanelVisibilityWrapper({
  comp,
  openTable,
  context,
  onUpdate,
  onFieldFocus,
  showAllHelpIcons,
}: {
  comp: DialogComponent;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
  onFieldFocus?: (info: FieldInfo) => void;
  showAllHelpIcons?: boolean;
}) {
  const [panelVisible, setPanelVisible] = useState<boolean>(true);

  const contextRef = useRef(context);
  contextRef.current = context;

  const normalizeExpression = useCallback((expression: string) => {
    if (!expression.includes('{') && !expression.includes('(') && !expression.includes(' ')) {
      return `{${expression}}`;
    }
    return expression;
  }, []);

  const conditionContextKey = useMemo(() => {
    const expressions = [
      comp.visibility_condition,
      comp.enabled_condition,
      comp.name === 'wmiLiveStateDialog' ? 'isWmiEnabled' : null,
    ].filter(Boolean) as string[];

    const vars = new Set<string>();
    for (const expression of expressions) {
      for (const match of expression.matchAll(/\{?(\w+)\}?/g)) {
        vars.add(match[1]);
      }
    }

    return Array.from(vars)
      .sort()
      .map((varName) => `${varName}:${context[varName] ?? 0}`)
      .join('|');
  }, [comp.visibility_condition, comp.enabled_condition, comp.name, context]);

  useEffect(() => {
    let cancelled = false;

    const evaluateCondition = async (expression: string) => {
      return invoke<boolean>('evaluate_expression', {
        expression: normalizeExpression(expression),
        context: contextRef.current,
      });
    };

    (async () => {
      try {
        if (comp.visibility_condition) {
          const visible = await evaluateCondition(comp.visibility_condition);
          if (cancelled) return;
          if (!visible) {
            setPanelVisible(false);
            return;
          }
        }

        if (comp.enabled_condition) {
          const enabled = await evaluateCondition(comp.enabled_condition);
          if (cancelled) return;
          if (!enabled) {
            setPanelVisible(false);
            return;
          }
        }

        // WMI live readouts have no panel-level condition in the INI — gate on isWmiEnabled.
        if (
          !comp.visibility_condition &&
          !comp.enabled_condition &&
          comp.name === 'wmiLiveStateDialog'
        ) {
          const enabled = await evaluateCondition('isWmiEnabled');
          if (cancelled) return;
          if (!enabled) {
            setPanelVisible(false);
            return;
          }
        }

        if (!cancelled) {
          setPanelVisible(true);
        }
      } catch (err) {
        console.warn(
          `[PanelVisibilityWrapper] Failed to evaluate panel conditions for '${comp.name}':`,
          err,
        );
        if (!cancelled) {
          setPanelVisible(true);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [
    comp.visibility_condition,
    comp.enabled_condition,
    comp.name,
    conditionContextKey,
    normalizeExpression,
  ]);

  if (!panelVisible || !comp.name) {
    return null;
  }

  return (
    <RecursivePanel
      key={`panel-${comp.name}`}
      name={comp.name}
      openTable={openTable}
      context={context}
      onUpdate={onUpdate}
      onFieldFocus={onFieldFocus}
      showAllHelpIcons={showAllHelpIcons}
    />
  );
}

export function DialogComponentRenderer({
  comp,
  openTable,
  context,
  onUpdate,
  onOptimisticUpdate,
  onFieldFocus,
  showAllHelpIcons,
}: {
  comp: DialogComponent;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
  onOptimisticUpdate?: (name: string, value: number) => void;
  onFieldFocus?: (info: FieldInfo) => void;
  showAllHelpIcons?: boolean;
}) {
  if (comp.type === 'Field' && comp.name) {
    return <DialogFieldWrapper comp={comp} context={context} onUpdate={onUpdate} onOptimisticUpdate={onOptimisticUpdate} onFieldFocus={onFieldFocus} showAllHelpIcons={showAllHelpIcons} />;
  }
  if (comp.type === 'RuntimeValue' && comp.name) {
    if (isUserTableLiveChannel(comp.name)) {
      return null;
    }
    return (
      <RuntimeValueReadout
        label={comp.label || comp.name}
        channel={comp.name}
        visibilityCondition={comp.visibility_condition}
        context={context}
      />
    );
  }
  if (comp.type === 'Label' && comp.text) {
    const text = comp.text.trim();
    if (/^https?:\/\//i.test(text)) {
      return (
        <div className="dialog-label dialog-link">
          <a href={text} target="_blank" rel="noopener noreferrer">
            {text}
          </a>
        </div>
      );
    }
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
    console.log(`[DialogComponentRenderer] Rendering Panel component: ${comp.name}, visibility_condition: ${comp.visibility_condition || 'none'}`);
    return <PanelVisibilityWrapper comp={comp} openTable={openTable} context={context} onUpdate={onUpdate} onFieldFocus={onFieldFocus} showAllHelpIcons={showAllHelpIcons} />;
  }
  if (comp.type === 'Indicator') {
    return <Indicator comp={comp} context={context} />;
  }
  if (comp.type === 'CommandButton' && comp.command) {
    return <CommandButton comp={comp} context={context} />;
  }
  return null;
}


