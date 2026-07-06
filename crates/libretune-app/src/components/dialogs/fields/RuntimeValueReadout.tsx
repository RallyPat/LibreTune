import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useChannels } from '../../../stores/realtimeStore';
import { formatLiveReadoutValue } from '../../tables/tableLiveChannels';

export function RuntimeValueReadout({
  label,
  channel,
  visibilityCondition,
  context,
}: {
  label: string;
  channel: string;
  visibilityCondition?: string;
  context: Record<string, number>;
}) {
  const [visible, setVisible] = useState(true);
  const live = useChannels([channel]);
  const value = live[channel];

  useEffect(() => {
    if (!visibilityCondition) {
      setVisible(true);
      return;
    }

    let expression = visibilityCondition;
    if (!expression.includes('{') && !expression.includes('(') && !expression.includes(' ')) {
      expression = `{${expression}}`;
    }

    invoke<boolean>('evaluate_expression', { expression, context })
      .then(setVisible)
      .catch(() => setVisible(true));
  }, [visibilityCondition, context]);

  if (!visible) return null;

  const kind = channel.match(/^userTableOutput\d$/i) ? 'output' : 'axis';

  return (
    <div className="runtime-value-row">
      <span className="runtime-value-label">{label}</span>
      <span className="runtime-value-display">
        {formatLiveReadoutValue(value, kind, channel)}
      </span>
    </div>
  );
}
