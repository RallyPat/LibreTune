import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ setTitle: vi.fn().mockResolvedValue(undefined) }),
}));

import { listen } from '@tauri-apps/api/event';
import { useEcuEventListeners, type UseEcuEventListenersDeps } from '../useEcuEventListeners';

// Real App.tsx sources these from LoadingContext, where they are already
// useCallback-memoized (stable across renders) — mirror that here so this
// test isolates the fix under test (doSync/checkStatus/fetchConstants/
// fetchMenuTree being non-memoized) rather than incidentally exercising a
// different, already-fine dependency.
const stableShowLoading = vi.fn();
const stableHideLoading = vi.fn();

function baseDeps(overrides: Partial<UseEcuEventListenersDeps> = {}): UseEcuEventListenersDeps {
  return {
    isTauri: true,
    status: { state: 'Connected' } as UseEcuEventListenersDeps['status'],
    currentProject: null,
    activeTabId: null,
    doSync: vi.fn().mockResolvedValue(null),
    checkStatus: vi.fn().mockResolvedValue(undefined),
    fetchConstants: vi.fn().mockResolvedValue({}),
    fetchMenuTree: vi.fn().mockResolvedValue(undefined),
    showLoading: stableShowLoading,
    hideLoading: stableHideLoading,
    ...overrides,
  };
}

describe('useEcuEventListeners', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    (listen as unknown as ReturnType<typeof vi.fn>).mockResolvedValue(() => {});
  });

  it('registers ini:changed/demo:changed listeners once, even as doSync/checkStatus/fetchConstants/fetchMenuTree get new references every render', async () => {
    const { rerender } = renderHook(
      (props: { deps: UseEcuEventListenersDeps }) => useEcuEventListeners(props.deps),
      { initialProps: { deps: baseDeps() } }
    );

    await Promise.resolve();
    await Promise.resolve();

    const countAfterMount = (listen as unknown as ReturnType<typeof vi.fn>).mock.calls.length;
    expect(countAfterMount).toBeGreaterThan(0);

    for (let i = 0; i < 5; i++) {
      rerender({ deps: baseDeps() }); // brand-new function references each time, like App.tsx
    }
    await Promise.resolve();

    expect((listen as unknown as ReturnType<typeof vi.fn>).mock.calls.length).toBe(countAfterMount);
  });

  it('demo:changed handler calls the latest checkStatus/fetchConstants/fetchMenuTree, not stale closures', async () => {
    let demoHandler: ((event: { payload: unknown }) => void | Promise<void>) | undefined;
    (listen as unknown as ReturnType<typeof vi.fn>).mockImplementation(async (evt: string, handler: any) => {
      if (evt === 'demo:changed') demoHandler = handler;
      return () => {};
    });

    const stale = baseDeps();
    const { rerender } = renderHook(
      (props: { deps: UseEcuEventListenersDeps }) => useEcuEventListeners(props.deps),
      { initialProps: { deps: stale } }
    );
    await Promise.resolve();
    await Promise.resolve();

    const latest = baseDeps();
    rerender({ deps: latest });

    await demoHandler!({ payload: false });

    expect(latest.checkStatus).toHaveBeenCalledTimes(1);
    expect(latest.fetchConstants).toHaveBeenCalledTimes(1);
    expect(latest.fetchMenuTree).toHaveBeenCalledTimes(1);
    expect(stale.checkStatus).not.toHaveBeenCalled();
    expect(stale.fetchConstants).not.toHaveBeenCalled();
    expect(stale.fetchMenuTree).not.toHaveBeenCalled();
  });

  it('ini:changed handler calls the latest doSync when resync is required', async () => {
    let iniHandler: ((event: { payload: unknown }) => void | Promise<void>) | undefined;
    (listen as unknown as ReturnType<typeof vi.fn>).mockImplementation(async (evt: string, handler: any) => {
      if (evt === 'ini:changed') iniHandler = handler;
      return () => {};
    });

    const stale = baseDeps();
    const { rerender } = renderHook(
      (props: { deps: UseEcuEventListenersDeps }) => useEcuEventListeners(props.deps),
      { initialProps: { deps: stale } }
    );
    await Promise.resolve();
    await Promise.resolve();

    const latest = baseDeps();
    rerender({ deps: latest });

    await iniHandler!({ payload: 'resync_required' });

    expect(latest.doSync).toHaveBeenCalledTimes(1);
    expect(stale.doSync).not.toHaveBeenCalled();
  });
});
