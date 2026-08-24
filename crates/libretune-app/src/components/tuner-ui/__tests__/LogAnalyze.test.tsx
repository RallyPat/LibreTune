import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { LogAnalyze } from '../LogAnalyze';

const TABLE_DATA = {
  name: 'veTable1Tbl',
  title: 'VE Table',
  x_bins: [1000, 2000],
  y_bins: [20, 80],
  z_values: [
    [50, 60],
    [70, 80],
  ],
  x_axis_name: 'rpmBins',
  y_axis_name: 'loadBins',
};

const LOG = [
  'Time,rpm,map,afr,veCurr,coolant,tps,TPSdot,DFCOOn',
  ...Array.from({ length: 40 }, (_, i) => `${i * 0.1},2000,80,15.2,70,90,20,0,0`),
].join('\n');

/** A report shaped like the backend's, with the parts the view must surface. */
function report(over: Record<string, unknown> = {}) {
  return {
    cells: [
      {
        x: 1, y: 1, rpm: 2000, load: 80,
        current_ve: 80, proposed_ve: 84, delta: 4,
        hits: 30, weight: 30, confidence: 1,
        target_afr: 14.0, mean_afr: 15.2,
      },
    ],
    total_samples: 30,
    rejections: [['clt below min_clt', 7]],
    verdicts: Array.from({ length: 40 }, (_, i) => ({
      rejected_because: i < 7 ? 'clt below min_clt' : null,
      cell: i < 7 ? null : [1, 1],
    })),
    validation: { gain_pct: 22.5, worsened_pct: 6, scored: 120, folds: 5 },
    coverage: [[0, 0], [0, 30]],
    ...over,
  };
}

