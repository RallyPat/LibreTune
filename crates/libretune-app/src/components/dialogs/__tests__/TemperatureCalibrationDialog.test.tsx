import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { vi } from 'vitest';
import TemperatureCalibrationDialog from '../TemperatureCalibrationDialog';
import { setupTauriMocks, tearDownTauriMocks } from '../../../test-utils/tauriMocks';
import { invoke } from '@tauri-apps/api/core';

const LEGACY_BINS = Array.from({ length: 32 }, (_, x) => x * 32);

function writeCall(): { sensor: string; tempsC: number[] } | null {
  const call = (invoke as any).mock.calls.find(
    (c: any[]) => c[0] === 'write_temperature_calibration',
  );
  return call ? call[1] : null;
}

describe('TemperatureCalibrationDialog', () => {
  beforeEach(() => {
    setupTauriMocks({
      get_temperature_calibration_bins: {
        bins: LEGACY_BINS,
        modern_protocol: false,
        connected: true,
      },
      write_temperature_calibration: null,
    });
  });

  afterEach(() => tearDownTauriMocks());

  it('fits the wizard defaults and writes 32 temperatures at the ECU bins', async () => {
    render(
      <TemperatureCalibrationDialog
        isOpen
        onClose={() => {}}
        connected
        showToast={vi.fn()}
      />,
    );

    // Walk the wizard: Data Entry → Curve Fit → Generate Table.
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    fireEvent.click(screen.getByRole('button', { name: /apply calibration/i }));

    await waitFor(() => expect(writeCall()).not.toBeNull());
    const { sensor, tempsC } = writeCall()!;
    expect(sensor).toBe('clt');
    expect(tempsC).toHaveLength(32);

    // NTC on the bottom of the divider: low ADC = low resistance = hot.
    expect(tempsC[0]).toBeGreaterThan(tempsC[31]);
    expect(tempsC[0]).toBeGreaterThan(100);
    expect(tempsC[31]).toBeLessThan(0);
    for (const t of tempsC) {
      expect(Number.isFinite(t)).toBe(true);
      expect(t).toBeGreaterThanOrEqual(-60);
      expect(t).toBeLessThanOrEqual(300);
    }
  });

  it('does not write when disconnected', async () => {
    const showToast = vi.fn();
    render(
      <TemperatureCalibrationDialog
        isOpen
        onClose={() => {}}
        connected={false}
        showToast={showToast}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    fireEvent.click(screen.getByRole('button', { name: /apply calibration/i }));

    await waitFor(() => expect(showToast).toHaveBeenCalled());
    expect(writeCall()).toBeNull();
  });
});
