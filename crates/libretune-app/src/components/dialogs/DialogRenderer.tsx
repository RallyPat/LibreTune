import React, { useState, useEffect, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ArrowLeft } from 'lucide-react';
import './DialogRenderer.css';
import {
  type DialogComponent,
  type DialogDefinition,
  type FieldInfo,
} from './types';
import { DialogComponentRenderer } from './PanelComponents';
import { applyTableSettingsLayout, rowHasTableSplitLayout } from './dialogLayout';

export interface DialogRendererProps {
  definition: DialogDefinition;
  onBack: () => void;
  openTable: (name: string) => void;
  context: Record<string, number>;
  onUpdate?: () => void;
  onOptimisticUpdate?: (name: string, value: number) => void;
  /** Override title for display (formatted as "Menu Label (ini_name)") */
  displayTitle?: string;
  /** Search term to highlight matching fields (scroll into view and flash animation) */
  highlightTerm?: string;
}

export default function DialogRenderer({ definition, onBack, openTable, context, onUpdate, onOptimisticUpdate, displayTitle, highlightTerm }: DialogRendererProps) {
  // The context is already dynamic - it contains the current values of all constants
  // Conditions like {cylindersCount > 5} will automatically evaluate based on the current cylindersCount value
  // This works for any cylinder count: 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, etc.
  
  // State for showing field description in bottom panel
  const [selectedField, setSelectedField] = useState<FieldInfo | null>(null);
  
  // State for help icon visibility setting (default true = show on all fields)
  const [showAllHelpIcons, setShowAllHelpIcons] = useState(true);
  
  // Ref for scrolling to highlighted field
  const containerRef = useRef<HTMLDivElement>(null);
  
  // Fetch the help icon visibility setting on mount
  useEffect(() => {
    invoke<{ show_all_help_icons?: boolean }>("get_settings")
      .then((settings) => {
        if (settings.show_all_help_icons !== undefined) {
          setShowAllHelpIcons(settings.show_all_help_icons);
        }
      })
      .catch(console.error);
  }, []);

  
  // Scroll to and highlight matching field when highlightTerm is provided
  useEffect(() => {
    if (!highlightTerm || !containerRef.current) return;
    
    // Wait for DOM to render
    const timer = setTimeout(() => {
      const container = containerRef.current;
      if (!container) return;
      
      // Find field labels that match the search term
      const lowerTerm = highlightTerm.toLowerCase();
      const labels = container.querySelectorAll('.dialog-field label, .dialog-field-label');
      
      for (const label of labels) {
        if (label.textContent?.toLowerCase().includes(lowerTerm)) {
          // Found a matching label - scroll to its parent field row
          const fieldRow = label.closest('.dialog-field') || label.closest('.dialog-row');
          if (fieldRow) {
            fieldRow.scrollIntoView({ behavior: 'smooth', block: 'center' });
            // Add flash animation class
            fieldRow.classList.add('search-highlight-flash');
            // Remove class after animation
            setTimeout(() => {
              fieldRow.classList.remove('search-highlight-flash');
            }, 2000);
            break;
          }
        }
      }
    }, 100);
    
    return () => clearTimeout(timer);
  }, [highlightTerm, definition.name]);
  
  const handleFieldFocus = (info: FieldInfo) => {
    setSelectedField(info);
  };
  
  // Log all components for debugging
  useEffect(() => {
    console.log(`[DialogRenderer] Rendering dialog '${definition.name}' with ${definition.components.length} components:`);
    definition.components.forEach((comp, i) => {
      console.log(`  [${i}] type=${comp.type}, name=${comp.name || 'N/A'}, position=${comp.position || 'none'}, visibility=${comp.visibility_condition || 'none'}`);
    });
  }, [definition]);
  
  // Group components by position for multi-column layout
  const organizeComponents = (components: DialogComponent[]) => {
    const rows: { west: DialogComponent[], east: DialogComponent[], unpositioned: DialogComponent[] }[] = [];
    let currentRow: { west: DialogComponent[], east: DialogComponent[], unpositioned: DialogComponent[] } | null = null;
    
    for (const comp of components) {
      const position = comp.position?.toLowerCase();
      
      if (position === 'west' || position === 'east') {
        // New row only when prior row had unpositioned content (labels above a split row)
        if (!currentRow || currentRow.unpositioned.length > 0) {
          currentRow = { west: [], east: [], unpositioned: [] };
          rows.push(currentRow);
        }

        if (position === 'west') {
          currentRow.west.push(comp);
        } else {
          currentRow.east.push(comp);
        }
      } else {
        // Unpositioned components - add to unpositioned array
        if (!currentRow) {
          currentRow = { west: [], east: [], unpositioned: [] };
          rows.push(currentRow);
        }
        currentRow.unpositioned.push(comp);
      }
    }
    
    return rows;
  };
  
  const componentRows = useMemo(() => {
    const { components } = applyTableSettingsLayout(definition.name, definition.components);
    return organizeComponents(components);
  }, [definition.name, definition.components]);
  
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

      <div className="glass-card dialog-container" ref={containerRef}>
        {componentRows.map((row, rowIndex) => {
          const hasPositioned = row.west.length > 0 || row.east.length > 0;
          
          if (!hasPositioned) {
            // No positioned components - render unpositioned components normally
            return (
              <React.Fragment key={`row-${rowIndex}`}>
                {row.unpositioned.map((comp, i) => (
                  <DialogComponentRenderer 
                    key={`unpositioned-${rowIndex}-${i}`} 
                    comp={comp} 
                    openTable={openTable} 
                    context={context} 
                    onUpdate={onUpdate} 
                    onOptimisticUpdate={onOptimisticUpdate} 
                    onFieldFocus={handleFieldFocus} 
                    showAllHelpIcons={showAllHelpIcons} 
                  />
                ))}
              </React.Fragment>
            );
          }
          
          // Has positioned components - use grid layout
          const tableSplitLayout = rowHasTableSplitLayout(row);

          return (
            <React.Fragment key={`row-${rowIndex}`}>
              {row.unpositioned.map((comp, i) => (
                <DialogComponentRenderer 
                  key={`pre-${rowIndex}-${i}`} 
                  comp={comp} 
                  openTable={openTable} 
                  context={context} 
                  onUpdate={onUpdate} 
                  onOptimisticUpdate={onOptimisticUpdate} 
                  onFieldFocus={handleFieldFocus} 
                  showAllHelpIcons={showAllHelpIcons} 
                />
              ))}
              <div className={`dialog-row-container${tableSplitLayout ? ' dialog-row-container--table-settings' : ''}`}>
                {row.west.length > 0 && (
                  <div className="dialog-column">
                    {row.west.map((comp, i) => (
                      <DialogComponentRenderer 
                        key={`west-${rowIndex}-${i}`} 
                        comp={comp} 
                        openTable={openTable} 
                        context={context} 
                        onUpdate={onUpdate} 
                        onOptimisticUpdate={onOptimisticUpdate} 
                        onFieldFocus={handleFieldFocus} 
                        showAllHelpIcons={showAllHelpIcons} 
                      />
                    ))}
                  </div>
                )}
                {row.east.length > 0 && (
                  <div className="dialog-column">
                    {row.east.map((comp, i) => (
                      <DialogComponentRenderer 
                        key={`east-${rowIndex}-${i}`} 
                        comp={comp} 
                        openTable={openTable} 
                        context={context} 
                        onUpdate={onUpdate} 
                        onOptimisticUpdate={onOptimisticUpdate} 
                        onFieldFocus={handleFieldFocus} 
                        showAllHelpIcons={showAllHelpIcons} 
                      />
                    ))}
                  </div>
                )}
              </div>
            </React.Fragment>
          );
        })}
      </div>
      
      <div className="dialog-description-panel">
        {selectedField ? (
          <>
            <strong>{selectedField.label}</strong>
            <p>{selectedField.help || 'No description available for this setting.'}</p>
          </>
        ) : (
          <p className="description-placeholder">Click the ? icon next to any setting to see its description</p>
        )}
      </div>
    </div>
  );
}

// Export types for use in App.tsx
export type { DialogDefinition, DialogComponent };
