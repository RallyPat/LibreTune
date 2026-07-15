/**
 * LambdaPreviewTable — read-only companion for Target AFR tables.
 *
 * Mirrors the AFR table's values converted to lambda for a selectable fuel
 * blend (E0…E100). Purely a viewing aid: no editing, no effect on the tune.
 */
import { Fragment, useMemo, useState } from 'react';
import './LambdaPreviewTable.css';

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

  const rowOrder = useMemo(() => {
    const order = zValues.map((_, i) => i);
    return yAxisBottom ? order.reverse() : order;
  }, [zValues.length, yAxisBottom]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleFuelChange = (id: string) => {
    setFuelId(id);
    try {
      localStorage.setItem(STORAGE_KEY, id);
    } catch {
      // localStorage unavailable — selection just won't persist
    }
  };

  return (
    <div className="lambda-preview">
      <div className="lambda-preview-header">
        <span className="lambda-preview-title">λ view</span>
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
              <div key={`${x}-${y}`} className="lambda-preview-cell">
                {(afr / fuel.stoich).toFixed(2)}
              </div>
            ))}
          </Fragment>
        ))}
      </div>
    </div>
  );
}
