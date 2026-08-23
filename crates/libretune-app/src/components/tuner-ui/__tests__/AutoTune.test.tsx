import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { ToastProvider } from '../../../contexts/ToastContext';
import { AutoTune } from '../AutoTune';

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

function mockInvoke() {
  (invoke as unknown as any).mockImplementation((cmd: string) => {
    if (cmd === 'get_ve_analyze_config') return Promise.resolve(null);
    if (cmd === 'get_tables') return Promise.resolve([{ name: 'veTable1Tbl', title: 'VE Table' }]);
    if (cmd === 'get_table_data') return Promise.resolve(TABLE_DATA);
    if (cmd === 'get_available_channels') return Promise.resolve([]);
    return Promise.resolve();
  });
}

// Regression test: AutoTune previously had no isConnected awareness at all
// (TabContentRouter.tsx never passed it, unlike every other connection-aware
// component). Clicking Start while disconnected silently called
// start_autotune anyway -- it "succeeded" but no live data ever streamed in,
// so nothing visible ever happened, matching GitHub issue #132 ("when I hit
// Start, nothing happens -- it is as if it isn't connected").
describe('AutoTune connection awareness', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockInvoke();
  });

  it('disables Start and shows a DISCONNECTED badge when not connected', async () => {
    render(
      <ToastProvider>
        <AutoTune tableName="veTable1Tbl" isConnected={false} />
      </ToastProvider>
    );

    await waitFor(() => expect(screen.getByText('DISCONNECTED')).toBeInTheDocument());

    const startBtn = screen.getByRole('button', { name: /start/i });
    expect(startBtn).toBeDisabled();

    (invoke as unknown as any).mockClear();
    await userEvent.click(startBtn);
    expect(invoke).not.toHaveBeenCalledWith('start_autotune', expect.anything());
  });

  // Start no longer launches the session directly: it runs the preflight
  // check first, because AutoTune will otherwise run a whole drive against a
  // missing target table or a filter that rejects every sample and say nothing.
  it('enables Start and runs the preflight check when connected', async () => {
    render(
      <ToastProvider>
        <AutoTune tableName="veTable1Tbl" isConnected={true} />
      </ToastProvider>
    );

    await waitFor(() => expect(screen.queryByText('DISCONNECTED')).not.toBeInTheDocument());

    const startBtn = screen.getByRole('button', { name: /start/i });
    expect(startBtn).not.toBeDisabled();

    await userEvent.click(startBtn);
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        'preflight_autotune',
        expect.objectContaining({ tableName: 'veTable1Tbl' })
      )
    );
    // and NOT straight into the session
    expect(invoke).not.toHaveBeenCalledWith('start_autotune', expect.anything());
  });
});

