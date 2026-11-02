/**
 * Frontend regression tests for the AutoTune issue #132 follow-ups:
 * the trace line over the recommendation grid, the Y-axis flip, and
 * mid-session table switching (selectors stay live while a session runs,
 * with an unsent-recommendations guard on the way out).
 */
import { render, screen, waitFor, act, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi, describe, it, expect, beforeEach } from 'vitest';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { ToastProvider } from '../../../contexts/ToastContext';
import { AutoTune } from '../AutoTune';
import { useRealtimeStore } from '../../../stores/realtimeStore';

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

const TABLE_DATA = {
  name: 'veTable1Tbl',
  title: 'VE Table',
  x_bins: [1000, 2000],
  y_bins: [20, 80],
  z_values: [
    [50, 60],
    [70, 80],
  ],
  x_output_channel: 'rpm',
  y_output_channel: 'map',
};

const TWO_VE_TABLES = [
  { name: 'veTable1Tbl', title: 'VE Table' },
  { name: 'veTable2Tbl', title: 'VE Table 2' },
];

interface SessionStatus {
  running: boolean;
  tableName: string | null;
  secondaryTableName: string | null;
  acceptedSamples?: number;
  rejections?: { reason: string; count: number }[];
  resolvedLoadSource?: 'map' | 'maf' | 'tps';
}

/**
 * Mock invoke with a live session status the test can mutate, plus optional
 * recommendations for the switch guard. `yAxisSetting` is read by every
 * `get_settings` call, so flipping it (and remounting) exercises the
 * reversed-row rendering without needing a Tauri event.
 */
function mockInvokeWith(opts: {
  sessionStatus?: SessionStatus | null;
  recommendations?: unknown[];
  tables?: { name: string; title: string }[];
  yAxisSetting?: boolean;
}) {
  const { sessionStatus = null, recommendations = [], tables = TWO_VE_TABLES, yAxisSetting = false } = opts;
  invokeMock.mockReset();
  invokeMock.mockImplementation((cmd: string, args: any) => {
    if (cmd === 'get_ve_analyze_config') return Promise.resolve(null);
    if (cmd === 'get_tables') return Promise.resolve(tables);
    if (cmd === 'list_tunable_tables') return Promise.resolve(tables.map((t) => t.name));
    if (cmd === 'get_table_data')
      return Promise.resolve(tables.some((t) => t.name === args?.tableName)
        ? { ...TABLE_DATA, name: args.tableName }
        : Promise.reject(new Error('unknown table')));
    if (cmd === 'get_available_channels') return Promise.resolve([]);
    if (cmd === 'get_settings') return Promise.resolve({ table_y_axis_bottom: yAxisSetting });
    if (cmd === 'get_autotune_status')
      return Promise.resolve(
        sessionStatus
          ? {
              running: sessionStatus.running,
              tableName: sessionStatus.tableName,
              secondaryTableName: sessionStatus.secondaryTableName,
              acceptedSamples: sessionStatus.acceptedSamples ?? 0,
              rejections: sessionStatus.rejections ?? [],
              resolvedLoadSource: sessionStatus.resolvedLoadSource ?? 'map',
            }
          : { running: false, tableName: null, secondaryTableName: null, acceptedSamples: 0, rejections: [], resolvedLoadSource: null },
      );
    if (cmd === 'get_autotune_recommendations') return Promise.resolve(recommendations);
    return Promise.resolve();
  });
}

const rowHeaderValues = (container: HTMLElement) =>
  Array.from(container.querySelectorAll('tbody th')).map((th) => th.textContent);

describe('AutoTune trace line (issue #132 follow-up)', () => {
  beforeEach(() => {
    localStorage.clear();
    useRealtimeStore.getState().clearChannels();
    mockInvokeWith({});
  });

  it('draws the trail overlay and current-cell highlight when live data flows', async () => {
    const { container } = render(
      <ToastProvider>
        <AutoTune tableName="veTable1Tbl" isConnected />
      </ToastProvider>,
    );
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /toggle trace line/i })).toBeInTheDocument(),
    );
    // No live data yet: no trail, no current cell.
    expect(container.querySelector('svg.table-trail-overlay')).toBeNull();

    // rpm 1200 → col 0; map 60 → nearest bin 80 → row 1.
    act(() => {
      useRealtimeStore.getState().updateChannels({ rpm: 1200, map: 60 });
    });

    await waitFor(() =>
      expect(container.querySelector('svg.table-trail-overlay')).not.toBeNull(),
    );
    const cells = container.querySelectorAll('td.autotune-cell');
    // Row-major [row][col]: (1,0) is the third cell.
    expect(cells[2].className).toContain('current');
  });

  it('hides (and clears) the trail when the toolbar toggle is switched off', async () => {
    const { container } = render(
      <ToastProvider>
        <AutoTune tableName="veTable1Tbl" isConnected />
      </ToastProvider>,
    );
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /toggle trace line/i })).toBeInTheDocument(),
    );
    act(() => {
      useRealtimeStore.getState().updateChannels({ rpm: 1200, map: 60 });
    });
    await waitFor(() =>
      expect(container.querySelector('svg.table-trail-overlay')).not.toBeNull(),
    );

    await userEvent.click(screen.getByRole('button', { name: /toggle trace line/i }));
    expect(container.querySelector('svg.table-trail-overlay')).toBeNull();
    // Persisted as a view preference.
    expect(localStorage.getItem('libretune.autotune.settings.v2.view')).toContain(
      '"showTrail":false',
    );
  });
});

