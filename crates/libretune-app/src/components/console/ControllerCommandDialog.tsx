import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertTriangle } from 'lucide-react';
import { useToast } from '../../contexts/ToastContext';
import '../dialogs/DialogRenderer.css';

export interface ControllerCommandPromptDetail {
  commandName: string;
  label?: string;
  description?: string;
}

const COMMAND_WARNINGS_DISABLED_KEY = 'libretune_command_warnings_disabled';

/**
 * Global confirmation dialog for raw controller commands (e.g. Enter DFU Mode).
 * Trigger via: window.dispatchEvent(new CustomEvent('controller-command:prompt', { detail: { commandName, label } }))
 */
export function ControllerCommandDialog() {
  const [prompt, setPrompt] = useState<ControllerCommandPromptDetail | null>(null);
  const [isExecuting, setIsExecuting] = useState(false);
  const { showToast } = useToast();

  const executeCommand = useCallback(async (detail: ControllerCommandPromptDetail) => {
    setIsExecuting(true);
    const isDfu = detail.commandName.toLowerCase().includes('dfu');
    try {
      const timeoutMs = 20000;
      await Promise.race([
        invoke('execute_controller_command', { commandName: detail.commandName }),
        new Promise((_, reject) => setTimeout(() => reject(new Error('Command timed out')), timeoutMs)),
      ]);

      showToast(
        isDfu
          ? `${detail.label ?? detail.commandName} sent — ECU should reboot into DFU mode`
          : `${detail.label ?? detail.commandName} sent to ECU`,
        'success',
      );
    } catch (err) {
      console.error('Controller command failed:', err);
      showToast(`Command failed: ${err}`, 'error');
    } finally {
      setIsExecuting(false);
      setPrompt(null);
    }
  }, [showToast]);

  useEffect(() => {
    const handler = (event: Event) => {
      const detail = (event as CustomEvent<ControllerCommandPromptDetail>).detail;
      if (!detail?.commandName) return;

      if (localStorage.getItem(COMMAND_WARNINGS_DISABLED_KEY) === 'true') {
        void executeCommand(detail);
      } else {
        setPrompt(detail);
      }
    };

    window.addEventListener('controller-command:prompt', handler);
    return () => window.removeEventListener('controller-command:prompt', handler);
  }, [executeCommand]);

  const handleConfirm = (disableWarnings: boolean) => {
    if (!prompt) return;
    if (disableWarnings) {
      localStorage.setItem(COMMAND_WARNINGS_DISABLED_KEY, 'true');
    }
    void executeCommand(prompt);
  };

  if (!prompt) return null;

  const title = prompt.label ?? prompt.commandName;
  const isDfu = prompt.commandName.toLowerCase().includes('dfu');

  return (
    <div className="command-warning-overlay" onClick={() => !isExecuting && setPrompt(null)}>
      <div className="command-warning-dialog" onClick={(e) => e.stopPropagation()}>
        <h3 style={{ display: 'inline-flex', alignItems: 'center', gap: 8 }}>
          <AlertTriangle size={20} aria-hidden /> {title}
        </h3>
        <p>
          {prompt.description ?? (
            isDfu
              ? 'This will reset the ECU into DFU (firmware update) mode. The serial connection will drop and you will need external tools (e.g. STM32CubeProgrammer, dfu-util) to flash new firmware.'
              : 'This sends a raw command directly to the ECU, bypassing normal memory synchronization.'
          )}
        </p>
        {!isDfu && <p>Only proceed if you understand what this command does.</p>}
        <div className="command-warning-buttons">
          <button onClick={() => setPrompt(null)} disabled={isExecuting}>Cancel</button>
          <button onClick={() => handleConfirm(false)} disabled={isExecuting}>
            {isExecuting ? 'Sending…' : 'Execute Once'}
          </button>
          {!isDfu && (
            <button onClick={() => handleConfirm(true)} className="danger" disabled={isExecuting}>
              Always Allow
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export function promptControllerCommand(detail: ControllerCommandPromptDetail) {
  window.dispatchEvent(new CustomEvent('controller-command:prompt', { detail }));
}
