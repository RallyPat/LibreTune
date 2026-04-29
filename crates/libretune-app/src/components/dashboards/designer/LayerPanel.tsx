/**
 * LayerPanel — Plan v2 / D-7b.
 *
 * Lists every component on the dashboard and lets the user:
 *  - select/focus a component (single-click)
 *  - reorder z-stack (▲/▼ buttons; later index = drawn on top)
 *  - toggle visibility via an `enabled_condition` of "false" / cleared
 *  - delete
 *
 * Z-ordering uses the array order in `gauge_cluster.components` since
 * the dashboard renders strictly in that sequence.
 */

import { DashFile, DashComponent, isGauge, isIndicator } from '../dashTypes';

interface Props {
  dashFile: DashFile;
  selectedGaugeId: string | null;
  onSelect: (id: string | null) => void;
  onChange: (file: DashFile) => void;
}

function componentId(c: DashComponent): string {
  if (isGauge(c)) return c.Gauge.id;
  if (isIndicator(c)) return c.Indicator.id;
  return '';
}

function componentLabel(c: DashComponent): string {
  if (isGauge(c)) return c.Gauge.title || c.Gauge.output_channel || c.Gauge.id;
  if (isIndicator(c)) return c.Indicator.on_text || c.Indicator.output_channel || c.Indicator.id;
  return '?';
}

function componentKind(c: DashComponent): string {
  if (isGauge(c)) return c.Gauge.gauge_painter;
  if (isIndicator(c)) return c.Indicator.indicator_painter;
  return '';
}

function isHidden(c: DashComponent): boolean {
  const cond = isGauge(c)
    ? c.Gauge.enabled_condition
    : isIndicator(c)
      ? c.Indicator.enabled_condition
      : null;
  return cond?.trim().toLowerCase() === 'false';
}

function withHidden(c: DashComponent, hidden: boolean): DashComponent {
  const cond = hidden ? 'false' : null;
  if (isGauge(c)) return { Gauge: { ...c.Gauge, enabled_condition: cond } };
  if (isIndicator(c)) return { Indicator: { ...c.Indicator, enabled_condition: cond } };
  return c;
}

export default function LayerPanel({ dashFile, selectedGaugeId, onSelect, onChange }: Props) {
  const components = dashFile.gauge_cluster.components;

  const replaceComponents = (next: DashComponent[]) => {
    onChange({
      ...dashFile,
      gauge_cluster: { ...dashFile.gauge_cluster, components: next },
    });
  };

  const move = (from: number, to: number) => {
    if (to < 0 || to >= components.length || from === to) return;
    const next = [...components];
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    replaceComponents(next);
  };

  const toggleHidden = (i: number) => {
    const next = [...components];
    next[i] = withHidden(next[i], !isHidden(next[i]));
    replaceComponents(next);
  };

  const remove = (i: number) => {
    const next = [...components];
    next.splice(i, 1);
    replaceComponents(next);
    if (selectedGaugeId && componentId(components[i]) === selectedGaugeId) {
      onSelect(null);
    }
  };

  return (
    <div className="layer-panel">
      <h4>Layers</h4>
      {components.length === 0 ? (
        <p className="no-selection">No components yet</p>
      ) : (
        <ul className="layer-list">
          {/* Render top-down (last item = top of z-stack). */}
          {components.map((_c, i) => i).reverse().map((i) => {
            const c = components[i];
            const id = componentId(c);
            const isSel = id === selectedGaugeId;
            const hidden = isHidden(c);
            return (
              <li
                key={id || `idx-${i}`}
                className={`layer-row ${isSel ? 'selected' : ''} ${hidden ? 'hidden' : ''}`}
                onClick={() => onSelect(id || null)}
                title={`${componentKind(c)} — index ${i}`}
              >
                <span className="layer-name">{componentLabel(c)}</span>
                <span className="layer-kind">{componentKind(c)}</span>
                <button
                  type="button"
                  title="Move up (toward top of stack)"
                  onClick={(e) => { e.stopPropagation(); move(i, i + 1); }}
                  disabled={i === components.length - 1}
                >▲</button>
                <button
                  type="button"
                  title="Move down (toward bottom of stack)"
                  onClick={(e) => { e.stopPropagation(); move(i, i - 1); }}
                  disabled={i === 0}
                >▼</button>
                <button
                  type="button"
                  title={hidden ? 'Show' : 'Hide'}
                  onClick={(e) => { e.stopPropagation(); toggleHidden(i); }}
                >{hidden ? '○' : '●'}</button>
                <button
                  type="button"
                  title="Delete"
                  onClick={(e) => { e.stopPropagation(); remove(i); }}
                >✕</button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
