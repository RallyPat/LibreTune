/**
 * useDashboardScale — fits the dashboard in its container.
 *
 * On wide screens: scale down slightly if needed to fit height.
 * On laptop/small screens: keep full width (readable text) and allow vertical
 * scroll instead of crushing the layout with a tiny transform scale.
 */

import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';

const MIN_DASH_WIDTH = 600;
/** Below this scale, labels and sparklines become hard to read on a laptop. */
const MIN_READABLE_SCALE = 0.78;

export interface DashboardScaleState {
  scale: number;
  scrollable: boolean;
}

export function useDashboardScale(
  aspectRatio: number,
): {
  scale: number;
  scrollable: boolean;
  wrapperRef: RefObject<HTMLDivElement>;
  recompute: () => void;
} {
  const [state, setState] = useState<DashboardScaleState>({ scale: 1, scrollable: false });
  const wrapperRef = useRef<HTMLDivElement>(null);

  const recompute = useCallback(() => {
    const wrapper = wrapperRef.current;
    if (!wrapper) return;

    const { width: containerWidth, height: containerHeight } = wrapper.getBoundingClientRect();
    if (containerWidth < 1 || containerHeight < 1) return;

    const naturalHeight = containerWidth / Math.max(0.1, aspectRatio);
    const minDashHeight = MIN_DASH_WIDTH / Math.max(0.1, aspectRatio);
    const scaleX = containerWidth / MIN_DASH_WIDTH;
    const scaleY = containerHeight / minDashHeight;
    const fitScale = Math.min(1, scaleX, scaleY);

    // Prefer scrolling over shrinking when the full-width dashboard is taller than the viewport.
    if (naturalHeight > containerHeight + 2 || fitScale < MIN_READABLE_SCALE) {
      setState({ scale: 1, scrollable: true });
      return;
    }

    setState({ scale: fitScale, scrollable: false });
  }, [aspectRatio]);

  useEffect(() => {
    const wrapper = wrapperRef.current;
    if (!wrapper) return;

    const resizeObserver = new ResizeObserver(() => recompute());
    resizeObserver.observe(wrapper);
    recompute();
    return () => resizeObserver.disconnect();
  }, [recompute]);

  return { scale: state.scale, scrollable: state.scrollable, wrapperRef, recompute };
}
