import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { vi } from 'vitest';
import TemperatureCalibrationDialog from '../TemperatureCalibrationDialog';
import { setupTauriMocks, tearDownTauriMocks } from '../../../test-utils/tauriMocks';
import { invoke } from '@tauri-apps/api/core';

/** 32 temperatures, hot at ADC 0 (NTC on the bottom of the divider). */
const CURVE = Array.from({ length: 32 }, (_, i) => 150 - i * 6);

function call(cmd: string): any | null {
  const c = (invoke as any).mock.calls.find((x: any[]) => x[0] === cmd);
  return c ? c[1] : null;
}

/** Walk the wizard: Data Entry → Curve Fit → Generate Table → apply. */
function applyWizard() {
  fireEvent.click(screen.getByRole('button', { name: /next/i }));
  fireEvent.click(screen.getByRole('button', { name: /next/i }));
  fireEvent.click(screen.getByRole('button', { name: /apply calibration/i }));
}

describe('TemperatureCalibrationDialog', () => {
  beforeEach(() => {
    setupTauriMocks({
      build_thermistor_curve: CURVE,
      write_temperature_calibration: {
        table: 'clt',
        verified: true,
        verify_note: null,
        transfer_function: '-36.0–150.0 °C across 32 ADC bins',
      },
    });
  });

  afterEach(() => tearDownTauriMocks());

  it('fits on the backend at the ECU bins and writes the returned 32 points', async () => {
    render(
      <TemperatureCalibrationDialog isOpen onClose={() => {}} connected showToast={vi.fn()} />,
    );

    applyWizard();

    await waitFor(() => expect(call('build_thermistor_curve')).not.toBeNull());
    const { biasResistor, points } = call('build_thermistor_curve');
    expect(biasResistor).toBe(2490);
    // The wizard's three defaults, coldest → hottest, as (°C, Ω).
    expect(points).toEqual([[-40, 100000], [20, 2500], [100, 200]]);

    await waitFor(() => expect(call('write_temperature_calibration')).not.toBeNull());
    const { sensor, tempsC } = call('write_temperature_calibration');
    expect(sensor).toBe('clt');
    // Written verbatim: the frontend does not re-derive what the backend fitted.
    expect(tempsC).toEqual(CURVE);

    expect(await screen.findByText(/verified by read-back/i)).toBeInTheDocument();
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

    applyWizard();

    await waitFor(() => expect(showToast).toHaveBeenCalled());
    expect(call('write_temperature_calibration')).toBeNull();
    expect(screen.getByText(/not connected to an ECU/i)).toBeInTheDocument();
  });

  it('shows a backend failure in the dialog', async () => {
    (invoke as any).mockImplementation((cmd: string) =>
      cmd === 'build_thermistor_curve'
        ? Promise.reject('the three thermistor points are collinear in log-resistance')
        : Promise.resolve(),
    );
    render(
      <TemperatureCalibrationDialog isOpen onClose={() => {}} connected showToast={vi.fn()} />,
    );

    applyWizard();

    expect(await screen.findByText(/collinear in log-resistance/)).toBeInTheDocument();
    expect(call('write_temperature_calibration')).toBeNull();
  });
});
