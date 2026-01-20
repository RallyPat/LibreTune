import { vi, expect } from 'vitest';
import { setupTauriMocks, tearDownTauriMocks } from '../tauriMocks';
import { waitFor } from '@testing-library/react';

describe('setupTauriMocks', () => {
  afterEach(() => {
    tearDownTauriMocks();
  });

  it('delivers queued events emitted before listen registration', async () => {
    const handle = setupTauriMocks();

    const payload = { tx_bps: 1234, rx_bps: 5678, timestamp_ms: Date.now() };

    // Emit BEFORE any listener is registered
    handle.emit('connection:metrics', payload);

    // Queued events should be recorded
    expect(handle.getQueued && handle.getQueued('connection:metrics')).toEqual([payload]);

    // Now register a listener via the handle.listen helper and let it auto-drain
    const handler = vi.fn();
    const unlisten = await handle.listen?.('connection:metrics', handler as any);

    // Handler should have been called once with an event-like object containing payload
    await waitFor(() => expect(handler).toHaveBeenCalledTimes(1));
    expect(handler).toHaveBeenCalledWith(expect.objectContaining({ payload }));

    // Clean up
    if (typeof unlisten === 'function') unlisten();
  });

  it('does not deliver to listener after unlisten is called', async () => {
    const handle = setupTauriMocks();

    const payload1 = { tx_bps: 10, rx_bps: 20, timestamp_ms: Date.now() };
    const payload2 = { tx_bps: 30, rx_bps: 40, timestamp_ms: Date.now() };

    const handler = vi.fn();
    const unlisten = await handle.listen?.('connection:metrics', handler as any);

    // Emit first payload - should be delivered
    handle.emit('connection:metrics', payload1);
    await waitFor(() => expect(handler).toHaveBeenCalledWith(expect.objectContaining({ payload: payload1 })));

    // Unregister and emit again
    if (typeof unlisten === 'function') unlisten();
    handle.emit('connection:metrics', payload2);

    // Handler should not have been called with payload2
    await new Promise((r) => setTimeout(r, 0));
    expect(handler).not.toHaveBeenCalledWith(expect.objectContaining({ payload: payload2 }));
  });
});