// Speeduino names its VE load-axis output channel `fuelLoad` regardless of
// the fuel algorithm, so channel-name detection can never fire on a real
// Speeduino tune. The `algorithm` constant must fill the gap: 1 = TPS /
// Alpha-N (issue #132 — an ITB user's tune silently stayed on the MAP load
// source and AutoTune attributed samples to the wrong cells).
describe('AutoTune Alpha-N detection via fuel algorithm', () => {
  const loadSourceSelect = async () => {
    const label = await screen.findByText('Load Source:');
    const select = label.parentElement?.querySelector('select');
    if (!select) throw new Error('load source select not found next to its label');
    return select as HTMLSelectElement;
  };

  beforeEach(() => {
    vi.resetAllMocks();
    (invoke as unknown as any).mockImplementation((cmd: string, args: any) => {
      if (cmd === 'get_ve_analyze_config') return Promise.resolve(null);
      if (cmd === 'get_tables')
        return Promise.resolve([{ name: 'veTable1Tbl', title: 'VE Table' }]);
      if (cmd === 'get_table_data')
        return Promise.resolve({ ...TABLE_DATA, y_output_channel: 'fuelLoad' });
      if (cmd === 'get_available_channels') return Promise.resolve([]);
      if (cmd === 'get_constant_value' && args?.name === 'algorithm')
        return Promise.resolve(1);
      return Promise.resolve();
    });
  });

  it('selects the TPS load source when the algorithm constant is Alpha-N', async () => {
    render(
      <ToastProvider>
        <AutoTune tableName="veTable1Tbl" isConnected />
      </ToastProvider>
    );

    await waitFor(() =>
      expect(screen.getByText(/Fuel algorithm is TPS/)).toBeInTheDocument()
    );
    expect((await loadSourceSelect()).value).toBe('tps');
  });

  it('stays on MAP when the algorithm constant is speed density', async () => {
    (invoke as unknown as any).mockImplementation((cmd: string, args: any) => {
      if (cmd === 'get_ve_analyze_config') return Promise.resolve(null);
      if (cmd === 'get_tables')
        return Promise.resolve([{ name: 'veTable1Tbl', title: 'VE Table' }]);
      if (cmd === 'get_table_data')
        return Promise.resolve({ ...TABLE_DATA, y_output_channel: 'fuelLoad' });
      if (cmd === 'get_available_channels') return Promise.resolve([]);
      if (cmd === 'get_constant_value' && args?.name === 'algorithm')
        return Promise.resolve(0);
      return Promise.resolve();
    });

    render(
      <ToastProvider>
        <AutoTune tableName="veTable1Tbl" isConnected />
      </ToastProvider>
    );

    // Give the async detection a chance to (wrongly) fire, then confirm it
    // did not.
    await waitFor(() =>
      expect((invoke as unknown as any).mock.calls).toContainEqual(
        ['get_constant_value', { name: 'algorithm' }],
      ),
    );
    expect((await loadSourceSelect()).value).toBe('map');
  });

  it('does not override a manual load source choice', async () => {
    // A MAF channel must exist, or the (pre-existing) MAF-verify effect would
    // demote the manual choice to MAP — a different behavior than the one
    // under test here.
    (invoke as unknown as any).mockImplementation((cmd: string, args: any) => {
      if (cmd === 'get_ve_analyze_config') return Promise.resolve(null);
      if (cmd === 'get_tables')
        return Promise.resolve([{ name: 'veTable1Tbl', title: 'VE Table' }]);
      if (cmd === 'get_table_data')
        return Promise.resolve({ ...TABLE_DATA, y_output_channel: 'fuelLoad' });
      if (cmd === 'get_available_channels')
        return Promise.resolve([{ name: 'maf', label: 'Mass Air Flow' }]);
      if (cmd === 'get_constant_value' && args?.name === 'algorithm')
        return Promise.resolve(1);
      return Promise.resolve();
    });

    render(
      <ToastProvider>
        <AutoTune tableName="veTable1Tbl" isConnected />
      </ToastProvider>
    );

    const select = await loadSourceSelect();
    // Manually pick MAF; algorithm-based detection must respect that.
    await userEvent.selectOptions(select, 'maf');
    expect(select.value).toBe('maf');

    await new Promise((r) => setTimeout(r, 50));
    expect(select.value).toBe('maf');
    expect(screen.queryByText(/Fuel algorithm is TPS/)).not.toBeInTheDocument();
  });
});

// The lambda delay is a measured per-engine fact (about 470 ms at idle on the
// reference NA6), not view state. It used to reset on every launch, and the
// default of 0 does not mean "no delay" - it means "fall back to the built-in
// RPM curve", which caps at 200 ms. A whole 59-minute drive was tuned against
// that fallback because the setting had silently reverted, inflating every
// low-load correction.
//
// Queried by row rather than by label: the settings rows render <label> and
// <input> as siblings with no htmlFor, so getByLabelText cannot resolve them.
describe('AutoTune settings persistence', () => {
  const delayInput = async () => {
    const label = await screen.findByText(/(Lambda|Idle) Delay \(ms\):/);
    const input = label.parentElement?.querySelector('input[type="number"]');
    if (!input) throw new Error('delay input not found next to its label');
    return input as HTMLInputElement;
  };

  beforeEach(() => {
    localStorage.clear();
    mockInvoke();
  });

  it('restores a measured lambda delay across a remount', async () => {
    const { unmount } = render(
      <ToastProvider><AutoTune isConnected onClose={() => {}} /></ToastProvider>,
    );

    const delay = await delayInput();
    await userEvent.clear(delay);
    await userEvent.type(delay, '470');
    await waitFor(() =>
      expect(localStorage.getItem('libretune.autotune.settings.v1.settings'))
        .toContain('470'),
    );

    unmount();
    render(<ToastProvider><AutoTune isConnected onClose={() => {}} /></ToastProvider>);
    expect((await delayInput()).value).toBe('470');
  });

  it('falls back to defaults when stored state is corrupt', async () => {
    localStorage.setItem('libretune.autotune.settings.v1.settings', '{not json');
    render(<ToastProvider><AutoTune isConnected onClose={() => {}} /></ToastProvider>);
    expect((await delayInput()).value).toBe('0');
  });
});
