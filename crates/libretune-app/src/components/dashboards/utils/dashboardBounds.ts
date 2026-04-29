/**
 * Compute the bounding box and aspect ratio of a dashboard's components.
 * Used to drive the canvas's `aspect-ratio` style and the dynamic
 * scaling logic (so the dashboard fits its container regardless of
 * how the gauges were laid out).
 */

import { DashFile, isGauge, isIndicator } from '../dashTypes';

export interface DashboardBounds {
  maxX: number;
  maxY: number;
  aspectRatio: number;
  /** Smallest `shortest_size` across all gauges, or 50 if none specify it. */
  minSize: number;
}

const DEFAULT_BOUNDS: DashboardBounds = {
  maxX: 1.0,
  maxY: 1.0,
  aspectRatio: 1.0,
  minSize: 50,
};

export function computeDashboardBounds(dashFile: DashFile | null): DashboardBounds {
  if (!dashFile) return DEFAULT_BOUNDS;

  const components = dashFile.gauge_cluster.components;
  let maxX = 0;
  let maxY = 0;
  let minShortestSize = Infinity;

  components.forEach((comp) => {
    if (isGauge(comp)) {
      const g = comp.Gauge;
      maxX = Math.max(maxX, (g.relative_x ?? 0) + (g.relative_width ?? 0.25));
      maxY = Math.max(maxY, (g.relative_y ?? 0) + (g.relative_height ?? 0.25));
      if (g.shortest_size > 0) {
        minShortestSize = Math.min(minShortestSize, g.shortest_size);
      }
    } else if (isIndicator(comp)) {
      const i = comp.Indicator;
      maxX = Math.max(maxX, (i.relative_x ?? 0) + (i.relative_width ?? 0.1));
      maxY = Math.max(maxY, (i.relative_y ?? 0) + (i.relative_height ?? 0.05));
    }
  });

  // Clamp to reasonable bounds (at least 1.0 to cover the full area)
  maxX = Math.max(1.0, maxX);
  maxY = Math.max(1.0, maxY);

  const forceAspect = dashFile.gauge_cluster.force_aspect
    && dashFile.gauge_cluster.force_aspect_width > 0
    && dashFile.gauge_cluster.force_aspect_height > 0;
  const forcedRatio = forceAspect
    ? dashFile.gauge_cluster.force_aspect_width / dashFile.gauge_cluster.force_aspect_height
    : null;

  const aspectRatio = forcedRatio ?? (maxX / maxY);
  const minSize = minShortestSize === Infinity ? 50 : minShortestSize;

  return { maxX, maxY, aspectRatio, minSize };
}
