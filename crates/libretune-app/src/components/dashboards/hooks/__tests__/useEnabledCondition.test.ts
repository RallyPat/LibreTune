import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { useEnabledCondition } from '../useEnabledCondition';

const invokeMock = vi.mocked(invoke);

describe('useEnabledCondition', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(true);
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('returns true immediately (no gating) when expr is null/empty', () => {
    const { result } = renderHook(() => useEnabledCondition(null));
    expect(result.current).toBe(true);

    const { result: result2 } = renderHook(() => useEnabledCondition('   '));
    expect(result2.current).toBe(true);

    // Neither should have called the backend at all.
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('evaluates a gated expression via evaluate_expression', async () => {
    invokeMock.mockResolvedValueOnce(false);
    const { result } = renderHook(() => useEnabledCondition('hasLambdaSensor'));

    await vi.waitFor(() => expect(result.current).toBe(false));
    expect(invokeMock).toHaveBeenCalledWith('evaluate_expression', {
      expression: 'hasLambdaSensor',
      context: expect.anything(),
    });
  });

  it('falls back to true (permissive) when the backend call rejects', async () => {
    invokeMock.mockReset();
    invokeMock.mockRejectedValueOnce(new Error('not connected'));
    const { result } = renderHook(() => useEnabledCondition('rpm > 0'));

    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalled());
    expect(result.current).toBe(true);
  });

  it('shares one poll across multiple mounted components with the same expression, instead of one timer per instance', async () => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(true);

    const a = renderHook(() => useEnabledCondition('hasLambdaSensor'));
    const b = renderHook(() => useEnabledCondition('hasLambdaSensor'));
    const c = renderHook(() => useEnabledCondition('hasLambdaSensor'));

    // The initial (immediate, not waiting for a tick) evaluation is
    // deduplicated too: only the first subscriber for a brand-new expression
    // triggers an evaluation; the rest join in on its result.
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    invokeMock.mockClear();

    // Advance past one shared poll tick (250ms). If each instance still ran
    // its own timer, this would be 3 calls; centralized, it must be exactly 1
    // regardless of how many components share the expression.
    await vi.advanceTimersByTimeAsync(250);

    expect(invokeMock).toHaveBeenCalledTimes(1);

    a.unmount();
    b.unmount();
    c.unmount();
  });

  it('evaluates distinct expressions independently, one call each per tick', async () => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(true);

    const a = renderHook(() => useEnabledCondition('hasLambdaSensor'));
    const b = renderHook(() => useEnabledCondition('rpm > 0'));

    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));

    invokeMock.mockClear();
    await vi.advanceTimersByTimeAsync(250);

    const calledExpressions = invokeMock.mock.calls.map(([, args]) => (args as any).expression).sort();
    expect(calledExpressions).toEqual(['hasLambdaSensor', 'rpm > 0']);

    a.unmount();
    b.unmount();
  });

  it('stops the shared poll once every subscriber for an expression unmounts', async () => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(true);

    const a = renderHook(() => useEnabledCondition('hasLambdaSensor'));
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    a.unmount();
    invokeMock.mockClear();

    // No subscribers left for this expression (and none for any other in
    // this test), so the shared timer should have been torn down — nothing
    // should fire on the next tick.
    await vi.advanceTimersByTimeAsync(500);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('hands a late-joining subscriber the already-known result instead of the true default', async () => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValueOnce(false);

    const a = renderHook(() => useEnabledCondition('hasLambdaSensor'));
    await vi.waitFor(() => expect(a.result.current).toBe(false));

    // A second component mounts with the same already-resolved expression —
    // it should see the known `false` immediately, not the `true` default
    // while waiting for the next shared tick.
    invokeMock.mockResolvedValue(false);
    const b = renderHook(() => useEnabledCondition('hasLambdaSensor'));
    expect(b.result.current).toBe(false);

    a.unmount();
    b.unmount();
  });
});
