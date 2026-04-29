import { useState, useCallback } from 'react';
import { AlertTriangle, Check, Flame } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { Dialog, Button } from '../../common';
import { DialogProps } from './types';
import '../Dialogs.css';

interface BurnDialogProps extends DialogProps {
  connected: boolean;
  onBurned?: () => void;
}

export function BurnDialog({ isOpen, onClose, connected, onBurned }: BurnDialogProps) {
  const [isBurning, setIsBurning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const handleBurn = useCallback(async () => {
    setIsBurning(true);
    setError(null);
    setSuccess(false);
    
    try {
      await invoke('burn_to_ecu');
      setSuccess(true);
      onBurned?.();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsBurning(false);
    }
  }, [onClose, onBurned]);

  return (
    <Dialog
      open={isOpen}
      onClose={onClose}
      title="Burn to ECU"
      size="md"
      closeOnBackdrop={!isBurning}
    >
      <Dialog.Body>
        {error && <div className="dialog-error">{error}</div>}
        {success && <div className="dialog-success" style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}><Check size={14} /> Burn completed successfully!</div>}

        {!connected ? (
          <div className="dialog-warning" style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
            <AlertTriangle size={14} /> Not connected to ECU. Please connect first.
          </div>
        ) : (
          <div className="dialog-info">
            <p>This will write all changes from ECU RAM to flash memory.</p>
            <p><strong>Warning:</strong> This operation cannot be undone.</p>
            <p>Make sure your tune is tested before burning.</p>
          </div>
        )}
      </Dialog.Body>

      <Dialog.Footer>
        <Button variant="secondary" onClick={onClose} disabled={isBurning}>Cancel</Button>
        <Button
          variant="danger"
          onClick={handleBurn}
          disabled={isBurning || !connected}
        >
          {isBurning ? 'Burning...' : <><Flame size={14} /> Burn to ECU</>}
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
}
