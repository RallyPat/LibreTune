import { useEffect } from 'react';

interface UseDesignerKeyboardArgs {
  onDelete: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onSave: () => void;
  onDeselect: () => void;
}

/**
 * Window-level keyboard shortcuts for the dashboard designer.
 *  - Delete / Backspace: delete selected
 *  - Ctrl/Cmd+Z: undo, Ctrl+Shift+Z / Ctrl+Y: redo
 *  - Ctrl/Cmd+C / V: copy / paste
 *  - Ctrl/Cmd+S: save
 *  - Esc: clear selection
 */
export function useDesignerKeyboard({
  onDelete,
  onUndo,
  onRedo,
  onCopy,
  onPaste,
  onSave,
  onDeselect,
}: UseDesignerKeyboardArgs): void {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Delete' || e.key === 'Backspace') {
        onDelete();
      } else if (e.ctrlKey || e.metaKey) {
        if (e.key === 'z' && !e.shiftKey) {
          e.preventDefault();
          onUndo();
        } else if ((e.key === 'z' && e.shiftKey) || e.key === 'y') {
          e.preventDefault();
          onRedo();
        } else if (e.key === 'c') {
          e.preventDefault();
          onCopy();
        } else if (e.key === 'v') {
          e.preventDefault();
          onPaste();
        } else if (e.key === 's') {
          e.preventDefault();
          onSave();
        }
      } else if (e.key === 'Escape') {
        onDeselect();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onDelete, onUndo, onRedo, onCopy, onPaste, onSave, onDeselect]);
}
