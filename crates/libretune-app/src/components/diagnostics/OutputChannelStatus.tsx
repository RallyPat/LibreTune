/**
 * Output Channel Communication Status
 *
 * Shows live diagnostics about the ECU output-channel stream:
 * - Total channels, consumed/computed/math breakdown
 * - ochBlockSize, maxUnusedRuntimeRange
 * - Transfer mode (Burst/OCH/Demo) and reason
 * - Stream statistics (records/s, success/skip/error ticks)
 * - Data rate from connection metrics
 *
 * All values update live while the tab is open.
 */

import { useEffect, useState, useCallback, useRef } from "react";
import { AlertTriangle } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import "./OutputChannelStatus.css";

interface StreamStats {
  ticksTotal: number;
  ticksSuccess: number;
  ticksSkipped: number;
  ticksError: number;
  transferMode: string;
  transferReason: string;
  intervalMs: number;
  startedAtMs: number;
}

interface OutputChannelStatusData {
  totalChannels: number;
  channelsConsumed: number;
  channelsComputed: number;
  channelsMath: number;
  ochBlockSize: number;
  maxUnusedRuntimeRange: number;
  ochBlocksNeeded: number;
  transferMode: string;
  transferReason: string;
  stream: StreamStats;
  recordsPerSecond: number;
}

interface MetricsPayload {
  tx_bps: number;
  rx_bps: number;
  tx_pkts_s: number;
  rx_pkts_s: number;
  tx_total: number;
  rx_total: number;
  timestamp_ms: number;
  stream?: {
    ticksTotal: number;
    ticksSuccess: number;
    ticksSkipped: number;
    ticksError: number;
    transferMode: string;
    transferReason: string;
    intervalMs: number;
    startedAtMs: number;
  };
}

function formatBytes(bytes: number): string {
  if (bytes >= 1_048_576) return (bytes / 1_048_576).toFixed(1) + " MB";
  if (bytes >= 1024) return (bytes / 1024).toFixed(1) + " KB";
  return bytes + " B";
}

function formatBps(bps: number): string {
  if (bps >= 1_000_000) return (bps / 1_000_000).toFixed(2) + " MB/s";
  if (bps >= 1000) return (bps / 1000).toFixed(1) + " kB/s";
  return bps.toFixed(0) + " B/s";
}

function formatUptime(startMs: number): string {
  if (!startMs) return "—";
  const elapsed = Date.now() - startMs;
  if (elapsed < 0) return "—";
  const secs = Math.floor(elapsed / 1000);
  const mins = Math.floor(secs / 60);
  const hrs = Math.floor(mins / 60);
  if (hrs > 0) return `${hrs}h ${mins % 60}m ${secs % 60}s`;
  if (mins > 0) return `${mins}m ${secs % 60}s`;
  return `${secs}s`;
}

function transferModeLabel(mode: string): string {
  switch (mode) {
    case "Burst": return "Burst (Full Block)";
    case "OCH": return "Optimized (OCH)";
    case "Demo": return "Demo Simulation";
    default: return mode || "Unknown";
  }
}

