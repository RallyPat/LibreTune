/**
 * SVG trail line overlay for table grids (issue #132).
 *
 * Draws the visited-cell path — line segments and dots that fade with age,
 * plus a halo + solid dot on the current cell — over a measured grid. This
 * is the drawing half of the trail extracted from the tab-view TableEditor's
 * inline `renderTrailOverlay`, so other grid views (AutoTune) can show the
 * same trail without a third hand-rolled copy.
 *
 * Must be rendered inside the grid's scroll host (the geometry is in the
 * host's content space) and after the cells in DOM order.
 */
import { useCallback } from 'react';
import { trailDisplayRow, type TrailPoint } from '../../utils/tableTrail';
import type { GridGeometry } from '../../hooks/useTableGridGeometry';
import './TableTrailOverlay.css';

export interface TableTrailOverlayProps {
  /** Measured geometry of the grid (null renders nothing). */
  geometry: GridGeometry | null;
  /** Trail points, oldest first, data-space rows. */
  trail: TrailPoint[];
  /** Total grid rows, for the yAxisBottom flip. */
  rowCount: number;
  /** Whether rows render reversed (origin at bottom-left). */
  yAxisBottom: boolean;
  /** Point lifetime in ms (Infinity = never expire). */
  fadeMs: number;
  /** Current cell, data-space. Falls back to the newest trail point. */
  currentCell?: { row: number; col: number } | null;
}

export default function TableTrailOverlay({
  geometry,
  trail,
  rowCount,
  yAxisBottom,
  fadeMs,
  currentCell,
}: TableTrailOverlayProps) {
  const cellCenter = useCallback(
    (row: number, col: number): { cx: number; cy: number } | null => {
      if (!geometry) return null;
      const c = geometry.cols[col];
      const r = geometry.rows[trailDisplayRow(row, rowCount, yAxisBottom)];
      if (!c || !r) return null;
      return { cx: c.left + c.width / 2, cy: r.top + r.height / 2 };
    },
    [geometry, rowCount, yAxisBottom],
  );

  if (!geometry) return null;

  const now = Date.now();
  const fade = (t: number) =>
    fadeMs === Infinity ? 1 : Math.max(0, 1 - (now - t) / fadeMs);

  const points = trail
    .map((e) => {
      const c = cellCenter(e.row, e.col);
      return c ? { cx: c.cx, cy: c.cy, fade: fade(e.time) } : null;
    })
    .filter((p): p is { cx: number; cy: number; fade: number } => p !== null);

  const current = currentCell ? cellCenter(currentCell.row, currentCell.col) : null;
  if (points.length === 0 && !current) return null;

  const segments = points.slice(1).map((p, i) => {
    const a = points[i];
    return { x1: a.cx, y1: a.cy, x2: p.cx, y2: p.cy, opacity: 0.75 * p.fade };
  });
  const head = current ?? points[points.length - 1];

  return (
    <svg
      className="table-trail-overlay"
      style={{
        left: geometry.left,
        top: geometry.top,
        width: geometry.width,
        height: geometry.height,
      }}
    >
      {segments.map((s, i) => (
        <line
          key={i}
          x1={s.x1}
          y1={s.y1}
          x2={s.x2}
          y2={s.y2}
          style={{ stroke: 'var(--cursor-trail, #4A90E2)' }}
          strokeWidth="3"
          strokeOpacity={s.opacity}
          strokeLinecap="round"
        />
      ))}
      {points.map((p, i) => (
        <circle
          key={i}
          cx={p.cx}
          cy={p.cy}
          r="4"
          style={{ fill: 'var(--cursor-trail, #4A90E2)' }}
          fillOpacity={0.85 * p.fade}
        />
      ))}
      {head && (
        <>
          <circle
            cx={head.cx}
            cy={head.cy}
            r="9"
            style={{ fill: 'var(--cursor-trail, #4A90E2)' }}
            fillOpacity={0.3}
          />
          <circle
            cx={head.cx}
            cy={head.cy}
            r="6"
            style={{ fill: 'var(--cursor-trail, #4A90E2)' }}
            fillOpacity={1}
          />
        </>
      )}
    </svg>
  );
}
