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

describe('DialogRenderer slider field', () => {
  // ADCFILTER_* sliders in speeduino.ini: U08 0..240, digits 0
  const mockSliderConstant = {
    name: 'ADCFILTER_TPS',
    label: 'Throttle Position sensor',
    units: '%',
    digits: 0,
    min: 0,
    max: 240,
    value_type: 'scalar',
    bit_options: [],
  };

  const setupSliderMocks = (constantValue: number, extra: Record<string, any> = {}) =>
    setupTauriMocks({
      get_constant: mockSliderConstant,
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
          definition={{ name: 'sensorFilters', title: 'Analog sensor filters', components }}
          onBack={() => {}}
          openTable={() => {}}
          context={{}}
        />
      </ToastProvider>
    );

  it('renders a range control bounded by the constant min/max with a value readout', async () => {
    setupSliderMocks(50);
    renderDialog([{ type: 'Slider', label: 'Throttle Position sensor', name: 'ADCFILTER_TPS' }]);

    const slider = (await screen.findByRole('slider', {
      name: /throttle position sensor/i,
    })) as HTMLInputElement;
    expect(slider).toBeInTheDocument();
    expect(parseFloat(slider.min)).toBe(0);
    expect(parseFloat(slider.max)).toBe(240);
    expect(slider.value).toBe('50');
    // Value readout with units next to the control
    expect(screen.getByText('50 %')).toBeInTheDocument();
  });

  it('updates the display while dragging but only writes on release', async () => {
    setupSliderMocks(50);
    renderDialog([{ type: 'Slider', label: 'Throttle Position sensor', name: 'ADCFILTER_TPS' }]);

    const slider = (await screen.findByRole('slider', {
      name: /throttle position sensor/i,
    })) as HTMLInputElement;

    // Drag to 60: display follows, no ECU write yet
    fireEvent.change(slider, { target: { value: '60' } });
    await waitFor(() => expect(screen.getByText('60 %')).toBeInTheDocument());
    expect(invoke).not.toHaveBeenCalledWith('update_constant', expect.anything());

    // Release: single committed write (invoke also carries get_constant* calls)
    fireEvent.pointerUp(slider, { target: { value: '60' } });
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('update_constant', {
        name: 'ADCFILTER_TPS',
        value: 60,
      })
    );
    const updateCalls = (invoke as any).mock.calls.filter(
      ([cmd]: [string]) => cmd === 'update_constant'
    );
    expect(updateCalls).toHaveLength(1);
  });

  it('reverts cleanly when released at the original position (no write)', async () => {
    setupSliderMocks(50);
    renderDialog([{ type: 'Slider', label: 'Throttle Position sensor', name: 'ADCFILTER_TPS' }]);

    const slider = (await screen.findByRole('slider', {
      name: /throttle position sensor/i,
    })) as HTMLInputElement;

    // Drag away and back, then release at the original value
    fireEvent.change(slider, { target: { value: '80' } });
    fireEvent.change(slider, { target: { value: '50' } });
    fireEvent.pointerUp(slider, { target: { value: '50' } });

    // Give any accidental write a chance to surface
    await new Promise((r) => setTimeout(r, 50));
    expect(invoke).not.toHaveBeenCalledWith('update_constant', expect.anything());
    expect(screen.getByText('50 %')).toBeInTheDocument();
  });

  it('disables the slider when the enable condition is false', async () => {
    setupSliderMocks(50, { evaluate_expression: false });
    renderDialog([
      {
        type: 'Slider',
        label: 'Throttle Position sensor',
        name: 'ADCFILTER_TPS',
        enabled_condition: 'alwaysOn',
      },
    ]);

    const slider = (await screen.findByRole('slider', {
      name: /throttle position sensor/i,
    })) as HTMLInputElement;
    await waitFor(() => expect(slider.disabled).toBe(true));
    expect(slider).toBeDisabled();
  });
});

