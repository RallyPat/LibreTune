/**
 * useGaugeSweep — sportscar-style gauge sweep animation (min → max → min)
 * triggered when a dashboard loads while the engine is not running.
 *
 * Values are written to the non-reactive gaugeOverride module (read each
 * animation frame by useGaugeRenderer), so the 60fps sweep never
 * re-renders the React tree.
 */

import { useCallback, useEffect, useRef } from 'react';
import { DashFile, isGauge } from '../dashTypes';
import { setGaugeOverrides } from '../../../stores/gaugeOverride';

const SWEEP_DURATION_MS = 1500;

export function useGaugeSweep() {
  const sweepActiveRef = useRef(false);
  const sweepAnimRef = useRef<number | null>(null);

  const startGaugeSweep = useCallback((file: DashFile) => {
    if (sweepActiveRef.current) return;
    sweepActiveRef.current = true;

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
      setGaugeOverrides(newValues, true);

      if (rawProgress < 1) {
        sweepAnimRef.current = requestAnimationFrame(animate);
      } else {
        sweepAnimRef.current = null;
        sweepActiveRef.current = false;
        setGaugeOverrides({}, false);
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
      if (sweepActiveRef.current) {
        sweepActiveRef.current = false;
        setGaugeOverrides({}, false);
      }
    };
  }, []);

  return { startGaugeSweep };
}
