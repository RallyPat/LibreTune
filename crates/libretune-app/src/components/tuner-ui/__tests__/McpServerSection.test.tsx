import { render, screen, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';

import { invoke } from '@tauri-apps/api/core';

import { McpServerSection } from '../dialogs/McpServerSection';

/** Let the mount effect's promises settle before asserting. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

describe('McpServerSection', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('shows the stopped state and the saved port on mount', async () => {
    (invoke as unknown as any).mockImplementation((cmd: string) => {
      if (cmd === 'mcp_status') return Promise.resolve({ running: false, port: 0 });
      if (cmd === 'get_settings') return Promise.resolve({ mcp_port: 9100 });
      return Promise.resolve();
    });

    render(<McpServerSection />);
    await settle();

    expect(screen.getByText('Stopped')).toBeInTheDocument();
    expect(screen.getByLabelText('Port')).toHaveValue(9100);
  });

  it('renders the bound port when the server is already running', async () => {
    (invoke as unknown as any).mockImplementation((cmd: string) => {
      if (cmd === 'mcp_status') return Promise.resolve({ running: true, port: 8765 });
      if (cmd === 'get_settings') return Promise.resolve({ mcp_port: 8765 });
      return Promise.resolve();
    });

    render(<McpServerSection />);
    await settle();

    expect(screen.getByText('Running on 127.0.0.1:8765')).toBeInTheDocument();
  });

  it('survives commands that resolve to nothing', async () => {
    // A stubbed or unregistered command resolves to `undefined`; reading
    // `.running` off that would take the whole Settings dialog down.
    (invoke as unknown as any).mockImplementation(() => Promise.resolve());

    render(<McpServerSection />);
    await settle();

    expect(screen.getByText('Stopped')).toBeInTheDocument();
  });

  it('enables the server through mcp_set_enabled and shows the bound port', async () => {
    (invoke as unknown as any).mockImplementation((cmd: string) => {
      if (cmd === 'mcp_status') return Promise.resolve({ running: false, port: 0 });
      if (cmd === 'get_settings') return Promise.resolve({ mcp_port: 8765 });
      if (cmd === 'mcp_set_enabled') return Promise.resolve({ running: true, port: 8765 });
      return Promise.resolve();
    });

    render(<McpServerSection />);
    await settle();

    await userEvent.click(screen.getByLabelText('Expose tools over MCP'));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('mcp_set_enabled', { enabled: true });
    });
    expect(await screen.findByText('Running on 127.0.0.1:8765')).toBeInTheDocument();
  });

  it('surfaces a failed start instead of silently staying stopped', async () => {
    (invoke as unknown as any).mockImplementation((cmd: string) => {
      if (cmd === 'mcp_status') return Promise.resolve({ running: false, port: 0 });
      if (cmd === 'get_settings') return Promise.resolve({ mcp_port: 8765 });
      if (cmd === 'mcp_set_enabled') return Promise.reject('Address already in use');
      return Promise.resolve();
    });

    render(<McpServerSection />);
    await settle();

    await userEvent.click(screen.getByLabelText('Expose tools over MCP'));

    expect(await screen.findByRole('alert')).toHaveTextContent('Address already in use');
    expect(screen.getByText('Stopped')).toBeInTheDocument();
  });

  it('keeps the token hidden until asked, then reveals it', async () => {
    (invoke as unknown as any).mockImplementation((cmd: string) => {
      if (cmd === 'mcp_status') return Promise.resolve({ running: false, port: 0 });
      if (cmd === 'get_settings') return Promise.resolve({ mcp_port: 8765 });
      if (cmd === 'mcp_get_token') return Promise.resolve('a'.repeat(64));
      return Promise.resolve();
    });

    render(<McpServerSection />);
    await settle();

    const field = screen.getByLabelText('Access token');
    expect(field).toHaveValue('••••••••••••••••');

    await userEvent.click(screen.getByRole('button', { name: 'Show' }));

    await waitFor(() => expect(field).toHaveValue('a'.repeat(64)));
  });

  it('replaces the shown token when regenerating', async () => {
    (invoke as unknown as any).mockImplementation((cmd: string) => {
      if (cmd === 'mcp_status') return Promise.resolve({ running: false, port: 0 });
      if (cmd === 'get_settings') return Promise.resolve({ mcp_port: 8765 });
      if (cmd === 'mcp_get_token') return Promise.resolve('a'.repeat(64));
      if (cmd === 'mcp_regenerate_token') return Promise.resolve('b'.repeat(64));
      return Promise.resolve();
    });

    render(<McpServerSection />);
    await settle();

    await userEvent.click(screen.getByRole('button', { name: 'Show' }));
    await waitFor(() =>
      expect(screen.getByLabelText('Access token')).toHaveValue('a'.repeat(64))
    );

    await userEvent.click(screen.getByRole('button', { name: 'Regenerate' }));

    await waitFor(() =>
      expect(screen.getByLabelText('Access token')).toHaveValue('b'.repeat(64))
    );
  });

  it('rejects a port below the minimum before calling the backend', async () => {
    (invoke as unknown as any).mockImplementation((cmd: string) => {
      if (cmd === 'mcp_status') return Promise.resolve({ running: false, port: 0 });
      if (cmd === 'get_settings') return Promise.resolve({ mcp_port: 8765 });
      return Promise.resolve();
    });

    render(<McpServerSection />);
    await settle();

    await userEvent.clear(screen.getByLabelText('Port'));
    await userEvent.type(screen.getByLabelText('Port'), '80');

    expect(screen.getByRole('button', { name: 'Apply port' })).toBeDisabled();
    expect(invoke).not.toHaveBeenCalledWith('mcp_set_port', expect.anything());
  });
});
