import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Dialog, Button } from '../../common';
import { DialogProps } from './types';
import '../Dialogs.css';

interface NewTuneDialogProps extends DialogProps {
  onCreated?: () => void;
}

export function NewTuneDialog({ isOpen, onClose, onCreated }: NewTuneDialogProps) {
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleCreate = useCallback(async () => {
    setIsCreating(true);
    setError(null);
    try {
      await invoke('new_tune');
      onCreated?.();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsCreating(false);
    }
  }, [onClose, onCreated]);

  return (
    <Dialog
      open={isOpen}
      onClose={onClose}
      title="New Tune"
      size="sm"
      closeOnBackdrop={!isCreating}
    >
      <Dialog.Body>
        {error && <div className="dialog-error">{error}</div>}

        <div className="dialog-info">
          <p>Create a new tune file for the currently loaded ECU definition.</p>
          <p>Any unsaved changes to the current tune will be lost.</p>
        </div>
      </Dialog.Body>

      <Dialog.Footer>
        <Button variant="secondary" onClick={onClose} disabled={isCreating}>Cancel</Button>
        <Button variant="primary" onClick={handleCreate} disabled={isCreating}>
          {isCreating ? 'Creating...' : 'Create New Tune'}
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
}
