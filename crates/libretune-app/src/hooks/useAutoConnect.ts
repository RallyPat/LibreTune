import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ConnectionStatus, CurrentProject } from '../types/app';
import type { ConnectionPhase } from '../utils/connectionWorkflow';

export interface ConnectOptions {
  /** Only attempt the requested port — never fall back to another COM port. */
  strictPort?: boolean;
  /** Suppress failure toasts (background auto-connect). */
  silent?: boolean;
  /** Override the port for this attempt (defaults to UI selection). */
  port?: string;
  /** Override the baud rate for this attempt (defaults to UI selection). */
  baudRate?: number;
  /** Override the connection type for this attempt (defaults to UI selection). */
  connectionType?: 'Serial' | 'Tcp';
  /** Override the TCP host for this attempt (defaults to UI selection). */
  tcpHost?: string;
  /** Override the TCP port for this attempt (defaults to UI selection). */
  tcpPort?: number;
}

export interface UseAutoConnectDeps {
  currentProject: CurrentProject | null;
  lastSerialPort: string | null;
  status: ConnectionStatus;
  connecting: boolean;
  syncing: boolean;
  connect: (options?: ConnectOptions) => Promise<void>;
}

const POLL_INTERVAL_MS = 400;
const INITIAL_DELAY_MS = 200;

function isConnectedStatus(connection: ConnectionStatus): boolean {
  return connection.state === 'Connected';
}

/**
 * When auto-connect is enabled for the project, periodically checks whether
 * the remembered port is available and attempts a connection.
 */
export function useAutoConnect({
  currentProject,
  lastSerialPort,
  status,
  connecting,
  syncing,
  connect,
}: UseAutoConnectDeps): ConnectionPhase | null {
  const connectRef = useRef(connect);
  const statusRef = useRef(status);
  const connectingRef = useRef(connecting);
  const syncingRef = useRef(syncing);
  const attemptInFlightRef = useRef(false);
  const [autoConnectPhase, setAutoConnectPhase] = useState<ConnectionPhase | null>(null);

  connectRef.current = connect;
  statusRef.current = status;
  connectingRef.current = connecting;
  syncingRef.current = syncing;

  const enabled = !!currentProject?.connection.auto_connect;
  const savedPort = currentProject?.connection.port ?? lastSerialPort;

  useEffect(() => {
    if (!enabled || !savedPort) {
      setAutoConnectPhase(null);
      return undefined;
    }

    if (isConnectedStatus(statusRef.current)) {
      setAutoConnectPhase(null);
      return undefined;
    }

    let cancelled = false;

    const attempt = async () => {
      if (
        cancelled ||
        attemptInFlightRef.current ||
        connectingRef.current ||
        syncingRef.current ||
        isConnectedStatus(statusRef.current)
      ) {
        return;
      }

      const portList = await invoke<string[]>('get_serial_ports', { probe: false });
      if (cancelled) {
        return;
      }

      if (!portList.includes(savedPort)) {
        setAutoConnectPhase('auto-connect-waiting');
        return;
      }

      setAutoConnectPhase('auto-connect');
      attemptInFlightRef.current = true;
      try {
        await connectRef.current({ strictPort: true, silent: true, port: savedPort });
      } finally {
        attemptInFlightRef.current = false;
        if (!cancelled) {
          setAutoConnectPhase(
            isConnectedStatus(statusRef.current) ? null : 'auto-connect-waiting',
          );
        }
      }
    };

    setAutoConnectPhase('auto-connect-waiting');

    const initialTimer = window.setTimeout(() => {
      void attempt();
    }, INITIAL_DELAY_MS);

    const interval = window.setInterval(() => {
      void attempt();
    }, POLL_INTERVAL_MS);

    return () => {
      cancelled = true;
      window.clearTimeout(initialTimer);
      window.clearInterval(interval);
      setAutoConnectPhase(null);
    };
  }, [enabled, savedPort, status.state]);

  return enabled && !isConnectedStatus(status) ? autoConnectPhase : null;
}
