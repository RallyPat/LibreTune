import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';
import TuneMismatchDialog from '../TuneMismatchDialog';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invoke(cmd, args),
}));

const mismatch = { ecu_pages: [0], project_pages: [0], diff_pages: [0] };

const view = {
  name: 'hardwareTest',
  title: 'Hardware test',
  definition: {
    name: 'hardwareTest',
    title: 'Hardware test',
    components: [{ type: 'Field', label: 'On Time', name: 'onTime' }],
  },
  changed_names: ['onTime'],
  project_numbers: { onTime: 8 },
  ecu_numbers: { onTime: 4 },
  project_strings: {},
  ecu_strings: {},
  project_tables: {},
  ecu_tables: {},
  project_curves: {},
  ecu_curves: {},
};

function mockInvoke() {
  invoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_tune_mismatch_dialog_index') {
      return [{ name: 'hardwareTest', title: 'Hardware test', changed_count: 1 }];
    }
    if (cmd === 'get_tune_mismatch_dialog_view') return view;
    if (cmd === 'get_constant') {
      return {
        name: 'onTime',
        units: 'ms',
        digits: 3,
        min: 0,
        max: 20,
        value_type: 'scalar',
        bit_options: [],
      };
    }
    if (cmd === 'get_settings') return {};
    if (cmd === 'use_project_tune' || cmd === 'use_ecu_tune') return;
    return undefined;
  });
}

beforeEach(() => {
  invoke.mockReset();
  mockInvoke();
});

it('Ignore closes without applying', async () => {
  const onClose = vi.fn();
  render(
    <TuneMismatchDialog
      isOpen
      mismatchInfo={mismatch}
      onClose={onClose}
      onUseProject={vi.fn()}
      onUseECU={vi.fn()}
    />,
  );
  await waitFor(() => screen.getByRole('button', { name: 'Ignore' }));
  await userEvent.click(screen.getByRole('button', { name: 'Ignore' }));
  expect(onClose).toHaveBeenCalled();
  expect(invoke.mock.calls.some(([cmd]) => cmd === 'use_project_tune' || cmd === 'use_ecu_tune')).toBe(false);
});

it('renders snapshot values without live ECU reads', async () => {
  render(
    <TuneMismatchDialog
      isOpen
      mismatchInfo={mismatch}
      onClose={vi.fn()}
      onUseProject={vi.fn()}
      onUseECU={vi.fn()}
    />,
  );
  await waitFor(() => {
    expect(screen.getByDisplayValue('8')).toBeInTheDocument();
    expect(screen.getByDisplayValue('4')).toBeInTheDocument();
  });
  expect(
    invoke.mock.calls.some(([cmd]) =>
      [
        'get_constant_value',
        'get_constant_string_value',
        'get_table_data',
        'get_curve_data',
      ].includes(cmd),
    ),
  ).toBe(false);
});

it('Use LibreTune Settings calls use_project_tune', async () => {
  const onUseProject = vi.fn();
  render(
    <TuneMismatchDialog
      isOpen
      mismatchInfo={mismatch}
      onClose={vi.fn()}
      onUseProject={onUseProject}
      onUseECU={vi.fn()}
    />,
  );
  await waitFor(() => screen.getByRole('button', { name: 'Use LibreTune Settings' }));
  await userEvent.click(screen.getByRole('button', { name: 'Use LibreTune Settings' }));
  await waitFor(() =>
    expect(invoke.mock.calls.some(([cmd]) => cmd === 'use_project_tune')).toBe(true),
  );
  expect(onUseProject).toHaveBeenCalled();
});
