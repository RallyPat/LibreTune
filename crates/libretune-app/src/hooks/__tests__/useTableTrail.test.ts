import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useTableTrail } from '../useTableTrail';

describe('useTableTrail', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
  });

  afterEach(() => {
    act(() => {
      vi.runOnlyPendingTimers();
    });
    vi.useRealTimers();
  });

  it('appends a point per cell change and dedupes same-cell repeats', async () => {
    const { result, rerender } = renderHook(
      ({ cell }: { cell: { row: number; col: number } | null }) =>
        useTableTrail(cell, 8),
      { initialProps: { cell: { row: 1, col: 1 } } },
    );
    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.trail).toHaveLength(1);
    expect(result.current.trail[0]).toMatchObject({ row: 1, col: 1 });

    // A NEW object for the SAME cell (what a realtime tick produces) must not
    // append a duplicate.
    await act(async () => {
      rerender({ cell: { row: 1, col: 1 } });
      await Promise.resolve();
    });
    expect(result.current.trail).toHaveLength(1);

    await act(async () => {
      rerender({ cell: { row: 2, col: 0 } });
      await Promise.resolve();
    });
    expect(result.current.trail).toHaveLength(2);
    expect(result.current.trail[1]).toMatchObject({ row: 2, col: 0 });
  });

  it('expires entries once the fade lifetime passes', async () => {
    const { result, rerender } = renderHook(
      ({ cell }: { cell: { row: number; col: number } | null }) =>
        useTableTrail(cell, 8),
      { initialProps: { cell: { row: 0, col: 0 } } },
    );
    await act(async () => {
      await Promise.resolve();
    });

    // t=3s: move to a new cell. The first point is 3 s old — still alive.
    act(() => {
      vi.setSystemTime(3000);
      rerender({ cell: { row: 1, col: 0 } });
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.trail).toHaveLength(2);

    // t=9s: the first point (t=0) is past the 8 s lifetime and the ~5 Hz
    // cleanup tick has run.
    act(() => {
      vi.setSystemTime(9000);
      vi.advanceTimersByTime(250);
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.trail).toHaveLength(1);
    expect(result.current.trail[0]).toMatchObject({ row: 1, col: 0 });
  });

  it('keeps the trail while the live cell is briefly unknown', async () => {
    const { result, rerender } = renderHook(
      ({ cell }: { cell: { row: number; col: number } | null }) =>
        useTableTrail(cell, 8),
      {
        initialProps: {
          cell: { row: 0, col: 0 },
        } as { cell: { row: number; col: number } | null },
      },
    );
    await act(async () => {
      await Promise.resolve();
    });

    // A dropped reading (null) must not wipe history — points simply age out.
    await act(async () => {
      rerender({ cell: null });
      await Promise.resolve();
    });
    expect(result.current.trail).toHaveLength(1);

    // Returning to the cell that is still the newest entry does not duplicate
    // it (same dedupe as the editors' trails); a *different* cell appends.
    await act(async () => {
      rerender({ cell: { row: 0, col: 0 } });
      await Promise.resolve();
    });
    expect(result.current.trail).toHaveLength(1);
    await act(async () => {
      rerender({ cell: { row: 1, col: 1 } });
      await Promise.resolve();
    });
    expect(result.current.trail).toHaveLength(2);
  });

  it('drops the trail when disabled, matching the editors\' toggle-off', async () => {
    const { result, rerender } = renderHook(
      ({ cell, enabled }: { cell: { row: number; col: number } | null; enabled: boolean }) =>
        useTableTrail(cell, 8, enabled),
      { initialProps: { cell: { row: 0, col: 0 }, enabled: true } },
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.trail).toHaveLength(1);

    await act(async () => {
      rerender({ cell: { row: 0, col: 0 }, enabled: false });
      await Promise.resolve();
    });
    expect(result.current.trail).toHaveLength(0);
  });
});
