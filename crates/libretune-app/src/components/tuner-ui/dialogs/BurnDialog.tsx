import { useState, useCallback, useEffect } from 'react';
import { AlertTriangle, Check, Flame } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { Dialog, Button } from '../../common';
import { DialogProps } from './types';
import '../Dialogs.css';

interface PinConflict {
  pin_label: string;
  constants: string[];
}

interface PinConflictReport {
  conflicts: PinConflict[];
}

interface BurnDialogProps extends DialogProps {
  connected: boolean;
  onBurned?: () => void;
}

export function BurnDialog({ isOpen, onClose, connected, onBurned }: BurnDialogProps) {
  const [isBurning, setIsBurning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);
  const [conflicts, setConflicts] = useState<PinConflict[]>([]);
  const [forceAck, setForceAck] = useState(false);

  useEffect(() => {
    if (!isOpen) {
      setError(null);
      setSuccess(false);
      setConflicts([]);
      setForceAck(false);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const report = await invoke<PinConflictReport>('check_pin_conflicts');
        if (!cancelled) {
          setConflicts(report.conflicts ?? []);
        }
      } catch {
        if (!cancelled) {
          setConflicts([]);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [isOpen]);

  const hasConflicts = conflicts.length > 0;

  const handleBurn = useCallback(async () => {
    if (hasConflicts && !forceAck) {
      setError('Resolve pin conflicts, or acknowledge them to burn anyway.');
      return;
    }

    setIsBurning(true);
    setError(null);
    setSuccess(false);

    try {
      await invoke('burn_to_ecu', { force: hasConflicts && forceAck ? true : null });
      setSuccess(true);
      onBurned?.();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsBurning(false);
    }
  }, [onBurned, hasConflicts, forceAck]);

  return (
    <Dialog
      open={isOpen}
      onClose={onClose}
      title="Burn to ECU"
      size="md"
      closeOnBackdrop={!isBurning}
    >
      <Dialog.Body>
        {error && <div className="dialog-error" style={{ whiteSpace: 'pre-wrap' }}>{error}</div>}
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

        {hasConflicts && (
          <div className="dialog-warning" style={{ marginTop: 12, whiteSpace: 'pre-wrap' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 8 }}>
              <AlertTriangle size={14} />
              <strong>Pin assignment conflict</strong>
            </div>
            <p style={{ margin: '0 0 8px' }}>
              The same pin is used by multiple outputs. Burning may leave the ECU in a Settings Error state.
            </p>
            <ul style={{ margin: '0 0 8px', paddingLeft: 18 }}>
              {conflicts.map((c) => (
                <li key={c.pin_label}>
                  Pin <strong>{c.pin_label}</strong>: {c.constants.join(', ')}
                </li>
              ))}
            </ul>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer' }}>
              <input
                type="checkbox"
                checked={forceAck}
                onChange={(e) => setForceAck(e.target.checked)}
              />
              I understand — burn anyway
            </label>
          </div>
        )}
      </Dialog.Body>

      <Dialog.Footer>
        <Button variant="secondary" onClick={onClose} disabled={isBurning}>Cancel</Button>
        <Button
          variant="danger"
          onClick={handleBurn}
          disabled={isBurning || !connected || (hasConflicts && !forceAck)}
        >
          {isBurning ? 'Burning...' : <><Flame size={14} /> Burn to ECU</>}
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
}
