import { DashComponent, TsGaugeConfig, TsIndicatorConfig, isGauge, isIndicator, PAINTER_LABELS, SUPPORTED_GAUGE_PAINTERS } from '../dashTypes';
import PercentField from './PercentField';
import NumberField from './NumberField';

interface Props {
  component: DashComponent;
  onChange: (component: DashComponent) => void;
}

/**
 * Property editor panel for gauges and indicators.
 * Extracted from DashboardDesigner during Phase D.
 */
export default function PropertyEditor({ component, onChange }: Props) {
  if (isGauge(component)) {
    const gauge = component.Gauge;

    const updateGauge = (updates: Partial<TsGaugeConfig>) => {
      onChange({ Gauge: { ...gauge, ...updates } });
    };

    return (
      <div className="property-editor">
        <div className="property-group">
          <label>Title</label>
          <input
            type="text"
            value={gauge.title || ''}
            onChange={(e) => updateGauge({ title: e.target.value })}
          />
        </div>

        <div className="property-group">
          <label>Output Channel</label>
          <input
            type="text"
            value={gauge.output_channel}
            onChange={(e) => updateGauge({ output_channel: e.target.value })}
          />
        </div>

        <div className="property-row">
          <div className="property-group half">
            <label>Min</label>
            <NumberField
              label="Min"
              value={gauge.min}
              onChange={(v) => updateGauge({ min: v ?? 0 })}
            />
          </div>
          <div className="property-group half">
            <label>Max</label>
            <NumberField
              label="Max"
              value={gauge.max}
              onChange={(v) => updateGauge({ max: v ?? 100 })}
            />
          </div>
        </div>

        <div className="property-group">
          <label>Units</label>
          <input
            type="text"
            value={gauge.units || ''}
            onChange={(e) => updateGauge({ units: e.target.value })}
          />
        </div>

        <div className="property-row">
          <div className="property-group half">
            <label>Warning</label>
            <NumberField
              label="Warning"
              value={gauge.high_warning}
              nullable
              onChange={(v) => updateGauge({ high_warning: v })}
            />
          </div>
          <div className="property-group half">
            <label>Critical</label>
            <NumberField
              label="Critical"
              value={gauge.high_critical}
              nullable
              onChange={(v) => updateGauge({ high_critical: v })}
            />
          </div>
        </div>

        <div className="property-group">
          <label>Gauge Type</label>
          <select
            value={gauge.gauge_painter || 'AnalogGauge'}
            onChange={(e) => updateGauge({ gauge_painter: e.target.value as TsGaugeConfig['gauge_painter'] })}
          >
            {SUPPORTED_GAUGE_PAINTERS.map((painter) => (
              <option key={painter} value={painter}>{PAINTER_LABELS[painter]}</option>
            ))}
          </select>
        </div>

        <div className="property-group">
          <label>Digits</label>
          <NumberField
            label="Digits"
            integer
            min={0}
            max={5}
            value={gauge.value_digits ?? 1}
            onChange={(v) => updateGauge({ value_digits: v ?? 0 })}
          />
        </div>

        <div className="property-group">
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={gauge.shape_locked_to_aspect ?? false}
              onChange={(e) => updateGauge({ shape_locked_to_aspect: e.target.checked })}
            />
            Lock Aspect Ratio
          </label>
        </div>

        {gauge.gauge_painter === 'MultiChannelTrend' && (
          <TrendSeriesEditor gauge={gauge} updateGauge={updateGauge} />
        )}

        <div className="property-section">
          <h4>Position & Size</h4>
          <div className="property-row">
            <PercentField
              label="X (%)"
              value={gauge.relative_x ?? 0}
              onChange={(v) => updateGauge({ relative_x: v })}
            />
            <PercentField
              label="Y (%)"
              value={gauge.relative_y ?? 0}
              onChange={(v) => updateGauge({ relative_y: v })}
            />
          </div>
          <div className="property-row">
            <PercentField
              label="Width (%)"
              value={gauge.relative_width ?? 0.25}
              onChange={(v) => updateGauge({ relative_width: v })}
            />
            <PercentField
              label="Height (%)"
              value={gauge.relative_height ?? 0.25}
              onChange={(v) => updateGauge({ relative_height: v })}
            />
          </div>
        </div>

        <div className="property-section">
          <h4>Conditions & Behavior</h4>
          <div className="property-group">
            <label title="INI-style boolean expression. Gauge is hidden when false. Empty = always shown.">
              Enabled Condition
            </label>
            <input
              type="text"
              placeholder="e.g. hasLambdaSensor"
              value={gauge.enabled_condition ?? ''}
              onChange={(e) =>
                updateGauge({ enabled_condition: e.target.value.trim() ? e.target.value : null })
              }
            />
          </div>
          <div className="property-group">
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={gauge.peak_hold ?? false}
                onChange={(e) => updateGauge({ peak_hold: e.target.checked })}
              />
              Peak Hold (show all-time max marker)
            </label>
          </div>
          <div className="property-group">
            <label title="Channel-units deadband on warning/critical state transitions.">
              Hysteresis
            </label>
            <NumberField
              label="Hysteresis"
              value={gauge.hysteresis}
              nullable
              step={0.1}
              placeholder="0"
              onChange={(v) => updateGauge({ hysteresis: v })}
            />
          </div>
        </div>
      </div>
    );
  }

  if (isIndicator(component)) {
    const indicator = component.Indicator;

    const updateIndicator = (updates: Partial<TsIndicatorConfig>) => {
      onChange({ Indicator: { ...indicator, ...updates } });
    };

    return (
      <div className="property-editor">
        <div className="property-group">
          <label>Output Channel</label>
          <input
            type="text"
            value={indicator.output_channel}
            onChange={(e) => updateIndicator({ output_channel: e.target.value })}
          />
        </div>

        <div className="property-group">
          <label>On Label</label>
          <input
            type="text"
            value={indicator.on_text || ''}
            onChange={(e) => updateIndicator({ on_text: e.target.value })}
          />
        </div>

        <div className="property-group">
          <label>Off Label</label>
          <input
            type="text"
            value={indicator.off_text || ''}
            onChange={(e) => updateIndicator({ off_text: e.target.value })}
          />
        </div>

        <div className="property-group">
          <label>Indicator Type</label>
          <select
            value={indicator.indicator_painter || 'BasicRectangleIndicator'}
            onChange={(e) => updateIndicator({ indicator_painter: e.target.value as TsIndicatorConfig['indicator_painter'] })}
          >
            <option value="BasicRectangleIndicator">Rectangle</option>
            <option value="BulbIndicator">Bulb</option>
            <option value="Led">LED</option>
          </select>
        </div>

        <div className="property-group">
          <label title="INI-style boolean expression. Indicator is hidden when false.">
            Enabled Condition
          </label>
          <input
            type="text"
            placeholder="e.g. hasIacStepper"
            value={indicator.enabled_condition ?? ''}
            onChange={(e) =>
              updateIndicator({ enabled_condition: e.target.value.trim() ? e.target.value : null })
            }
          />
        </div>

        <div className="property-section">
          <h4>Position & Size</h4>
          <div className="property-row">
            <PercentField
              label="X (%)"
              value={indicator.relative_x ?? 0}
              onChange={(v) => updateIndicator({ relative_x: v })}
            />
            <PercentField
              label="Y (%)"
              value={indicator.relative_y ?? 0}
              onChange={(v) => updateIndicator({ relative_y: v })}
            />
          </div>
          <div className="property-row">
            <PercentField
              label="Width (%)"
              value={indicator.relative_width ?? 0.1}
              onChange={(v) => updateIndicator({ relative_width: v })}
            />
            <PercentField
              label="Height (%)"
              value={indicator.relative_height ?? 0.05}
              onChange={(v) => updateIndicator({ relative_height: v })}
            />
          </div>
        </div>
      </div>
    );
  }

  return <p>Unknown component type</p>;
}

