import { useState, useEffect, useCallback } from 'react';
import { AlertTriangle } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { Dialog, Button } from '../../common';
import { DialogProps, TuneInfo } from './types';
import '../Dialogs.css';

interface SaveDialogProps extends DialogProps {
  onSaved?: (path: string) => void;
  autoBurnOnClose?: boolean;
}

export function SaveDialog({ isOpen, onClose, onSaved, autoBurnOnClose }: SaveDialogProps) {
  const [tuneInfo, setTuneInfo] = useState<TuneInfo | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [showBurnConfirm, setShowBurnConfirm] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      invoke<TuneInfo>('get_tune_info')
        .then(setTuneInfo)
        .catch((e) => setError(String(e)));
    }
  }, [isOpen]);

  const handleSave = useCallback(async () => {
    setIsSaving(true);
    setError(null);
    try {
      const path = await invoke<string>('save_tune', { path: null });
      onSaved?.(path);

      // Auto-burn on close with confirmation
      if (autoBurnOnClose) {
        setShowBurnConfirm(true);
      } else {
        onClose();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setIsSaving(false);
    }
  }, [onClose, onSaved, autoBurnOnClose]);

  // Handle burn after save with confirmation
  const handleBurnConfirm = useCallback(async () => {
    setShowBurnConfirm(false);
    try {
      await invoke('burn_to_ecu');
      onClose();
    } catch (e) {
      setError(String(e));
    }
  }, [onClose]);

  const handleBurnCancel = useCallback(() => {
    setShowBurnConfirm(false);
    onClose();
  }, [onClose]);

  const handleSaveAs = useCallback(async () => {
    setIsSaving(true);
    setError(null);
    try {
      const selected = await save({
        title: 'Save Tune As',
        filters: [
          { name: 'MSQ Tune File', extensions: ['msq'] },
          { name: 'JSON Tune File', extensions: ['json'] },
        ],
        defaultPath: tuneInfo?.path || undefined,
      });
      
      if (selected) {
        const path = await invoke<string>('save_tune_as', { path: selected });
        onSaved?.(path);
        onClose();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setIsSaving(false);
    }
  }, [onClose, onSaved, tuneInfo]);

  if (!isOpen && !showBurnConfirm) return null;

  return (
    <>
      <Dialog
        open={isOpen}
        onClose={onClose}
        title="Save Tune"
        size="md"
        closeOnBackdrop={!isSaving}
      >
        <Dialog.Body>
          {error && <div className="dialog-error">{error}</div>}

          <div className="dialog-info">
            <p><strong>ECU:</strong> {tuneInfo?.signature || 'Unknown'}</p>
            {tuneInfo?.path && (
              <p><strong>Current File:</strong> {tuneInfo.path.split('/').pop()}</p>
            )}
            {tuneInfo?.modified && (
              <p className="dialog-warning" style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
                <AlertTriangle size={14} /> Tune has unsaved changes
              </p>
            )}
          </div>

          <div className="dialog-help">
            <p>Save your tune to a file for backup or transfer.</p>
            <p><strong>MSQ format</strong> is compatible with other ECU tuning software.</p>
          </div>
        </Dialog.Body>

        <Dialog.Footer>
          <Button variant="secondary" onClick={onClose} disabled={isSaving}>Cancel</Button>
          <Button variant="secondary" onClick={handleSaveAs} disabled={isSaving}>Save As...</Button>
          <Button
            variant="primary"
            onClick={handleSave}
            disabled={isSaving || !tuneInfo?.path}
          >
            {isSaving ? 'Saving...' : 'Save'}
          </Button>
        </Dialog.Footer>
      </Dialog>

      {showBurnConfirm && (
        <Dialog
          open
          onClose={handleBurnCancel}
          title="Burn Tune to ECU?"
          size="sm"
        >
          <Dialog.Body>
            <p>Tune saved successfully. Would you like to burn it to the ECU now?</p>
            <p className="dialog-warning" style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
              <AlertTriangle size={14} /> This will write to ECU memory and may take several seconds.
            </p>
          </Dialog.Body>
          <Dialog.Footer>
            <Button variant="secondary" onClick={handleBurnCancel}>Cancel</Button>
            <Button variant="primary" onClick={handleBurnConfirm}>Burn to ECU</Button>
          </Dialog.Footer>
        </Dialog>
      )}
    </>
  );
}
