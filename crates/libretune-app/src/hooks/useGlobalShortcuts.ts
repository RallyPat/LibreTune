import { useEffect } from "react";

export interface UseGlobalShortcutsDeps {
  isConnected: boolean;
  tuneModified: boolean;
  setNewProjectDialogOpen: (open: boolean) => void;
  setLoadDialogOpen: (open: boolean) => void;
  setSaveDialogOpen: (open: boolean) => void;
  setBurnDialogOpen: (open: boolean) => void;
}

/**
 * Global keyboard shortcuts:
 *   Ctrl/Cmd + N → New Project
 *   Ctrl/Cmd + O → Load Tune
 *   Ctrl/Cmd + S → Save Tune
 *   Ctrl/Cmd + B → Burn Tune (requires connection)
 */
export function useGlobalShortcuts(deps: UseGlobalShortcutsDeps) {
  const {
    isConnected,
    tuneModified,
    setNewProjectDialogOpen,
    setLoadDialogOpen,
    setSaveDialogOpen,
    setBurnDialogOpen,
  } = deps;

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const isCtrl = e.ctrlKey || e.metaKey;
      if (isCtrl && !e.shiftKey) {
        switch (e.key.toLowerCase()) {
          case 'n':
            e.preventDefault();
            setNewProjectDialogOpen(true);
            break;
          case 'o':
            e.preventDefault();
            setLoadDialogOpen(true);
            break;
          case 's':
            e.preventDefault();
            setSaveDialogOpen(true);
            break;
          case 'b':
            e.preventDefault();
            if (isConnected && tuneModified) {
              setBurnDialogOpen(true);
            }
            break;
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isConnected, tuneModified, setNewProjectDialogOpen, setLoadDialogOpen, setSaveDialogOpen, setBurnDialogOpen]);
}
