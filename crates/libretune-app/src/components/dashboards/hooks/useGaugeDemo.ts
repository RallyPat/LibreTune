/**
 * useGaugeDemo — sinusoidal demo animation that drives every gauge
 * across its min/max range. Used by the "Gauge Demo" context-menu toggle.
 *
 * Values are written to the non-reactive gaugeOverride module (read each
 * animation frame by useGaugeRenderer), so the 20Hz demo tick never
 * re-renders the React tree.
 */

import { useEffect } from 'react';
import { DashFile, isGauge } from '../dashTypes';
import { setGaugeOverrides } from '../../../stores/gaugeOverride';

function hashString(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) {
    h = (h * 31 + s.charCodeAt(i)) | 0;
  }
  return Math.abs(h);
}

export function useGaugeDemo(active: boolean, dashFile: DashFile | null) {
  useEffect(() => {
    if (!active || !dashFile) {
      setGaugeOverrides({}, false);
      return;
    }

    const interval = setInterval(() => {
      const time = Date.now() / 1000;
      const newValues: Record<string, number> = {};

      dashFile.gauge_cluster.components.forEach((comp) => {
        if (isGauge(comp)) {
          const gauge = comp.Gauge;
          const range = gauge.max - gauge.min;
          // Sinusoidal demo with per-gauge phase (full id hash so gauges
          // sharing a first character don't move in lockstep).
          const phase = (hashString(gauge.id) % 628) / 100;
          const value = gauge.min + (range / 2) * (1 + Math.sin(time * 0.5 + phase));
          newValues[gauge.output_channel] = value;
        }
      });

      setGaugeOverrides(newValues, true);
    }, 50);

    return () => {
      clearInterval(interval);
      setGaugeOverrides({}, false);
    };
  }, [active, dashFile]);
}
