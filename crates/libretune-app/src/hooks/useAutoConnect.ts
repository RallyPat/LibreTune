import { useEffect, useRef } from 'react';
import type { ConnectionStatus, CurrentProject } from '../types/app';

export interface ConnectOptions {
  /** Only attempt the requested port — never fall back to another COM port. */
  strictPort?: boolean;
  /** Suppress failure toasts (background auto-connect). */
  silent?: boolean;
  /** Override the port for this attempt (defaults to UI selection). */
  port?: string;
}

export interface UseAutoConnectDeps {
  currentProject: CurrentProject | null;
  status: ConnectionStatus;
  connecting: boolean;
  connect: (options?: ConnectOptions) => Promise<void>;
  refreshPorts: () => Promise<string[]>;
}

const POLL_INTERVAL_MS = 2500;
const INITIAL_DELAY_MS = 600;

/**
 * When auto-connect is enabled for the project, periodically checks whether
 * the last successful port is available and attempts a connection.
 */
export function useAutoConnect({
  currentProject,
  status,
  connecting,
  connect,
  refreshPorts,
}: UseAutoConnectDeps) {
  const connectRef = useRef(connect);
  const refreshPortsRef = useRef(refreshPorts);
  const statusRef = useRef(status);
  const connectingRef = useRef(connecting);
  const attemptInFlightRef = useRef(false);

  connectRef.current = connect;
  refreshPortsRef.current = refreshPorts;
  statusRef.current = status;
  connectingRef.current = connecting;

  const enabled = !!currentProject?.connection.auto_connect;
  const savedPort = currentProject?.connection.port ?? null;

  useEffect(() => {
    if (!enabled || !savedPort) return undefined;

    let cancelled = false;

    const attempt = async () => {
      if (
        cancelled ||
        attemptInFlightRef.current ||
        connectingRef.current ||
        statusRef.current.state === 'Connected'
      ) {
        return;
      }

      const portList = await refreshPortsRef.current();
      if (cancelled || !portList.includes(savedPort)) {
        return;
      }

      attemptInFlightRef.current = true;
      try {
        await connectRef.current({ strictPort: true, silent: true, port: savedPort });
      } finally {
        attemptInFlightRef.current = false;
      }
    };

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
    };
  }, [enabled, savedPort]);
}
