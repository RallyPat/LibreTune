/**
 * Internal panel/field/component renderers used by DialogRenderer.
 * These are mutually recursive (RecursivePanel -> DialogComponentRenderer ->
 * RecursivePanel) so they live together in this file.
 */

import { useState, useEffect, useLayoutEffect, memo, useMemo, useCallback, Suspense } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Activity, Grid3X3, AlertTriangle } from 'lucide-react';
import { LazyCurveEditor } from '../TabContentLazy';
import type { SimpleGaugeInfo } from '../curves/CurveEditor';
import TableEditor2D from '../tables/TableEditor2D';
import { resolveEmbeddedPanelKind } from '../../utils/resolveEmbeddedPanelKind';
import { evaluateIniBoolean } from '../../utils/iniExpression';
import { getConstantValues, useConstantValuesStore } from '../../stores/constantValuesStore';
import {
  prefetchFieldsForDefinition,
} from '../../stores/constantsMetadataCache';
import {
  fetchPanelDefinitionPriority,
  getCachedPanelDefinition,
} from '../../stores/panelDefinitionCache';
import {
  type DialogComponent,
  type DialogDefinition,
  type TableInfo,
  type BackendTableData,
  type CurveData,
  type FieldInfo,
  type IndicatorPanel,
  type PortEditorConfig,
  buildStdPlaceholderDefinition,
} from './types';
import { Indicator } from './fields/Indicator';
import { IndicatorPanelRenderer } from './fields/IndicatorPanelRenderer';
import { CommandButton } from './fields/CommandButton';
import DialogField from './fields/DialogField';

function patchConstantValue(name: string, value: number) {
  useConstantValuesStore.getState().patch(name, value);
}

type CmdGroup = { __cmdGroup: true; items: DialogComponent[] };
function groupCommandButtons(components: DialogComponent[]): Array<DialogComponent | CmdGroup> {
  const result: Array<DialogComponent | CmdGroup> = [];
  let batch: DialogComponent[] = [];
  for (const comp of components) {
    if (comp.type === 'CommandButton' && comp.command) {
      batch.push(comp);
    } else {
      if (batch.length) { result.push({ __cmdGroup: true, items: batch }); batch = []; }
      result.push(comp);
    }
  }
  if (batch.length) result.push({ __cmdGroup: true, items: batch });
  return result;
}
export type { CmdGroup };
export { groupCommandButtons };

