import { listen } from "@tauri-apps/api/event";
import { useRealtimeStore } from "../stores/realtimeStore";

// Registered once and never unregistered — prevents race conditions when React
// effects re-run during connect/disconnect cycles.
let _realtimeListenerPromise: Promise<void> | null = null;

export function ensureRealtimeListener(): Promise<void> {
  if (_realtimeListenerPromise) return _realtimeListenerPromise;
  _realtimeListenerPromise = (async () => {
    await listen("realtime:update", (event) => {
      useRealtimeStore.getState().updateChannels(event.payload as Record<string, number>);
    });
    await listen("realtime:error", (event) => {
      console.error("Realtime error:", event.payload);
    });
  })();
  return _realtimeListenerPromise;
}
