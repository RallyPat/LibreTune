import { useMemo } from 'react';
import { useChannels } from '../../../stores/realtimeStore';
import type { ReadoutPanel } from '../types';

function formatReadoutValue(
  value: number | undefined,
  precision: number,
): string {
  if (value === undefined || Number.isNaN(value)) return '—';
  if (precision <= 0) {
    return String(Math.round(value));
  }
  return value.toFixed(precision);
}

export function ReadoutPanelRenderer({ panel }: { panel: ReadoutPanel }) {
  const channelNames = useMemo(
    () => panel.readouts.map((r) => r.channel),
    [panel.readouts],
  );
  const live = useChannels(channelNames);
  const columns = panel.columns || 1;

  return (
    <div
      className="readout-panel"
      style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}
    >
      {panel.readouts.map((readout) => {
        const formatted = formatReadoutValue(live[readout.channel], readout.precision);
        const label = readout.units
          ? `${readout.title} (${readout.units})`
          : readout.title;

        return (
          <div key={readout.channel} className="readout-gauge">
            <div className="readout-gauge-value">{formatted}</div>
            <div className="readout-gauge-label">{label}</div>
          </div>
        );
      })}
    </div>
  );
}
