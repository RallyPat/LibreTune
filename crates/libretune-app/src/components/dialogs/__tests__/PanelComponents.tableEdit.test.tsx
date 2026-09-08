import { render, fireEvent, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { RecursivePanel } from '../PanelComponents';
import { ToastProvider } from '../../../contexts/ToastContext';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

const invokeMock = vi.mocked(invoke);

const X_BINS = [500, 1000, 1500];
const Y_BINS = [20, 40, 60];
const Z_VALUES = [
  [10, 11, 12],
  [20, 21, 22],
  [30, 31, 32],
];

const TABLE_DATA = {
  name: 'veTable1Tbl',
  title: 'VE Table',
  x_axis_name: 'RPM',
  y_axis_name: 'MAP',
  x_bins: X_BINS,
  y_bins: Y_BINS,
  z_values: Z_VALUES,
};

/**
 * RecursivePanel resolves a panel name by trying, in order: indicatorPanel,
 * readoutPanel, dialog, table. Reject the first three so it falls through to
 * the table branch that renders an embedded TableEditor2D.
 */
function mockInvokeForEmbeddedTable() {
  invokeMock.mockImplementation((cmd: string) => {
    switch (cmd) {
      case 'get_indicator_panel':
      case 'get_readout_panel':
      case 'get_dialog_definition':
        return Promise.reject(new Error(`not a ${cmd}`));
      case 'get_table_info':
        return Promise.resolve({ name: 'veTable1Tbl', title: 'VE Table' });
      case 'get_table_data':
        return Promise.resolve(TABLE_DATA);
      case 'get_settings':
        return Promise.resolve({});
      case 'update_table_data':
        return Promise.resolve();
      default:
        return Promise.resolve();
    }
  });
}

describe('RecursivePanel embedded table edit (PanelComponents.tsx wiring)', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    mockInvokeForEmbeddedTable();
  });

  it('persists a cell edit exactly once via update_table_data, not twice', async () => {
    const onUpdate = vi.fn();
    const { container } = render(
      <ToastProvider>
        <RecursivePanel name="veTable1Tbl" openTable={() => {}} context={{}} onUpdate={onUpdate} />
      </ToastProvider>
    );

    // Wait for the embedded TableEditor2D to mount.
    await waitFor(() => expect(container.querySelectorAll('.table-cell').length).toBeGreaterThan(0));

    const cells = container.querySelectorAll('.table-cell');
    fireEvent.mouseDown(cells[0]);

    invokeMock.mockClear();

    // Trigger one edit — a nudge is the simplest local mutation that goes
    // through setLocalZValues (TableEditor2D.tsx) and therefore through the
    // onValuesChange callback PanelComponents.tsx wires up. The toolbar
    // button is hidden in embedded mode, but the keyboard shortcut isn't.
    fireEvent.keyDown(document, { key: '.' });

    await waitFor(() => {
      const updateCalls = invokeMock.mock.calls.filter(([cmd]) => cmd === 'update_table_data');
      expect(updateCalls.length).toBeGreaterThan(0);
    });

    const updateCalls = invokeMock.mock.calls.filter(([cmd]) => cmd === 'update_table_data');
    // Regression guard: PanelComponents.tsx's onValuesChange used to call
    // invoke('update_table_data') a second time on top of TableEditor2D's own
    // persist, doubling the backend write per cell edit.
    expect(updateCalls).toHaveLength(1);
    expect(onUpdate).toHaveBeenCalled();
  });
});