export const RecursivePanel = memo(function RecursivePanel({
  name,
  openTable,
  context,
  onUpdate,
  onOptimisticUpdate,
  onFieldFocus,
  showAllHelpIcons,
  searchFilter,
}: {
  name: string;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
  onOptimisticUpdate?: (name: string, value: number) => void;
  onFieldFocus?: (info: FieldInfo) => void;
  showAllHelpIcons?: boolean;
  searchFilter?: string;
}) {
  const applyOptimistic = onOptimisticUpdate ?? patchConstantValue;
  const [definition, setDefinition] = useState<DialogDefinition | null>(null);
  const [indicatorPanel, setIndicatorPanel] = useState<IndicatorPanel | null>(null);
  const [tableInfo, setTableInfo] = useState<TableInfo | null>(null);
  const [tableData, setTableData] = useState<BackendTableData | null>(null);
  const [curveData, setCurveData] = useState<CurveData | null>(null);
  const [gaugeConfig, setGaugeConfig] = useState<SimpleGaugeInfo | null>(null);
  const [portEditor, setPortEditor] = useState<PortEditorConfig | null>(null);
  const [panelType, setPanelType] = useState<'loading' | 'dialog' | 'indicatorPanel' | 'table' | 'curve' | 'portEditor' | 'unknown'>('loading');

  // Prefetch field metadata in the background — never block panel render (DialogField loads per-field).
  useEffect(() => {
    if (panelType === 'dialog' && definition) {
      void prefetchFieldsForDefinition(definition);
    }
  }, [panelType, definition]);

  useLayoutEffect(() => {
    let cancelled = false;

    const cachedDialog = getCachedPanelDefinition(name);
    if (cachedDialog) {
      setDefinition(cachedDialog);
      setPanelType('dialog');
      return () => { cancelled = true; };
    }

    setPanelType('loading');
    setDefinition(null);
    setIndicatorPanel(null);
    setTableInfo(null);
    setTableData(null);
    setCurveData(null);
    setGaugeConfig(null);
    setPortEditor(null);

    const stdPlaceholder = buildStdPlaceholderDefinition(name);
    if (stdPlaceholder) {
      setDefinition(stdPlaceholder);
      setPanelType('dialog');
      return () => { cancelled = true; };
    }

    // Dialog panels are the common case — resolve first (not indicatorPanel).
    void fetchPanelDefinitionPriority(name).then((def) => {
      if (cancelled) return;
      if (def) {
        setDefinition(def);
        setPanelType('dialog');
        return;
      }

      invoke<IndicatorPanel>('get_indicator_panel', { name })
        .then((panel) => {
          if (cancelled) return;
          setIndicatorPanel(panel);
          setPanelType('indicatorPanel');
        })
        .catch(() => {
          if (cancelled) return;

          const tryPortEditor = () => {
            invoke<PortEditorConfig>('get_port_editor', { name })
              .then((editor) => {
                if (cancelled) return;
                setPortEditor(editor);
                setPanelType('portEditor');
              })
              .catch(() => {
                if (cancelled) return;
                setPanelType('unknown');
              });
          };

          const loadCurve = () => {
            invoke<CurveData>('get_curve_data', { curveName: name })
              .then((data) => {
                if (cancelled) return;
                setCurveData(data);
                setPanelType('curve');
                if (data.gauge) {
                  invoke<SimpleGaugeInfo>('get_gauge_config', { gaugeName: data.gauge })
                    .then((gc) => { if (!cancelled) setGaugeConfig(gc); })
                    .catch(() => {});
                }
              })
              .catch(() => {
                if (cancelled) return;
                tryPortEditor();
              });
          };

          const loadTable = () => {
            let tableInfoLoaded = false;
            invoke<TableInfo>('get_table_info', { tableName: name })
              .then((info) => {
                if (cancelled) return undefined;
                tableInfoLoaded = true;
                setTableInfo(info);
                return invoke<BackendTableData>('get_table_data', { tableName: name });
              })
              .then((data) => {
                if (cancelled || data === undefined) return;
                setTableData(data);
                setPanelType('table');
              })
              .catch(() => {
                if (cancelled) return;
                // Only enter 'table' state if get_table_info succeeded (tableInfo is set).
                // If get_table_info itself failed, fall through to unknown so the panel
                // doesn't go blank.
                if (tableInfoLoaded) {
                  setPanelType('table');
                } else {
                  setPanelType('unknown');
                }
              });
          };

          resolveEmbeddedPanelKind(name)
            .then((kind) => {
              if (cancelled) return;
              if (kind === 'curve') loadCurve();
              else if (kind === 'table') loadTable();
              else tryPortEditor();
            })
            .catch(() => {
              if (cancelled) return;
              tryPortEditor();
            });
        });
    });

    return () => {
      cancelled = true;
    };
  }, [name]);

  const refreshTableData = useCallback(() => {
    if (panelType !== 'table') return;
    invoke<BackendTableData>('get_table_data', { tableName: name })
      .then((data) => setTableData(data))
      .catch((err) => console.debug(`Could not refresh table data for '${name}':`, err));
  }, [panelType, name]);

  useEffect(() => {
    if (panelType !== 'table') return;
    let unlisten: (() => void) | undefined;
    listen('tune:loaded', refreshTableData).then((fn) => {
      unlisten = fn;
    });
    const onConstantsUpdated = () => refreshTableData();
    window.addEventListener('constants:updated', onConstantsUpdated);
    return () => {
      unlisten?.();
      window.removeEventListener('constants:updated', onConstantsUpdated);
    };
  }, [panelType, name, refreshTableData]);

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
        z_output_channel={tableData.z_output_channel}
        embedded={true}
        onOpenInTab={() => openTable(name)}
        onValuesChange={() => onUpdate?.()}
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
    if (!hasValidBins) {
      console.warn(`[RecursivePanel] Curve '${name}' has empty bins`);
    }
    return (
      <Suspense fallback={<div className="panel-loading">Loading curve...</div>}>
        <LazyCurveEditor
          data={curveData}
          embedded={true}
          simpleGaugeInfo={gaugeConfig}
          onValuesChange={() => {
            onUpdate?.();
          }}
        />
      </Suspense>
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
          {groupCommandButtons(definition.components ?? []).map((item, i) =>
            '__cmdGroup' in item ? (
              <div key={i} className="command-button-group">
                {item.items.map((comp, j) => (
                  <CommandButton key={j} comp={comp} context={context} />
                ))}
              </div>
            ) : (
              <DialogComponentRenderer key={i} comp={item} openTable={openTable} context={context} onUpdate={onUpdate} onOptimisticUpdate={applyOptimistic} onFieldFocus={onFieldFocus} showAllHelpIcons={showAllHelpIcons} searchFilter={searchFilter} />
            )
          )}
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
  showAllHelpIcons,
  searchFilter,
}: { 
  comp: DialogComponent; 
  context: Record<string, number>; 
  onUpdate?: () => void;
  onOptimisticUpdate?: (name: string, value: number) => void;
  onFieldFocus?: (info: FieldInfo) => void;
  showAllHelpIcons?: boolean;
  searchFilter?: string;
}) {
  const [fieldVisible, setFieldVisible] = useState<boolean>(true);
  const [fieldEnabled, setFieldEnabled] = useState<boolean>(true);
  
  const visCondition = comp.visibility_condition || (comp.condition && comp.enabled_condition ? undefined : comp.condition);
  const enCondition = comp.enabled_condition || (comp.condition && !comp.visibility_condition ? comp.condition : undefined);

  const recomputeVisibility = useCallback(() => {
    const ctx = getConstantValues();
    if (!visCondition) {
      setFieldVisible(true);
    } else {
      setFieldVisible(evaluateIniBoolean(visCondition, ctx));
    }
    if (!enCondition) {
      setFieldEnabled(true);
    } else {
      setFieldEnabled(evaluateIniBoolean(enCondition, ctx));
    }
  }, [visCondition, enCondition]);

  useEffect(() => {
    recomputeVisibility();
    const onConstantsUpdated = () => recomputeVisibility();
    window.addEventListener('constants:updated', onConstantsUpdated);
    return () => window.removeEventListener('constants:updated', onConstantsUpdated);
  }, [recomputeVisibility]);
  
  // Hide field if visibility condition is false
  if (!fieldVisible || !comp.name) return null;

  if (searchFilter?.trim()) {
    const q = searchFilter.toLowerCase();
    const label = (comp.label || comp.name || '').toLowerCase();
    const name = (comp.name || '').toLowerCase();
    if (!label.includes(q) && !name.includes(q)) return null;
  }
  
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
  onOptimisticUpdate,
  onFieldFocus,
  showAllHelpIcons,
  searchFilter,
}: {
  comp: DialogComponent;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
  onOptimisticUpdate?: (name: string, value: number) => void;
  onFieldFocus?: (info: FieldInfo) => void;
  showAllHelpIcons?: boolean;
  searchFilter?: string;
}) {
  const [panelVisible, setPanelVisible] = useState<boolean>(true);

  const normalizedVisibilityExpression = useMemo(() => {
    if (!comp.visibility_condition) return '';
    let expression = comp.visibility_condition;
    if (!expression.includes('{') && !expression.includes('(') && !expression.includes(' ')) {
      expression = `{${expression}}`;
    }
    return expression;
  }, [comp.visibility_condition]);

  const recomputePanelVisibility = useCallback(() => {
    if (!normalizedVisibilityExpression) {
      setPanelVisible(true);
      return;
    }
    setPanelVisible(evaluateIniBoolean(normalizedVisibilityExpression, getConstantValues()));
  }, [normalizedVisibilityExpression]);

  useEffect(() => {
    recomputePanelVisibility();
    const onConstantsUpdated = () => recomputePanelVisibility();
    window.addEventListener('constants:updated', onConstantsUpdated);
    return () => window.removeEventListener('constants:updated', onConstantsUpdated);
  }, [recomputePanelVisibility]);
  
  if (!panelVisible || !comp.name) {
    return null;
  }

  if (searchFilter?.trim()) {
    const q = searchFilter.toLowerCase();
    const label = (comp.label || comp.name || '').toLowerCase();
    if (!label.includes(q) && !comp.name.toLowerCase().includes(q)) {
      return null;
    }
  }
  
  return <RecursivePanel key={`panel-${comp.name}`} name={comp.name} openTable={openTable} context={context} onUpdate={onUpdate} onOptimisticUpdate={onOptimisticUpdate} onFieldFocus={onFieldFocus} showAllHelpIcons={showAllHelpIcons} searchFilter={searchFilter} />;
}

