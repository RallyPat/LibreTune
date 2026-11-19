import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { Button, FormField } from '../../common';

interface ChatGptStatus {
  active: string | null;
  accounts: { id: string; label: string; connected: boolean }[];
}

interface PlanModel {
  slug: string;
  display_name: string;
}

const USAGE_URL = 'https://chatgpt.com/#settings/Usage';

export function ChatGptPlanSection({ model, onModelChange }: {
  model: string;
  onModelChange: (model: string) => void;
}) {
  const [status, setStatus] = useState<ChatGptStatus>();
  const [models, setModels] = useState<PlanModel[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const load = useCallback(async () => {
    const current = await invoke<ChatGptStatus>('chatgpt_status');
    setStatus(current);
    setModels([]);
    if (current.accounts.some((account) => account.id === current.active && account.connected)) {
      setModels(await invoke<PlanModel[]>('chatgpt_models'));
    }
  }, []);

  const run = useCallback(async (action: () => Promise<void>) => {
    setError(undefined);
    setBusy(true);
    try {
      await action();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    void run(load);
  }, [run, load]);

  const active = status?.accounts.find((account) => account.id === status.active);
  const signIn = (accountId: string | null) => run(async () => {
    await invoke<ChatGptStatus>('chatgpt_sign_in', { accountId });
    onModelChange('');
    await load();
  });

  return (
    <>
      <FormField label="ChatGPT account" help="Eligible requests use your ChatGPT plan allowance. Review limits and credit permissions in ChatGPT.">
        {() => (
          <div>
            <p role="status">
              {busy ? 'Waiting for ChatGPT…' : active?.connected
                ? 'ChatGPT plan connected.'
                : 'Connect your ChatGPT plan in your browser.'}
            </p>
            {!!status?.accounts.length && (
              <select aria-label="Saved ChatGPT account" value={status.active ?? ''} disabled={busy}
                onChange={(event) => {
                  const accountId = event.target.value;
                  void run(async () => {
                    await invoke<ChatGptStatus>('chatgpt_select_account', { accountId });
                    onModelChange('');
                    await load();
                  });
                }}>
                {status.accounts.map((account) => (
                  <option key={account.id} value={account.id}>{account.label}</option>
                ))}
              </select>
            )}
            <Button disabled={busy} onClick={() => void signIn(active?.id ?? null)}>
              {active?.connected ? 'Reconnect ChatGPT' : 'Continue with ChatGPT'}
            </Button>
            {active && (
              <Button disabled={busy} onClick={() => void signIn(null)}>Add ChatGPT account</Button>
            )}
            {active?.connected && (
              <Button disabled={busy} onClick={() => void run(async () => {
                const revoked = await invoke<boolean>('chatgpt_sign_out');
                await load();
                if (!revoked) {
                  setError('Signed out locally. Open ChatGPT settings to check whether access was revoked.');
                }
              })}>
                Sign out
              </Button>
            )}
            <Button disabled={busy} onClick={() => void run(() => openUrl(USAGE_URL))}>
              Manage usage
            </Button>
            {error && <p role="alert">{error}</p>}
          </div>
        )}
      </FormField>
      <FormField label="Model">
        {(id) => (
          <select id={id} value={models.some((item) => item.slug === model) ? model : ''}
            disabled={busy || models.length === 0}
            onChange={(event) => onModelChange(event.target.value)}>
            <option value="" disabled>Choose a ChatGPT model…</option>
            {models.map((item) => (
              <option key={item.slug} value={item.slug}>{item.display_name}</option>
            ))}
          </select>
        )}
      </FormField>
    </>
  );
}