describe('DialogRenderer displayOnlyField', () => {
  const mockBitsConstant = {
    name: 'fuelPumpPin',
    label: 'Fuel pump pin',
    units: '',
    digits: 0,
    min: 0,
    max: 63,
    value_type: 'bits',
    bit_options: ['Board Default', 'INVALID', 'INVALID', '3', '4'],
    help: 'The pin the fuel pump is on',
  };

  afterEach(() => {
    tearDownTauriMocks();
  });

  it('shows the bits option text for a bound constant (read-only)', async () => {
    setupTauriMocks({
      get_constant: mockBitsConstant,
      // raw index 3 = option "3"
      get_constant_value: 3,
      get_settings: { show_all_help_icons: true },
    });
    render(
      <ToastProvider>
        <DialogRenderer
          definition={{
            name: 'ioSummary',
            title: 'I/O Summary',
            components: [
              { type: 'DisplayOnlyField', label: 'Fuel pump pin', name: 'fuelPumpPin' },
            ],
          }}
          onBack={() => {}}
          openTable={() => {}}
          context={{}}
        />
      </ToastProvider>
    );

    // Read-only value shows the option text, not the raw index
    await waitFor(() => expect(screen.getByText('3')).toBeInTheDocument());
    // No editable control rendered for the row
    expect(screen.queryByRole('combobox')).toBeNull();
    expect(screen.queryByRole('slider')).toBeNull();
  });

  it('renders text-only lines with note styling', async () => {
    setupTauriMocks({ get_settings: { show_all_help_icons: true } });
    render(
      <ToastProvider>
        <DialogRenderer
          definition={{
            name: 'closedLoopIdle',
            title: 'Closed Loop Idle',
            components: [
              {
                type: 'DisplayOnlyField',
                label: '#     !!! Please note that 1.0 means 100% !!!',
              },
            ],
          }}
          onBack={() => {}}
          openTable={() => {}}
          context={{}}
        />
      </ToastProvider>
    );

    const note = await screen.findByText('!!! Please note that 1.0 means 100% !!!');
    expect(note).toBeInTheDocument();
    expect(note.className).toMatch(/dialog-section-header|dialog-label/);
  });

  it('hides the line when the visibility condition is false', async () => {
    setupTauriMocks({
      get_constant: mockBitsConstant,
      get_constant_value: 0,
      evaluate_expression: false,
      get_settings: { show_all_help_icons: true },
    });
    render(
      <ToastProvider>
        <DialogRenderer
          definition={{
            name: 'ioSummary',
            title: 'I/O Summary',
            components: [
              {
                type: 'DisplayOnlyField',
                label: 'Fuel pump pin',
                name: 'fuelPumpPin',
                visibility_condition: 'fuelPumpEnable',
              },
            ],
          }}
          onBack={() => {}}
          openTable={() => {}}
          context={{}}
        />
      </ToastProvider>
    );

    // Wrapper hides the whole row when the condition is false
    await waitFor(() =>
      expect(screen.queryByText('Fuel pump pin')).toBeNull()
    );
  });
});

describe('DialogRenderer requiresPowerCycle badge', () => {
  afterEach(() => {
    tearDownTauriMocks();
  });

  it('shows a power-cycle badge on a Field marked requiresPowerCycle', async () => {
    setupTauriMocks({
      get_constant: {
        name: 'nCylinders',
        label: 'Cylinder Count',
        units: '',
        digits: 0,
        min: 1,
        max: 12,
        value_type: 'scalar',
        bit_options: [],
        requires_power_cycle: true,
      },
      get_constant_value: 4,
      get_settings: { show_all_help_icons: true },
    });
    render(
      <ToastProvider>
        <DialogRenderer
          definition={{
            name: 'engineConstants',
            title: 'Engine Constants',
            components: [{ type: 'Field', label: 'Cylinder Count', name: 'nCylinders' }],
          }}
          onBack={() => {}}
          openTable={() => {}}
          context={{}}
        />
      </ToastProvider>
    );

    const badge = await screen.findByText('power cycle');
    expect(badge).toBeInTheDocument();
    expect(badge.closest('.power-cycle-badge')).toBeInTheDocument();
  });

  it('shows no badge when the constant is not marked', async () => {
    setupTauriMocks({
      get_constant: {
        name: 'reqFuel',
        label: 'Required Fuel',
        units: 'ms',
        digits: 1,
        min: 0,
        max: 25.5,
        value_type: 'scalar',
        bit_options: [],
        requires_power_cycle: false,
      },
      get_constant_value: 12.6,
      get_settings: { show_all_help_icons: true },
    });
    render(
      <ToastProvider>
        <DialogRenderer
          definition={{
            name: 'engineConstants',
            title: 'Engine Constants',
            components: [{ type: 'Field', label: 'Required Fuel', name: 'reqFuel' }],
          }}
          onBack={() => {}}
          openTable={() => {}}
          context={{}}
        />
      </ToastProvider>
    );

    await waitFor(() => expect(screen.getByDisplayValue('12.6')).toBeInTheDocument());
    expect(screen.queryByText('power cycle')).toBeNull();
  });

  it('renders a header help link for dialogs with topicHelp', async () => {
    setupTauriMocks({ get_settings: { show_all_help_icons: true } });
    render(
      <ToastProvider>
        <DialogRenderer
          definition={{
            name: 'fanSettings',
            title: 'Fan Settings',
            topic_help: 'http://wiki.speeduino.com/en/configuration/Thermo_fan',
            components: [],
          }}
          onBack={() => {}}
          openTable={() => {}}
          context={{}}
        />
      </ToastProvider>
    );

    const helpBtn = await screen.findByTitle(
      'http://wiki.speeduino.com/en/configuration/Thermo_fan'
    );
    expect(helpBtn).toBeInTheDocument();
    expect(helpBtn.className).toMatch(/dialog-topic-help/);
  });
});
