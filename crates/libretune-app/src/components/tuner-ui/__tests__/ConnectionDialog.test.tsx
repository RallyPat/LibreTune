import { render, screen, fireEvent } from '@testing-library/react';
import { vi } from 'vitest';

import { ConnectionDialog } from '../Dialogs';

describe('ConnectionDialog', () => {
  it('calls onRuntimePacketModeChange when user selects a different mode', async () => {
    const onChange = vi.fn();

    render(
      <ConnectionDialog
        isOpen={true}
        onClose={() => {}}
        ports={['/dev/ttyUSB0']}
        selectedPort={'/dev/ttyUSB0'}
        baudRate={115200}
        timeoutMs={2000}
        connected={false}
        connecting={false}
        onPortChange={() => {}}
        onBaudChange={() => {}}
        onTimeoutChange={() => {}}
        onConnect={() => {}}
        onDisconnect={() => {}}
        onRefreshPorts={() => {}}
        runtimePacketMode={'Auto'}
        onRuntimePacketModeChange={onChange}
      />
    );

    // The dialog uses non-associated <label> elements, so find the select by role
    const selects = screen.getAllByRole('combobox');
    // The runtime packet select contains the option with value 'Auto'
    const runtimeSelect = selects.find(s => s.querySelector('option[value="Auto"]')) as HTMLSelectElement;
    expect(runtimeSelect).toBeInTheDocument();

    // Trigger change event (userEvent.selectOptions may not trigger in all environments)
    fireEvent.change(runtimeSelect, { target: { value: 'ForceOCH' } });

    expect(onChange).toHaveBeenCalledWith('ForceOCH');
  });

  it('shows a short explanation of OCH in the connection dialog', () => {
    render(
      <ConnectionDialog
        isOpen={true}
        onClose={() => {}}
        ports={[]}
        selectedPort={''}
        baudRate={115200}
        timeoutMs={2000}
        connected={false}
        connecting={false}
        onPortChange={() => {}}
        onBaudChange={() => {}}
        onTimeoutChange={() => {}}
        onConnect={() => {}}
        onDisconnect={() => {}}
        onRefreshPorts={() => {}}
      />
    );

    expect(screen.getByText(/On-Controller Block Read/)).toBeInTheDocument();
    expect(screen.getByText(/ochGetCommand/)).toBeInTheDocument();
  });

  it('auto-closes when connection succeeds', () => {
    const onClose = vi.fn();
    const { rerender } = render(
      <ConnectionDialog
        isOpen={true}
        onClose={onClose}
        ports={['/dev/ttyUSB0']}
        selectedPort={'/dev/ttyUSB0'}
        baudRate={115200}
        timeoutMs={2000}
        connected={false}
        connecting={false}
        onPortChange={() => {}}
        onBaudChange={() => {}}
        onTimeoutChange={() => {}}
        onConnect={() => {}}
        onDisconnect={() => {}}
        onRefreshPorts={() => {}}
      />
    );

    // Initially, onClose should not be called
    expect(onClose).not.toHaveBeenCalled();

    // Simulate successful connection
    rerender(
      <ConnectionDialog
        isOpen={true}
        onClose={onClose}
        ports={['/dev/ttyUSB0']}
        selectedPort={'/dev/ttyUSB0'}
        baudRate={115200}
        timeoutMs={2000}
        connected={true}
        connecting={false}
        onPortChange={() => {}}
        onBaudChange={() => {}}
        onTimeoutChange={() => {}}
        onConnect={() => {}}
        onDisconnect={() => {}}
        onRefreshPorts={() => {}}
      />
    );

    // After connection succeeds, onClose should be called automatically
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('does not auto-close when still connecting', () => {
    const onClose = vi.fn();
    const { rerender } = render(
      <ConnectionDialog
        isOpen={true}
        onClose={onClose}
        ports={['/dev/ttyUSB0']}
        selectedPort={'/dev/ttyUSB0'}
        baudRate={115200}
        timeoutMs={2000}
        connected={false}
        connecting={false}
        onPortChange={() => {}}
        onBaudChange={() => {}}
        onTimeoutChange={() => {}}
        onConnect={() => {}}
        onDisconnect={() => {}}
        onRefreshPorts={() => {}}
      />
    );

    // Simulate connecting state with connected=true (still syncing)
    rerender(
      <ConnectionDialog
        isOpen={true}
        onClose={onClose}
        ports={['/dev/ttyUSB0']}
        selectedPort={'/dev/ttyUSB0'}
        baudRate={115200}
        timeoutMs={2000}
        connected={true}
        connecting={true}
        onPortChange={() => {}}
        onBaudChange={() => {}}
        onTimeoutChange={() => {}}
        onConnect={() => {}}
        onDisconnect={() => {}}
        onRefreshPorts={() => {}}
      />
    );

    // Should not auto-close while still connecting/syncing
    expect(onClose).not.toHaveBeenCalled();
  });
});
