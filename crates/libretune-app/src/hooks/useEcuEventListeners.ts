import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CurrentProject, ConnectionStatus } from "../types/app";

export interface UseEcuEventListenersDeps {
  isTauri: boolean;
  connecting: boolean;
  selectedPort: string;
  baudRate: number;
  timeoutMs: number;
  connectionRuntimePacketMode: 'Auto' | 'ForceBurst' | 'ForceOCH' | 'Disabled';
  defaultRuntimePacketMode: 'Auto' | 'ForceBurst' | 'ForceOCH' | 'Disabled';
  status: ConnectionStatus;
  currentProject: CurrentProject | null;
  activeTabId: string | null;
  connect: () => Promise<void>;
  doSync: () => Promise<unknown>;
  checkStatus: () => Promise<void>;
  fetchConstants: () => Promise<Record<string, number>>;
  fetchMenuTree: (context?: Record<string, number>) => Promise<void>;
  showLoading: (msg: string) => void;
  hideLoading: () => void;
  showToast: (msg: string, type: 'info' | 'success' | 'error' | 'warning') => void;
}

/**
 * Bundles app-level lifecycle/event listeners that don't belong to a more
 * specific concern: window title, active tab persistence, reconnect:request,
 * ini:changed, demo:changed.
 */
export function useEcuEventListeners(deps: UseEcuEventListenersDeps) {
  const {
    isTauri,
    connecting,
    selectedPort,
    baudRate,
    timeoutMs,
    connectionRuntimePacketMode,
    defaultRuntimePacketMode,
    status,
    currentProject,
    activeTabId,
    connect,
    doSync,
    checkStatus,
    fetchConstants,
    fetchMenuTree,
    showLoading,
    hideLoading,
    showToast,
  } = deps;

  // Update window title with project name
  useEffect(() => {
    const base = "LibreTune";
    if (currentProject) {
      const title = `${currentProject.name} — ${base}`;
      document.title = title;
      getCurrentWindow().setTitle(title).catch(() => {});
    } else {
      document.title = base;
      getCurrentWindow().setTitle(base).catch(() => {});
    }
  }, [currentProject]);

  // Persist active tab state
  useEffect(() => {
    if (activeTabId && currentProject) {
      invoke("update_setting", { key: "last_active_tab", value: activeTabId })
        .catch(e => console.warn("Failed to save last_active_tab", e));
    }
  }, [activeTabId, currentProject]);

  // Listen for frontend reconnect requests
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        unlisten = await listen<any>("reconnect:request", async (event) => {
          console.log('Reconnect requested from:', event.payload);
          try {
            if (typeof import.meta !== 'undefined' && (import.meta as any).env && (import.meta as any).env.MODE !== 'production') {
              console.debug('reconnect:request received', event.payload);
              try { (window as any).__libretuneTelemetry?.trackEvent?.('reconnect_request_received', { source: event.payload?.source ?? 'unknown' }); } catch (_e) { /* ignore */ }
            }
          } catch (dbgErr) {
            console.error('Failed to log reconnect telemetry:', dbgErr);
          }
          if (!connecting) {
            await connect();
          } else {
            showToast('Reconnect requested but connection is already in progress', 'info');
          }
        });
      } catch (e) {
        console.error('Failed to listen for reconnect:request events:', e);
      }
    })();
    return () => { if (unlisten) unlisten(); };
  }, [connecting, selectedPort, baudRate, timeoutMs, connectionRuntimePacketMode, defaultRuntimePacketMode, connect, showToast]);

  // Listen for INI change events (requires re-sync)
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        unlisten = await listen<string>("ini:changed", async (event) => {
          console.log("INI changed:", event.payload);
          if (event.payload === "resync_required" && status.state === "Connected") {
            showLoading("Syncing with ECU...");
            try {
              await doSync();
            } finally {
              hideLoading();
            }
          }
        });
      } catch (e) {
        console.error("Failed to listen for ini:changed events:", e);
      }
    })();
    return () => { if (unlisten) unlisten(); };
  }, [status.state, showLoading, hideLoading, doSync]);

  // Listen for demo:changed events
  useEffect(() => {
    if (!isTauri) return;
    let unlistenDemo: UnlistenFn | null = null;
    (async () => {
      try {
        unlistenDemo = await listen('demo:changed', async (event) => {
          try {
            await checkStatus();
            const values = await fetchConstants();
            await fetchMenuTree(values);
            const demoEnabled = Boolean(event.payload as unknown as boolean);
            if (demoEnabled) {
              try { await invoke('start_realtime_stream', { intervalMs: 50 }); } catch (e) { /* ignore */ }
            } else {
              try { await invoke('stop_realtime_stream'); } catch (e) { /* ignore */ }
            }
          } catch (e) {
            console.error('Error handling demo:changed event', e);
          }
        });
      } catch (e) {
        console.error('Failed to listen for demo events', e);
      }
    })();
    return () => { if (unlistenDemo) unlistenDemo(); };
  }, [isTauri, checkStatus, fetchConstants, fetchMenuTree]);
}
