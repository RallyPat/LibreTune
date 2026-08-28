/**
 * Preset selection → live preview.
 *
 * Guards the invoke argument names as much as the behaviour: issue #191 was a
 * calibration dialog calling a command with a misspelled argument, which the
 * backend rejected while the dialog reported success.
 */
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { vi } from 'vitest';
import AfrCalibrationDialog from '../AfrCalibrationDialog';
import { setupTauriMocks, tearDownTauriMocks } from '../../../test-utils/tauriMocks';
import { invoke } from '@tauri-apps/api/core';

function curveOf(lo: number, hi: number) {
  return {
    afr: Array.from({ length: 1024 }, (_, i) => lo + ((hi - lo) * i) / 1023),
    preview: Array.from({ length: 33 }, (_, i) => [(i * 5) / 32, lo + ((hi - lo) * i) / 32]),
    transfer_function: `${lo.toFixed(2)}–${hi.toFixed(2)} AFR over 0–5 V`,
    clips: false,
  };
}

const PRESETS = {
  afr_label: 'Calibrate AFR Table...',
  afr_presets: [
    { label: '14Point7 Spartan', expression: '10 + adcValue', generator: null, usable: true, note: null },
    {
      label: 'Zeitronix - Non Linear',
      expression: 'table(adcValue, "zeitronix.inc")',
      generator: null,
      usable: false,
      note: "preset needs the lookup file 'zeitronix.inc', which is not available",
    },
    { label: 'Innovate LC-2', expression: '7.35 + adcValue', generator: null, usable: true, note: null },
    { label: 'Custom Linear WB', expression: null, generator: 'linearGenerator', usable: false, note: null },
  ],
  linear_defaults: [1, 4, 9.7, 18.7],
  therm_label: null,
  therm_presets: [],
};

/** Curves keyed by preset label, so a preview can be told apart by its values. */
const CURVES: Record<string, ReturnType<typeof curveOf>> = {
  '14Point7 Spartan': curveOf(10, 20),
  'Innovate LC-2': curveOf(7.35, 22.4),
};

function previewCalls(): any[] {
  return (invoke as any).mock.calls
    .filter((c: any[]) => c[0] === 'preview_afr_calibration')
    .map((c: any[]) => c[1]);
}

function mockBackend(presets: any = PRESETS) {
  (invoke as any).mockImplementation((cmd: string, args: any) => {
    if (cmd === 'list_calibration_presets') {
      return presets instanceof Error ? Promise.reject(presets.message) : Promise.resolve(presets);
    }
    if (cmd === 'preview_afr_calibration') {
      if (args?.linear) {
        const [[, afr1], [, afr2]] = args.linear;
        return Promise.resolve(curveOf(afr1, afr2));
      }
      const curve = CURVES[args?.preset];
      return curve
        ? Promise.resolve(curve)
        : Promise.reject(`no calibration preset named '${args?.preset}'`);
    }
    return Promise.resolve();
  });
}

describe('AfrCalibrationDialog preset preview', () => {
  beforeEach(() => {
    localStorage.removeItem('lt.afrCalibration');
    setupTauriMocks();
    mockBackend();
  });

  afterEach(() => {
    tearDownTauriMocks();
    localStorage.removeItem('lt.afrCalibration');
  });

  it('previews the first usable INI preset, then the one the user picks', async () => {
    render(<AfrCalibrationDialog isOpen onClose={() => {}} connected showToast={vi.fn()} />);

    // Opens on the first usable preset from the INI, previewed by the backend.
    await waitFor(() => expect(previewCalls()).toContainEqual({ preset: '14Point7 Spartan' }));
    expect(await screen.findByText('0 V → 10.00')).toBeInTheDocument();
    expect(screen.getByText('5 V → 20.00')).toBeInTheDocument();

    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'Innovate LC-2' } });

    // Exact argument name: `preset`, not a paraphrase of it.
    await waitFor(() => expect(previewCalls()).toContainEqual({ preset: 'Innovate LC-2' }));
    expect(await screen.findByText('0 V → 7.35')).toBeInTheDocument();
    expect(screen.getByText('5 V → 22.40')).toBeInTheDocument();
  });

  it('lists the INI presets, disabling the ones with no usable formula', async () => {
    render(<AfrCalibrationDialog isOpen onClose={() => {}} connected showToast={vi.fn()} />);

    await screen.findByRole('option', { name: /14Point7 Spartan/ });
    expect(screen.getByRole('option', { name: /Zeitronix - Non Linear/ })).toBeDisabled();
    // The linearGenerator preset is the two-point editor, not a preset.
    expect(screen.queryByRole('option', { name: 'Custom Linear WB' })).not.toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Custom Linear WB…' })).toBeInTheDocument();
  });

  it('previews the two-point editor seeded from the INI linear bounds', async () => {
    render(<AfrCalibrationDialog isOpen onClose={() => {}} connected showToast={vi.fn()} />);

    await screen.findByRole('option', { name: /14Point7 Spartan/ });
    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'custom' } });

    // linear_defaults = (xLow, xHigh, yLow, yHigh) → (1 V, 9.7) and (4 V, 18.7).
    await waitFor(() =>
      expect(previewCalls()).toContainEqual({ linear: [[1, 9.7], [4, 18.7]] }),
    );
  });

  it('surfaces a preset-list failure instead of showing an empty dropdown', async () => {
    mockBackend(new Error('Definition not loaded'));
    render(<AfrCalibrationDialog isOpen onClose={() => {}} connected showToast={vi.fn()} />);

    expect(await screen.findByText(/Definition not loaded/)).toBeInTheDocument();
  });
});
