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
  const configuredColumns = panel.columns || 1;
  const columns =
    configuredColumns === 1 && panel.readouts.length >= 4
      ? 2
      : configuredColumns;

  return (
    <div
      className="readout-panel readout-panel--compact"
      style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}
    >
      {panel.readouts.map((readout) => {
        const formatted = formatReadoutValue(live[readout.channel], readout.precision);
        const label = readout.units
          ? `${readout.title} (${readout.units})`
          : readout.title;

        return (
          <div key={readout.channel} className="readout-row">
            <span className="readout-row-label">{label}</span>
            <span className="readout-row-value">{formatted}</span>
          </div>
        );
      })}
    </div>
  );
}