export function DialogComponentRenderer({
  comp,
  openTable,
  context,
  onUpdate,
  onOptimisticUpdate,
  onFieldFocus,
  showAllHelpIcons,
  searchFilter,
}: {
  comp: DialogComponent;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
  onOptimisticUpdate?: (name: string, value: number) => void;
  onFieldFocus?: (info: FieldInfo) => void;
  showAllHelpIcons?: boolean;
  searchFilter?: string;
}) {
  if (comp.type === 'Field' && comp.name) {
    return <DialogFieldWrapper comp={comp} context={context} onUpdate={onUpdate} onOptimisticUpdate={onOptimisticUpdate} onFieldFocus={onFieldFocus} showAllHelpIcons={showAllHelpIcons} searchFilter={searchFilter} />;
  }
  if (comp.type === 'Label' && comp.text) {
    if (searchFilter?.trim() && !comp.text.toLowerCase().includes(searchFilter.toLowerCase())) return null;
    return <div className="dialog-label">{comp.text}</div>;
  }
  if (comp.type === 'Table' && comp.name) {
    return <RecursivePanel name={comp.name} openTable={openTable} context={context} onUpdate={onUpdate} onOptimisticUpdate={onOptimisticUpdate} />;
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
    return <PanelVisibilityWrapper comp={comp} openTable={openTable} context={context} onUpdate={onUpdate} onOptimisticUpdate={onOptimisticUpdate} onFieldFocus={onFieldFocus} showAllHelpIcons={showAllHelpIcons} searchFilter={searchFilter} />;
  }
  if (comp.type === 'Indicator') {
    return <Indicator comp={comp} context={context} />;
  }
  if (comp.type === 'CommandButton' && comp.command) {
    return <CommandButton comp={comp} context={context} />;
  }
  if (comp.type === 'CommandButton') {
    return null;
  }
  const knownTypes = ['Panel', 'Field', 'LiveGraph', 'Table', 'Label', 'Indicator', 'CommandButton'];
  if (!knownTypes.includes(comp.type)) {
    return null;
  }
  return (
    <div className="dialog-label dialog-label--unsupported">
      Unsupported setting control ({String((comp as { type?: string }).type ?? 'unknown')})
    </div>
  );
}


