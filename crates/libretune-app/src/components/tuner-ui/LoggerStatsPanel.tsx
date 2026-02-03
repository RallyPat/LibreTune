import React, { useMemo } from 'react';
import './LoggerStatsPanel.css';

export interface ChannelStats {
  mean: number;
  median: number;
  stdDev: number;
  min: number;
  max: number;
  p25: number;
  p75: number;
  sampleCount: number;
}

interface LoggerStatsPanelProps {
  data: { values: Record<string, number> }[];
  selectedChannels: string[];
  onChannelSelect?: (channel: string) => void;
}

/**
 * Calculate statistics for a channel's data points
 */
export function calculateChannelStats(values: number[]): ChannelStats {
  if (values.length === 0) {
    return {
      mean: 0,
      median: 0,
      stdDev: 0,
      min: 0,
      max: 0,
      p25: 0,
      p75: 0,
      sampleCount: 0,
    };
  }

  // Sort for median and percentiles
  const sorted = [...values].sort((a, b) => a - b);
  const n = sorted.length;

  // Mean
  const mean = values.reduce((a, b) => a + b, 0) / n;

  // Median
  const median = n % 2 === 0 ? (sorted[n / 2 - 1] + sorted[n / 2]) / 2 : sorted[Math.floor(n / 2)];

  // Standard deviation
  const variance = values.reduce((sum, val) => sum + Math.pow(val - mean, 2), 0) / n;
  const stdDev = Math.sqrt(variance);

  // Min/Max
  const min = sorted[0];
  const max = sorted[n - 1];

  // Percentiles
  const getPercentile = (p: number) => {
    const idx = Math.ceil((p / 100) * n) - 1;
    return sorted[Math.max(0, idx)];
  };
  const p25 = getPercentile(25);
  const p75 = getPercentile(75);

  return {
    mean,
    median,
    stdDev,
    min,
    max,
    p25,
    p75,
    sampleCount: n,
  };
}

/**
 * Format a number for display with appropriate precision
 */
function formatStat(value: number, decimals: number = 2): string {
  if (!isFinite(value)) return '—';
  return value.toFixed(decimals);
}

const LoggerStatsPanel: React.FC<LoggerStatsPanelProps> = ({
  data,
  selectedChannels,
  onChannelSelect,
}) => {
  // Calculate stats for all selected channels
  const stats = useMemo(() => {
    const result: Record<string, ChannelStats> = {};

    selectedChannels.forEach((channel) => {
      const values = data.map((entry) => entry.values[channel] ?? 0).filter(isFinite);
      result[channel] = calculateChannelStats(values);
    });

    return result;
  }, [data, selectedChannels]);

  if (selectedChannels.length === 0) {
    return (
      <div className="logger-stats-panel empty">
        <p>Select channels to view statistics</p>
      </div>
    );
  }

  return (
    <div className="logger-stats-panel">
      <div className="stats-header">
        <h3>Statistics</h3>
        <p className="stats-subtitle">
          {data.length} samples across {selectedChannels.length} channel{selectedChannels.length !== 1 ? 's' : ''}
        </p>
      </div>

      {selectedChannels.length === 1 ? (
        // Single channel: detailed view
        <div className="stats-detail">
          {selectedChannels.map((channel) => {
            const s = stats[channel];
            return (
              <div key={channel} className="stat-card">
                <h4>{channel}</h4>
                <div className="stat-grid">
                  <div className="stat-row">
                    <span className="stat-label">Mean</span>
                    <span className="stat-value">{formatStat(s.mean)}</span>
                  </div>
                  <div className="stat-row">
                    <span className="stat-label">Median</span>
                    <span className="stat-value">{formatStat(s.median)}</span>
                  </div>
                  <div className="stat-row">
                    <span className="stat-label">Std Dev</span>
                    <span className="stat-value">{formatStat(s.stdDev)}</span>
                  </div>
                  <div className="stat-row">
                    <span className="stat-label">Min</span>
                    <span className="stat-value">{formatStat(s.min)}</span>
                  </div>
                  <div className="stat-row">
                    <span className="stat-label">Max</span>
                    <span className="stat-value">{formatStat(s.max)}</span>
                  </div>
                  <div className="stat-row">
                    <span className="stat-label">P25</span>
                    <span className="stat-value">{formatStat(s.p25)}</span>
                  </div>
                  <div className="stat-row">
                    <span className="stat-label">P75</span>
                    <span className="stat-value">{formatStat(s.p75)}</span>
                  </div>
                  <div className="stat-row">
                    <span className="stat-label">IQR</span>
                    <span className="stat-value">{formatStat(s.p75 - s.p25)}</span>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        // Multiple channels: table view
        <div className="stats-table-container">
          <table className="stats-table">
            <thead>
              <tr>
                <th>Channel</th>
                <th>Mean</th>
                <th>Median</th>
                <th>Std Dev</th>
                <th>Min</th>
                <th>Max</th>
                <th>P25</th>
                <th>P75</th>
              </tr>
            </thead>
            <tbody>
              {selectedChannels.map((channel) => {
                const s = stats[channel];
                return (
                  <tr key={channel} onClick={() => onChannelSelect?.(channel)} className="clickable">
                    <td className="channel-name">{channel}</td>
                    <td>{formatStat(s.mean)}</td>
                    <td>{formatStat(s.median)}</td>
                    <td>{formatStat(s.stdDev)}</td>
                    <td>{formatStat(s.min)}</td>
                    <td>{formatStat(s.max)}</td>
                    <td>{formatStat(s.p25)}</td>
                    <td>{formatStat(s.p75)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
};

export default LoggerStatsPanel;
