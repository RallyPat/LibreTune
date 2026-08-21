import { render, act } from '@testing-library/react';
import { Profiler } from 'react';
import { vi, describe, it, expect, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { TableEditor } from '../TableEditor';
import { useRealtimeStore } from '../../../stores/realtimeStore';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }));
// three.js is heavy and never mounted here (the 3D view stays off).
vi.mock('../../tables/TableEditor3D', () => ({ default: () => null }));

const invokeMock = vi.mocked(invoke);

const DATA = {
  name: 'veTable1Tbl',
  xAxis: [500, 1000, 1500],
  yAxis: [20, 40, 60],
  zValues: [
    [10, 11, 12],
    [20, 21, 22],
    [30, 31, 32],
  ],
  xOutputChannel: 'RPM',
  yOutputChannel: 'MAP',
};

const COLS = 3;

function cellAt(container: HTMLElement, row: number, col: number): HTMLElement {
  const cells = container.querySelectorAll('td.table-cell');
  return cells[row * COLS + col] as HTMLElement;
}

function trailOpacity(cell: HTMLElement): number {
  return parseFloat(cell.style.getPropertyValue('--trail-opacity'));
}

/**
 * Regression tests for the live-data render storm that made an open table
 * freeze the window with a stream running (issue #132):
 *
 * 1. The live position object must keep a stable identity while the cursor
 *    stays in the same cell — the raw memo produced a fresh `{row, col}` per
 *    realtime tick, re-firing the trail effect and queuing a second render
 *    on every tick (~40 renders/sec once combined with the trail interval).
 * 2. A re-entered cell must fade from its *newest* visit. The per-cell
 *    `Array.find` returned the oldest duplicate entry, so a freshly
 *    re-entered cell could render as (nearly) faded.
 */
describe('TableEditor live trail', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
    invokeMock.mockReset();
    invokeMock.mockImplementation(() => Promise.resolve({}));
    useRealtimeStore.getState().clearChannels();
  });

  afterEach(() => {
    act(() => {
      vi.runOnlyPendingTimers();
    });
    vi.useRealTimers();
  });

  it('fades a re-entered cell from its newest visit, not its oldest', async () => {
    const { container } = render(
      <TableEditor data={{ ...DATA }} onChange={() => {}} />
    );
    // Flush the async get_settings loads (trail fade defaults to 8 s).
    await act(async () => {
      await Promise.resolve();
    });

    // t=0: live in cell (1,1) — first visit. (MAP 45 is nearest bin 40;
    // avoid ties: 30 is equidistant between 20 and 40 and picks row 0.)
    act(() => {
      useRealtimeStore.getState().updateChannels({ RPM: 1200, MAP: 45 });
    });
    expect(cellAt(container, 1, 1).className).toContain('live');

    // t=3s: move to (2,2). Cell (1,1) becomes trail with opacity
    // 1 - 3000/8000 = 0.625.
    act(() => {
      vi.advanceTimersByTime(3000);
    });
    act(() => {
      useRealtimeStore.getState().updateChannels({ RPM: 2200, MAP: 55 });
    });

    const firstVisit = cellAt(container, 1, 1);
    expect(firstVisit.className).toContain('trail');
    expect(trailOpacity(firstVisit)).toBeCloseTo(0.625, 1);

    // Re-enter (1,1) and leave again immediately: the trail must reflect the
    // newest visit (opacity ≈ 1), not the t=0 one (0.625).
    act(() => {
      useRealtimeStore.getState().updateChannels({ RPM: 1200, MAP: 45 });
    });
    act(() => {
      useRealtimeStore.getState().updateChannels({ RPM: 2200, MAP: 55 });
    });

    const reentered = cellAt(container, 1, 1);
    expect(reentered.className).toContain('trail');
    expect(trailOpacity(reentered)).toBeGreaterThan(0.9);
  });

  it('does not queue an extra render when the live cell does not change', async () => {
    let commits = 0;
    const onRender = () => {
      commits += 1;
    };
    render(
      <Profiler id="editor" onRender={onRender}>
        <TableEditor data={{ ...DATA }} onChange={() => {}} />
      </Profiler>
    );
    await act(async () => {
      await Promise.resolve();
    });
    // First tick establishes the live position (and its first trail entry —
    // a legitimate extra commit).
    act(() => {
      useRealtimeStore.getState().updateChannels({ RPM: 1200, MAP: 45 });
    });
    const before = commits;

    // Two ticks with different values that bin into the SAME cell (1,1)
    // (RPM under 1250 keeps col 1; MAP 40-50 keeps row 1). Each store change
    // commits once; the trail effect must not add another (the old code
    // re-fired it per tick via a fresh position object and a new-array
    // filter result).
    act(() => {
      useRealtimeStore.getState().updateChannels({ RPM: 1150, MAP: 46 });
    });
    act(() => {
      useRealtimeStore.getState().updateChannels({ RPM: 1100, MAP: 44 });
    });

    expect(commits - before).toBe(2);
  });
});
