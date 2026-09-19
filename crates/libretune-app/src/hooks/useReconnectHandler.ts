import { useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ConnectionStatus } from '../types/app';
import type { ConnectOptions } from './useAutoConnect';
import {
  type ReconnectRequestDetail,
  sleep,
} from '../utils/connectionWorkflow';

export interface UseReconnectHandlerDeps {
  connecting: boolean;
  syncing: boolean;
  status: ConnectionStatus;
  projectPort: string | null;
  lastSerialPort: string | null;
  connect: (options?: ConnectOptions) => Promise<void>;
  showToast: (msg: string, type: 'info' | 'success' | 'error' | 'warning') => void;
}

/**
 * Handles `reconnect:request` window events with optional delay and retries.
 * Used after controller commands, firmware updates, and other ECU reboot flows.
 */
export function useReconnectHandler(deps: UseReconnectHandlerDeps) {
  const depsRef = useRef(deps);
  depsRef.current = deps;

  useEffect(() => {
    const handler = async (event: Event) => {
      const detail = (event as CustomEvent<ReconnectRequestDetail>).detail ?? {
        source: 'unknown',
      };
      const {
        connecting,
        syncing,
        status,
        projectPort,
        lastSerialPort,
        connect,
        showToast,
      } = depsRef.current;

      if (connecting || syncing) {
        showToast('Reconnect requested but a connection is already in progress', 'info');
        return;
      }
      if (status.state === 'Connected') {
        return;
      }

      const source = detail.source ?? 'unknown';
      const isFirmware = source.includes('firmware');

      try {
        const settings = await invoke<{
          auto_reconnect_after_controller_command?: boolean;
          auto_reconnect_after_firmware?: boolean;
        }>('get_settings');

        if (
          source.includes('controller-command') &&
          settings.auto_reconnect_after_controller_command === false
        ) {
          return;
        }
        if (
          isFirmware &&
          settings.auto_reconnect_after_firmware === false
        ) {
          return;
        }
      } catch (e) {
        console.warn('Could not read reconnect settings:', e);
      }

      // Delays and retry counts default higher for firmware flashes than for
      // controller commands: a firmware update reboots the whole ECU (serial
      // port disappears and re-enumerates, bootloader handshake adds seconds),
      // whereas a controller command (e.g. reset) just bounces the app and
      // comes back quickly. The 8s/10x firmware budget vs 2s/4x command budget
      // reflects that observed difference; callers can override per-event.
      const delayMs = detail.delayMs ?? (isFirmware ? 8000 : 2000);
      const maxRetries = detail.retries ?? (isFirmware ? 10 : 4);
      const targetPort =
        detail.port ?? projectPort ?? lastSerialPort ?? undefined;

      if (delayMs > 0) {
        await sleep(delayMs);
      }

      for (let attempt = 0; attempt < maxRetries; attempt++) {
        if (depsRef.current.status.state === 'Connected') {
          return;
        }

        const ports = await invoke<string[]>('get_serial_ports', { probe: false });
        const port =
          targetPort && ports.includes(targetPort)
            ? targetPort
            : ports.length === 1
              ? ports[0]
              : undefined;

        if (!port) {
          await sleep(400);
          continue;
        }

        try {
          await connect({
            strictPort: !!targetPort,
            silent: attempt > 0,
            port,
          });
          const latest = await invoke<ConnectionStatus>('get_connection_status');
          if (latest.state === 'Connected') {
            if (attempt > 0 || isFirmware) {
              showToast(`Reconnected on ${port}`, 'success');
            }
            return;
          }
        } catch (e) {
          console.debug('Reconnect attempt failed:', e);
        }

        await sleep(400);
      }

      showToast(
        'Automatic reconnect failed — open Connection and connect when the ECU is ready.',
        'warning',
      );
    };

    window.addEventListener('reconnect:request', handler);
    return () => window.removeEventListener('reconnect:request', handler);
  }, []);
}
