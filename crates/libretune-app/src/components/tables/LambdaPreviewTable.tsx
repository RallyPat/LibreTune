/**
 * LambdaPreviewTable — read-only companion for Target AFR tables.
 *
 * Mirrors the AFR table's values converted to lambda for a selectable fuel
 * blend (E0…E100). Purely a viewing aid: no editing, no effect on the tune.
 */
import { Fragment, useEffect, useMemo, useRef, useState, type CSSProperties } from 'react';
import { useChannels } from '../../stores/realtimeStore';
import { useHeatmapSettings } from '../../utils/useHeatmapSettings';
import './LambdaPreviewTable.css';

/** Live λ and AFR readouts, isolated so realtime updates only re-render
 *  the gauges and not the whole preview grid. */
function LiveLambdaAfrGauges() {
  const values = useChannels(['lambda', 'afr']);
  const lambda = values['lambda'];
  const afr = values['afr'];

  // The ECU's effective stoich ratio is AFR/λ; the ethanol blend follows
  // from where that sits between E0 (14.7:1) and E100 (9.0:1), linearly.
  let fuelBlend: string = '—';
  if (lambda !== undefined && afr !== undefined && lambda > 0.01) {
    const stoich = afr / lambda;
    const ethanol = ((14.7 - stoich) / (14.7 - 9.0)) * 100;
    fuelBlend = `E${Math.round(Math.min(100, Math.max(0, ethanol)))}`;
  }

  return (
    <div className="lambda-preview-gauges">
      <div className="lambda-preview-gauge">
        <span className="lambda-preview-gauge-label">λ</span>
        <span className="lambda-preview-gauge-value">
          {lambda !== undefined ? lambda.toFixed(2) : '—'}
        </span>
      </div>
      <div className="lambda-preview-gauge">
        <span className="lambda-preview-gauge-label">AFR</span>
        <span className="lambda-preview-gauge-value">
          {afr !== undefined ? afr.toFixed(1) : '—'}
        </span>
      </div>
      <div className="lambda-preview-gauge" title="Ethanol blend derived from AFR ÷ λ (effective stoich ratio)">
        <span className="lambda-preview-gauge-label">calculated fuel</span>
        <span className="lambda-preview-gauge-value">{fuelBlend}</span>
      </div>
    </div>
  );
}

/** Stoichiometric AFR per ethanol blend (linear gasoline/ethanol mix) */
const FUEL_STOICH: Array<{ id: string; label: string; stoich: number }> = [
  { id: 'e0', label: 'E0', stoich: 14.7 },
  { id: 'e10', label: 'E10', stoich: 14.13 },
  { id: 'e30', label: 'E30', stoich: 12.99 },
  { id: 'e85', label: 'E85', stoich: 9.85 },
  { id: 'e100', label: 'E100', stoich: 9.0 },
];

const STORAGE_KEY = 'libretune-lambda-preview-fuel';

interface LambdaPreviewTableProps {
  /** AFR values [row][col] — mirrored live from the editable table */
  zValues: number[][];
  xBins: number[];
  yBins: number[];
  /** Match the main table's row orientation */
  yAxisBottom?: boolean;
}

export default function LambdaPreviewTable({
  zValues,
  xBins,
  yBins,
  yAxisBottom = false,
}: LambdaPreviewTableProps) {
  const [fuelId, setFuelId] = useState<string>(
    () => localStorage.getItem(STORAGE_KEY) ?? 'e0',
  );
  const fuel = FUEL_STOICH.find((f) => f.id === fuelId) ?? FUEL_STOICH[0];

  // Pin the panel's height to the main table grid so both read as one unit.
  const panelRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const panel = panelRef.current;
    const mainGrid = panel?.parentElement?.querySelector('.table-grid-container');
    if (!panel || !mainGrid) return;
    const sync = () => {
      panel.style.height = `${(mainGrid as HTMLElement).offsetHeight}px`;
    };
    sync();
    const observer = new ResizeObserver(sync);
    observer.observe(mainGrid);
    return () => observer.disconnect();
  }, []);

  const rowOrder = useMemo(() => {
    const order = zValues.map((_, i) => i);
    return yAxisBottom ? order.reverse() : order;
  }, [zValues.length, yAxisBottom]); // eslint-disable-line react-hooks/exhaustive-deps

  // Muted heatmap: same scheme as the main table, blended into the dark
  // background so the companion doesn't compete visually.
  const { getColor: getHeatmapColor } = useHeatmapSettings();
  const [minVal, maxVal] = useMemo(() => {
    const flat = zValues.flat();
    return flat.length > 0 ? [Math.min(...flat), Math.max(...flat)] : [0, 1];
  }, [zValues]);
  const cellStyle = (afr: number): CSSProperties => {
    if (minVal === maxVal) return {};
    const heat = getHeatmapColor(afr, minVal, maxVal, 'value');
    return { background: `color-mix(in srgb, ${heat} 35%, var(--bg-primary, #10141f))` };
  };

  const handleFuelChange = (id: string) => {
    setFuelId(id);
    try {
      localStorage.setItem(STORAGE_KEY, id);
    } catch {
      // localStorage unavailable — selection just won't persist
    }
  };

  return (
    <div className="lambda-preview" ref={panelRef}>
      <div className="lambda-preview-header">
        <span className="lambda-preview-title">λ view</span>
        <LiveLambdaAfrGauges />
        <select
          value={fuel.id}
          onChange={(e) => handleFuelChange(e.target.value)}
          title={`Stoichiometric AFR ${fuel.stoich}:1`}
        >
          {FUEL_STOICH.map((f) => (
            <option key={f.id} value={f.id}>
              {f.label} ({f.stoich}:1)
            </option>
          ))}
        </select>
      </div>
      <div
        className="lambda-preview-grid"
        style={{ gridTemplateColumns: `auto repeat(${xBins.length}, 1fr)` }}
      >
        <div className="lambda-preview-corner" />
        {xBins.map((x, i) => (
          <div key={`x-${i}`} className="lambda-preview-axis">
            {x}
          </div>
        ))}
        {rowOrder.map((y) => (
          <Fragment key={`row-${y}`}>
            <div className="lambda-preview-axis">{yBins[y]}</div>
            {zValues[y]?.map((afr, x) => (
              <div key={`${x}-${y}`} className="lambda-preview-cell" style={cellStyle(afr)}>
                {(afr / fuel.stoich).toFixed(2)}
              </div>
            ))}
          </Fragment>
        ))}
      </div>
    </div>
  );
}
