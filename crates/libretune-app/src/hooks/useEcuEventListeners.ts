import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CurrentProject, ConnectionStatus } from "../types/app";

export interface UseEcuEventListenersDeps {
  isTauri: boolean;
  status: ConnectionStatus;
  currentProject: CurrentProject | null;
  activeTabId: string | null;
  doSync: () => Promise<unknown>;
  checkStatus: () => Promise<void>;
  fetchConstants: () => Promise<Record<string, number>>;
  fetchMenuTree: (context?: Record<string, number>) => Promise<void>;
  showLoading: (msg: string) => void;
  hideLoading: () => void;
}

/**
 * Bundles app-level lifecycle/event listeners that don't belong to a more
 * specific concern: window title, active tab persistence, reconnect:request,
 * ini:changed, demo:changed.
 */
export function useEcuEventListeners(deps: UseEcuEventListenersDeps) {
  const {
    isTauri,
    status,
    currentProject,
    activeTabId,
    doSync,
    checkStatus,
    fetchConstants,
    fetchMenuTree,
    showLoading,
    hideLoading,
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
