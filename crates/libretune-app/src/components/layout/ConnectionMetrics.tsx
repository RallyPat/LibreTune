import { useEffect, useState } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

interface Metrics {
  tx_bps: number;
  rx_bps: number;
  tx_pkts_s: number;
  rx_pkts_s: number;
  tx_total: number;
  rx_total: number;
  timestamp_ms: number;
}

function formatBytesPerSecond(bps: number) {
  if (bps >= 1_000_000) return (bps / 1_000_000).toFixed(2) + ' MB/s';
  if (bps >= 1000) return (bps / 1000).toFixed(1) + ' kB/s';
  return bps.toFixed(0) + ' B/s';
}

export default function ConnectionMetrics({ compact }: { compact?: boolean }) {
  const [metrics, setMetrics] = useState<Metrics | null>(null);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    // Add a test hook for Playwright / E2E tests synchronously so it's available
    // even when Tauri's `listen` isn't present in the browser environment.
    const pwHandler = (ev: any) => {
      try {
        // Support both { payload } and CustomEvent with detail.payload
        const payload = ev.payload ?? ev.detail?.payload;
        if (!payload) return;
        console.debug('[ConnectionMetrics][playwright] event payload:', payload);
        setMetrics({
          tx_bps: Number(payload.tx_bps) || 0,
          rx_bps: Number(payload.rx_bps) || 0,
          tx_pkts_s: Number(payload.tx_pkts_s) || 0,
          rx_pkts_s: Number(payload.rx_pkts_s) || 0,
          tx_total: Number(payload.tx_total) || 0,
          rx_total: Number(payload.rx_total) || 0,
          timestamp_ms: Number(payload.timestamp_ms) || Date.now(),
        });
      } catch (e) {
        console.error('[ConnectionMetrics][playwright] Failed to parse payload:', e);
      }
    };

    window.addEventListener('playwright:connection:metrics', pwHandler as EventListener);

    (async () => {
      try {
        unlisten = await listen('connection:metrics', (event) => {
          try {
            const payload = event.payload as any;
            // Debug log to help diagnose missing metrics at runtime
            console.debug('[ConnectionMetrics] event payload:', payload);
            setMetrics({
              tx_bps: Number(payload.tx_bps) || 0,
              rx_bps: Number(payload.rx_bps) || 0,
              tx_pkts_s: Number(payload.tx_pkts_s) || 0,
              rx_pkts_s: Number(payload.rx_pkts_s) || 0,
              tx_total: Number(payload.tx_total) || 0,
              rx_total: Number(payload.rx_total) || 0,
              timestamp_ms: Number(payload.timestamp_ms) || Date.now(),
            });
          } catch (e) {
            console.error('[ConnectionMetrics] Failed to parse payload:', e);
          }
        });
      } catch (e) {
        // If listen isn't available (non-Tauri environment), ignore - Playwright uses the window hook
      }
    })();

    return () => {
      if (unlisten) unlisten();
      window.removeEventListener('playwright:connection:metrics', pwHandler as EventListener);
    };
  }, []);

  if (!metrics) {
    return <div className={compact ? 'conn-metrics compact' : 'conn-metrics'}>—</div>;
  }

  return (
    <div className={compact ? 'conn-metrics compact' : 'conn-metrics'} title={`Last update: ${new Date(metrics.timestamp_ms).toLocaleTimeString()}`}>
      <span className="rx">Rx: {formatBytesPerSecond(metrics.rx_bps)}</span>
      <span style={{ margin: '0 0.4rem' }}>·</span>
      <span className="tx">Tx: {formatBytesPerSecond(metrics.tx_bps)}</span>
      {!compact && (
        <span style={{ marginLeft: '0.6rem', color: 'var(--text-muted)', fontSize: '0.85rem' }}>{metrics.rx_pkts_s.toFixed(0)} pkt/s</span>
      )}
    </div>
  );
}
