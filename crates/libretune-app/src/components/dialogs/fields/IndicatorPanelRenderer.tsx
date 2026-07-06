import React, { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
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

function tileShadow(ind: IndicatorDef, isOn: boolean): string {
  const color = (isOn ? ind.color_on_fg : ind.color_off_fg)?.toLowerCase() ?? '';
  const glowByColor: Record<string, string> = {
    green: '0 6px 18px rgba(34, 197, 94, 0.42)',
    red: '0 6px 18px rgba(239, 68, 68, 0.45)',
    yellow: '0 6px 18px rgba(234, 179, 8, 0.38)',
    white: '0 4px 14px rgba(255, 255, 255, 0.12)',
  };
  const glow = glowByColor[color] ?? '0 4px 14px rgba(0, 0, 0, 0.28)';
  return [
    'inset 0 1px 0 rgba(255, 255, 255, 0.38)',
    'inset 0 -2px 0 rgba(0, 0, 0, 0.14)',
    glow,
  ].join(', ');
}

function tileStyle(ind: IndicatorDef, isOn: boolean): React.CSSProperties {
  return {
    background: isOn
      ? (ind.color_on_fg || 'var(--success)')
      : (ind.color_off_fg || 'var(--border-strong)'),
    color: isOn
      ? (ind.color_on_bg || '#000')
      : (ind.color_off_bg || '#000'),
    boxShadow: tileShadow(ind, isOn),
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
  const [indicatorValues, setIndicatorValues] = useState<Record<string, boolean>>({});

  useEffect(() => {
    const evaluations = panel.indicators.map((ind) =>
      invoke<boolean>('evaluate_expression', {
        expression: ind.expression,
        context,
      })
        .then((value) => ({ expression: ind.expression, value }))
        .catch(() => ({ expression: ind.expression, value: false })),
    );

    Promise.all(evaluations).then((results) => {
      const values: Record<string, boolean> = {};
      results.forEach(({ expression, value }) => {
        values[expression] = value;
      });
      setIndicatorValues(values);
    });
  }, [panel.indicators, context]);

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
