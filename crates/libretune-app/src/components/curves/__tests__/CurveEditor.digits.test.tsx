import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import CurveEditor, { type CurveData } from '../CurveEditor';

/**
 * Regression tests for issue #331: the Curve Editor ignored the `digits`
 * column of the underlying [Constants] and always rendered 2 decimals.
 * A sensor calibration with `digits = 3` (e.g. ADC volts at 0.001
 * resolution) showed `1.23` instead of `1.234` — and opening the cell for
 * edit prefilled the truncated value, so saving quantized the tune.
 */
function curveData(overrides: Partial<CurveData> = {}): CurveData {
  return {
    name: 'IATCalibCurve',
    title: 'IAT Calibration',
    x_bins: [0, 1.234, 5],
    y_bins: [-40, 20, 125],
    x_label: 'ADC Voltage',
    y_label: 'Air Temperature',
    x_digits: 3,
    y_digits: 0,
    ...overrides,
  };
}

describe('CurveEditor digits (issue #331)', () => {
  it('renders X bins with the INI digits precision', () => {
    render(<CurveEditor data={curveData()} />);
    expect(screen.getByText('1.234')).toBeTruthy();
    // Exact-match lookup: must not find a truncated 2-decimal cell.
    expect(screen.queryByText('1.23')).toBeNull();
  });

  it('renders Y bins with their own digits precision', () => {
    render(<CurveEditor data={curveData()} />);
    expect(screen.getByText('20')).toBeTruthy();
  });

  it('falls back to 2 decimals when the backend sends no digits', () => {
    const { x_digits: _x, y_digits: _y, ...legacy } = curveData();
    render(<CurveEditor data={legacy} />);
    expect(screen.getByText('1.23')).toBeTruthy();
  });

  it('prefills the edit box with full precision instead of truncated text', () => {
    render(<CurveEditor data={curveData()} />);
    fireEvent.doubleClick(screen.getByText('1.234'));
    const input = screen.getByDisplayValue('1.234') as HTMLInputElement;
    expect(input.value).toBe('1.234');
  });
});
