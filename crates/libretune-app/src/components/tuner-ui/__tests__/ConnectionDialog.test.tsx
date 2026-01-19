import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
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
});
