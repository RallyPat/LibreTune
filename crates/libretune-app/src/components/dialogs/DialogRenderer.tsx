import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  ConfiguratorShell,
  ConfiguratorHeader,
  ConfiguratorSearch,
  ConfiguratorBody,
  ConfiguratorGroups,
  ConfiguratorGroup,
  ConfiguratorFooter,
} from '../common/ConfiguratorLayout';
import { SidebarNodeIcon } from '../tuner-ui/SidebarNodeIcon';
import { resolveSidebarIcon } from '../../utils/sidebarIcons';
import '../common/ConfiguratorLayout.css';
import './DialogRenderer.css';
import {
  type DialogComponent,
  type DialogDefinition,
  type FieldInfo,
} from './types';
import { DialogComponentRenderer, groupCommandButtons } from './PanelComponents';
import { CommandButton } from './fields/CommandButton';

export interface DialogRendererProps {
  definition: DialogDefinition;
  onBack: () => void;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
  onOptimisticUpdate?: (name: string, value: number) => void;
  displayTitle?: string;
  highlightTerm?: string;
}

function countFieldLike(components: DialogComponent[]): number {
  return components.filter((c) => c.type === 'Field' || c.type === 'Indicator').length;
}

function formatPanelTitle(name: string, label?: string): string {
  if (label?.trim()) return label.trim();
  return name.replace(/([A-Z])/g, ' $1').replace(/_/g, ' ').trim();
}

export default function DialogRenderer({
  definition,
  onBack,
  openTable,
  context,
  onUpdate,
  onOptimisticUpdate,
  displayTitle,
  highlightTerm,
}: DialogRendererProps) {
  const [selectedField, setSelectedField] = useState<FieldInfo | null>(null);
  const [showAllHelpIcons, setShowAllHelpIcons] = useState(true);
  const [searchFilter, setSearchFilter] = useState(highlightTerm || '');
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invoke<{ show_all_help_icons?: boolean }>('get_settings')
      .then((settings) => {
        if (settings.show_all_help_icons !== undefined) {
          setShowAllHelpIcons(settings.show_all_help_icons);
        }
      })
      .catch(console.error);
  }, []);

  useEffect(() => {
    if (highlightTerm) setSearchFilter(highlightTerm);
  }, [highlightTerm]);

  useEffect(() => {
    if (!highlightTerm || !containerRef.current || !definition?.name) return;

    const timer = setTimeout(() => {
      const container = containerRef.current;
      if (!container) return;

      const lowerTerm = highlightTerm.toLowerCase();
      const labels = container.querySelectorAll('.settings-field label, .dialog-field-label');

      for (const label of labels) {
        if (label.textContent?.toLowerCase().includes(lowerTerm)) {
          const fieldRow = label.closest('.settings-field') || label.closest('.dialog-row');
          if (fieldRow) {
            fieldRow.scrollIntoView({ behavior: 'smooth', block: 'center' });
            fieldRow.classList.add('search-highlight-flash');
            setTimeout(() => fieldRow.classList.remove('search-highlight-flash'), 2000);
            break;
          }
        }
      }
    }, 100);

    return () => clearTimeout(timer);
  }, [highlightTerm, definition?.name]);

  const title = displayTitle || definition?.title || definition?.name || 'Settings';
  const headerIconKey = resolveSidebarIcon({
    id: definition?.name ?? 'unknown',
    label: title,
    type: 'dialog',
  });

  if (!definition?.name) {
    return (
      <div className="dialog-view panel-load-error">
        <span>This settings page could not be loaded (missing definition).</span>
      </div>
    );
  }

  const handleFieldFocus = (info: FieldInfo) => {
    setSelectedField(info);
  };

  const renderComponent = (comp: DialogComponent, key: string) => (
    <DialogComponentRenderer
      key={key}
      comp={comp}
      openTable={openTable}
      context={context}
      onUpdate={onUpdate}
      onOptimisticUpdate={onOptimisticUpdate}
      onFieldFocus={handleFieldFocus}
      showAllHelpIcons={showAllHelpIcons}
      searchFilter={searchFilter}
    />
  );

  const components = definition.components ?? [];

  const groupedContent = (() => {
    const panels = components.filter((c) => c.type === 'Panel');
    const others = components.filter((c) => c.type !== 'Panel');
    const generalCount = countFieldLike(others);

    const blocks: React.ReactNode[] = [];

    if (others.length > 0) {
      blocks.push(
        <ConfiguratorGroup
          key="general"
          title={panels.length > 0 ? 'General' : title}
          icon={<SidebarNodeIcon icon={headerIconKey} />}
          count={generalCount || others.length}
        >
          {groupCommandButtons(others).map((item, i) =>
            '__cmdGroup' in item ? (
              <div key={`cmd-group-${i}`} className="command-button-group">
                {item.items.map((comp, j) => (
                  <CommandButton key={j} comp={comp} context={context ?? {}} />
                ))}
              </div>
            ) : (
              renderComponent(item, `general-${i}`)
            )
          )}
        </ConfiguratorGroup>,
      );
    }

    panels.forEach((comp, i) => {
      const panelTitle = formatPanelTitle(comp.name || '', comp.label);
      const panelIcon = resolveSidebarIcon({
        id: comp.name || '',
        label: panelTitle,
        type: 'folder',
      });
      blocks.push(
        <ConfiguratorGroup
          key={`panel-${comp.name}-${i}`}
          title={panelTitle}
          icon={<SidebarNodeIcon icon={panelIcon} />}
          defaultExpanded={i === 0}
        >
          {renderComponent(comp, `panel-${i}`)}
        </ConfiguratorGroup>,
      );
    });

    return blocks;
  })();

  return (
    <ConfiguratorShell className="dialog-view view-transition">
      <ConfiguratorHeader
        icon={<SidebarNodeIcon icon={headerIconKey} size={22} />}
        title={title}
        subtitle={definition.name}
        onBack={onBack}
      />

      <ConfiguratorSearch
        value={searchFilter}
        onChange={setSearchFilter}
        placeholder="Search settings…"
      />

      <ConfiguratorBody>
        <div ref={containerRef}>
          <ConfiguratorGroups>{groupedContent}</ConfiguratorGroups>
        </div>
      </ConfiguratorBody>

      <ConfiguratorFooter>
        {selectedField ? (
          <>
            <strong>{selectedField.label}</strong>
            <p>{selectedField.help || 'No description available for this setting.'}</p>
          </>
        ) : (
          <p className="description-placeholder">
            Click the ? icon next to any setting to see its description
          </p>
        )}
      </ConfiguratorFooter>
    </ConfiguratorShell>
  );
}

export type { DialogDefinition, DialogComponent };
