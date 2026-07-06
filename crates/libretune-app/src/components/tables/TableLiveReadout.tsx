import { useMemo } from 'react';
import { useChannels } from '../../stores/realtimeStore';
import {
  formatLiveReadoutValue,
  isGppwmTable,
  resolveGppwmOutputChannel,
} from './tableLiveChannels';

export interface TableLiveReadoutProps {
  tableName: string;
  xLabel?: string;
  yLabel?: string;
  xChannel?: string | null;
  yChannel?: string | null;
}

interface LiveRow {
  key: string;
  label: string;
  value: string;
  highlight?: boolean;
}

export default function TableLiveReadout({
  tableName,
  xLabel = 'X',
  yLabel = 'Y',
  xChannel,
  yChannel,
}: TableLiveReadoutProps) {
  const outputChannel = useMemo(
    () => resolveGppwmOutputChannel(tableName, xChannel, yChannel),
    [tableName, xChannel, yChannel],
  );

  const gppwm = isGppwmTable(tableName, xChannel, yChannel);

  const channelNames = useMemo(() => {
    const names: string[] = [];
    if (xChannel) names.push(xChannel);
    if (yChannel) names.push(yChannel);
    if (outputChannel) names.push(outputChannel);
    return names;
  }, [xChannel, yChannel, outputChannel]);

  const live = useChannels(channelNames);

  const rows = useMemo((): LiveRow[] => {
    const items: LiveRow[] = [];

    if (gppwm && outputChannel) {
      items.push({
        key: 'switch',
        label: 'switch',
        value: formatLiveReadoutValue(live[outputChannel], 'switch'),
      });
    }

    if (xChannel) {
      items.push({
        key: 'x',
        label: gppwm ? 'x' : xLabel,
        value: formatLiveReadoutValue(live[xChannel], 'axis', xChannel),
      });
    }

    if (yChannel) {
      items.push({
        key: 'y',
        label: gppwm ? 'y' : yLabel,
        value: formatLiveReadoutValue(live[yChannel], 'axis', yChannel),
      });
    }

    if (outputChannel) {
      items.push({
        key: 'output',
        label: 'output',
        value: formatLiveReadoutValue(live[outputChannel], 'output', outputChannel),
        highlight: true,
      });
    }

    return items;
  }, [gppwm, xChannel, yChannel, outputChannel, xLabel, yLabel, live]);

  if (rows.length === 0) return null;

  const hasData = channelNames.some((name) => live[name] !== undefined);

  return (
    <aside className="table-live-readout" aria-label="Live table values">
      {!hasData && <div className="table-live-readout-hint">Connect ECU for live values</div>}
      <dl className="table-live-readout-list">
        {rows.map((row) => (
          <div key={row.key} className={`table-live-readout-row${row.highlight ? ' is-highlight' : ''}`}>
            <dt className="table-live-readout-label">{row.label}</dt>
            <dd className="table-live-readout-value">{row.value}</dd>
          </div>
        ))}
      </dl>
    </aside>
  );
}
