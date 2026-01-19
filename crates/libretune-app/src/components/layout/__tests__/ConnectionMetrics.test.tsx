import { render, screen } from '@testing-library/react';
import { vi } from 'vitest';
import ConnectionMetrics from '../ConnectionMetrics';
import * as ev from '@tauri-apps/api/event';

describe('ConnectionMetrics', () => {
  it('renders and updates on event', async () => {
    // Mock listen to immediately call handler with a payload
    const listenMock = vi.spyOn(ev, 'listen').mockImplementation(async (event, handler) => {
      // Simulate initial payload
      const fakeEvent = { payload: { tx_bps: 1024, rx_bps: 2048, tx_pkts_s: 2, rx_pkts_s: 4, tx_total: 1000, rx_total: 2000, timestamp_ms: Date.now() } } as any;
      handler(fakeEvent);
      return () => {};
    });

    render(<ConnectionMetrics />);

    expect(await screen.findByText(/Rx:/)).toBeInTheDocument();

    listenMock.mockRestore();
  });
});
