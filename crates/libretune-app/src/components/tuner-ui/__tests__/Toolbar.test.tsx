import { render, screen } from '@testing-library/react';
import { Toolbar } from '../Toolbar';
import ConnectionMetrics from '../../layout/ConnectionMetrics';
import { ToolbarItem } from '../TunerLayout';

describe('Toolbar', () => {
  it('renders connection-info content with packet mode and metrics (connected)', () => {
    const items: ToolbarItem[] = [
      {
        id: 'connection-info',
        icon: 'connection-info',
        tooltip: 'Connection status and packet mode',
        content: (
          <div className="toolbar-connection-info">
            <ConnectionMetrics compact />
            <span className="packet-mode">Auto</span>
          </div>
        ),
      },
    ];

    const { container } = render(<Toolbar items={items} />);

    // Packet mode label present
    expect(screen.getByText('Auto')).toBeInTheDocument();

    // Connection metrics placeholder (initially renders '—')
    const metricsEl = container.querySelector('.conn-metrics');
    expect(metricsEl).toBeTruthy();

    // Container has toolbar-connection-info wrapper
    const wrapper = container.querySelector('.toolbar-connection-info');
    expect(wrapper).toBeTruthy();
  });

  it('shows placeholder packet mode when disconnected', () => {
    const items: ToolbarItem[] = [
      {
        id: 'connection-info',
        icon: 'connection-info',
        tooltip: 'Connection status and packet mode',
        content: (
          <div className="toolbar-connection-info">
            <ConnectionMetrics compact />
            <span className="packet-mode">—</span>
          </div>
        ),
      },
    ];

    const { container } = render(<Toolbar items={items} />);

    // Ensure the packet mode element explicitly shows the placeholder (avoid matching metrics placeholder)
    const packetModeEl = container.querySelector('.packet-mode');
    expect(packetModeEl).toBeTruthy();
    expect(packetModeEl!.textContent).toBe('—');
    expect(container.querySelector('.conn-metrics')).toBeTruthy();
  });
});
