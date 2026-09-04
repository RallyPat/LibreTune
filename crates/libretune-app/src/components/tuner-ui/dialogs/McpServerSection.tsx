/**
 * McpServerSection — Settings → AI Assistant → MCP server.
 *
 * Lets external agents (Claude Code, Claude Desktop, any MCP client) call
 * LibreTune's read-only tune tools over a loopback HTTP server.
 *
 * Unlike the rest of the Settings dialog, these controls apply immediately
 * rather than on OK/Apply: each one has to bind or release a real socket,
 * and the result (bound port, or the reason it failed) is only knowable
 * after the fact. Batching them would leave the status line lying until the
 * dialog closed.
 */
import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button, FormField } from '../../common';

/** Mirrors the Rust `McpStatus`. */
interface McpStatus {
  running: boolean;
  /** The bound port while running; 0 when stopped. */
  port: number;
}

/** Matches `MIN_MCP_PORT` in `src-tauri/src/mcp/server.rs`. */
const MIN_PORT = 1024;

const STOPPED: McpStatus = { running: false, port: 0 };

/**
 * Accept a status only if it actually is one. The backend always returns a
 * well-formed `McpStatus`, but a stubbed `invoke` (tests) or a command that
 * was never registered resolves to `undefined`, and rendering
 * `status.running` off that takes the whole Settings dialog down with it.
 */
function asStatus(value: unknown): McpStatus {
  const candidate = value as Partial<McpStatus> | undefined;
  if (candidate && typeof candidate.running === 'boolean' && typeof candidate.port === 'number') {
    return { running: candidate.running, port: candidate.port };
  }
  return STOPPED;
}

export function McpServerSection() {
  const [status, setStatus] = useState<McpStatus>(STOPPED);
  const [port, setPort] = useState(8765);
  const [token, setToken] = useState('');
  const [tokenVisible, setTokenVisible] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [current, settings] = await Promise.all([
          invoke<McpStatus>('mcp_status'),
          invoke<Record<string, unknown> | undefined>('get_settings'),
        ]);
        if (cancelled) return;
        setStatus(asStatus(current));
        if (typeof settings?.mcp_port === 'number') setPort(settings.mcp_port);
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  /** Run one MCP command, surfacing its error instead of throwing into the dialog. */
  const run = useCallback(async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, []);

  const toggle = (enabled: boolean) =>
    run(async () => {
      setStatus(asStatus(await invoke<McpStatus>('mcp_set_enabled', { enabled })));
    });

  const applyPort = () =>
    run(async () => {
      setStatus(asStatus(await invoke<McpStatus>('mcp_set_port', { port })));
    });

  const revealToken = () =>
    run(async () => {
      setToken(String((await invoke<string>('mcp_get_token')) ?? ''));
      setTokenVisible(true);
    });

  const regenerateToken = () =>
    run(async () => {
      setToken(String((await invoke<string>('mcp_regenerate_token')) ?? ''));
      setTokenVisible(true);
    });

  const connectCommand = `claude mcp add --transport http libretune http://127.0.0.1:${
    status.running ? status.port : port
  }/mcp --header "Authorization: Bearer ${tokenVisible && token ? token : '<TOKEN>'}"`;

  return (
    <>
      <h3 style={{ marginTop: '1.5rem', marginBottom: '0.5rem' }}>MCP server (local)</h3>
      <span className="dialog-form-note">
        Exposes LibreTune's <strong>read-only</strong> tune tools to external MCP clients on
        127.0.0.1 only — never off this machine. No tool here can edit a tune or touch the ECU.
        These controls apply immediately, not on OK.
      </span>

      <FormField
        label="Expose tools over MCP"
        help="Starts a loopback HTTP server so an outside agent can inspect the loaded tune"
      >
        {(id) => (
          <label className="dialog-checkbox-option" style={{ display: 'inline-flex', gap: '0.4rem' }}>
            <input
              id={id}
              type="checkbox"
              checked={status.running}
              disabled={busy}
              onChange={(e) => toggle(e.target.checked)}
            />
            <span>
              {status.running ? `Running on 127.0.0.1:${status.port}` : 'Stopped'}
            </span>
          </label>
        )}
      </FormField>

      <FormField label="Port" help={`Minimum ${MIN_PORT}. Changing it restarts the server.`}>
        {(id) => (
          <span style={{ display: 'inline-flex', gap: '0.5rem', alignItems: 'center' }}>
            <input
              id={id}
              type="number"
              min={MIN_PORT}
              max={65535}
              value={port}
              disabled={busy}
              onChange={(e) => setPort(Number(e.target.value))}
              style={{ width: '8rem', fontFamily: 'monospace' }}
            />
            <Button onClick={applyPort} disabled={busy || port < MIN_PORT}>
              Apply port
            </Button>
          </span>
        )}
      </FormField>

      <FormField
        label="Access token"
        help="Every request must send this as a bearer token. Regenerating invalidates the old one immediately."
      >
        {(id) => (
          <span style={{ display: 'inline-flex', gap: '0.5rem', alignItems: 'center' }}>
            <input
              id={id}
              type="text"
              readOnly
              value={tokenVisible ? token : '••••••••••••••••'}
              style={{ width: '22rem', fontFamily: 'monospace' }}
              onFocus={(e) => e.currentTarget.select()}
            />
            <Button onClick={revealToken} disabled={busy}>
              Show
            </Button>
            <Button onClick={regenerateToken} disabled={busy}>
              Regenerate
            </Button>
          </span>
        )}
      </FormField>

      <div className="dialog-form-group">
        <label>Connect Claude Code</label>
        <input
          type="text"
          readOnly
          value={connectCommand}
          style={{ fontFamily: 'monospace' }}
          onFocus={(e) => e.currentTarget.select()}
        />
        <span className="dialog-form-note">
          Click to select, then copy. Claude Desktop needs the `mcp-remote` bridge — see the MCP
          page in the manual.
        </span>
      </div>

      {error && (
        <span className="dialog-form-note" role="alert" style={{ color: 'var(--color-error, #d33)' }}>
          {error}
        </span>
      )}
    </>
  );
}

export default McpServerSection;
