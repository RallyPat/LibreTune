import React, { useMemo } from 'react';
import type { DialogComponent, FieldInfo } from './types';
import { DialogComponentRenderer } from './PanelComponents';
import {
  applyTableSettingsLayout,
  applyConfigLiveStateLayout,
  isHardwareTestDialog,
  isReferenceGaugePanel,
  organizeComponents,
  partitionHardwareTestComponents,
  rowHasConfigLiveStateSplit,
  rowHasDualSettingsSplit,
  rowHasTableSplitLayout,
} from './dialogLayout';

export interface DialogComponentsLayoutProps {
  dialogName: string;
  components: DialogComponent[];
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
  onOptimisticUpdate?: (name: string, value: number) => void;
  onFieldFocus?: (info: FieldInfo) => void;
  showAllHelpIcons?: boolean;
}

export function DialogComponentsLayout({
  dialogName,
  components,
  openTable,
  context,
  onUpdate,
  onOptimisticUpdate,
  onFieldFocus,
  showAllHelpIcons,
}: DialogComponentsLayoutProps) {
  const { components: tableLaidOut, hasTableSplit: _tableSplit } = useMemo(
    () => applyTableSettingsLayout(dialogName, components),
    [dialogName, components],
  );

  const { components: laidOutComponents, useConfigLiveSplit } = useMemo(
    () => applyConfigLiveStateLayout(dialogName, tableLaidOut),
    [dialogName, tableLaidOut],
  );

  const componentRows = useMemo(
    () => organizeComponents(laidOutComponents),
    [laidOutComponents],
  );

  const renderComponent = (comp: DialogComponent, key: string) => (
    <DialogComponentRenderer
      key={key}
      comp={comp}
      openTable={openTable}
      context={context}
      onUpdate={onUpdate}
      onOptimisticUpdate={onOptimisticUpdate}
      onFieldFocus={onFieldFocus}
      showAllHelpIcons={showAllHelpIcons}
    />
  );

  const hardwareTestLayout = isHardwareTestDialog(dialogName);

  return (
    <>
      {componentRows.map((row, rowIndex) => {
        const hasPositioned = row.west.length > 0 || row.east.length > 0;

        if (!hasPositioned) {
          const items = row.unpositioned.map((comp, i) =>
            renderComponent(comp, `unpositioned-${rowIndex}-${i}`),
          );
          if (hardwareTestLayout && items.length > 0) {
            const { compact, auxiliary } = partitionHardwareTestComponents(row.unpositioned);
            const wrapCell = (comp: DialogComponent, key: string) => {
              const panelName =
                comp.type === 'Panel' && comp.name ? comp.name.toLowerCase() : undefined;
              const refGauge = isReferenceGaugePanel(comp.type === 'Panel' ? comp.name : undefined);
              return (
                <div
                  key={key}
                  className={`hardware-test-cell${refGauge ? ' hardware-test-cell--gauges' : ''}`}
                  data-hw-panel={panelName}
                >
                  {renderComponent(comp, key)}
                </div>
              );
            };

            return (
              <div key={`row-${rowIndex}`} className="hardware-test-layout">
                {compact.map((comp, i) => wrapCell(comp, `hw-compact-${rowIndex}-${i}`))}
                {auxiliary.map((comp, i) => wrapCell(comp, `hw-aux-${rowIndex}-${i}`))}
              </div>
            );
          }
          return <React.Fragment key={`row-${rowIndex}`}>{items}</React.Fragment>;
        }

        const tableSplitLayout = rowHasTableSplitLayout(row);
        const configLiveSplit = useConfigLiveSplit || rowHasConfigLiveStateSplit(row);
        const dualSettingsSplit = rowHasDualSettingsSplit(row);

        return (
          <React.Fragment key={`row-${rowIndex}`}>
            {row.unpositioned.map((comp, i) =>
              renderComponent(comp, `pre-${rowIndex}-${i}`),
            )}
            <div
              className={`dialog-row-container${tableSplitLayout ? ' dialog-row-container--table-settings' : ''}${configLiveSplit ? ' dialog-row-container--config-live' : ''}${dualSettingsSplit ? ' dialog-row-container--settings-split' : ''}`}
            >
              {row.west.length > 0 && (
                <div className="dialog-column">
                  {row.west.map((comp, i) =>
                    renderComponent(comp, `west-${rowIndex}-${i}`),
                  )}
                </div>
              )}
              {row.east.length > 0 && (
                <div className="dialog-column">
                  {row.east.map((comp, i) =>
                    renderComponent(comp, `east-${rowIndex}-${i}`),
                  )}
                </div>
              )}
            </div>
          </React.Fragment>
        );
      })}
    </>
  );
}
