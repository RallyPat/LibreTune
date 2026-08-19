import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { vi } from 'vitest';
import AfrCalibrationDialog from '../AfrCalibrationDialog';
import { setupTauriMocks, tearDownTauriMocks } from '../../../test-utils/tauriMocks';
import { invoke } from '@tauri-apps/api/core';

/** A backend AfrCurve, shaped exactly as `preview_afr_calibration` returns it. */
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
    { label: 'Custom Linear WB', expression: null, generator: 'linearGenerator', usable: false, note: null },
  ],
  linear_defaults: [1, 4, 9.7, 18.7],
  therm_label: null,
  therm_presets: [],
};

function writeCall(): { afrValues: number[]; transferFunction: string } | null {
  const call = (invoke as any).mock.calls.find((c: any[]) => c[0] === 'write_afr_calibration');
  return call ? call[1] : null;
}

describe('AfrCalibrationDialog', () => {
  beforeEach(() => {
    localStorage.removeItem('lt.afrCalibration');
    setupTauriMocks({
      list_calibration_presets: PRESETS,
      preview_afr_calibration: curveOf(10, 20),
      write_afr_calibration: {
        table: 'o2',
        verified: false,
        verify_note: 'This ECU is on the legacy protocol, which has no calibration read-back.',
        transfer_function: '10.00–20.00 AFR over 0–5 V',
      },
    });
  });

  afterEach(() => {
    tearDownTauriMocks();
    localStorage.removeItem('lt.afrCalibration');
  });

  it('writes the previewed curve and reports that it was not verified', async () => {
    render(
      <AfrCalibrationDialog isOpen onClose={() => {}} connected showToast={vi.fn()} />,
    );

    const write = await screen.findByRole('button', { name: /write calibration/i });
    await waitFor(() => expect(write).not.toBeDisabled());
    fireEvent.click(write);

    await waitFor(() => expect(writeCall()).not.toBeNull());
    const { afrValues, transferFunction } = writeCall()!;
    // The bytes written are the previewed curve itself, not a second
    // client-side calculation that could drift from it.
    expect(afrValues).toHaveLength(1024);
    expect(afrValues[0]).toBeCloseTo(10, 4);
    expect(afrValues[1023]).toBeCloseTo(20, 4);
    expect(transferFunction).toBe('10.00–20.00 AFR over 0–5 V');

    // A legacy write is unverified, and the dialog must say so rather than
    // claiming success.
    expect(await screen.findByText(/NOT verified/i)).toBeInTheDocument();
    expect(screen.getByText(/no calibration read-back/i)).toBeInTheDocument();
  });

  it('shows a write failure in the dialog instead of swallowing it', async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === 'list_calibration_presets') return Promise.resolve(PRESETS);
      if (cmd === 'preview_afr_calibration') return Promise.resolve(curveOf(10, 20));
      if (cmd === 'write_afr_calibration') return Promise.reject('The engine is running (850 rpm).');
      return Promise.resolve();
    });
    const showToast = vi.fn();
    render(
      <AfrCalibrationDialog isOpen onClose={() => {}} connected showToast={showToast} />,
    );

    const write = await screen.findByRole('button', { name: /write calibration/i });
    await waitFor(() => expect(write).not.toBeDisabled());
    fireEvent.click(write);

    expect(await screen.findByText(/engine is running/i)).toBeInTheDocument();
    expect(showToast).toHaveBeenCalledWith(expect.stringMatching(/engine is running/i), 'error');
  });

  it('refuses to write while disconnected', async () => {
    render(
      <AfrCalibrationDialog
        isOpen
        onClose={() => {}}
        connected={false}
        showToast={vi.fn()}
      />,
    );
    expect(await screen.findByRole('button', { name: /write calibration/i })).toBeDisabled();
    expect(screen.getByText(/not connected/i)).toBeInTheDocument();
  });
});
