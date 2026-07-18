import { useChannels } from '../../stores/realtimeStore';
import './WidebandLiveStatus.css';

const CMD_STATUS: Record<number, { label: string; cls: string }> = {
  0: { label: 'Idle', cls: 'idle' },
  1: { label: 'Done', cls: 'done' },
  2: { label: 'Writing…', cls: 'busy' },
  3: { label: 'Failed', cls: 'failed' },
};

/** Infer the configured sensor type from the heater's ESR regulation target
 *  (LSU 4.9 regulates to ~300 ohm, LSU 4.2 to ~80-100 ohm). Only meaningful
 *  once the sensor is heated. */
function inferSensorType(esr: number | undefined, tempC: number | undefined): string {
  if (esr === undefined || tempC === undefined) return '—';
  if (tempC < 500) return 'sensor cold';
  if (esr >= 180 && esr <= 450) return 'LSU 4.9';
  if (esr >= 40 && esr < 160) return 'LSU 4.2';
  return '?';
}

export default function WidebandLiveStatus() {
  const v = useChannels([
    'lambda', 'wb1tempC', 'wb1esr', 'wb1heaterDuty', 'wb1pumpDuty',
    'wb1stateCode', 'canReWidebandCmdStatus',
  ]);

  const status = CMD_STATUS[Math.round(v['canReWidebandCmdStatus'] ?? -1)];
  const fmt = (x: number | undefined, digits = 0) =>
    x === undefined ? '—' : x.toFixed(digits);

  const tiles: Array<[string, string, string?]> = [
    ['λ', fmt(v['lambda'], 2)],
    ['Sensor temp', fmt(v['wb1tempC']), '°C'],
    ['ESR', fmt(v['wb1esr']), 'Ω'],
    ['Heater', fmt(v['wb1heaterDuty']), '%'],
    ['Pump', fmt(v['wb1pumpDuty']), '%'],
    ['State code', fmt(v['wb1stateCode'])],
  ];

  return (
    <div className="wbo-live">
      <div className="wbo-live-title">WBO Live</div>
      <div className={`wbo-live-status wbo-live-status--${status?.cls ?? 'unknown'}`}>
        {status?.label ?? 'No data'}
      </div>
      <div className="wbo-live-tiles">
        {tiles.map(([label, value, unit]) => (
          <div key={label} className="wbo-live-tile">
            <span className="wbo-live-tile-label">{label}</span>
            <span className="wbo-live-tile-value">
              {value}
              {unit && <small> {unit}</small>}
            </span>
          </div>
        ))}
      </div>
      <div className="wbo-live-tile wbo-live-tile--wide">
        <span className="wbo-live-tile-label">Configured as (from ESR)</span>
        <span className="wbo-live-tile-value">{inferSensorType(v['wb1esr'], v['wb1tempC'])}</span>
      </div>
    </div>
  );
}
