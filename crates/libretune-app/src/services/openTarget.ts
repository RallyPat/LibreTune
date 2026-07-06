import { invoke } from "@tauri-apps/api/core";
import type { Tab } from "../components/tuner-ui";
import type { SimpleGaugeInfo } from "../components/curves/CurveEditor";
import type { DialogDefinition as RendererDialogDef } from "../components/dialogs/DialogRenderer";
import type { PinConfig } from "../components/hardware/PortEditor";
import {
  type BackendTableData,
  type BackendCurveData,
  type IniCapabilities,
  type TabContent,
  toTunerTableData,
  toCurveData,
} from "../types/app";

export interface OpenTargetDeps {
  tabs: Tab[];
  tabContents: Record<string, TabContent>;
  iniCapabilities: IniCapabilities | null;
  setTabs: (tabs: Tab[]) => void;
  setTabContents: (contents: Record<string, TabContent>) => void;
  setActiveTabId: (id: string) => void;
  setPortEditorAssignments: React.Dispatch<React.SetStateAction<Record<string, PinConfig[]>>>;
  showToast: (msg: string, kind?: "info" | "success" | "error" | "warning") => void;
}

export async function openTargetImpl(
  deps: OpenTargetDeps,
  name: string,
  title?: string,
  highlightTerm?: string,
): Promise<void> {
  const {
    tabs, tabContents, iniCapabilities, setTabs, setTabContents, setActiveTabId,
    setPortEditorAssignments, showToast,
  } = deps;

  console.log("[openTarget] Opening:", name, "title:", title, "highlightTerm:", highlightTerm);

  // Check if already open
  const existingTab = tabs.find((t) => t.id === name);
  if (existingTab) {
    setActiveTabId(name);
    return;
  }

  // Special built-in views
  if (name === "autotune") {
    setTabs([...tabs, { id: "autotune", title: "AutoTune", icon: "autotune" }]);
    setTabContents({ ...tabContents, autotune: { type: "autotune", data: "" } });
    setActiveTabId("autotune");
    return;
  }

  if (name === "datalog") {
    if (!iniCapabilities?.has_datalog_entries && !iniCapabilities?.has_output_channels) {
      showToast("Data Logging is not available for this ECU definition.", "warning");
      return;
    }
    setTabs([...tabs, { id: "datalog", title: "Data Logging", icon: "datalog" }]);
    setTabContents({ ...tabContents, datalog: { type: "datalog" } });
    setActiveTabId("datalog");
    return;
  }

  if (name === "console") {
    if (!iniCapabilities?.supports_console) {
      showToast("ECU Console is not available for this ECU definition.", "warning");
      return;
    }
    setTabs([...tabs, { id: "console", title: title || "ECU Console", icon: "terminal" }]);
    setTabContents({ ...tabContents, console: { type: "console" } });
    setActiveTabId("console");
    return;
  }

  if (name === "lua-console") {
    if (!iniCapabilities?.supports_console) {
      showToast("Lua Console is not available for this ECU definition.", "warning");
      return;
    }
    if (!iniCapabilities?.lua_script_constant) {
      showToast("This ECU definition has no luaScript constant.", "warning");
      return;
    }
    setTabs([...tabs, { id: "lua-console", title: title || "ECU Lua Editor", icon: "terminal" }]);
    setTabContents({ ...tabContents, "lua-console": { type: "lua-console" } });
    setActiveTabId("lua-console");
    return;
  }

  if (name === "tooth-logger") {
    setTabs([...tabs, { id: "tooth-logger", title: title || "Tooth Logger", icon: "scope" }]);
    setTabContents({ ...tabContents, "tooth-logger": { type: "tooth-logger" } });
    setActiveTabId("tooth-logger");
    return;
  }

  if (name === "composite-logger") {
    setTabs([...tabs, { id: "composite-logger", title: title || "Composite Logger", icon: "scope" }]);
    setTabContents({ ...tabContents, "composite-logger": { type: "composite-logger" } });
    setActiveTabId("composite-logger");
    return;
  }

  if (name === "och-status") {
    setTabs([...tabs, { id: "och-status", title: title || "Output Channel Status", icon: "dashboard" }]);
    setTabContents({ ...tabContents, "och-status": { type: "och-status" } });
    setActiveTabId("och-status");
    return;
  }

  // Try table first
  let tableErr: unknown = null;
  try {
    const data = await invoke<BackendTableData>("get_table_data", { tableName: name });
    const tableData = toTunerTableData(data);

    const displayTitle = title && title !== name ? `${title} (${name})` : data.title || name;
    setTabs([...tabs, { id: name, title: displayTitle, icon: "table" }]);
    setTabContents({ ...tabContents, [name]: { type: "table", data: tableData } });
    setActiveTabId(name);
    return;
  } catch (err) {
    tableErr = err;
  }

  // Try curve second
  let curveErr: unknown = null;
  try {
    const data = await invoke<BackendCurveData>("get_curve_data", { curveName: name });
    const curveData = toCurveData(data);

    let gaugeInfo: SimpleGaugeInfo | null = null;
    if (data.gauge) {
      try {
        gaugeInfo = await invoke<SimpleGaugeInfo>("get_gauge_config", { gaugeName: data.gauge });
      } catch (gaugeErr) {
        console.warn(`[openTarget] Failed to load gauge '${data.gauge}':`, gaugeErr);
      }
    }

    const displayTitle = title && title !== name ? `${title} (${name})` : data.title || name;
    setTabs([...tabs, { id: name, title: displayTitle, icon: "curve" }]);
    setTabContents({ ...tabContents, [name]: { type: "curve", data: curveData, gauge: gaugeInfo } });
    setActiveTabId(name);
    return;
  } catch (err) {
    curveErr = err;
  }

  // Try dialog third
  let dialogErr: unknown = null;
  try {
    const def = await invoke<RendererDialogDef>("get_dialog_definition", { name });

    const displayTitle = title && title !== name ? `${title} (${name})` : def.title || name;
    setTabs([...tabs, { id: name, title: displayTitle, icon: "dialog" }]);
    setTabContents({ ...tabContents, [name]: { type: "dialog", data: def, highlightTerm } });
    setActiveTabId(name);
    return;
  } catch (err) {
    dialogErr = err;
  }

  // Try portEditor fourth
  try {
    const portEditor = await invoke<{ name: string; label: string; enable_condition?: string }>(
      "get_port_editor",
      { name },
    );

    try {
      const assignments = await invoke<PinConfig[]>("get_port_editor_assignments", { name: portEditor.name });
      setPortEditorAssignments(prev => ({ ...prev, [portEditor.name]: assignments }));
    } catch (assignErr) {
      console.warn("[openTarget] Failed to load port editor assignments:", assignErr);
      showToast("Failed to load saved port assignments", "warning");
    }

    const displayTitle = title && title !== name ? `${title} (${name})` : portEditor.label || name;
    setTabs([...tabs, { id: name, title: displayTitle, icon: "dialog" }]);
    setTabContents({ ...tabContents, [name]: { type: "portEditor", data: portEditor } });
    setActiveTabId(name);
    return;
  } catch (portErr) {
    const tableErrStr = tableErr instanceof Error ? tableErr.message : String(tableErr);
    const curveErrStr = curveErr instanceof Error ? curveErr.message : String(curveErr);
    const dialogErrStr = dialogErr instanceof Error ? dialogErr.message : String(dialogErr);
    console.error(
      "[openTarget] Failed to open target:", name,
      "table error:", tableErr,
      "curve error:", curveErr,
      "dialog error:", dialogErr,
      "portEditor error:", portErr,
    );

    const detail =
      !curveErrStr.includes("not found") && !curveErrStr.includes("Definition not loaded")
        ? curveErrStr
        : !dialogErrStr.includes("not found") && !dialogErrStr.includes("Definition not loaded")
          ? dialogErrStr
          : tableErrStr;

    if (tableErrStr.includes("Definition not loaded")) {
      showToast(`Please wait - INI definition is still loading...`, "warning");
    } else {
      showToast(`Could not open "${title || name}" - ${detail}`, "warning");
    }
  }
}
