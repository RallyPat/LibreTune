import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useTableCurveRefresh } from '../useTableCurveRefresh';
import type { TabContent } from '../../types/app';
import type { Tab } from '../../components/tuner-ui';

describe('useTableCurveRefresh', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('registers the tune:loaded listener exactly once, even as tabs/tabContents change across renders', async () => {
    (listen as unknown as ReturnType<typeof vi.fn>).mockResolvedValue(() => {});

    const tabs: Tab[] = [{ id: 'tab-1', title: 'Table 1' } as Tab];
    const tabContents: Record<string, TabContent> = {};
    const setTabContents = vi.fn();

    const { rerender } = renderHook(
      (props: { tabs: Tab[]; tabContents: Record<string, TabContent> }) =>
        useTableCurveRefresh({
          tabs: props.tabs,
          tabContents: props.tabContents,
          setTabContents,
          activeTabId: null,
        }),
      { initialProps: { tabs, tabContents } }
    );

    // Flush the async listen() registration.
    await Promise.resolve();
    await Promise.resolve();

    expect(listen).toHaveBeenCalledTimes(1);

    // Simulate what happens on every table/curve edit: a brand new
    // tabContents object reference (as produced by setTabContents({...}))
    // and re-renders of the hook's consumer.
    for (let i = 0; i < 5; i++) {
      rerender({
        tabs,
        tabContents: { ...tabContents, [`edit-${i}`]: { type: 'table', data: {} } as unknown as TabContent },
      });
    }

    await Promise.resolve();

    // The listener must not be torn down and re-registered on every edit.
    expect(listen).toHaveBeenCalledTimes(1);
  });

  it('reads the latest tabs/tabContents (via refs) when tune:loaded fires, not a stale closure', async () => {
    let capturedHandler: ((event: { payload: string }) => void | Promise<void>) | undefined;
    (listen as unknown as ReturnType<typeof vi.fn>).mockImplementation(async (_evt: string, handler: any) => {
      capturedHandler = handler;
      return () => {};
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockImplementation(async (cmd: string) => {
      if (cmd === 'get_table_data') {
        return { z_values: [1, 2, 3], x_bins: [], y_bins: [], num_rows: 1, num_cols: 3 };
      }
      throw new Error(`unexpected invoke: ${cmd}`);
    });

    const setTabContents = vi.fn();
    const initialTabs: Tab[] = [];
    const initialTabContents: Record<string, TabContent> = {};

    const { rerender } = renderHook(
      (props: { tabs: Tab[]; tabContents: Record<string, TabContent> }) =>
        useTableCurveRefresh({
          tabs: props.tabs,
          tabContents: props.tabContents,
          setTabContents,
          activeTabId: null,
        }),
      { initialProps: { tabs: initialTabs, tabContents: initialTabContents } }
    );

    await Promise.resolve();
    await Promise.resolve();
    expect(capturedHandler).toBeDefined();

    // Add a table tab AFTER the listener was registered — this is exactly
    // the scenario a stale closure over the initial `tabs`/`tabContents`
    // would get wrong.
    const laterTabs: Tab[] = [{ id: 'table-a', title: 'Table A' } as Tab];
    const laterTabContents: Record<string, TabContent> = {
      'table-a': { type: 'table', data: {} } as unknown as TabContent,
    };
    rerender({ tabs: laterTabs, tabContents: laterTabContents });

    await capturedHandler!({ payload: 'test-tune.msq' });
    // Allow the 50ms delay + async refresh work inside the handler to settle.
    await new Promise((resolve) => setTimeout(resolve, 60));
    await Promise.resolve();

    expect(invoke).toHaveBeenCalledWith('get_table_data', { tableName: 'table-a' });
    expect(setTabContents).toHaveBeenCalled();
  });
});