/**
 * Editor for the extra overlay series on a `MultiChannelTrend` gauge.
 * Series 1 is always the gauge's own Output Channel / Title / Min / Max
 * (edited above); slots 2-3 are stored as `lt_seriesN_*` keys in
 * `extra_attrs` so they round-trip through the `.ltdash.xml` format
 * without requiring a schema change.
 */
function TrendSeriesEditor({
  gauge,
  updateGauge,
}: {
  gauge: TsGaugeConfig;
  updateGauge: (updates: Partial<TsGaugeConfig>) => void;
}) {
  const attrs = gauge.extra_attrs || {};

  const setAttr = (key: string, value: string) => {
    const next = { ...attrs };
    if (value) next[key] = value;
    else delete next[key];
    updateGauge({ extra_attrs: next });
  };

  const seriesSlots = [2, 3] as const;

  return (
    <div className="property-section">
      <h4>Trend Series (Overlay Channels)</h4>
      <p className="property-hint">
        The chart always plots the Output Channel above as series 1. Add up to two more
        channels here to overlay on the same graph, each scaled to its own min/max.
      </p>
      {seriesSlots.map((n) => (
        <div key={n} className="property-subsection">
          <label className="property-subsection-label">Series {n}</label>
          <div className="property-row">
            <div className="property-group half">
              <label>Channel</label>
              <input
                type="text"
                placeholder="e.g. boost"
                value={attrs[`lt_series${n}_channel`] || ''}
                onChange={(e) => setAttr(`lt_series${n}_channel`, e.target.value)}
              />
            </div>
            <div className="property-group half">
              <label>Label</label>
              <input
                type="text"
                placeholder="e.g. BOOST"
                value={attrs[`lt_series${n}_label`] || ''}
                onChange={(e) => setAttr(`lt_series${n}_label`, e.target.value)}
              />
            </div>
          </div>
          <div className="property-row">
            <div className="property-group half">
              <label>Min</label>
              <input
                type="number"
                value={attrs[`lt_series${n}_min`] ?? ''}
                onChange={(e) => setAttr(`lt_series${n}_min`, e.target.value)}
              />
            </div>
            <div className="property-group half">
              <label>Max</label>
              <input
                type="number"
                value={attrs[`lt_series${n}_max`] ?? ''}
                onChange={(e) => setAttr(`lt_series${n}_max`, e.target.value)}
              />
            </div>
          </div>
          <div className="property-group">
            <label>Color</label>
            <input
              type="color"
              value={attrs[`lt_series${n}_color`] || '#94a3b8'}
              onChange={(e) => setAttr(`lt_series${n}_color`, e.target.value)}
            />
          </div>
        </div>
      ))}
    </div>
  );
}
