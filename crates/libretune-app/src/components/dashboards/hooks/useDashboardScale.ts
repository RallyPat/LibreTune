/**
 * useDashboardScale — scales the dashboard down when the viewport is too
 * small for the design's intended (600px-wide @ 1.0) baseline. Returns a
 * scale factor in the range [0.5, 1.0] and a ref to attach to the
 * dashboard wrapper element.
 */

import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';

const MIN_DASH_WIDTH = 600;
const MIN_SCALE = 0.5;

export function useDashboardScale(
  aspectRatio: number,
): { scale: number; wrapperRef: RefObject<HTMLDivElement>; recompute: () => void } {
  const [scale, setScale] = useState(1);
  const wrapperRef = useRef<HTMLDivElement>(null);

  const recompute = useCallback(() => {
    const wrapper = wrapperRef.current;
    if (!wrapper) return;

    const { width: containerWidth, height: containerHeight } = wrapper.getBoundingClientRect();
    const minDashHeight = MIN_DASH_WIDTH / Math.max(0.1, aspectRatio);
    const scaleX = containerWidth / MIN_DASH_WIDTH;
    const scaleY = containerHeight / minDashHeight;
    const newScale = Math.min(1, Math.min(scaleX, scaleY));
    setScale(Math.max(MIN_SCALE, newScale));
  }, [aspectRatio]);

  useEffect(() => {
    const wrapper = wrapperRef.current;
    if (!wrapper) return;

    const resizeObserver = new ResizeObserver(() => recompute());
    resizeObserver.observe(wrapper);
    recompute();
    return () => resizeObserver.disconnect();
  }, [recompute]);

  return { scale, wrapperRef, recompute };
}
