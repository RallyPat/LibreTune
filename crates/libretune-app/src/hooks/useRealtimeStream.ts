import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useRealtimeStore } from "../stores/realtimeStore";
import { ensureRealtimeListener } from "../services/realtimeListener";
import type { ConnectionStatus } from "../types/app";

/**
 * Manages the realtime ECU data stream lifecycle.
 * The Tauri event listener is registered once at module level (ensureRealtimeListener).
 */
export function useRealtimeStream(
  status: ConnectionStatus,
  fetchRealtimeData: () => Promise<void>,
): void {
  useEffect(() => {
    let pollIntervalHandle: NodeJS.Timeout | null = null;
    let heartbeatHandle: NodeJS.Timeout | null = null;
    let cancelled = false;

    if (status.state === "Connected" && status.has_definition) {
      (async () => {
        await ensureRealtimeListener();
        if (cancelled) return;

        try {
          await invoke("start_realtime_stream", { intervalMs: 50 });
        } catch (e) {
          console.warn("Realtime stream failed, falling back to polling with backoff:", e);
          if (cancelled) return;
          let pollInterval = 500;
          let failureCount = 0;
          const maxInterval = 2000;

          const startPolling = () => {
            pollIntervalHandle = setInterval(async () => {
              try {
                await fetchRealtimeData();
                if (pollInterval > 100) {
                  pollInterval = Math.max(100, pollInterval / 1.5);
                  if (pollIntervalHandle) clearInterval(pollIntervalHandle);
                  startPolling();
                }
                failureCount = 0;
              } catch {
                failureCount++;
                if (failureCount >= 3) {
                  pollInterval = Math.min(maxInterval, pollInterval * 1.5);
                  if (pollIntervalHandle) clearInterval(pollIntervalHandle);
                  startPolling();
                  failureCount = 0;
                }
              }
            }, pollInterval);
          };

          startPolling();
        }

        let lastRestartTime = 0;
        heartbeatHandle = setInterval(() => {
          if (cancelled) return;
          const lastUpdate = useRealtimeStore.getState().lastUpdateTime;
          const now = Date.now();
          if (lastUpdate > 0 && now - lastUpdate > 2000 && now - lastRestartTime > 10000) {
            lastRestartTime = now;
            invoke("start_realtime_stream", { intervalMs: 50 }).catch(() => {});
          }
        }, 2000);
      })();
    }

    return () => {
      cancelled = true;
      if (pollIntervalHandle) clearInterval(pollIntervalHandle);
      if (heartbeatHandle) clearInterval(heartbeatHandle);
      useRealtimeStore.getState().clearChannels();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status.state, status.has_definition]);
}
