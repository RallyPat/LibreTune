import { useState, useCallback, useMemo, useEffect } from 'react';
import { RotateCcw, Download, Lightbulb, AlertTriangle } from 'lucide-react';
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
      'table.interpolateHorizontal': {
        id: 'table.interpolateHorizontal',
        action: 'Interpolate horizontally',
        currentBinding: 'h',
        defaultBinding: 'h',
        category: 'table',
        description: 'Interpolate each selected row between its end cells',
      },
      'table.interpolateVertical': {
        id: 'table.interpolateVertical',
        action: 'Interpolate vertically',
        currentBinding: 'v',
        defaultBinding: 'v',
        category: 'table',
        description: 'Interpolate each selected column between its end cells',
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
        let changed = false;
        const updated: Record<string, HotkeyEntry> = { ...prev };
        Object.entries(initialBindings).forEach(([action, binding]) => {
          const entry = updated[action];
          if (entry && entry.currentBinding !== binding) {
            updated[action] = { ...entry, currentBinding: binding };
            changed = true;
          }
        });
        // Return the SAME reference when nothing changed so React bails out
        // instead of re-rendering. Returning a fresh-but-equal object here
        // fed the onChange <-> bindings-prop feedback loop below and produced
        // "Maximum update depth exceeded".
        return changed ? updated : prev;
      });
    }
  }, [initialBindings]);

  // Report the given entries to the parent as a flat bindings map.
  // Called ONLY from user-action handlers (edit / reset / import) — never
  // from an effect. An effect watching `hotkeys` used to call onChange on
  // every change, including the ones caused by applying the `bindings` prop,
  // which fed the parent <-> child setState loop ("Maximum update depth
  // exceeded") whenever saved bindings differed from the defaults.
  const notifyParent = useCallback(
    (entries: Record<string, HotkeyEntry>) => {
      if (!onChange) return;
      onChange(
        Object.fromEntries(
          Object.entries(entries).map(([id, entry]) => [id, entry.currentBinding])
        )
      );
    },
    [onChange]
  );

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
      setConflictWarning(`Key binding conflicts detected:\n${conflicts.join('\n')}`);
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
      notifyParent(updated);
    },
    [hotkeys, detectConflicts, notifyParent]
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
    notifyParent(reset);
  }, [hotkeys, detectConflicts, notifyParent]);

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
        <div className="hotkey-conflict-warning" style={{ display: 'flex', alignItems: 'flex-start', gap: 6 }}>
          <AlertTriangle size={16} aria-hidden style={{ flexShrink: 0, marginTop: 2 }} />
          <span style={{ whiteSpace: 'pre-wrap' }}>{conflictWarning}</span>
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
          <RotateCcw size={14} /> Reset to Defaults
        </button>
        <button onClick={handleExport} className="hotkey-export-btn">
          <Download size={14} /> Export Scheme
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
                      aria-label="Reset to default"
                    >
                      <RotateCcw size={12} />
                    </button>
                  )}
                </>
              )}
            </div>
          </div>
        ))}
      </div>

      <div className="hotkey-editor-footer">
        <p className="hotkey-note" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <Lightbulb size={14} aria-hidden /> Tip: You can also use key combinations like Ctrl+Shift+Z or Alt+A
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
