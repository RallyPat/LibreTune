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

describe('reference correction', () => {
  beforeEach(() => {
    localStorage.removeItem('lt.afrCalibration');
    setupTauriMocks({
      list_calibration_presets: PRESETS,
      preview_afr_calibration: curveOf(10, 20),
      write_afr_calibration: {
        table: 'o2',
        verified: true,
        verify_note: null,
        transfer_function: '10.00–20.00 AFR over 0–5 V',
      },
    });
  });

  afterEach(() => {
    tearDownTauriMocks();
    localStorage.removeItem('lt.afrCalibration');
  });

  function correctionSentToPreview(): unknown {
    const calls = (invoke as any).mock.calls.filter(
      (c: any[]) => c[0] === 'preview_afr_calibration',
    );
    return calls.length ? calls[calls.length - 1][1].correction : undefined;
  }

  it('is off by default and sends no correction', async () => {
    render(
      <AfrCalibrationDialog isOpen onClose={() => {}} connected showToast={vi.fn()} />,
    );
    await screen.findByLabelText(/correct against a reference/i);
    await waitFor(() => expect(correctionSentToPreview()).toBeUndefined());
    expect(screen.queryByLabelText(/measured \(ecu\)/i)).toBeNull();
  });

  it('single point sends one measured/expected pair to the preview', async () => {
    render(
      <AfrCalibrationDialog isOpen onClose={() => {}} connected showToast={vi.fn()} />,
    );
    const mode = await screen.findByLabelText(/correct against a reference/i);
    fireEvent.change(mode, { target: { value: 'one' } });

    const measured = await screen.findByLabelText(/^measured \(ecu\)/i);
    const expected = screen.getByLabelText(/^expected \(reference\)/i);
    fireEvent.change(measured, { target: { value: '14.2' } });
    fireEvent.change(expected, { target: { value: '14.7' } });

    await waitFor(() => expect(correctionSentToPreview()).toEqual([[14.2, 14.7]]));
    // Two-point rows must not be shown in single-point mode.
    expect(screen.queryByLabelText(/point 2 measured/i)).toBeNull();
  });

  it('two point sends both pairs, in order', async () => {
    render(
      <AfrCalibrationDialog isOpen onClose={() => {}} connected showToast={vi.fn()} />,
    );
    const mode = await screen.findByLabelText(/correct against a reference/i);
    fireEvent.change(mode, { target: { value: 'two' } });

    fireEvent.change(await screen.findByLabelText(/point 1 measured/i), {
      target: { value: '12.5' },
    });
    fireEvent.change(screen.getByLabelText(/point 1 expected/i), {
      target: { value: '12.08' },
    });
    fireEvent.change(screen.getByLabelText(/point 2 measured/i), {
      target: { value: '19' },
    });
    fireEvent.change(screen.getByLabelText(/point 2 expected/i), {
      target: { value: '18.25' },
    });

    await waitFor(() =>
      expect(correctionSentToPreview()).toEqual([
        [12.5, 12.08],
        [19, 18.25],
      ]),
    );
  });

  it('writes exactly the previewed (corrected) curve, never a local recomputation', async () => {
    // The backend owns the correction math; the dialog must write curve.afr
    // verbatim. If the dialog ever starts correcting locally, the mock curve
    // here would differ from what write receives.
    render(
      <AfrCalibrationDialog isOpen onClose={() => {}} connected showToast={vi.fn()} />,
    );
    const mode = await screen.findByLabelText(/correct against a reference/i);
    fireEvent.change(mode, { target: { value: 'one' } });

    const write = await screen.findByRole('button', { name: /write calibration/i });
    await waitFor(() => expect(write).not.toBeDisabled());
    fireEvent.click(write);

    await waitFor(() => expect(writeCall()).not.toBeNull());
    expect(writeCall()!.afrValues).toEqual(curveOf(10, 20).afr);
  });
});
