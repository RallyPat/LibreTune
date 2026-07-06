import React, { useMemo } from 'react';
import type { DialogComponent, FieldInfo } from './types';
import { DialogComponentRenderer } from './PanelComponents';
import {
  applyTableSettingsLayout,
  isHardwareTestDialog,
  organizeComponents,
  partitionHardwareTestComponents,
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
  const componentRows = useMemo(() => {
    const { components: laidOut } = applyTableSettingsLayout(dialogName, components);
    return organizeComponents(laidOut);
  }, [dialogName, components]);

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
              return (
                <div
                  key={key}
                  className="hardware-test-cell"
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

        return (
          <React.Fragment key={`row-${rowIndex}`}>
            {row.unpositioned.map((comp, i) =>
              renderComponent(comp, `pre-${rowIndex}-${i}`),
            )}
            <div
              className={`dialog-row-container${tableSplitLayout ? ' dialog-row-container--table-settings' : ''}`}
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
