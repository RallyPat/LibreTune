import { useChannelValue } from '../../stores/realtimeStore';
import type { SimpleGaugeInfo } from '../curves/CurveEditor';
import type { TsGaugeConfig } from '../dashboards/dashTypes';

function formatGaugeValue(value: number | undefined, digits: number): string {
  if (value === undefined || Number.isNaN(value)) return '—';
  if (digits <= 0) return String(Math.round(value));
  return value.toFixed(digits);
}

export interface GaugeLiveReadoutProps {
  gaugeInfo?: SimpleGaugeInfo | null;
  gaugeConfig?: TsGaugeConfig | null;
  /** Override live value (e.g. curve X-axis output channel). */
  value?: number;
  className?: string;
}

/** Compact live numeric readout for INI-referenced gauges in dialogs and curves. */
export function GaugeLiveReadout({
  gaugeInfo,
  gaugeConfig,
  value,
  className,
}: GaugeLiveReadoutProps) {
  const channel = gaugeConfig?.output_channel ?? gaugeInfo?.channel ?? '';
  const label = gaugeConfig?.title ?? gaugeInfo?.title ?? channel;
  const units = gaugeConfig?.units ?? gaugeInfo?.units ?? '';
  const digits = gaugeConfig?.value_digits ?? gaugeInfo?.digits ?? 1;

  const channelValue = useChannelValue(channel, undefined);
  const liveValue = value ?? channelValue;
  const formatted = formatGaugeValue(liveValue, digits);
  const display = units.trim().length > 0 ? `${formatted} ${units}` : formatted;

  const classes = ['gauge-live-readout', 'runtime-value-row', className].filter(Boolean).join(' ');

  return (
    <div className={classes} title={label}>
      <span className="runtime-value-label">{label}</span>
      <span className="runtime-value-display">{display}</span>
    </div>
  );
}
