import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';

// Mock the file dialog plugin early so imports use the mock
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { UnitPreferencesProvider } from '../../../contexts/useUnitPreferences';

import { SettingsDialog } from '../Dialogs';

describe('SettingsDialog', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('loads runtime_packet_mode from settings and applies updates on Apply', async () => {
    (invoke as unknown as any).mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ runtime_packet_mode: 'ForceOCH', auto_reconnect_after_controller_command: false });
      }
      return Promise.resolve();
    });

    const consoleSpy = vi.spyOn(console, 'error');

    render(
      <UnitPreferencesProvider>
        <SettingsDialog theme={'dark'} isOpen={true} onClose={() => {}} onSettingsChange={() => {}} onThemeChange={() => {}} />
      </UnitPreferencesProvider>
    );

    // The dialog also fires get_hotkey_bindings/get_available_channels/
    // get_demo_mode on mount (unmocked here, so they resolve via the
    // catch-all Promise.resolve() branch above). Flush a macrotask tick
    // inside act() so their .then() chains fully settle now — otherwise
    // one can resolve in the gap between two later awaits and log a real
    // (if harmless) "not wrapped in act(...)" warning.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    // Find the runtime select and ensure value is loaded
    // Locate the runtime select specifically by its label
    const label = screen.getByText('Default Runtime Packet Mode');
    const runtimeSelect = (label.parentElement as HTMLElement).querySelector('select') as HTMLSelectElement;

    // Wait for select to be present
    await waitFor(() => expect(runtimeSelect).toBeInTheDocument());

    // Explicitly set the runtime packet mode to ForceOCH to ensure deterministic behavior
    fireEvent.change(runtimeSelect, { target: { value: 'ForceOCH' } });
    await waitFor(() => expect(runtimeSelect.value).toBe('ForceOCH'));

    // Click Apply
    const applyBtn = screen.getByText('Apply');
    await userEvent.click(applyBtn);

    // Settings are now saved in a single batched `update_settings` call
    // (one disk read + write on the backend) rather than ~30 sequential
    // `update_setting` calls. Assert the batch includes the expected pairs.
    const expectedMode = runtimeSelect.value;
    await waitFor(() => {
      const calls = (invoke as unknown as any).mock.calls.filter((c: any[]) => c[0] === 'update_settings');
      expect(calls.length).toBeGreaterThan(0);
      const updates = calls[calls.length - 1][1].updates as [string, string][];
      expect(updates).toContainEqual(['runtime_packet_mode', expectedMode]);
    });

    // Also ensure auto_reconnect setting is in the batch
    await waitFor(() => {
      const calls = (invoke as unknown as any).mock.calls.filter((c: any[]) => c[0] === 'update_settings');
      const updates = calls[calls.length - 1][1].updates as [string, string][];
      expect(updates).toContainEqual(['auto_reconnect_after_controller_command', 'false']);
    });

    // Flush once more so any state update Apply triggers after its last
    // update_setting call (e.g. a save-status flag) settles inside act()
    // before we assert on console.error below.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    // No unexpected console errors (React warnings)
    expect(consoleSpy).not.toHaveBeenCalled();
    consoleSpy.mockRestore();
  });

  it('clicking INI Change button opens file picker and updates filename', async () => {
    // Mock the plugin-dialog open function to return our chosen INI path
    const dialogModule = await import('@tauri-apps/plugin-dialog');
    (dialogModule.open as unknown as any).mockResolvedValue('/home/pat/definitions/new_def.ini');

    (invoke as unknown as any).mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ last_ini_path: '/home/pat/definitions/old_def.ini' });
      if (cmd === 'get_available_channels') return Promise.resolve([]);
      if (cmd === 'get_demo_mode') return Promise.resolve(false);
      if (cmd === 'update_project_ini') return Promise.resolve();
      return Promise.resolve();
    });

    // Spy on alert to suppress dialogs
    const alertSpy = vi.spyOn(window, 'alert').mockImplementation(() => {});

    render(
      <UnitPreferencesProvider>
        <SettingsDialog theme={'dark'} isOpen={true} onClose={() => {}} onSettingsChange={() => {}} onThemeChange={() => {}} currentProject={{ name: 'Test', path: '/proj', signature: 'sig', has_tune: true, tune_modified: false, connection: { port: null, baud_rate: 115200, auto_connect: false } }} />
      </UnitPreferencesProvider>
    );

    // Button initially shows old_def.ini
    const iniButton = await screen.findByRole('button', { name: /old_def.ini/i });
    expect(iniButton).toBeInTheDocument();

    // Click to change INI
    userEvent.click(iniButton);

    // Wait for update_project_ini to be invoked
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('update_project_ini', { iniPath: '/home/pat/definitions/new_def.ini', forceResync: false });
    });

    // New filename should be displayed
    await waitFor(() => expect(screen.getByRole('button', { name: /new_def.ini/i })).toBeInTheDocument());

    alertSpy.mockRestore();
  });
});
