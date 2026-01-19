import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import DialogRenderer, { DialogDefinition } from '../DialogRenderer';
import { ToastProvider } from '../../ToastContext';
import { setupTauriMocks, tearDownTauriMocks } from '../../../test-utils/tauriMocks';
import { vi } from 'vitest';

describe('DialogRenderer CommandButton', () => {
  let tauriHandle: ReturnType<typeof setupTauriMocks> | null = null;

  beforeEach(() => {
    // Disable command warnings to allow immediate execution
    localStorage.setItem('libretune_command_warnings_disabled', 'true');
    tauriHandle = setupTauriMocks({
      execute_controller_command: null,
      sync_ecu_data: { pages_synced: 3, pages_failed: 0, total_pages: 3, errors: [] },
      get_settings: { auto_reconnect_after_controller_command: true, show_all_help_icons: true },
    });
  });

  afterEach(() => {
    localStorage.removeItem('libretune_command_warnings_disabled');
    if (tauriHandle) {
      tearDownTauriMocks();
      tauriHandle = null;
    }
  });

  it('executes controller command and triggers ECU sync, showing a success toast', async () => {
    const def: DialogDefinition = {
      name: 'engineTypeDialog',
      title: 'Popular vehicles',
      components: [
        {
          type: 'CommandButton',
          label: 'Apply Base Map',
          command: 'cmd_set_engine_type_default',
        },
      ],
    };

    render(
      <ToastProvider>
        <DialogRenderer definition={def} onBack={() => {}} openTable={() => {}} context={{}} />
      </ToastProvider>
    );

    const btn = screen.getByText('Apply Base Map');

    // Listen for reconnect event
    let reconnectFired = false;
    const handler = () => { reconnectFired = true; };
    window.addEventListener('reconnect:request', handler as EventListener);

    // Spy on console.debug for dev-only telemetry message
    const debugSpy = vi.spyOn(console, 'debug').mockImplementation(() => {});
    // Provide a mock telemetry sink on global window
    const telemetryMock = { trackEvent: vi.fn() };
    (window as any).__libretuneTelemetry = telemetryMock;

    fireEvent.click(btn);

    // Wait for the toast showing sync success
    await waitFor(() => expect(screen.getByText(/Sync complete:/)).toBeInTheDocument());
    expect(screen.getByText(/Sync complete: 3 pages/)).toBeInTheDocument();

    // And wait for reconnect event to be dispatched
    await waitFor(() => expect(reconnectFired).toBe(true));

    // Expect debug telemetry called
    expect(debugSpy).toHaveBeenCalledWith(expect.stringContaining('reconnect:request dispatched'), expect.any(Object));

    // Telemetry sink should have been called
    expect(telemetryMock.trackEvent).toHaveBeenCalledWith('reconnect_request', { source: 'controller-command' });

    debugSpy.mockRestore();
    delete (window as any).__libretuneTelemetry;

    window.removeEventListener('reconnect:request', handler as EventListener);
  });
});
