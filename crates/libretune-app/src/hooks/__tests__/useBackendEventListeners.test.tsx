import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

import { listen } from '@tauri-apps/api/event';
import { useBackendEventListeners } from '../useBackendEventListeners';

describe('useBackendEventListeners', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    (listen as unknown as ReturnType<typeof vi.fn>).mockResolvedValue(() => {});
  });

  it('registers each backend listener exactly once, even when checkStatus is a new function every render (as App.tsx produces)', async () => {
    const baseDeps = {
      setSignatureMismatchInfo: vi.fn(),
      setSignatureMismatchOpen: vi.fn(),
      setMigrationReportOpen: vi.fn(),
      setTuneMismatchInfo: vi.fn(),
      setTuneMismatchOpen: vi.fn(),
    };

    const { rerender } = renderHook(
      (props: { checkStatus: () => void }) => useBackendEventListeners({ ...baseDeps, checkStatus: props.checkStatus }),
      { initialProps: { checkStatus: () => {} } }
    );

    await Promise.resolve();
    await Promise.resolve();

    const callCountAfterMount = (listen as unknown as ReturnType<typeof vi.fn>).mock.calls.length;
    expect(callCountAfterMount).toBeGreaterThan(0);

    // Simulate App.tsx's plain `async function checkStatus() {}` being a
    // brand-new function reference on every render.
    for (let i = 0; i < 5; i++) {
      rerender({ checkStatus: () => {} });
    }
    await Promise.resolve();

    expect((listen as unknown as ReturnType<typeof vi.fn>).mock.calls.length).toBe(callCountAfterMount);
  });

  it('invokes the latest checkStatus (not a stale one) when definition:loaded fires', async () => {
    let definitionLoadedHandler: ((event: { payload: unknown }) => void) | undefined;
    (listen as unknown as ReturnType<typeof vi.fn>).mockImplementation(async (evt: string, handler: any) => {
      if (evt === 'definition:loaded') definitionLoadedHandler = handler;
      return () => {};
    });

    const baseDeps = {
      setSignatureMismatchInfo: vi.fn(),
      setSignatureMismatchOpen: vi.fn(),
      setMigrationReportOpen: vi.fn(),
      setTuneMismatchInfo: vi.fn(),
      setTuneMismatchOpen: vi.fn(),
    };

    const firstCheckStatus = vi.fn();
    const { rerender } = renderHook(
      (props: { checkStatus: () => void }) => useBackendEventListeners({ ...baseDeps, checkStatus: props.checkStatus }),
      { initialProps: { checkStatus: firstCheckStatus } }
    );
    await Promise.resolve();
    await Promise.resolve();

    const latestCheckStatus = vi.fn();
    rerender({ checkStatus: latestCheckStatus });

    definitionLoadedHandler!({ payload: {} });

    expect(latestCheckStatus).toHaveBeenCalledTimes(1);
    expect(firstCheckStatus).not.toHaveBeenCalled();
  });
});
