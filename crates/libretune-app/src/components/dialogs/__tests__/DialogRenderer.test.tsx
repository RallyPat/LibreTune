import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { useState } from 'react';
import DialogRenderer, { DialogDefinition } from '../DialogRenderer';
import { ToastProvider } from '../../../contexts/ToastContext';
import { setupTauriMocks, tearDownTauriMocks } from '../../../test-utils/tauriMocks';
import { invoke } from '@tauri-apps/api/core';
import { useRealtimeStore } from '../../../stores/realtimeStore';

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

  it('executes controller command and shows a success toast without syncing tune', async () => {
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
    fireEvent.click(btn);

    await waitFor(() =>
      expect(screen.getByText('Apply Base Map sent to ECU')).toBeInTheDocument()
    );

    expect(invoke).toHaveBeenCalledWith('execute_controller_command', {
      commandName: 'cmd_set_engine_type_default',
    });
    expect(invoke).not.toHaveBeenCalledWith('sync_ecu_data', expect.anything());
  });

  it('merges live output channels into command-button enable context', async () => {
    useRealtimeStore.getState().clearChannels();
    useRealtimeStore.getState().updateChannels({
      hasIgnitionVoltage: 1,
      etbFeedForwardTuneActive: 0,
    });

    const def: DialogDefinition = {
      name: 'etbTpsBiasDialog',
      title: 'ETB Bias Curve (Feed Forward)',
      components: [
        {
          type: 'CommandButton',
          label: 'Start guarded feed-forward tune',
          command: 'cmd_etb_ff_autotune',
          enabled_condition: 'hasIgnitionVoltage && isEtbEnabled && !etbFeedForwardTuneActive',
        },
      ],
    };

    render(
      <ToastProvider>
        <DialogRenderer
          definition={def}
          onBack={() => {}}
          openTable={() => {}}
          context={{ isEtbEnabled: 1 }}
        />
      </ToastProvider>
    );

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('evaluate_expression', {
        expression: 'hasIgnitionVoltage && isEtbEnabled && !etbFeedForwardTuneActive',
        context: expect.objectContaining({
          hasIgnitionVoltage: 1,
          isEtbEnabled: 1,
          etbFeedForwardTuneActive: 0,
        }),
      })
    );
    useRealtimeStore.getState().clearChannels();
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

describe('DialogRenderer checkbox field', () => {
  const mockGetConstant = {
    name: 'use_prediction',
    label: 'Use Prediction',
    units: '',
    digits: 0,
    min: 0,
    max: 1,
    value_type: 'scalar',
    bit_options: [],
  };

  // setInvokeResponse replaces the whole invoke mock (other commands then
  // resolve undefined), so each test configures its full response set here.
  const setupCheckboxMocks = (constantValue: number, extra: Record<string, any> = {}) =>
    setupTauriMocks({
      get_constant: mockGetConstant,
      get_constant_value: constantValue,
      update_constant: null,
      get_settings: { show_all_help_icons: true },
      ...extra,
    });

  afterEach(() => {
    tearDownTauriMocks();
  });

  const renderDialog = (components: DialogDefinition['components']) =>
    render(
      <ToastProvider>
        <DialogRenderer
          definition={{ name: 'flagsDialog', title: 'Flags', components }}
          onBack={() => {}}
          openTable={() => {}}
          context={{}}
        />
      </ToastProvider>
    );

  it('renders a checkbox bound to a constant (issue #338)', async () => {
    setupCheckboxMocks(0);
    renderDialog([{ type: 'Checkbox', label: 'Use Prediction', name: 'use_prediction' }]);

    const cb = await screen.findByRole('checkbox', { name: /use prediction/i });
    expect(cb).toBeInTheDocument();
    expect((cb as HTMLInputElement).checked).toBe(false);
    expect(screen.queryByRole('combobox')).toBeNull(); // checkbox, not a dropdown
  });

  it('checks when value is non-zero and writes 1 on toggle', async () => {
    setupCheckboxMocks(1);
    renderDialog([{ type: 'Checkbox', label: 'Use Prediction', name: 'use_prediction' }]);

    const cb = (await screen.findByRole('checkbox', { name: /use prediction/i })) as HTMLInputElement;
    expect(cb.checked).toBe(true);

    // Uncheck it — should write 0
    fireEvent.click(cb);
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('update_constant', {
        name: 'use_prediction',
        value: 0,
      })
    );
  });

  it('inverts sense with flipValue (checked ⇄ 0)', async () => {
    // value 0 + flipValue ⇒ checked
    setupCheckboxMocks(0);
    renderDialog([
      { type: 'Checkbox', label: "Don't use Prediction", name: 'use_prediction', flip_value: true },
    ]);

    const cb = (await screen.findByRole('checkbox', { name: /don't use prediction/i })) as HTMLInputElement;
    expect(cb.checked).toBe(true);

    // Uncheck it — flipValue writes 1
    fireEvent.click(cb);
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('update_constant', {
        name: 'use_prediction',
        value: 1,
      })
    );
  });

  it('disables the checkbox when the enable condition is false', async () => {
    setupCheckboxMocks(0, { evaluate_expression: false });
    renderDialog([
      {
        type: 'Checkbox',
        label: 'Use Prediction',
        name: 'use_prediction',
        enabled_condition: '!use_prediction',
      },
    ]);

    const cb = (await screen.findByRole('checkbox', { name: /use prediction/i })) as HTMLInputElement;
    await waitFor(() => expect(cb.disabled).toBe(true));
    expect(cb).toBeDisabled();
  });

  it('keeps dual checkboxes on the same constant in step (reporter scenario)', async () => {
    setupCheckboxMocks(0);

    // Mimic the tab shell (TabContentRouter): context holds all constant
    // values and is updated optimistically after every write.
    function Harness() {
      const [ctx, setCtx] = useState<Record<string, number>>({});
      return (
        <ToastProvider>
          <DialogRenderer
            definition={{
              name: 'flagsDialog',
              title: 'Flags',
              components: [
                { type: 'Checkbox', label: 'Use Prediction', name: 'use_prediction' },
                {
                  type: 'Checkbox',
                  label: "Don't use Prediction",
                  name: 'use_prediction',
                  flip_value: true,
                },
              ],
            }}
            onBack={() => {}}
            openTable={() => {}}
            context={ctx}
            onOptimisticUpdate={(name, value) =>
              setCtx((prev) => ({ ...prev, [name]: value }))
            }
          />
        </ToastProvider>
      );
    }

    render(<Harness />);

    const useIt = (await screen.findByRole('checkbox', {
      name: /^use prediction$/i,
    })) as HTMLInputElement;
    const dontUseIt = screen.getByRole('checkbox', {
      name: /don't use prediction/i,
    }) as HTMLInputElement;

    // value 0: normal box unchecked, flipped box checked
    expect(useIt.checked).toBe(false);
    expect(dontUseIt.checked).toBe(true);

    // Toggle the normal box on ⇒ writes 1; the optimistic context update must
    // flip the sibling box too (both derive from the constant's value).
    fireEvent.click(useIt);
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('update_constant', {
        name: 'use_prediction',
        value: 1,
      })
    );
    await waitFor(() => expect(useIt.checked).toBe(true));
    await waitFor(() => expect(dontUseIt.checked).toBe(false));
  });
});
