/**
 * useGaugeSweep — sportscar-style gauge sweep animation (min → max → min)
 * triggered when a dashboard loads while the engine is not running.
 *
 * Returns:
 *  - `sweepActive`: true while the animation is running
 *  - `sweepValues`: per-channel interpolated values for sweeping gauges
 *  - `startGaugeSweep(file)`: begin the animation for the given dash
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { DashFile, isGauge } from '../dashTypes';

const SWEEP_DURATION_MS = 1500;

export function useGaugeSweep() {
  const [sweepActive, setSweepActive] = useState(false);
  const [sweepValues, setSweepValues] = useState<Record<string, number>>({});

  const sweepActiveRef = useRef(false);
  const sweepAnimRef = useRef<number | null>(null);

  const startGaugeSweep = useCallback((file: DashFile) => {
    if (sweepActiveRef.current) return;
    sweepActiveRef.current = true;
    setSweepActive(true);

    if (sweepAnimRef.current !== null) {
      cancelAnimationFrame(sweepAnimRef.current);
      sweepAnimRef.current = null;
    }

    const startTime = performance.now();
    const easeInOut = (t: number) =>
      t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;

    const animate = (currentTime: number) => {
      const elapsed = currentTime - startTime;
      const rawProgress = Math.min(elapsed / SWEEP_DURATION_MS, 1);

      // 0-0.5 progress = sweep up (0→1), 0.5-1 progress = sweep down (1→0)
      const sweepPosition = rawProgress < 0.5
        ? easeInOut(rawProgress * 2)
        : easeInOut(1 - (rawProgress - 0.5) * 2);

      const newValues: Record<string, number> = {};
      file.gauge_cluster.components.forEach((comp) => {
        if (isGauge(comp)) {
          const gauge = comp.Gauge;
          const range = gauge.max - gauge.min;
          newValues[gauge.output_channel] = gauge.min + range * sweepPosition;
        }
      });
      setSweepValues(newValues);

      if (rawProgress < 1) {
        sweepAnimRef.current = requestAnimationFrame(animate);
      } else {
        sweepAnimRef.current = null;
        sweepActiveRef.current = false;
        setSweepActive(false);
        setSweepValues({});
      }
    };

    sweepAnimRef.current = requestAnimationFrame(animate);
  }, []);

  // Cleanup any running animation on unmount
  useEffect(() => {
    return () => {
      if (sweepAnimRef.current !== null) {
        cancelAnimationFrame(sweepAnimRef.current);
        sweepAnimRef.current = null;
      }
      sweepActiveRef.current = false;
    };
  }, []);

  return { sweepActive, sweepValues, startGaugeSweep };
}
