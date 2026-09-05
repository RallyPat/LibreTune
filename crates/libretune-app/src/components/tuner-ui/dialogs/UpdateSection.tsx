import { useCallback, useEffect, useState } from 'react';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { Button } from '../../common';

type UpdateState =
  | { kind: 'checking' }
  | { kind: 'idle'; checked: boolean }
  | { kind: 'available'; update: Update }
  | { kind: 'installing'; update: Update }
  | { kind: 'error'; message: string };

async function requestUpdate(): Promise<UpdateState> {
  try {
    const update = await check();
    return update ? { kind: 'available', update } : { kind: 'idle', checked: true };
  } catch (e) {
    return { kind: 'error', message: e instanceof Error ? e.message : String(e) };
  }
}

/**
 * Fire-and-forget startup check. Resolves to the available version, or null
 * when there is none or the check failed (offline, no release yet). Never
 * throws, never blocks anything. The plugin-side resource is released here;
 * installing goes through the About dialog, which runs its own check.
 */
export async function checkForUpdateQuietly(): Promise<string | null> {
  const state = await requestUpdate();
  if (state.kind !== 'available') return null;
  const { version } = state.update;
  await state.update.close().catch(() => undefined);
  return version;
}

/** Every non-null `check()` holds a Tauri resource until closed. */
function discard(state: UpdateState) {
  if (state.kind === 'available' || state.kind === 'installing') {
    void state.update.close().catch(() => undefined);
  }
}

/** "Check for updates" block shown in the About dialog. */
export function UpdateSection() {
  const [state, setState] = useState<UpdateState>({ kind: 'idle', checked: false });

  const checkNow = useCallback(async () => {
    setState((prev) => {
      discard(prev);
      return { kind: 'checking' };
    });
    setState(await requestUpdate());
  }, []);

  useEffect(() => {
    let active = true;
    setState({ kind: 'checking' });
    void requestUpdate().then((next) => {
      if (active) setState(next);
      else discard(next);
    });
    return () => {
      active = false;
      setState((prev) => {
        discard(prev);
        return prev;
      });
    };
  }, []);

  const install = async (update: Update) => {
    setState({ kind: 'installing', update });
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (e) {
      setState({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
    }
  };

  if (state.kind === 'available' || state.kind === 'installing') {
    const { update } = state;
    const installing = state.kind === 'installing';
    return (
      <section className="dialog-update" role="status">
        <p>
          Version <strong>{update.version}</strong> is available.
        </p>
        {update.body && <p className="dialog-update-notes">{update.body}</p>}
        <div className="dialog-update-actions">
          <Button
            variant="primary"
            size="sm"
            disabled={installing}
            aria-busy={installing}
            onClick={() => void install(update)}
          >
            {installing ? 'Installing…' : 'Install and restart'}
          </Button>
          <Button
            size="sm"
            disabled={installing}
            onClick={() => {
              discard(state);
              setState({ kind: 'idle', checked: false });
            }}
          >
            Later
          </Button>
        </div>
      </section>
    );
  }

  return (
    <section className="dialog-update" aria-label="Application updates">
      {state.kind === 'error' && (
        <p className="dialog-update-error" role="alert">
          Update check failed: {state.message}
        </p>
      )}
      {state.kind === 'idle' && state.checked && (
        <p role="status">LibreTune is up to date.</p>
      )}
      <Button
        size="sm"
        disabled={state.kind === 'checking'}
        aria-busy={state.kind === 'checking'}
        onClick={() => void checkNow()}
      >
        {state.kind === 'checking' ? 'Checking…' : 'Check for updates'}
      </Button>
    </section>
  );
}