function mockInvoke(rep: unknown = report()) {
  (invoke as unknown as any).mockImplementation((cmd: string) => {
    if (cmd === 'list_tunable_tables') return Promise.resolve(['veTable1Tbl']);
    if (cmd === 'get_table_data') return Promise.resolve(TABLE_DATA);
    if (cmd === 'read_file_contents') return Promise.resolve(LOG);
    if (cmd === 'analyse_log') return Promise.resolve(rep);
    if (cmd === 'update_table_data') return Promise.resolve();
    return Promise.resolve();
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  (open as unknown as any).mockResolvedValue('C:/logs/drive.csv');
});

async function loadAndAnalyse() {
  const user = userEvent.setup();
  render(<LogAnalyze isConnected />);
  await waitFor(() => expect(screen.getByText('veTable1Tbl')).toBeInTheDocument());
  await user.click(screen.getByRole('button', { name: /Open log/i }));
  await waitFor(() => expect(screen.getByText(/40 samples/)).toBeInTheDocument());
  await user.click(screen.getByRole('button', { name: /Analyse/i }));
  return user;
}

test('a log is read, analysed, and the proposal shown', async () => {
  mockInvoke();
  await loadAndAnalyse();

  await waitFor(() => {
    const call = (invoke as unknown as any).mock.calls.find((c: any[]) => c[0] === 'analyse_log');
    expect(call).toBeTruthy();
    // The channels the backend needs must actually be extracted from the log,
    // not sent as empty arrays that would silently analyse nothing.
    expect(call[1].log.rpm).toHaveLength(40);
    expect(call[1].log.afr[0]).toBe(15.2);
    // Timestamps are re-based to zero so validation blocks divide sensibly.
    expect(call[1].log.time_ms[0]).toBe(0);
  });
});

/**
 * The held-out score is the only figure that separates a good proposal from a
 * large one, so a losing configuration has to say so rather than presenting its
 * changes with the same confidence.
 */
test('a proposal that fails validation says so', async () => {
  mockInvoke(report({ validation: { gain_pct: -18.4, worsened_pct: 44, scored: 300, folds: 5 } }));
  await loadAndAnalyse();

  await waitFor(() => {
    expect(screen.getByText('-18.4%')).toBeInTheDocument();
    expect(screen.getByText(/fitting noise/i)).toBeInTheDocument();
  });
});

test('a winning proposal reports the gain', async () => {
  mockInvoke();
  await loadAndAnalyse();
  await waitFor(() => {
    expect(screen.getByText('+22.5%')).toBeInTheDocument();
    expect(screen.getByText(/closer to target AFR/i)).toBeInTheDocument();
  });
});

/**
 * A session that collects nothing looks identical to a broken one unless the
 * filter that ate the samples is named.
 */
test('rejected samples are attributed to the filter that refused them', async () => {
  mockInvoke();
  await loadAndAnalyse();
  await waitFor(() => {
    expect(screen.getByText('clt below min_clt')).toBeInTheDocument();
  });
});

/**
 * Selecting a cell is how a number gets explained: 4 VE on its own says
 * nothing about whether 30 samples or 3 stood behind it.
 */
test('selecting a cell shows what the recommendation rests on', async () => {
  mockInvoke();
  const user = await loadAndAnalyse();
  await waitFor(() => expect(screen.getByText('+22.5%')).toBeInTheDocument());

  // The proposed value carries its own change, so the cell is findable by it.
  const cell = await screen.findByTitle(/30 samples/);
  await user.click(cell);

  expect(await screen.findByText('2000 rpm, load 80')).toBeInTheDocument();
  expect(screen.getByText('15.20')).toBeInTheDocument(); // measured
  expect(screen.getByText('14.00')).toBeInTheDocument(); // target
});

/** An empty cell has nothing to explain and must not invent a panel. */
test('selecting a cell with no data shows no detail', async () => {
  mockInvoke();
  const user = await loadAndAnalyse();
  await waitFor(() => expect(screen.getByText('+22.5%')).toBeInTheDocument());

  const empty = screen.getAllByTitle('no data')[0];
  await user.click(empty);
  expect(screen.queryByText(/rpm, load/)).not.toBeInTheDocument();
});

test('applying writes only the changed cells back to the table', async () => {
  mockInvoke();
  const user = await loadAndAnalyse();

  await waitFor(() => expect(screen.getByRole('button', { name: /Apply 1 cells/i })).toBeEnabled());
  await user.click(screen.getByRole('button', { name: /Apply/i }));

  await waitFor(() => {
    const call = (invoke as unknown as any).mock.calls.find(
      (c: any[]) => c[0] === 'update_table_data',
    );
    expect(call).toBeTruthy();
    // Only (1,1) moves; the rest of the table is passed through untouched.
    expect(call[1].zValues).toEqual([
      [50, 60],
      [70, 84],
    ]);
  });
});

/**
 * Without rpm, load and AFR nothing can be placed in a cell. Analysing anyway
 * would produce an empty result that reads as "your tune is perfect".
 */
test('a log missing a required channel refuses to analyse and says which', async () => {
  (invoke as unknown as any).mockImplementation((cmd: string) => {
    if (cmd === 'list_tunable_tables') return Promise.resolve(['veTable1Tbl']);
    if (cmd === 'get_table_data') return Promise.resolve(TABLE_DATA);
    if (cmd === 'read_file_contents') {
      return Promise.resolve('Time,rpm,coolant\n0.1,2000,90\n0.2,2000,90\n');
    }
    return Promise.resolve();
  });
  const user = userEvent.setup();
  render(<LogAnalyze isConnected />);
  await waitFor(() => expect(screen.getByText('veTable1Tbl')).toBeInTheDocument());
  await user.click(screen.getByRole('button', { name: /Open log/i }));

  await waitFor(() => {
    expect(screen.getByText(/no load, afr channel/i)).toBeInTheDocument();
  });
  expect(screen.getByRole('button', { name: /Analyse/i })).toBeDisabled();
});

/**
 * Traces is the plain "look at the log" view, so it must not depend on having
 * run an analysis — but it has nothing to draw before a log is opened.
 */
test('the Traces tab is offered only once a log is loaded', async () => {
  mockInvoke();
  const user = userEvent.setup();
  render(<LogAnalyze isConnected />);
  await waitFor(() => expect(screen.getByText('veTable1Tbl')).toBeInTheDocument());

  expect(screen.getByRole('tab', { name: 'Traces' })).toBeDisabled();

  await user.click(screen.getByRole('button', { name: /Open log/i }));
  await waitFor(() => expect(screen.getByRole('tab', { name: 'Traces' })).toBeEnabled());

  // Switching away from Analyse must not require an analysis to have run.
  await user.click(screen.getByRole('tab', { name: 'Traces' }));
  expect(screen.getByRole('tab', { name: 'Traces' })).toHaveAttribute('aria-selected', 'true');
  expect(screen.queryByText(/blue adds fuel/)).not.toBeInTheDocument();

  await user.click(screen.getByRole('tab', { name: 'Analyse' }));
  expect(await screen.findByText(/blue adds fuel/)).toBeInTheDocument();
});
