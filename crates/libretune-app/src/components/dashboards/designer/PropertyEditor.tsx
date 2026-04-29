import { DashComponent, TsGaugeConfig, TsIndicatorConfig, isGauge, isIndicator } from '../dashTypes';

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
            <input
              type="number"
              value={gauge.min}
              onChange={(e) => updateGauge({ min: parseFloat(e.target.value) || 0 })}
            />
          </div>
          <div className="property-group half">
            <label>Max</label>
            <input
              type="number"
              value={gauge.max}
              onChange={(e) => updateGauge({ max: parseFloat(e.target.value) || 100 })}
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
            <input
              type="number"
              value={gauge.high_warning ?? ''}
              onChange={(e) => updateGauge({ high_warning: e.target.value ? parseFloat(e.target.value) : null })}
            />
          </div>
          <div className="property-group half">
            <label>Critical</label>
            <input
              type="number"
              value={gauge.high_critical ?? ''}
              onChange={(e) => updateGauge({ high_critical: e.target.value ? parseFloat(e.target.value) : null })}
            />
          </div>
        </div>

        <div className="property-group">
          <label>Gauge Type</label>
          <select
            value={gauge.gauge_painter || 'AnalogGauge'}
            onChange={(e) => updateGauge({ gauge_painter: e.target.value as TsGaugeConfig['gauge_painter'] })}
          >
            <option value="AnalogGauge">Analog Gauge</option>
            <option value="BasicAnalogGauge">Basic Analog Gauge</option>
            <option value="CircleAnalogGauge">Circle Analog Gauge</option>
            <option value="BasicReadout">Digital Readout</option>
            <option value="HorizontalBarGauge">Horizontal Bar</option>
            <option value="HorizontalDashedBar">Horizontal Dashed Bar</option>
            <option value="VerticalBarGauge">Vertical Bar</option>
            <option value="VerticalDashedBar">Vertical Dashed Bar</option>
            <option value="HorizontalLineGauge">Horizontal Line</option>
            <option value="AnalogBarGauge">Analog Bar</option>
            <option value="AnalogMovingBarGauge">Analog Moving Bar</option>
            <option value="AsymmetricSweepGauge">Sweep Gauge</option>
            <option value="RoundGauge">Round Gauge</option>
            <option value="RoundDashedGauge">Round Dashed Gauge</option>
            <option value="Tachometer">Tachometer</option>
            <option value="FuelMeter">Fuel Meter</option>
            <option value="LineGraph">Line Graph</option>
            <option value="Histogram">Histogram</option>
          </select>
        </div>

        <div className="property-group">
          <label>Digits</label>
          <input
            type="number"
            min={0}
            max={5}
            value={gauge.value_digits ?? 1}
            onChange={(e) => updateGauge({ value_digits: parseInt(e.target.value) || 0 })}
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

        <div className="property-section">
          <h4>Position & Size</h4>
          <div className="property-row">
            <div className="property-group half">
              <label>X (%)</label>
              <input
                type="number"
                step={0.01}
                value={((gauge.relative_x ?? 0) * 100).toFixed(1)}
                onChange={(e) => updateGauge({ relative_x: parseFloat(e.target.value) / 100 })}
              />
            </div>
            <div className="property-group half">
              <label>Y (%)</label>
              <input
                type="number"
                step={0.01}
                value={((gauge.relative_y ?? 0) * 100).toFixed(1)}
                onChange={(e) => updateGauge({ relative_y: parseFloat(e.target.value) / 100 })}
              />
            </div>
          </div>
          <div className="property-row">
            <div className="property-group half">
              <label>Width (%)</label>
              <input
                type="number"
                step={0.01}
                value={((gauge.relative_width ?? 0.25) * 100).toFixed(1)}
                onChange={(e) => updateGauge({ relative_width: parseFloat(e.target.value) / 100 })}
              />
            </div>
            <div className="property-group half">
              <label>Height (%)</label>
              <input
                type="number"
                step={0.01}
                value={((gauge.relative_height ?? 0.25) * 100).toFixed(1)}
                onChange={(e) => updateGauge({ relative_height: parseFloat(e.target.value) / 100 })}
              />
            </div>
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
            <input
              type="number"
              step={0.1}
              value={gauge.hysteresis ?? ''}
              placeholder="0"
              onChange={(e) =>
                updateGauge({ hysteresis: e.target.value === '' ? null : parseFloat(e.target.value) })
              }
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
            <div className="property-group half">
              <label>X (%)</label>
              <input
                type="number"
                step={0.01}
                value={((indicator.relative_x ?? 0) * 100).toFixed(1)}
                onChange={(e) => updateIndicator({ relative_x: parseFloat(e.target.value) / 100 })}
              />
            </div>
            <div className="property-group half">
              <label>Y (%)</label>
              <input
                type="number"
                step={0.01}
                value={((indicator.relative_y ?? 0) * 100).toFixed(1)}
                onChange={(e) => updateIndicator({ relative_y: parseFloat(e.target.value) / 100 })}
              />
            </div>
          </div>
          <div className="property-row">
            <div className="property-group half">
              <label>Width (%)</label>
              <input
                type="number"
                step={0.01}
                value={((indicator.relative_width ?? 0.1) * 100).toFixed(1)}
                onChange={(e) => updateIndicator({ relative_width: parseFloat(e.target.value) / 100 })}
              />
            </div>
            <div className="property-group half">
              <label>Height (%)</label>
              <input
                type="number"
                step={0.01}
                value={((indicator.relative_height ?? 0.05) * 100).toFixed(1)}
                onChange={(e) => updateIndicator({ relative_height: parseFloat(e.target.value) / 100 })}
              />
            </div>
          </div>
        </div>
      </div>
    );
  }

  return <p>Unknown component type</p>;
}
