/** Shared connection workflow helpers (port memory, reconnect requests). */

export type ConnectionPhase =
  | 'connected'
  | 'disconnected'
  | 'connecting'
  | 'syncing'
  | 'auto-connect-waiting'
  | 'auto-connect'
  | 'signature-mismatch';

export interface ReconnectRequestDetail {
  source: string;
  delayMs?: number;
  retries?: number;
  port?: string;
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

export function requestReconnect(detail: ReconnectRequestDetail): void {
  window.dispatchEvent(new CustomEvent('reconnect:request', { detail }));
}

/** Pick the best serial port from project memory, app memory, or the live port list. */
export function resolvePreferredPort(opts: {
  projectPort: string | null;
  lastSerialPort: string | null;
  availablePorts: string[];
  currentSelected?: string;
}): string | null {
  const { projectPort, lastSerialPort, availablePorts, currentSelected } = opts;
  if (availablePorts.length === 0) {
    return currentSelected || null;
  }
  if (projectPort && availablePorts.includes(projectPort)) {
    return projectPort;
  }
  if (lastSerialPort && availablePorts.includes(lastSerialPort)) {
    return lastSerialPort;
  }
  if (currentSelected && availablePorts.includes(currentSelected)) {
    return currentSelected;
  }
  const acm0 = availablePorts.find((p) => p.endsWith('ttyACM0'));
  return acm0 || availablePorts[0];
}

export function connectionPhaseLabel(
  phase: ConnectionPhase,
  opts: { port?: string; ecuName?: string },
): string {
  const portHint = opts.port ? ` (${opts.port})` : '';
  switch (phase) {
    case 'connected':
      return opts.ecuName || 'Connected';
    case 'connecting':
      return `Connecting${portHint}…`;
    case 'syncing':
      return `Syncing${portHint}…`;
    case 'auto-connect-waiting':
      return opts.port
        ? `Waiting for ${opts.port}…`
        : 'Waiting for ECU port…';
    case 'auto-connect':
      return `Auto-connecting${portHint}…`;
    case 'signature-mismatch':
      return 'Connected — INI mismatch';
    default:
      return 'Disconnected';
  }
}
