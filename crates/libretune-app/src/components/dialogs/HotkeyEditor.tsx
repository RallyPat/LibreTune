import { useState, useCallback, useMemo, useEffect } from 'react';
import './HotkeyEditor.css';

export interface HotkeyEntry {
  id: string;
  action: string;
  currentBinding: string;
  defaultBinding: string;
  category: 'table' | 'dialog' | 'navigation' | 'view' | 'custom';
  description: string;
}

interface HotkeyEditorProps {
  onClose?: () => void;
  onSave?: (hotkeys: Record<string, string>) => void;
  bindings?: Record<string, string>;
  onChange?: (bindings: Record<string, string>) => void;
}

/**
 * Hotkey Editor Component
 * 
 * Allows users to:
 * - View all available hotkeys
 * - Customize keyboard bindings
 * - Detect and warn about conflicts
 * - Reset to defaults
 * - Import/export keybinding schemes
 */
export default function HotkeyEditor({ onClose, onSave, bindings: initialBindings, onChange }: HotkeyEditorProps) {
  const [hotkeys, setHotkeys] = useState<Record<string, HotkeyEntry>>({});
  const [editingId, setEditingId] = useState<string | null>(null);
  const [conflictWarning, setConflictWarning] = useState<string | null>(null);
  const [filterCategory, setFilterCategory] = useState<'all' | HotkeyEntry['category']>('all');

  // Initialize default hotkeys (these match HotkeyManager.ts)
  useMemo(() => {
    const defaults: Record<string, HotkeyEntry> = {
      'table.navigateUp': {
        id: 'table.navigateUp',
        action: 'Navigate cells up in table',
        currentBinding: 'ArrowUp',
        defaultBinding: 'ArrowUp',
        category: 'table',
        description: 'Move focus up one cell',
      },
      'table.navigateDown': {
        id: 'table.navigateDown',
        action: 'Navigate cells down in table',
        currentBinding: 'ArrowDown',
        defaultBinding: 'ArrowDown',
        category: 'table',
        description: 'Move focus down one cell',
      },
      'table.navigateLeft': {
        id: 'table.navigateLeft',
        action: 'Navigate cells left in table',
        currentBinding: 'ArrowLeft',
        defaultBinding: 'ArrowLeft',
        category: 'table',
        description: 'Move focus left one cell',
      },
      'table.navigateRight': {
        id: 'table.navigateRight',
        action: 'Navigate cells right in table',
        currentBinding: 'ArrowRight',
        defaultBinding: 'ArrowRight',
        category: 'table',
        description: 'Move focus right one cell',
      },
      'table.setEqual': {
        id: 'table.setEqual',
        action: 'Set selected cells to value',
        currentBinding: '=',
        defaultBinding: '=',
        category: 'table',
        description: 'Set all selected cells to their average value',
      },
      'table.increase': {
        id: 'table.increase',
        action: 'Increase selected cells',
        currentBinding: '>',
        defaultBinding: '>',
        category: 'table',
        description: 'Increase by increment (>, +, q)',
      },
      'table.decrease': {
        id: 'table.decrease',
        action: 'Decrease selected cells',
        currentBinding: '<',
        defaultBinding: '<',
        category: 'table',
        description: 'Decrease by increment (<, -, _)',
      },
      'table.scale': {
        id: 'table.scale',
        action: 'Scale selected cells',
        currentBinding: '*',
        defaultBinding: '*',
        category: 'table',
        description: 'Multiply selected cells by factor',
      },
      'table.interpolate': {
        id: 'table.interpolate',
        action: 'Interpolate cells',
        currentBinding: '/',
        defaultBinding: '/',
        category: 'table',
        description: 'Interpolate between corner cells',
      },
      'table.smooth': {
        id: 'table.smooth',
        action: 'Smooth cells',
        currentBinding: 's',
        defaultBinding: 's',
        category: 'table',
        description: 'Apply Gaussian blur to selected cells',
      },
      'table.toggleFollow': {
        id: 'table.toggleFollow',
        action: 'Toggle Follow Mode',
        currentBinding: 'f',
        defaultBinding: 'f',
        category: 'table',
        description: 'Enable/disable real-time tracking',
      },
      'table.copy': {
        id: 'table.copy',
        action: 'Copy cells',
        currentBinding: 'Ctrl+C',
        defaultBinding: 'Ctrl+C',
        category: 'table',
        description: 'Copy selected cells to clipboard',
      },
      'table.paste': {
        id: 'table.paste',
        action: 'Paste cells',
        currentBinding: 'Ctrl+V',
        defaultBinding: 'Ctrl+V',
        category: 'table',
        description: 'Paste cells from clipboard',
      },
      'dialog.save': {
        id: 'dialog.save',
        action: 'Save dialog',
        currentBinding: 'Ctrl+S',
        defaultBinding: 'Ctrl+S',
        category: 'dialog',
        description: 'Save current dialog',
      },
      'dialog.undo': {
        id: 'dialog.undo',
        action: 'Undo',
        currentBinding: 'Ctrl+Z',
        defaultBinding: 'Ctrl+Z',
        category: 'dialog',
        description: 'Undo last operation',
      },
      'dialog.redo': {
        id: 'dialog.redo',
        action: 'Redo',
        currentBinding: 'Ctrl+Y',
        defaultBinding: 'Ctrl+Y',
        category: 'dialog',
        description: 'Redo last operation',
      },
      'dialog.cancel': {
        id: 'dialog.cancel',
        action: 'Cancel/Close',
        currentBinding: 'Escape',
        defaultBinding: 'Escape',
        category: 'dialog',
        description: 'Close current dialog',
      },
      'nav.nextTab': {
        id: 'nav.nextTab',
        action: 'Next tab',
        currentBinding: 'Ctrl+Tab',
        defaultBinding: 'Ctrl+Tab',
        category: 'navigation',
        description: 'Switch to next tab',
      },
      'nav.prevTab': {
        id: 'nav.prevTab',
        action: 'Previous tab',
        currentBinding: 'Ctrl+Shift+Tab',
        defaultBinding: 'Ctrl+Shift+Tab',
        category: 'navigation',
        description: 'Switch to previous tab',
      },
      'nav.jumpToActive': {
        id: 'nav.jumpToActive',
        action: 'Jump to active position',
        currentBinding: 'g',
        defaultBinding: 'g',
        category: 'navigation',
        description: 'Jump to current RPM/MAP position',
      },
    };

    setHotkeys(defaults);
  }, []);

  // Apply initial bindings when component mounts or bindings prop changes
  useEffect(() => {
    if (initialBindings && Object.keys(initialBindings).length > 0) {
      setHotkeys((prev) => {
        const updated = { ...prev };
        Object.entries(initialBindings).forEach(([action, binding]) => {
          if (updated[action]) {
            updated[action].currentBinding = binding;
          }
        });
        return updated;
      });
    }
  }, [initialBindings]);

  // Notify parent of changes
  useEffect(() => {
    if (onChange && Object.keys(hotkeys).length > 0) {
      const bindings: Record<string, string> = Object.fromEntries(
        Object.entries(hotkeys).map(([id, entry]) => [id, entry.currentBinding])
      );
      onChange(bindings);
    }
  }, [hotkeys, onChange]);

  // Detect keybinding conflicts
  const detectConflicts = useCallback((updatedHotkeys: Record<string, HotkeyEntry>) => {
    const bindingMap = new Map<string, string[]>();

    Object.values(updatedHotkeys).forEach((entry) => {
      const key = entry.currentBinding.toLowerCase();
      if (!bindingMap.has(key)) {
        bindingMap.set(key, []);
      }
      bindingMap.get(key)!.push(entry.action);
    });

    // Find conflicts
    const conflicts: string[] = [];
    bindingMap.forEach((actions, key) => {
      if (actions.length > 1) {
        conflicts.push(`"${key}" is assigned to: ${actions.join(', ')}`);
      }
    });

    if (conflicts.length > 0) {
      setConflictWarning(`⚠ Key binding conflicts detected:\n${conflicts.join('\n')}`);
    } else {
      setConflictWarning(null);
    }

    return conflicts.length === 0;
  }, []);

  // Handle binding change
  const handleBindingChange = useCallback(
    (id: string, newBinding: string) => {
      const updated = {
        ...hotkeys,
        [id]: { ...hotkeys[id], currentBinding: newBinding },
      };
      setHotkeys(updated);
      detectConflicts(updated);
    },
    [hotkeys, detectConflicts]
  );

  // Reset to defaults
  const handleResetDefaults = useCallback(() => {
    const reset = Object.fromEntries(
      Object.entries(hotkeys).map(([id, entry]) => [
        id,
        { ...entry, currentBinding: entry.defaultBinding },
      ])
    );
    setHotkeys(reset);
    detectConflicts(reset);
    setConflictWarning(null);
  }, [hotkeys, detectConflicts]);

  // Export bindings
  const handleExport = useCallback(async () => {
    const bindings: Record<string, string> = {};
    Object.entries(hotkeys).forEach(([id, entry]) => {
      if (entry.currentBinding !== entry.defaultBinding) {
        bindings[id] = entry.currentBinding;
      }
    });

    const json = JSON.stringify(bindings, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `libretune-hotkeys-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }, [hotkeys]);

  // Filter hotkeys by category
  const filteredHotkeys = useMemo(() => {
    return Object.values(hotkeys).filter((h) =>
      filterCategory === 'all' ? true : h.category === filterCategory
    );
  }, [hotkeys, filterCategory]);

  return (
    <div className="hotkey-editor">
      <div className="hotkey-editor-header">
        <h3>Customize Keyboard Shortcuts</h3>
        <p>Click a binding to edit it. Red indicates conflicts.</p>
      </div>

      {conflictWarning && (
        <div className="hotkey-conflict-warning">
          {conflictWarning}
        </div>
      )}

      <div className="hotkey-editor-toolbar">
        <div className="hotkey-filter">
          <label>Filter by:</label>
          <select
            value={filterCategory}
            onChange={(e) => setFilterCategory(e.target.value as any)}
          >
            <option value="all">All Categories</option>
            <option value="table">Table Editing</option>
            <option value="dialog">Dialogs</option>
            <option value="navigation">Navigation</option>
            <option value="custom">Custom</option>
          </select>
        </div>
        <button onClick={handleResetDefaults} className="hotkey-reset-btn">
          ↻ Reset to Defaults
        </button>
        <button onClick={handleExport} className="hotkey-export-btn">
          ⬇ Export Scheme
        </button>
      </div>

      <div className="hotkey-list">
        {filteredHotkeys.map((entry) => (
          <div key={entry.id} className="hotkey-row">
            <div className="hotkey-info">
              <div className="hotkey-action">{entry.action}</div>
              <div className="hotkey-description">{entry.description}</div>
            </div>
            <div className="hotkey-binding-display">
              {editingId === entry.id ? (
                <input
                  type="text"
                  value={entry.currentBinding}
                  onChange={(e) => handleBindingChange(entry.id, e.target.value)}
                  onBlur={() => setEditingId(null)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      setEditingId(null);
                    } else if (e.key === 'Escape') {
                      setEditingId(null);
                    }
                  }}
                  className="hotkey-input"
                  placeholder="Press keys or type binding"
                  autoFocus
                />
              ) : (
                <>
                  <code
                    className={`hotkey-code ${
                      entry.currentBinding !== entry.defaultBinding ? 'modified' : ''
                    }`}
                    onClick={() => setEditingId(entry.id)}
                  >
                    {entry.currentBinding}
                  </code>
                  {entry.currentBinding !== entry.defaultBinding && (
                    <button
                      className="hotkey-reset-single"
                      onClick={() => handleBindingChange(entry.id, entry.defaultBinding)}
                      title="Reset to default"
                    >
                      ↻
                    </button>
                  )}
                </>
              )}
            </div>
          </div>
        ))}
      </div>

      <div className="hotkey-editor-footer">
        <p className="hotkey-note">
          💡 Tip: You can also use key combinations like Ctrl+Shift+Z or Alt+A
        </p>
        <div className="hotkey-buttons">
          <button onClick={onClose} className="hotkey-cancel-btn">
            Cancel
          </button>
          <button
            onClick={() => {
              const bindings: Record<string, string> = {};
              Object.entries(hotkeys).forEach(([id, entry]) => {
                bindings[id] = entry.currentBinding;
              });
              onSave?.(bindings);
              onClose?.();
            }}
            className="hotkey-save-btn"
            disabled={!!conflictWarning}
          >
            Save Changes
          </button>
        </div>
      </div>
    </div>
  );
}
