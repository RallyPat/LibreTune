import { render } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import TableEditor2D from '../TableEditor2D';
import { ToastProvider } from '../../../contexts/ToastContext';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

const X_BINS = [500, 1000, 1500];
const Y_BINS = [20, 40, 60];
const Z_VALUES = [
  [10, 11, 12],
  [20, 21, 22],
  [30, 31, 32],
];

const NEW_Z_VALUES = [
  [90, 91, 92],
  [93, 94, 95],
  [96, 97, 98],
];

describe('TableEditor2D resyncs local state when props change without a remount', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({
      table_name: 'veTable1Tbl',
      x_bins: X_BINS,
      y_bins: Y_BINS,
      z_values: Z_VALUES,
    });
  });

  it('reflects new z_values/x_bins/y_bins props on the same mounted instance (no key change)', () => {
    const { container, rerender } = render(
      <ToastProvider>
        <TableEditor2D
          title="VE Table 1"
          table_name="veTable1Tbl"
          x_axis_name="RPM"
          y_axis_name="MAP"
          x_bins={X_BINS}
          y_bins={Y_BINS}
          z_values={Z_VALUES}
        />
      </ToastProvider>
    );

    const initialFirstCell = container.querySelector('.table-cell .cell-value');
    expect(initialFirstCell?.textContent).toBe('10.0');

    // Same component instance (no `key` prop, so this is exactly a prop
    // update — e.g. a tune:loaded refresh delivering fresh backend data —
    // not a remount). Before the resync effect, local edit state never
    // picked this up and the grid kept showing the stale values.
    rerender(
      <ToastProvider>
        <TableEditor2D
          title="VE Table 1"
          table_name="veTable1Tbl"
          x_axis_name="RPM"
          y_axis_name="MAP"
          x_bins={X_BINS}
          y_bins={Y_BINS}
          z_values={NEW_Z_VALUES}
        />
      </ToastProvider>
    );

    const updatedFirstCell = container.querySelector('.table-cell .cell-value');
    expect(updatedFirstCell?.textContent).toBe('90.0');
  });
});
