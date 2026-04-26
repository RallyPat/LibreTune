import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import {
  type BackendTableData,
  type BackendCurveData,
  type TabContent,
  toTunerTableData,
  toCurveData,
} from "../types/app";
import { Tab } from "../components/tuner-ui";

export interface UseTableCurveRefreshDeps {
  tabs: Tab[];
  tabContents: Record<string, TabContent>;
  setTabContents: React.Dispatch<React.SetStateAction<Record<string, TabContent>>>;
  activeTabId: string | null;
}

/**
 * Refreshes table/curve tab contents when:
 *   1. The backend emits "tune:loaded" (e.g., new INI loaded, tune file loaded)
 *   2. The user activates a table/curve tab (ensures fresh data)
 */
export function useTableCurveRefresh(deps: UseTableCurveRefreshDeps) {
  const { tabs, tabContents, setTabContents, activeTabId } = deps;

  // Listen for tune loaded events to refresh ALL open tables/curves
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    (async () => {
      try {
        unlisten = await listen<string>("tune:loaded", async (event) => {
          console.log("Tune loaded from:", event.payload);
          await new Promise(resolve => setTimeout(resolve, 50));

          const tablesToRefresh: string[] = [];
          const curvesToRefresh: string[] = [];

          for (const tab of tabs) {
            const tabContent = tabContents[tab.id];
            if (tabContent && tabContent.type === "table") {
              tablesToRefresh.push(tab.id);
            } else if (tabContent && tabContent.type === "curve") {
              curvesToRefresh.push(tab.id);
            }
          }

          const totalToRefresh = tablesToRefresh.length + curvesToRefresh.length;
          if (totalToRefresh > 0) {
            console.log(`[tune:loaded] Refreshing ${tablesToRefresh.length} table(s) and ${curvesToRefresh.length} curve(s)`);
            const updatedTabs = { ...tabContents };

            await Promise.all([
              ...tablesToRefresh.map(async (tabId) => {
                try {
                  const data = await invoke<BackendTableData>("get_table_data", { tableName: tabId });
                  updatedTabs[tabId] = { type: "table", data: toTunerTableData(data) };
                  console.log(`[tune:loaded] ✓ Refreshed table '${tabId}': ${data.z_values.length} values`);
                } catch (e) {
                  console.error(`[tune:loaded] ✗ Failed to refresh table '${tabId}':`, e);
                }
              }),
              ...curvesToRefresh.map(async (tabId) => {
                try {
                  const data = await invoke<BackendCurveData>("get_curve_data", { curveName: tabId });
                  updatedTabs[tabId] = { type: "curve", data: toCurveData(data) };
                  console.log(`[tune:loaded] ✓ Refreshed curve '${tabId}': ${data.x_bins.length} points`);
                } catch (e) {
                  console.error(`[tune:loaded] ✗ Failed to refresh curve '${tabId}':`, e);
                }
              })
            ]);

            setTabContents(updatedTabs);
            console.log(`[tune:loaded] ✓ Completed refreshing ${totalToRefresh} item(s)`);
          } else {
            console.log("[tune:loaded] No open tables or curves to refresh");
          }
        });
      } catch (e) {
        console.error("Failed to listen for tune:loaded events:", e);
      }
    })();

    return () => {
      if (unlisten) unlisten();
    };
  }, [tabs, tabContents, setTabContents]);

  // Refresh table/curve data when its tab is activated
  useEffect(() => {
    if (!activeTabId) return;

    setTabContents((prev) => {
      const tabContent = prev[activeTabId];
      if (tabContent && tabContent.type === "table") {
        invoke<BackendTableData>("get_table_data", { tableName: activeTabId })
          .then((data) => {
            setTabContents((prevTabContents) => ({
              ...prevTabContents,
              [activeTabId]: { type: "table", data: toTunerTableData(data) },
            }));
            console.log(`[tab:activated] Refreshed table '${activeTabId}': ${data.z_values.length} values`);
          })
          .catch((e) => {
            console.error(`[tab:activated] Failed to refresh table '${activeTabId}':`, e);
          });
      } else if (tabContent && tabContent.type === "curve") {
        invoke<BackendCurveData>("get_curve_data", { curveName: activeTabId })
          .then((data) => {
            setTabContents((prevTabContents) => ({
              ...prevTabContents,
              [activeTabId]: { type: "curve", data: toCurveData(data) },
            }));
            console.log(`[tab:activated] Refreshed curve '${activeTabId}': ${data.x_bins.length} points`);
          })
          .catch((e) => {
            console.error(`[tab:activated] Failed to refresh curve '${activeTabId}':`, e);
          });
      }
      return prev;
    });
  }, [activeTabId, setTabContents]);
}
