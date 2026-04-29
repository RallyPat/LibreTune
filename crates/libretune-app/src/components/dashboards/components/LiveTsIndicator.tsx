/**
 * LiveTsIndicator — wraps `TsIndicator` with a per-channel store
 * subscription so each indicator re-renders only when its own channel
 * changes (preventing dashboard-wide re-render cascades at 20Hz).
 */

import React from 'react';
import { useChannelValue } from '../../../stores/realtimeStore';
import TsIndicator from '../../gauges/TsIndicator';
import type { TsIndicatorConfig } from '../dashTypes';

interface LiveTsIndicatorProps {
  config: TsIndicatorConfig;
  embeddedImages?: Map<string, string>;
}

const LiveTsIndicator = React.memo(function LiveTsIndicator({
  config,
  embeddedImages,
}: LiveTsIndicatorProps) {
  const liveValue = useChannelValue(config.output_channel, config.value);
  const isOn = liveValue !== 0;
  return <TsIndicator config={config} isOn={isOn} embeddedImages={embeddedImages} />;
});

export default LiveTsIndicator;
