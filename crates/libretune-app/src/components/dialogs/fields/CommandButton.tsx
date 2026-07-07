import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertTriangle } from 'lucide-react';
import { useToast } from '../../../contexts/ToastContext';
import type { DialogComponent } from '../types';

// Settings key for command warning preference
const COMMAND_WARNINGS_DISABLED_KEY = 'libretune_command_warnings_disabled';

/// Renders a controller-command button. Sends `comp.command` to the ECU via
/// `execute_controller_command`, optionally gated behind a one-time warning.
/// Output-test commands must not trigger a full tune sync.
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
  const { showToast } = useToast();

  // Load warning preference from localStorage
  useEffect(() => {
    const saved = localStorage.getItem(COMMAND_WARNINGS_DISABLED_KEY);
    if (saved === 'true') {
      setWarningsDisabled(true);
    }
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

      showToast(`${comp.label ?? comp.command} sent to ECU`, 'success');
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
