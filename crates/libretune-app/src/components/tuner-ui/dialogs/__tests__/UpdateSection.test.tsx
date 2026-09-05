import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { vi } from 'vitest';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { UpdateSection, checkForUpdateQuietly } from '../UpdateSection';

vi.mock('@tauri-apps/plugin-updater', () => ({ check: vi.fn() }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }));

function availableUpdate() {
  return {
    version: '0.2.0',
    body: 'Faster table editing',
    downloadAndInstall: vi.fn(async () => undefined),
    close: vi.fn(async () => undefined),
  };
}

describe('UpdateSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(relaunch).mockResolvedValue(undefined);
  });

  it('reports up to date and keeps a manual check when nothing is available', async () => {
    vi.mocked(check).mockResolvedValue(null);

    render(<UpdateSection />);

    await screen.findByText('LibreTune is up to date.');
    expect(check).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Check for updates' }));
    await waitFor(() => expect(check).toHaveBeenCalledTimes(2));
  });

  it('offers the update and installs then relaunches on confirm', async () => {
    const update = availableUpdate();
    vi.mocked(check).mockResolvedValue(update as never);

    render(<UpdateSection />);

    await screen.findByText('0.2.0');
    expect(screen.getByText('Faster table editing')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Install and restart' }));

    await waitFor(() => expect(update.downloadAndInstall).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(relaunch).toHaveBeenCalledTimes(1));
  });

  it('shows the failure reason and stays retryable', async () => {
    vi.mocked(check).mockRejectedValue(new Error('404 Not Found'));

    render(<UpdateSection />);

    await screen.findByRole('alert');
    expect(screen.getByRole('alert')).toHaveTextContent('404 Not Found');
    expect(screen.getByRole('button', { name: 'Check for updates' })).toBeEnabled();
  });

  it('never relaunches when the install fails', async () => {
    const update = availableUpdate();
    update.downloadAndInstall.mockRejectedValue(new Error('bad signature'));
    vi.mocked(check).mockResolvedValue(update as never);

    render(<UpdateSection />);

    fireEvent.click(await screen.findByRole('button', { name: 'Install and restart' }));

    await screen.findByRole('alert');
    expect(relaunch).not.toHaveBeenCalled();
  });

  it('releases the plugin resource when the user picks Later', async () => {
    const update = availableUpdate();
    vi.mocked(check).mockResolvedValue(update as never);

    render(<UpdateSection />);

    fireEvent.click(await screen.findByRole('button', { name: 'Later' }));

    await waitFor(() => expect(update.close).toHaveBeenCalledTimes(1));
    expect(screen.getByRole('button', { name: 'Check for updates' })).toBeInTheDocument();
  });
});

describe('checkForUpdateQuietly', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns null instead of throwing when the check fails', async () => {
    vi.mocked(check).mockRejectedValue(new Error('offline'));
    await expect(checkForUpdateQuietly()).resolves.toBeNull();
  });

  it('returns the version and releases the resource when one is available', async () => {
    const update = availableUpdate();
    vi.mocked(check).mockResolvedValue(update as never);
    await expect(checkForUpdateQuietly()).resolves.toBe('0.2.0');
    expect(update.close).toHaveBeenCalledTimes(1);
  });
});
