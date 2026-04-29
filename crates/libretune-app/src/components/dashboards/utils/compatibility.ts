/**
 * Computes a "compatibility report" describing painter usage in a
 * dashboard relative to the painters supported by LibreTune.
 */

import {
  DashFile,
  SUPPORTED_GAUGE_PAINTERS,
  SUPPORTED_INDICATOR_PAINTERS,
  isGauge,
  isIndicator,
} from '../dashTypes';

export interface CompatibilityReport {
  total_components: number;
  gauges: number;
  indicators: number;
  gauge_painters: Record<string, number>;
  indicator_painters: Record<string, number>;
  unsupported_gauge_painters: string[];
  unsupported_indicator_painters: string[];
}

export function computeCompatibilityReport(dashFile: DashFile): CompatibilityReport {
  const supportedGaugePainters = new Set(SUPPORTED_GAUGE_PAINTERS);
  const supportedIndicatorPainters = new Set(SUPPORTED_INDICATOR_PAINTERS);

  const gaugePainters: Record<string, number> = {};
  const indicatorPainters: Record<string, number> = {};
  const unsupportedGaugePainters = new Set<string>();
  const unsupportedIndicatorPainters = new Set<string>();

  let gauges = 0;
  let indicators = 0;

  dashFile.gauge_cluster.components.forEach((comp) => {
    if (isGauge(comp)) {
      gauges += 1;
      const painter = comp.Gauge.gauge_painter || 'BasicReadout';
      gaugePainters[painter] = (gaugePainters[painter] || 0) + 1;
      if (!supportedGaugePainters.has(painter)) {
        unsupportedGaugePainters.add(painter);
      }
    } else if (isIndicator(comp)) {
      indicators += 1;
      const painter = comp.Indicator.indicator_painter || 'BasicRectangleIndicator';
      indicatorPainters[painter] = (indicatorPainters[painter] || 0) + 1;
      if (!supportedIndicatorPainters.has(painter)) {
        unsupportedIndicatorPainters.add(painter);
      }
    }
  });

  return {
    total_components: dashFile.gauge_cluster.components.length,
    gauges,
    indicators,
    gauge_painters: gaugePainters,
    indicator_painters: indicatorPainters,
    unsupported_gauge_painters: Array.from(unsupportedGaugePainters),
    unsupported_indicator_painters: Array.from(unsupportedIndicatorPainters),
  };
}

export function hasCompatibilityIssues(report: CompatibilityReport | null): boolean {
  if (!report) return false;
  return (
    report.unsupported_gauge_painters.length > 0 ||
    report.unsupported_indicator_painters.length > 0
  );
}
