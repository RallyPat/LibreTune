import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { vi } from 'vitest';

// Mock Tauri APIs before importing App
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));


import { LoadingProvider } from '../components/LoadingContext';
import { ToastProvider } from '../components/ToastContext';
import { UnitPreferencesProvider } from '../utils/useUnitPreferences';
import { invoke } from '@tauri-apps/api/core';
// Note: `App` is imported dynamically inside tests after test-level mocks are installed
// so that module-level imports (e.g., `listen`) pick up our patched implementations.

import { setupTauriMocks, tearDownTauriMocks } from '../test-utils/tauriMocks';

describe('App integration (toolbar connection-info)', () => {

  beforeEach(() => {
    vi.resetAllMocks();
    setupTauriMocks({
      // sensible defaults for App.initializeApp
      init_ini_repository: undefined,
      list_repository_inis: [],
      list_projects: [],
      get_settings: { runtime_packet_mode: 'Auto', units_system: 'metric' },
      get_current_project: null,
      get_serial_ports: [],
      get_connection_status: { state: 'Connected', has_definition: true, signature: 'TEST', ini_name: 'test.ini' },
      get_protocol_defaults: { default_baud_rate: 115200, timeout_ms: 2000 },
      get_status_bar_defaults: [],
      get_available_channels: [],
      get_menu_tree: [],
      get_searchable_index: {},
    });
  });

  afterEach(() => {
    tearDownTauriMocks();
  });

  it('shows packet mode and receives metrics when connected', async () => {
    (invoke as unknown as any).mockImplementation((cmd: string) => {
      // Provide reasonable defaults for initialization & common commands
      switch (cmd) {
        case 'init_ini_repository':
          return Promise.resolve();
        case 'list_repository_inis':
          return Promise.resolve([]);
        case 'list_projects':
          return Promise.resolve([]);
        case 'get_settings':
          return Promise.resolve({ runtime_packet_mode: 'Auto', units_system: 'metric' });
        case 'get_current_project':
          return Promise.resolve(null);
        case 'get_serial_ports':
          return Promise.resolve([]);
        case 'get_connection_status':
          return Promise.resolve({ state: 'Connected', has_definition: true, signature: 'TEST', ini_name: 'test.ini' });
        case 'get_protocol_defaults':
          return Promise.resolve({ default_baud_rate: 115200, timeout_ms: 2000 });
        case 'get_status_bar_defaults':
          return Promise.resolve([]);
        case 'get_available_channels':
          return Promise.resolve([]);
        case 'get_menu_tree':
          return Promise.resolve([]);
        case 'get_searchable_index':
          return Promise.resolve({});
        // Default stub for any other backend calls used during mount
        default:
          return Promise.resolve();
      }
    });

    const { default: App } = await import('../App');

    // Spy on the event.listen used by ConnectionMetrics and capture the handler so
    // we can invoke it deterministically after mount. This avoids race conditions with
    // async module-level listen registration and ensures the metrics update is observed.
    const payload = { tx_bps: 2048, rx_bps: 1024, tx_pkts_s: 3, rx_pkts_s: 4, tx_total: 100, rx_total: 200, timestamp_ms: Date.now() };

    // Spy on the event.listen used by ConnectionMetrics and call the handler immediately
    const ev = await import('@tauri-apps/api/event');
    const listenMock = vi.spyOn(ev, 'listen').mockImplementation(async (name, handler) => {
      if (name === 'connection:metrics') {
        handler({ event: name, id: '1', payload } as any);
      }
      return () => {};
    });

    const { container } = render(
      <LoadingProvider>
        <ToastProvider>
          <UnitPreferencesProvider>
            <App />
          </UnitPreferencesProvider>
        </ToastProvider>
      </LoadingProvider>
    );

    // Packet mode label should show 'Auto' for connected state (wait for mount to stabilize)
    await waitFor(() => expect(screen.getByText('Auto')).toBeInTheDocument());

    // After our synthetic metrics event arrives (via the spy), the metrics element should show a unit like kB/s or MB/s
    await waitFor(() => {
      const text = container.querySelector('.conn-metrics')?.textContent || '';
      expect(/B\/s|kB\/s|MB\/s/.test(text)).toBe(true);
    });

    listenMock.mockRestore();

    // Connection metrics placeholder should be present initially and then update after event
    const metricsEl = container.querySelector('.conn-metrics');
    expect(metricsEl).toBeTruthy();

    // After our synthetic metrics event arrives, the metrics element should show a unit like kB/s or MB/s
    await waitFor(() => {
      const text = container.querySelector('.conn-metrics')?.textContent || '';
      expect(/B\/s|kB\/s|MB\/s/.test(text)).toBe(true);
    });
  });

  it('shows placeholder packet mode when disconnected', async () => {
    (invoke as unknown as any).mockImplementation((cmd: string) => {
      if (cmd === 'get_connection_status') return Promise.resolve({ state: 'Disconnected', has_definition: false });
      return Promise.resolve();
    });

    const { default: App } = await import('../App');

    const { container } = render(
      <LoadingProvider>
        <ToastProvider>
          <UnitPreferencesProvider>
            <App />
          </UnitPreferencesProvider>
        </ToastProvider>
      </LoadingProvider>
    );

    // Packet mode should show placeholder '—'
    await waitFor(() => expect(container.querySelector('.packet-mode')?.textContent).toBe('—'));
  });

  it('proceeds to sync on partial signature mismatch (advisory)', async () => {
    let syncCalled = false;

    (invoke as unknown as any).mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'get_settings':
          return Promise.resolve({ runtime_packet_mode: 'Auto', units_system: 'metric' });
        case 'get_current_project':
          return Promise.resolve(null);
        case 'get_serial_ports':
          return Promise.resolve(['/dev/ttyUSB0']);
        case 'connect_to_ecu':
          return Promise.resolve({
            signature: 'Speeduino 2023-05',
            mismatch_info: {
              ecu_signature: 'Speeduino 2023-05',
              ini_signature: 'Speeduino 2023-04',
              match_type: 'partial',
              current_ini_path: 'test.ini',
              matching_inis: [],
            },
          });
        case 'get_connection_status':
          return Promise.resolve({ state: 'Connected', has_definition: true, signature: 'Speeduino 2023-05', ini_name: 'test.ini' });
        case 'sync_ecu_data':
          syncCalled = true;
          return Promise.resolve({ success: true, pages_synced: 1, pages_failed: 0, total_pages: 1, errors: [] });
        default:
          return Promise.resolve();
      }
    });

    const { default: App } = await import('../App');

    const { container } = render(
      <LoadingProvider>
        <ToastProvider>
          <UnitPreferencesProvider>
            <App />
          </UnitPreferencesProvider>
        </ToastProvider>
      </LoadingProvider>
    );

    // Open connection dialog via toolbar connect button
    const connectBtn = await screen.findByTitle('Connect to ECU');
    expect(connectBtn).toBeTruthy();
    connectBtn.click();

    // Select port in dialog and click Connect
    // Refresh ports first (dialog shows 'No ports found' initially from global setup)
    const refreshBtn = await screen.findByText('🔄 Refresh');
    refreshBtn.click();

    // Wait for the select to contain the refreshed port and choose it
    await waitFor(() => {
      const selects = container.querySelectorAll('select');
      const select = Array.from(selects).find(s => Array.from(s.options).some(opt => opt.value === '/dev/ttyUSB0')) as HTMLSelectElement | undefined;
      expect(select).toBeTruthy();
      fireEvent.change(select!, { target: { value: '/dev/ttyUSB0' } });
    });

    const dialogConnect = await screen.findByText('Connect');
    dialogConnect.click();

    // Wait for sync to be called due to advisory partial mismatch
    await waitFor(() => expect(syncCalled).toBe(true));

    // A warning toast should have been shown informing the user about the partial match
    await waitFor(() => {
      const toastMsg = container.querySelector('.toast-message')?.textContent || '';
      if (!toastMsg.toLowerCase().includes('partially matches')) throw new Error('warning toast not found');
    });

    // Ensure the signature mismatch dialog did NOT open (partial is advisory)
    expect(screen.queryByText('INI Signature Mismatch')).toBeNull();
  });

  it('auto-selects runtime packet mode (Auto → ForceOCH when INI supports OCH)', async () => {
    let connectArgs: any = null;

    (invoke as unknown as any).mockImplementation((cmd: string, args?: any) => {
      switch (cmd) {
        case 'get_settings':
          return Promise.resolve({ runtime_packet_mode: 'Auto', units_system: 'metric' });
        case 'get_serial_ports':
          return Promise.resolve(['/dev/ttyUSB0']);
        case 'get_protocol_defaults':
          return Promise.resolve({ default_baud_rate: 115200, timeout_ms: 2000 });
        case 'get_protocol_capabilities':
          // Simulate INI that supports OCH
          return Promise.resolve({ supports_och: true });
        case 'connect_to_ecu':
          connectArgs = args;
          return Promise.resolve({ signature: 'rusEFI v1.2.3', mismatch_info: null });
        case 'get_connection_status':
          return Promise.resolve({ state: 'Connected', has_definition: true, signature: 'rusEFI v1.2.3', ini_name: 'test.ini' });
        default:
          return Promise.resolve();
      }
    });

    const { default: App } = await import('../App');

    const { container } = render(
      <LoadingProvider>
        <ToastProvider>
          <UnitPreferencesProvider>
            <App />
          </UnitPreferencesProvider>
        </ToastProvider>
      </LoadingProvider>
    );

    // Open connection dialog and connect with the refreshed port
    const connectBtn = await screen.findByTitle('Connect to ECU');
    expect(connectBtn).toBeTruthy();
    connectBtn.click();

    const refreshBtn = await screen.findByText('🔄 Refresh');
    refreshBtn.click();

    await waitFor(() => {
      const selects = container.querySelectorAll('select');
      const select = Array.from(selects).find(s => Array.from(s.options).some(opt => opt.value === '/dev/ttyUSB0')) as HTMLSelectElement | undefined;
      expect(select).toBeTruthy();
      fireEvent.change(select!, { target: { value: '/dev/ttyUSB0' } });
    });

    const dialogConnect = await screen.findByText('Connect');
    dialogConnect.click();

    // Wait for connect_to_ecu to be called and assert runtimePacketMode chosen
    await waitFor(() => expect(connectArgs).not.toBeNull());
    expect(connectArgs.runtimePacketMode).toBe('ForceOCH');

  });
});
