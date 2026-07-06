import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertTriangle } from 'lucide-react';
import { useToast } from '../../../contexts/ToastContext';
import type { DialogComponent } from '../types';

// Settings key for command warning preference
const COMMAND_WARNINGS_DISABLED_KEY = 'libretune_command_warnings_disabled';

interface SyncResult {
  pages_synced: number;
  pages_failed: number;
  total_pages: number;
  errors: string[];
}

/// Renders a controller-command button. Sends `comp.command` to the ECU via
/// `execute_controller_command`, optionally gated behind a one-time warning,
/// then auto-syncs (and optionally reconnects) afterwards.
export function CommandButton({
  comp,
  context,
}: {
  comp: DialogComponent;
  context: Record<string, number>;
}) {
  const [isEnabled, setIsEnabled] = useState(true);
  const [isExecuting, setIsExecuting] = useState(false);
  const [showWarning, setShowWarning] = useState(false);
  const [warningsDisabled, setWarningsDisabled] = useState(false);
  const [autoReconnectEnabled, setAutoReconnectEnabled] = useState<boolean>(false);
  const { showToast } = useToast();

  // Load warning preference from localStorage
  useEffect(() => {
    const saved = localStorage.getItem(COMMAND_WARNINGS_DISABLED_KEY);
    if (saved === 'true') {
      setWarningsDisabled(true);
    }
  }, []);

  // Load auto-reconnect preference from settings (persisted)
  useEffect(() => {
    invoke<any>('get_settings')
      .then((settings) => {
        if (settings.auto_reconnect_after_controller_command !== undefined) {
          setAutoReconnectEnabled(!!settings.auto_reconnect_after_controller_command);
        }
      })
      .catch(console.error);
  }, []);

  // Evaluate enable condition
  useEffect(() => {
    if (comp.enabled_condition) {
      invoke<boolean>('evaluate_expression', { expression: comp.enabled_condition, context })
        .then(setIsEnabled)
        .catch((err) => {
          console.error('Error evaluating command button condition:', err);
          setIsEnabled(true); // Default to enabled on error
        });
    }
  }, [comp.enabled_condition, context]);

  const executeCommand = async () => {
    if (!comp.command || isExecuting) return;

    setIsExecuting(true);
    try {
      // Add a client-side timeout so UI doesn't stay stuck on "Executing..." forever
      const timeoutMs = 20000; // 20 seconds
      await Promise.race([
        invoke('execute_controller_command', { commandName: comp.command }),
        new Promise((_, reject) => setTimeout(() => reject(new Error('Command timed out')), timeoutMs)),
      ]);

      // On success, trigger a sync so ECU-applied presets (base maps) are read back into app
      try {
        showToast('Controller command executed — syncing ECU...', 'info');
        const syncResult = await invoke<SyncResult>('sync_ecu_data');
        if (syncResult && syncResult.pages_synced > 0) {
          showToast(`Sync complete: ${syncResult.pages_synced} pages`, 'success');
        } else {
          showToast('Sync completed — no pages changed', 'info');
        }

        // If auto-reconnect is enabled, request reconnect (App will handle it)
        let shouldReconnect = autoReconnectEnabled;
        if (!shouldReconnect) {
          // If the local state isn't set yet (race in tests), fetch current setting directly
          try {
            const settings = await invoke<any>('get_settings');
            shouldReconnect = !!settings.auto_reconnect_after_controller_command;
          } catch (e) {
            console.error('Failed to read settings for reconnect:', e);
          }
        }

        if (shouldReconnect) {
          try {
            window.dispatchEvent(new CustomEvent('reconnect:request', { detail: { source: 'controller-command' } }));

            // Dev-only debug & telemetry hook: log reconnect requests and optionally forward to a telemetry sink
            // This is intentionally guarded by NODE_ENV so it doesn't run in production builds.
            try {
              if (typeof import.meta !== 'undefined' && (import.meta as any).env && (import.meta as any).env.MODE !== 'production') {
                console.debug('reconnect:request dispatched', { source: 'controller-command', timestamp: Date.now() });
                // Optional telemetry sink if provided by embedding environment (safe no-op otherwise)
                try { (window as any).__libretuneTelemetry?.trackEvent?.('reconnect_request', { source: 'controller-command' }); } catch (_e) { /* ignore errors */ }
              }
            } catch (dbgErr) {
              console.error('Failed to log reconnect telemetry:', dbgErr);
            }
          } catch (evtErr) {
            console.error('Failed to dispatch reconnect request:', evtErr);
          }
        }
      } catch (syncErr) {
        console.error('Sync after command failed:', syncErr);
        showToast(`Sync failed: ${syncErr}`, 'error');
      }
    } catch (err) {
      console.error('Command execution failed:', err);
      alert(`Command failed: ${err}`);
    } finally {
      setIsExecuting(false);
    }
  };

  const handleClick = () => {
    if (!isEnabled || isExecuting) return;

    // Show warning on first use if not disabled
    if (!warningsDisabled) {
      setShowWarning(true);
    } else {
      executeCommand();
    }
  };

  const handleWarningConfirm = (disableWarnings: boolean) => {
    setShowWarning(false);
    if (disableWarnings) {
      setWarningsDisabled(true);
      localStorage.setItem(COMMAND_WARNINGS_DISABLED_KEY, 'true');
    }
    executeCommand();
  };

  const isAbortAction =
    comp.label?.toLowerCase().includes('abort') ||
    comp.command?.toLowerCase().includes('cancel');

  return (
    <>
      <div
        className={`command-button-field${isAbortAction ? ' command-button-field--full-row' : ''}`}
      >
        <button
          className={`command-button ${isExecuting ? 'executing' : ''}`}
          onClick={handleClick}
          disabled={!isEnabled || isExecuting}
        >
          {isExecuting ? 'Executing...' : comp.label}
        </button>
      </div>

      {showWarning && (
        <div className="command-warning-overlay" onClick={() => setShowWarning(false)}>
          <div className="command-warning-dialog" onClick={(e) => e.stopPropagation()}>
            <h3 style={{ display: 'inline-flex', alignItems: 'center', gap: 8 }}>
              <AlertTriangle size={20} aria-hidden /> Controller Command Warning
            </h3>
            <p>
              This button sends raw commands directly to the ECU.
              These commands bypass normal memory synchronization and may:
            </p>
            <ul>
              <li>Cause the ECU tune to become out of sync</li>
              <li>Activate outputs (injectors, coils, etc.)</li>
              <li>Alter ECU behavior unexpectedly</li>
            </ul>
            <p>Only proceed if you understand what this command does.</p>
            <div style={{ marginTop: 8 }}>
              <label style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <input
                  type="checkbox"
                  checked={autoReconnectEnabled}
                  onChange={(e) => {
                    const val = e.target.checked;
                    setAutoReconnectEnabled(val);
                    // Persist user preference
                    invoke('update_setting', { key: 'auto_reconnect_after_controller_command', value: val }).catch(console.error);
                  }}
                />
                <span style={{ fontSize: '0.9em' }}>
                  Auto-sync and reconnect after executing (may reconnect the ECU)
                </span>
              </label>
            </div>

            <div className="command-warning-buttons">
              <button onClick={() => setShowWarning(false)}>Cancel</button>
              <button onClick={() => handleWarningConfirm(false)}>Execute Once</button>
              <button onClick={() => handleWarningConfirm(true)} className="danger">
                Always Allow
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
