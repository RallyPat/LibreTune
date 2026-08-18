import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { vi } from 'vitest';
import AfrCalibrationDialog from '../AfrCalibrationDialog';
import { setupTauriMocks, tearDownTauriMocks } from '../../../test-utils/tauriMocks';
import { invoke } from '@tauri-apps/api/core';

function writeCall(): { afrValues: number[] } | null {
  const call = (invoke as any).mock.calls.find((c: any[]) => c[0] === 'write_afr_calibration');
  return call ? call[1] : null;
}

describe('AfrCalibrationDialog', () => {
  beforeEach(() => {
    localStorage.removeItem('lt.afrCalibration');
    setupTauriMocks({ write_afr_calibration: null });
  });

  afterEach(() => {
    tearDownTauriMocks();
    localStorage.removeItem('lt.afrCalibration');
  });

  it('writes the 1024-point preset curve with the ground offset applied', async () => {
    render(
      <AfrCalibrationDialog isOpen onClose={() => {}} connected showToast={vi.fn()} />,
    );

    // Default preset is 14Point7 (10–20 AFR linear). A +125 mV ground offset
    // means the ECU reads 0.125 V above the sensor's output, so the curve
    // shifts down by 0.25 AFR: 0 V decodes to 9.75.
    const offsetInput = screen.getByRole('spinbutton');
    fireEvent.change(offsetInput, { target: { value: '125' } });

    fireEvent.click(screen.getByRole('button', { name: /write calibration/i }));

    await waitFor(() => expect(writeCall()).not.toBeNull());
    const { afrValues } = writeCall()!;
    expect(afrValues).toHaveLength(1024);
    expect(afrValues[0]).toBeCloseTo(9.75, 2);
    expect(afrValues[1023]).toBeCloseTo(19.75, 2);
  });

  it('supports a custom linear curve (the Spartan ground-offset endpoints)', async () => {
    render(
      <AfrCalibrationDialog isOpen onClose={() => {}} connected showToast={vi.fn()} />,
    );

    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'custom' } });

    // Order of number inputs: V1, AFR1, V2, AFR2, ground offset.
    const inputs = screen.getAllByRole('spinbutton');
    expect(inputs).toHaveLength(5);
    fireEvent.change(inputs[0], { target: { value: '0' } });
    fireEvent.change(inputs[1], { target: { value: '9.69' } });
    fireEvent.change(inputs[2], { target: { value: '5' } });
    fireEvent.change(inputs[3], { target: { value: '19.8' } });

    fireEvent.click(screen.getByRole('button', { name: /write calibration/i }));

    await waitFor(() => expect(writeCall()).not.toBeNull());
    const { afrValues } = writeCall()!;
    expect(afrValues[0]).toBeCloseTo(9.69, 2);
    expect(afrValues[512]).toBeCloseTo((9.69 + 19.8) / 2, 1);
    expect(afrValues[1023]).toBeCloseTo(19.8, 2);
  });

  it('refuses to write while disconnected', () => {
    render(
      <AfrCalibrationDialog
        isOpen
        onClose={() => {}}
        connected={false}
        showToast={vi.fn()}
      />,
    );
    expect(screen.getByRole('button', { name: /write calibration/i })).toBeDisabled();
    expect(screen.getByText(/not connected/i)).toBeInTheDocument();
  });
});
