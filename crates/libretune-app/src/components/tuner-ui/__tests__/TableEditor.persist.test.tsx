import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { TableEditor } from '../TableEditor';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock('../../tables/TableEditor3D', () => ({ default: () => null }));

const invokeMock = vi.mocked(invoke);

const DATA = {
  name: 'veTable1Tbl',
  xAxis: [500, 1000, 1500],
  yAxis: [20, 40, 60],
  zValues: [
    [10, 11, 12],
    [20, 21, 22],
    [30, 31, 32],
  ],
};

describe('TableEditor persistence', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({});
  });

  it('writes a cell edit to the backend, not only tab state', async () => {
    render(<TableEditor data={{ ...DATA }} onChange={() => {}} />);
    fireEvent.mouseDown(document.querySelector('td.table-cell')!);
    fireEvent.click(screen.getByTitle('Smooth (s)'));

    await waitFor(() => {
      const saved = invokeMock.mock.calls.find(([cmd]) => cmd === 'update_table_data');
      expect(saved, 'issue #325: tab-editor edits must reach the ECU').toBeTruthy();
    });
    const args = (invokeMock.mock.calls.find(([cmd]) => cmd === 'update_table_data')![1] ??
      {}) as Record<string, unknown>;
    expect(args.tableName).toBe('veTable1Tbl');
    expect(args.zValues).toBeTruthy();
  });
});
