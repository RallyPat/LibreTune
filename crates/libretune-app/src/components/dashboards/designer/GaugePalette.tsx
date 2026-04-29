/**
 * GaugePalette — Plan v2 / D-7c.
 *
 * A thin sidebar of draggable painter tiles. Each tile, when dragged
 * onto the designer canvas, drops a new placeholder gauge of that
 * painter type. The user can then assign an output_channel via the
 * property editor or by dragging a channel onto the gauge.
 */

import { GaugePainter, SUPPORTED_GAUGE_PAINTERS } from '../dashTypes';

interface Props {
  collapsed?: boolean;
}

const PAINTER_LABELS: Record<GaugePainter, string> = {
  AnalogGauge: 'Analog',
  BasicAnalogGauge: 'Basic Analog',
  CircleAnalogGauge: 'Circle Analog',
  AsymmetricSweepGauge: 'Sweep',
  BasicReadout: 'Digital Readout',
  HorizontalBarGauge: 'Horiz. Bar',
  HorizontalDashedBar: 'Horiz. Dashed',
  VerticalBarGauge: 'Vert. Bar',
  HorizontalLineGauge: 'Horiz. Line',
  VerticalDashedBar: 'Vert. Dashed',
  AnalogBarGauge: 'Analog Bar',
  AnalogMovingBarGauge: 'Moving Bar',
  Histogram: 'Histogram',
  LineGraph: 'Line Graph',
  RoundGauge: 'Round',
  RoundDashedGauge: 'Round Dashed',
  FuelMeter: 'Fuel Meter',
  Tachometer: 'Tachometer',
};

export default function GaugePalette({ collapsed = false }: Props) {
  if (collapsed) return null;
  return (
    <div className="gauge-palette">
      <h4>Gauge Palette</h4>
      <p className="palette-hint">Drag a tile onto the canvas to place a new gauge.</p>
      <div className="palette-grid">
        {SUPPORTED_GAUGE_PAINTERS.map((painter) => (
          <div
            key={painter}
            className="palette-tile"
            draggable
            onDragStart={(e) => {
              e.dataTransfer.effectAllowed = 'copy';
              e.dataTransfer.setData(
                'application/json',
                JSON.stringify({ type: 'painter', painter, label: PAINTER_LABELS[painter] }),
              );
            }}
            title={painter}
          >
            <span className="palette-tile-label">{PAINTER_LABELS[painter]}</span>
            <span className="palette-tile-kind">{painter}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
