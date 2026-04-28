/**
 * useGaugeDemo — sinusoidal demo animation that drives every gauge
 * across its min/max range. Used by the "Gauge Demo" context-menu toggle.
 */

import { useEffect, useState } from 'react';
import { DashFile, isGauge } from '../dashTypes';

export function useGaugeDemo(active: boolean, dashFile: DashFile | null) {
  const [demoValues, setDemoValues] = useState<Record<string, number>>({});

  useEffect(() => {
    if (!active || !dashFile) return;

    const interval = setInterval(() => {
      const time = Date.now() / 1000;
      const newValues: Record<string, number> = {};

      dashFile.gauge_cluster.components.forEach((comp) => {
        if (isGauge(comp)) {
          const gauge = comp.Gauge;
          const range = gauge.max - gauge.min;
          // Sinusoidal demo with random phase per gauge
          const phase = gauge.id.charCodeAt(0) / 10;
          const value = gauge.min + (range / 2) * (1 + Math.sin(time * 0.5 + phase));
          newValues[gauge.output_channel] = value;
        }
      });

      setDemoValues(newValues);
    }, 50);

    return () => clearInterval(interval);
  }, [active, dashFile]);

  return demoValues;
}
