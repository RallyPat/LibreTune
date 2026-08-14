/**
 * useGaugeSweep — sportscar-style gauge sweep animation (min → max → min)
 * triggered when a dashboard loads while the engine is not running.
 *
 * Values are written to the non-reactive gaugeOverride module (read each
 * animation frame by useGaugeRenderer), so the 60fps sweep never
 * re-renders the React tree.
 *
 * After the down-sweep reaches the minimums, the overrides HOLD them for
 * a short rest period before releasing the gauges back to the realtime
 * store. Without the hold, connected-but-engine-off values (battery ~12.8V,
 * IAT, MAP…) take over the instant the sweep ends, and the needles bounce
 * straight off their stops before the eye ever sees them rest.
 */

import { useCallback, useEffect, useRef } from 'react';
import { DashFile, isGauge } from '../dashTypes';
import { setGaugeOverrides } from '../../../stores/gaugeOverride';

const SWEEP_DURATION_MS = 1500;
const SWEEP_REST_MS = 600;

export function useGaugeSweep() {
  const sweepActiveRef = useRef(false);
  const sweepAnimRef = useRef<number | null>(null);
  const restTimerRef = useRef<number | null>(null);

  const cancelRest = useCallback(() => {
    if (restTimerRef.current !== null) {
      clearTimeout(restTimerRef.current);
      restTimerRef.current = null;
    }
  }, []);

  const startGaugeSweep = useCallback((file: DashFile) => {
    if (sweepActiveRef.current) return;
    sweepActiveRef.current = true;
    cancelRest();

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
        // Animation done — allow a new sweep (e.g. quick dashboard switch)
        // but keep holding the minimums for a visible rest before live
        // data is allowed to take the gauges back.
        sweepActiveRef.current = false;
        restTimerRef.current = window.setTimeout(() => {
          restTimerRef.current = null;
          setGaugeOverrides({}, false);
        }, SWEEP_REST_MS);
      }
    };

    sweepAnimRef.current = requestAnimationFrame(animate);
  }, [cancelRest]);

  // Cleanup any running animation or rest on unmount
  useEffect(() => {
    return () => {
      if (sweepAnimRef.current !== null) {
        cancelAnimationFrame(sweepAnimRef.current);
        sweepAnimRef.current = null;
      }
      // Mid-rest unmount: release the overrides too, or a remounted
      // dashboard would find them stuck holding the minimums forever.
      if (restTimerRef.current !== null) {
        clearTimeout(restTimerRef.current);
        restTimerRef.current = null;
        setGaugeOverrides({}, false);
      }
      if (sweepActiveRef.current) {
        sweepActiveRef.current = false;
        setGaugeOverrides({}, false);
      }
    };
  }, []);

  return { startGaugeSweep };
}