export function OutputChannelStatus() {
  const [status, setStatus] = useState<OutputChannelStatusData | null>(null);
  const [metrics, setMetrics] = useState<MetricsPayload | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchStatus = useCallback(async () => {
    try {
      const data = await invoke<OutputChannelStatusData>("get_output_channel_status");
      setStatus(data);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  // Initial fetch + periodic poll for structural data (every 2s)
  useEffect(() => {
    fetchStatus();
    pollRef.current = setInterval(fetchStatus, 2000);
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, [fetchStatus]);

  // Listen for connection:metrics events for live rate data
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        unlisten = await listen<MetricsPayload>("connection:metrics", (event) => {
          const p = event.payload as MetricsPayload;
          setMetrics({
            tx_bps: Number(p.tx_bps) || 0,
            rx_bps: Number(p.rx_bps) || 0,
            tx_pkts_s: Number(p.tx_pkts_s) || 0,
            rx_pkts_s: Number(p.rx_pkts_s) || 0,
            tx_total: Number(p.tx_total) || 0,
            rx_total: Number(p.rx_total) || 0,
            timestamp_ms: Number(p.timestamp_ms) || Date.now(),
            stream: p.stream,
          });
        });
      } catch {
        // Non-Tauri environment
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  if (loading) {
    return (
      <div className="och-status">
        <div className="och-status-loading">Loading output channel status...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="och-status">
        <div className="och-status-error">
          <span className="och-error-icon"><AlertTriangle size={14} /></span>
          <span>{error}</span>
        </div>
      </div>
    );
  }

  if (!status) return null;

  // Use live stream stats from metrics event if available, else from polled data
  const liveStream = metrics?.stream ?? status.stream;
  const liveRps = liveStream.startedAtMs
    ? liveStream.ticksSuccess / Math.max(1, (Date.now() - liveStream.startedAtMs) / 1000)
    : status.recordsPerSecond;

  const successPct = liveStream.ticksTotal > 0
    ? ((liveStream.ticksSuccess / liveStream.ticksTotal) * 100).toFixed(1)
    : "0.0";
  const skipPct = liveStream.ticksTotal > 0
    ? ((liveStream.ticksSkipped / liveStream.ticksTotal) * 100).toFixed(1)
    : "0.0";
  const errPct = liveStream.ticksTotal > 0
    ? ((liveStream.ticksError / liveStream.ticksTotal) * 100).toFixed(1)
    : "0.0";

  return (
    <div className="och-status">
      <div className="och-status-header">
        <h2>Output Channel Status</h2>
        <span className="och-status-badge" data-mode={status.transferMode.toLowerCase()}>
          {status.transferMode}
        </span>
      </div>

      <div className="och-status-grid">
        {/* Channel Summary */}
        <div className="och-card">
          <h3>Channel Summary</h3>
          <div className="och-stats-table">
            <div className="och-stat-row">
              <span className="och-stat-label">Total Output Channels</span>
              <span className="och-stat-value">{status.totalChannels}</span>
            </div>
            <div className="och-stat-row">
              <span className="och-stat-label">Channels Consumed (from block)</span>
              <span className="och-stat-value">{status.channelsConsumed}</span>
            </div>
            <div className="och-stat-row">
              <span className="och-stat-label">Computed Channels (expressions)</span>
              <span className="och-stat-value">{status.channelsComputed}</span>
            </div>
            <div className="och-stat-row">
              <span className="och-stat-label">User Math Channels</span>
              <span className="och-stat-value">{status.channelsMath}</span>
            </div>
          </div>
        </div>

        {/* Protocol Info */}
        <div className="och-card">
          <h3>Protocol</h3>
          <div className="och-stats-table">
            <div className="och-stat-row">
              <span className="och-stat-label">ochBlockSize</span>
              <span className="och-stat-value">{formatBytes(status.ochBlockSize)}<span className="och-stat-detail"> ({status.ochBlockSize} bytes)</span></span>
            </div>
            <div className="och-stat-row">
              <span className="och-stat-label">OCH Blocks per Read</span>
              <span className="och-stat-value">{status.ochBlocksNeeded}</span>
            </div>
            <div className="och-stat-row">
              <span className="och-stat-label">maxUnusedRuntimeRange</span>
              <span className="och-stat-value">{status.maxUnusedRuntimeRange}{status.maxUnusedRuntimeRange === 0 ? " (disabled)" : ""}</span>
            </div>
          </div>
        </div>

        {/* Transfer Mode */}
        <div className="och-card">
          <h3>Transfer Mode</h3>
          <div className="och-stats-table">
            <div className="och-stat-row">
              <span className="och-stat-label">Mode</span>
              <span className="och-stat-value och-mode">{transferModeLabel(status.transferMode)}</span>
            </div>
            <div className="och-stat-row">
              <span className="och-stat-label">Reason</span>
              <span className="och-stat-value och-reason">{status.transferReason || "—"}</span>
            </div>
            <div className="och-stat-row">
              <span className="och-stat-label">Stream Interval</span>
              <span className="och-stat-value">{liveStream.intervalMs} ms</span>
            </div>
          </div>
        </div>

        {/* Live Stream */}
        <div className="och-card och-card-wide">
          <h3>Live Stream</h3>
          <div className="och-live-grid">
            <div className="och-live-stat">
              <span className="och-live-value och-highlight">{liveRps.toFixed(1)}</span>
              <span className="och-live-unit">records/s</span>
            </div>
            <div className="och-live-stat">
              <span className="och-live-value">{liveStream.ticksTotal.toLocaleString()}</span>
              <span className="och-live-unit">total ticks</span>
            </div>
            <div className="och-live-stat">
              <span className="och-live-value och-success">{liveStream.ticksSuccess.toLocaleString()}</span>
              <span className="och-live-unit">success ({successPct}%)</span>
            </div>
            <div className="och-live-stat">
              <span className="och-live-value och-warning">{liveStream.ticksSkipped.toLocaleString()}</span>
              <span className="och-live-unit">skipped ({skipPct}%)</span>
            </div>
            <div className="och-live-stat">
              <span className="och-live-value och-error">{liveStream.ticksError.toLocaleString()}</span>
              <span className="och-live-unit">errors ({errPct}%)</span>
            </div>
            <div className="och-live-stat">
              <span className="och-live-value">{formatUptime(liveStream.startedAtMs)}</span>
              <span className="och-live-unit">uptime</span>
            </div>
          </div>
        </div>

        {/* Data Rates */}
        <div className="och-card och-card-wide">
          <h3>Data Rates</h3>
          <div className="och-live-grid">
            <div className="och-live-stat">
              <span className="och-live-value">{metrics ? formatBps(metrics.rx_bps) : "—"}</span>
              <span className="och-live-unit">Rx rate</span>
            </div>
            <div className="och-live-stat">
              <span className="och-live-value">{metrics ? formatBps(metrics.tx_bps) : "—"}</span>
              <span className="och-live-unit">Tx rate</span>
            </div>
            <div className="och-live-stat">
              <span className="och-live-value">{metrics ? metrics.rx_pkts_s.toFixed(0) + " pkt/s" : "—"}</span>
              <span className="och-live-unit">Rx packets</span>
            </div>
            <div className="och-live-stat">
              <span className="och-live-value">{metrics ? metrics.tx_pkts_s.toFixed(0) + " pkt/s" : "—"}</span>
              <span className="och-live-unit">Tx packets</span>
            </div>
            <div className="och-live-stat">
              <span className="och-live-value">{metrics ? formatBytes(metrics.rx_total) : "—"}</span>
              <span className="och-live-unit">Rx total</span>
            </div>
            <div className="och-live-stat">
              <span className="och-live-value">{metrics ? formatBytes(metrics.tx_total) : "—"}</span>
              <span className="och-live-unit">Tx total</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
