import { useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useRealtimeStore } from '../../../stores/realtimeStore';
import {
  evaluateIndicatorExpression,
  extractExpressionVariables,
} from '../../../utils/evaluateIndicatorExpression';
import { resolveIndicatorColor } from '../../../utils/indicatorColors';
import type { IndicatorPanel } from '../types';

type IndicatorDef = IndicatorPanel['indicators'][number];

function usesStatusTiles(indicators: IndicatorDef[]): boolean {
  return indicators.some(
    (ind) =>
      ind.color_off_fg ||
      ind.color_on_fg ||
      ind.color_off_bg ||
      ind.color_on_bg,
  );
}

function tileStyle(ind: IndicatorDef, isOn: boolean): React.CSSProperties {
  const background =
    resolveIndicatorColor(isOn ? ind.color_on_fg : ind.color_off_fg) ??
    (isOn ? 'var(--success)' : 'var(--border-strong)');
  const color =
    resolveIndicatorColor(isOn ? ind.color_on_bg : ind.color_off_bg) ?? '#000';

  return {
    background,
    color,
  };
}

/// Renders an `IndicatorPanel`. Panels with INI color definitions use
/// TunerStudio-style status tiles; others use compact LED + label rows.
export function IndicatorPanelRenderer({
  panel,
  context,
}: {
  panel: IndicatorPanel;
  context: Record<string, number>;
}) {
  const usedVars = useMemo(
    () => extractExpressionVariables(panel.indicators.map((ind) => ind.expression)),
    [panel.indicators],
  );

  const realtimeSlice = useRealtimeStore(
    useShallow((state) => {
      const slice: Record<string, number> = {};
      for (const name of usedVars) {
        const value = state.channels[name];
        if (value !== undefined) {
          slice[name] = value;
        }
      }
      return slice;
    }),
  );

  const indicatorValues = useMemo(() => {
    const values: Record<string, boolean> = {};
    for (const ind of panel.indicators) {
      values[ind.expression] = evaluateIndicatorExpression(
        ind.expression,
        realtimeSlice,
        context,
      );
    }
    return values;
  }, [panel.indicators, realtimeSlice, context]);

  const statusTiles = useMemo(
    () => usesStatusTiles(panel.indicators),
    [panel.indicators],
  );

  const columns = panel.columns || 2;
  const gridStyle: React.CSSProperties = useMemo(() => {
    const columnCount =
      statusTiles && columns === 1
        ? Math.max(panel.indicators.length, 1)
        : columns;
    return {
      display: 'grid',
      gridTemplateColumns: `repeat(${columnCount}, minmax(0, 1fr))`,
      gap: '8px',
    };
  }, [statusTiles, columns, panel.indicators.length]);

  return (
    <div className={`indicator-panel${statusTiles ? ' indicator-panel--tiles' : ''}`}>
      <div className="indicator-panel-grid" style={gridStyle}>
        {panel.indicators.map((ind, i) => {
          const isOn = indicatorValues[ind.expression] || false;
          const label = isOn ? ind.label_on : ind.label_off;

          if (statusTiles) {
            return (
              <div
                key={i}
                className="indicator-tile"
                style={tileStyle(ind, isOn)}
                title={label}
              >
                <span className="indicator-tile-label">{label}</span>
              </div>
            );
          }

          return (
            <div key={i} className="indicator-field">
              <div className={`indicator-light ${isOn ? 'on' : 'off'}`} />
              <span className="indicator-label">{label}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
