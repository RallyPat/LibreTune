import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import TsGauge from '../../gauges/TsGauge';
import { useChannelValue } from '../../../stores/realtimeStore';
import { toTsGaugeConfig, type SimpleGaugeInfo } from '../../curves/CurveEditor';

export function DialogGauge({ gaugeName }: { gaugeName: string }) {
  const [gaugeInfo, setGaugeInfo] = useState<SimpleGaugeInfo | null>(null);
  const [loadError, setLoadError] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoadError(false);
    invoke<SimpleGaugeInfo>('get_gauge_config', { gaugeName })
      .then((info) => {
        if (!cancelled) setGaugeInfo(info);
      })
      .catch(() => {
        if (!cancelled) {
          setGaugeInfo(null);
          setLoadError(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [gaugeName]);

  const channel = gaugeInfo?.channel ?? '';
  const liveValue = useChannelValue(channel);

  if (loadError) {
    return null;
  }

  if (!gaugeInfo) {
    return <div className="dialog-gauge-widget dialog-gauge-loading">Loading…</div>;
  }

  return (
    <div className="dialog-gauge-widget" title={gaugeInfo.title}>
      <TsGauge config={toTsGaugeConfig(gaugeInfo)} value={liveValue} />
    </div>
  );
}

export function DialogGaugeStack({
  gaugeNames,
  title,
}: {
  gaugeNames: string[];
  title?: string;
}) {
  const visibleGauges = gaugeNames.filter(Boolean);
  if (visibleGauges.length === 0) {
    return null;
  }

  return (
    <div className="dialog-gauge-stack">
      {title && title.trim().length > 0 && (
        <div className="dialog-gauge-stack-title">{title}</div>
      )}
      {visibleGauges.map((name) => (
        <DialogGauge key={name} gaugeName={name} />
      ))}
    </div>
  );
}
