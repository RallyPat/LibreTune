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

describe('DialogRenderer numeric input field', () => {
  let tauriHandle: ReturnType<typeof setupTauriMocks> | null = null;

  beforeEach(() => {
    const mockGetConstant = {
      name: 'testVoltage',
      label: 'Test Voltage',
      units: 'voltage',
      digits: 2,
      min: 0,
      max: 5,
      value_type: 'scalar',
      bit_options: [],
    };
    
    tauriHandle = setupTauriMocks({
      get_constant: mockGetConstant,
      get_constant_value: 1.5,
      update_constant: null,
      get_settings: { show_all_help_icons: true },
    });
  });

  afterEach(() => {
    if (tauriHandle) {
      tearDownTauriMocks();
      tauriHandle = null;
    }
  });

  it('preserves partial numeric input while typing', async () => {
    const def: DialogDefinition = {
      name: 'testDialog',
      title: 'Test Dialog',
      components: [
        {
          type: 'Field',
          name: 'testVoltage',
          label: 'Test Voltage',
        },
      ],
    };

    render(
      <ToastProvider>
        <DialogRenderer definition={def} onBack={() => {}} openTable={() => {}} context={{}} />
      </ToastProvider>
    );

    // Wait for field to load
    await waitFor(() => expect(screen.getByDisplayValue('1.5')).toBeInTheDocument(), { timeout: 3000 });

    const input = screen.getByDisplayValue('1.5') as HTMLInputElement;

    // Type "2.35" - the key test is that intermediate states like "2." don't cause issues
    fireEvent.change(input, { target: { value: '2.35' } });
    // In controlled inputs, we need to wait for React to update
    await waitFor(() => expect(input.value).toBe('2.35'));

    // Test that empty string is preserved during typing
    fireEvent.change(input, { target: { value: '' } });
    await waitFor(() => expect(input.value).toBe(''));

    // Type partial decimal
    fireEvent.change(input, { target: { value: '3.' } });
    await waitFor(() => expect(input.value).toBe('3.'));
  });

  it('handles empty input on blur by setting to zero', async () => {
    const def: DialogDefinition = {
      name: 'testDialog',
      title: 'Test Dialog',
      components: [
        {
          type: 'Field',
          name: 'testVoltage',
          label: 'Test Voltage',
        },
      ],
    };

    render(
      <ToastProvider>
        <DialogRenderer definition={def} onBack={() => {}} openTable={() => {}} context={{}} />
      </ToastProvider>
    );

    await waitFor(() => expect(screen.getByDisplayValue('1.5')).toBeInTheDocument(), { timeout: 3000 });

    const input = screen.getByDisplayValue('1.5') as HTMLInputElement;

    // Clear the input and blur
    fireEvent.change(input, { target: { value: '' } });
    fireEvent.blur(input);

    // Should reset to 0
    await waitFor(() => expect(input.value).toBe('0'));
  });
});
