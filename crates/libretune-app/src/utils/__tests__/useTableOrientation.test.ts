import { describe, it, expect, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { setTableYAxisBottom } from '../useTableOrientation';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(() => Promise.resolve()) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

describe('setTableYAxisBottom', () => {
  it('persists the flag via update_setting as a string', async () => {
    await setTableYAxisBottom(true);
    expect(invoke).toHaveBeenCalledWith('update_setting', { key: 'table_y_axis_bottom', value: 'true' });
  });

  it('swallows backend errors', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error('boom'));
    await expect(setTableYAxisBottom(false)).resolves.toBeUndefined();
  });
});