describe('AutoTune Y-axis flip (issue #132 follow-up)', () => {
  beforeEach(() => {
    localStorage.clear();
    useRealtimeStore.getState().clearChannels();
  });

  it('renders rows in storage order by default, reversed when the setting is on', async () => {
    mockInvokeWith({ yAxisSetting: false });
    const first = render(
      <ToastProvider>
        <AutoTune tableName="veTable1Tbl" isConnected />
      </ToastProvider>,
    );
    await waitFor(() => expect(rowHeaderValues(first.container)).toEqual(['20', '80']));
    first.unmount();

    mockInvokeWith({ yAxisSetting: true });
    const second = render(
      <ToastProvider>
        <AutoTune tableName="veTable1Tbl" isConnected />
      </ToastProvider>,
    );
    await waitFor(() => expect(rowHeaderValues(second.container)).toEqual(['80', '20']));
    // The flipped view carries the X-axis header in a tfoot, not a thead.
    expect(second.container.querySelector('.autotune-table thead')).toBeNull();
    expect(second.container.querySelector('.autotune-table tfoot th.autotune-corner')).not.toBeNull();
  });

  it('persists the flip through the global setting when the toolbar button is pressed', async () => {
    mockInvokeWith({});
    render(
      <ToastProvider>
        <AutoTune tableName="veTable1Tbl" isConnected />
      </ToastProvider>,
    );
    const flip = await screen.findByRole('button', { name: /Y axis zero at bottom/i });
    await userEvent.click(flip);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('update_setting', {
        key: 'table_y_axis_bottom',
        value: 'true',
      }),
    );
  });
});

describe('AutoTune mid-session switching (issue #132 follow-up)', () => {
  const primarySelect = async () => {
    const label = await screen.findByText('Primary:');
    const select = label.parentElement?.querySelector('select');
    if (!select) throw new Error('primary select not found next to its label');
    return select as HTMLSelectElement;
  };

  const secondaryCheckbox = async () => {
    const label = await screen.findByText(/Secondary:/i);
    const input = label.querySelector('input[type="checkbox"]');
    if (!input) throw new Error('secondary checkbox not found');
    return input as HTMLInputElement;
  };

  beforeEach(() => {
    localStorage.clear();
    useRealtimeStore.getState().clearChannels();
  });

  it('re-configures the live session in place when the primary table changes', async () => {
    mockInvokeWith({
      sessionStatus: { running: true, tableName: 'veTable1Tbl', secondaryTableName: null },
    });
    render(
      <ToastProvider>
        <AutoTune tableName="veTable1Tbl" isConnected />
      </ToastProvider>,
    );

    // Re-attach: the restart effect re-configures the session to the UI's
    // selection (a non-destructive no-op against the same tables).
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        'start_autotune',
        expect.objectContaining({ tableName: 'veTable1Tbl' }),
      ),
    );

    const select = await primarySelect();
    // The select must NOT be disabled while a session runs — that lock is
    // what made "selecting the second fuel table" impossible mid-session.
    await waitFor(() => expect(select.disabled).toBe(false));
    await userEvent.selectOptions(select, 'veTable2Tbl');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        'start_autotune',
        expect.objectContaining({ tableName: 'veTable2Tbl' }),
      ),
    );
  });

  it('raises the unsent-recommendations guard when a table leaves the session', async () => {
    mockInvokeWith({
      sessionStatus: { running: true, tableName: 'veTable1Tbl', secondaryTableName: 'veTable2Tbl' },
      recommendations: [{ cell_x: 0, cell_y: 0 }],
    });
    render(
      <ToastProvider>
        <AutoTune tableName="veTable1Tbl" isConnected />
      </ToastProvider>,
    );

    const checkbox = await secondaryCheckbox();
    await waitFor(() => expect(checkbox.checked).toBe(true));
    // The guard consults the backend for unsent recommendations first.
    await userEvent.click(checkbox);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_autotune_recommendations', {
        tableName: 'veTable2Tbl',
      }),
    );
    expect(
      await screen.findByText('Unsent AutoTune recommendations'),
    ).toBeInTheDocument();

    // Discard & Switch: the checkbox clears and the session is re-configured
    // without the secondary.
    await userEvent.click(screen.getByRole('button', { name: /discard & switch/i }));
    await waitFor(() => expect(checkbox.checked).toBe(false));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        'start_autotune',
        expect.objectContaining({ secondaryTableName: null }),
      ),
    );
  });

  it('switches immediately when the leaving table has nothing unsent', async () => {
    mockInvokeWith({
      sessionStatus: { running: true, tableName: 'veTable1Tbl', secondaryTableName: null },
      recommendations: [],
    });
    render(
      <ToastProvider>
        <AutoTune tableName="veTable1Tbl" isConnected />
      </ToastProvider>,
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        'start_autotune',
        expect.objectContaining({ tableName: 'veTable1Tbl' }),
      ),
    );

    // No dialog for a table with no recommendations.
    fireEvent.change(await primarySelect(), { target: { value: 'veTable2Tbl' } });
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        'start_autotune',
        expect.objectContaining({ tableName: 'veTable2Tbl' }),
      ),
    );
    expect(screen.queryByText('Unsent AutoTune recommendations')).not.toBeInTheDocument();
  });
});
