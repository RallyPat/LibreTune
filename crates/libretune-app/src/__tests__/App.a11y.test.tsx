import { render, screen, waitFor } from '@testing-library/react';
import axe from 'axe-core';
import { vi } from 'vitest';

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ setTitle: vi.fn().mockResolvedValue(undefined) }),
}));

import { LoadingProvider } from '../contexts/LoadingContext';
import { ToastProvider } from '../contexts/ToastContext';
import { UnitPreferencesProvider } from '../contexts/useUnitPreferences';
import { invoke } from '@tauri-apps/api/core';
import { setupTauriMocks, tearDownTauriMocks } from '../test-utils/tauriMocks';

const CONNECTED = { state: 'Connected', has_definition: true, signature: 'TEST', ini_name: 'test.ini' };
const DISCONNECTED = { state: 'Disconnected', has_definition: false };

function mockBackend(connectionStatus: unknown) {
  (invoke as unknown as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
    switch (cmd) {
      case 'list_repository_inis':
      case 'list_projects':
      case 'get_serial_ports':
      case 'get_status_bar_defaults':
      case 'get_available_channels':
      case 'get_menu_tree':
        return Promise.resolve([]);
      case 'get_settings':
        return Promise.resolve({ runtime_packet_mode: 'Auto', units_system: 'metric' });
      case 'get_current_project':
        return Promise.resolve(null);
      case 'is_onboarding_completed':
        return Promise.resolve(false);
      case 'get_connection_status':
        return Promise.resolve(connectionStatus);
      case 'get_protocol_defaults':
        return Promise.resolve({ default_baud_rate: 115200, timeout_ms: 2000 });
      case 'get_searchable_index':
        return Promise.resolve({});
      default:
        return Promise.resolve();
    }
  });
}

async function renderApp() {
  const { default: App } = await import('../App');
  return render(
    <LoadingProvider>
      <ToastProvider>
        <UnitPreferencesProvider>
          <App />
        </UnitPreferencesProvider>
      </ToastProvider>
    </LoadingProvider>,
  );
}

// `initializeApp` opens the onboarding dialog part-way through and hides the
// loading overlay in its `finally`; waiting for both means axe scans the
// settled DOM rather than the first synchronous render.
async function waitForInitialized(container: HTMLElement) {
  await screen.findByRole('dialog', {}, { timeout: 10_000 });
  await waitFor(() => expect(container.querySelector('.loading-overlay')).toBeNull(), {
    timeout: 10_000,
  });
}

async function expectNoViolations(container: HTMLElement) {
  const result = await axe.run(container, {
    rules: {
      // jsdom has no layout engine, so axe cannot compute contrast here.
      // Contrast stays a manual check in a real browser.
      'color-contrast': { enabled: false },
    },
  });
  expect(result.violations.map(({ id, help, nodes }) => ({
    id,
    help,
    targets: nodes.map((n) => n.target.join(' ')),
  }))).toEqual([]);
}

describe('App accessibility smoke (axe-core)', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    setupTauriMocks({});
  });

  afterEach(() => {
    tearDownTauriMocks();
  });

  it('has no automated violations while disconnected', async () => {
    mockBackend(DISCONNECTED);
    const { container } = await renderApp();
    await waitForInitialized(container);
    expect(container.querySelector('.packet-mode')?.textContent).toBe('—');

    await expectNoViolations(container);
  }, 15_000);

  it('has no automated violations while connected', async () => {
    mockBackend(CONNECTED);
    const { container } = await renderApp();
    await waitForInitialized(container);
    await waitFor(() => expect(screen.getByText('Auto')).toBeInTheDocument(), { timeout: 10_000 });

    await expectNoViolations(container);
  }, 15_000);
});
