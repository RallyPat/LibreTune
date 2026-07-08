import { invoke } from "@tauri-apps/api/core";
import {
  TableEditor,
  AutoTune,
  DataLogView,
  type TableData as TunerTableData,
} from "./tuner-ui";
import TsDashboard from "./dashboards/TsDashboard";
import { ToothLoggerView, CompositeLoggerView, OutputChannelStatus } from "./diagnostics";
import { EcuConsole } from "./console/EcuConsole";
import { LuaConsole } from "./console/LuaConsole";
import DialogRenderer, { type DialogDefinition as RendererDialogDef } from "./dialogs/DialogRenderer";
import CurveEditor, { type CurveData } from "./curves/CurveEditor";
import PortEditor, { type PinConfig } from "./hardware/PortEditor";
import WelcomeView from "./WelcomeView";
import { SettingsView } from "./SettingsView";
import type {
  ConnectionStatus,
  CurrentProject,
  IniCapabilities,
  ProjectInfo,
  TabContent,
  PortEditorConfig,
} from "../types/app";
import type { Tab } from "./tuner-ui";

export interface TabContentRouterProps {
  // Project / connection state
  currentProject: CurrentProject | null;
  availableProjects: ProjectInfo[];
  status: ConnectionStatus;
  ecuType: string;
  iniCapabilities: IniCapabilities | null;

  // Tab state
  activeTabId: string | null;
  tabs: Tab[];
  tabContents: Record<string, TabContent>;
  setTabContents: (next: Record<string, TabContent>) => void;

  // Welcome view actions
  openProject: (path: string) => void | Promise<void>;
  setNewProjectDialogOpen: (open: boolean) => void;
  setConnectionDialogOpen: (open: boolean) => void;
  setImportProjectOpen: (open: boolean) => void;
  handleDeleteProject: (name: string) => void | Promise<void>;

  // Editor / dialog actions
  setBurnDialogOpen: (open: boolean) => void;
  handleTabClose: (id: string) => void;
  openTarget: (target: string, label?: string) => void;
  fetchConstants: () => Promise<Record<string, number>>;
  fetchMenuTree: (context?: Record<string, number>) => Promise<void>;
  setConstantValues: React.Dispatch<React.SetStateAction<Record<string, number>>>;
  constantValues: Record<string, number>;

  // Port editor
  portEditorAssignments: Record<string, PinConfig[]>;
  setPortEditorAssignments: React.Dispatch<React.SetStateAction<Record<string, PinConfig[]>>>;

  // Toast
  showToast: (msg: string, level?: "info" | "success" | "error" | "warning") => void;
}

/**
 * Routes the active tab's content to the appropriate editor/view component.
 * Extracted from App.tsx to reduce the god-component footprint.
 */
export function TabContentRouter(props: TabContentRouterProps) {
  const {
    currentProject,
    availableProjects,
    status,
    ecuType,
    iniCapabilities,
    activeTabId,
    tabs,
    tabContents,
    setTabContents,
    openProject,
    setNewProjectDialogOpen,
    setConnectionDialogOpen,
    setImportProjectOpen,
    handleDeleteProject,
    setBurnDialogOpen,
    handleTabClose,
    openTarget,
    fetchConstants,
    fetchMenuTree,
    setConstantValues,
    constantValues,
    portEditorAssignments,
    setPortEditorAssignments,
    showToast,
  } = props;

  // If no project is open, show the welcome view
  if (!currentProject) {
    return (
      <WelcomeView
        projects={availableProjects}
        onOpenProject={(path) => openProject(path)}
        onNewProject={() => setNewProjectDialogOpen(true)}
        onConnect={() => setConnectionDialogOpen(true)}
        onImportTsProject={() => setImportProjectOpen(true)}
        onDeleteProject={handleDeleteProject}
      />
    );
  }

  if (!activeTabId) return null;
  const content = tabContents[activeTabId];
  if (!content) return null;

  switch (content.type) {
    case "dashboard":
      return <TsDashboard isConnected={status.state === "Connected"} />;
    case "table":
      return (
        <TableEditor
          data={content.data as TunerTableData}
          onChange={(newData) => {
            if (activeTabId) {
              setTabContents({
                ...tabContents,
                [activeTabId]: { type: "table", data: newData },
              });
            }
          }}
          onBurn={() => setBurnDialogOpen(true)}
        />
      );
    case "curve":
      return (
        <CurveEditor
          data={content.data as CurveData}
          embedded={false}
          simpleGaugeInfo={content.gauge}
          onValuesChange={async (values) => {
            if (activeTabId) {
              const curveData = content.data as CurveData;
              const updatedData = {
                ...curveData,
                x_bins: values.xBins,
                y_bins: values.yBins,
              };
              setTabContents({
                ...tabContents,
                [activeTabId]: { type: "curve", data: updatedData, gauge: content.gauge },
              });
              try {
                await invoke("update_curve_data", {
                  curveName: curveData.name,
                  xValues: values.xBins,
                  yValues: values.yBins,
                });
              } catch (err) {
                console.error("Failed to save curve data:", err);
                showToast("Failed to save curve changes", "error");
              }
            }
          }}
          onBack={() => activeTabId && handleTabClose(activeTabId)}
        />
      );
    case "dialog": {
      const activeTab = tabs.find((t) => t.id === activeTabId);
      return (
        <DialogRenderer
          definition={content.data as RendererDialogDef}
          onBack={() => activeTabId && handleTabClose(activeTabId)}
          openTable={(tableName) => openTarget(tableName)}
          context={constantValues}
          displayTitle={activeTab?.title}
          highlightTerm={content.highlightTerm}
          onOptimisticUpdate={(name, value) => {
            // Immediately update the context so sibling fields re-evaluate their conditions
            setConstantValues((prev) => ({ ...prev, [name]: value }));
          }}
          onUpdate={async () => {
            const values = await fetchConstants();
            await fetchMenuTree(values);
            setConstantValues(values);
          }}
        />
      );
    }
    case "portEditor": {
      const portEditorMeta = content.data as PortEditorConfig | undefined;
      if (!portEditorMeta) return null;
      return (
        <PortEditor
          ecuType={ecuType}
          title={portEditorMeta.label || "Port Editor"}
          initialConfig={portEditorAssignments[portEditorMeta.name] || []}
          onSave={async (config) => {
            try {
              await invoke("save_port_editor_assignments", {
                name: portEditorMeta.name,
                assignments: config,
              });
              setPortEditorAssignments((prev) => ({ ...prev, [portEditorMeta.name]: config }));
              showToast("Port assignments saved", "success");
            } catch (err) {
              console.error("Failed to save port editor assignments:", err);
              showToast("Failed to save port assignments", "error");
            }
          }}
          onCancel={() => activeTabId && handleTabClose(activeTabId)}
        />
      );
    }
    case "settings":
      return <SettingsView />;
    case "autotune":
      return (
        <AutoTune
          tableName={(content.data as string) || ""}
          onClose={() => handleTabClose("autotune")}
        />
      );
    case "datalog":
      return <DataLogView />;
    case "tooth-logger":
      return <ToothLoggerView onClose={() => handleTabClose("tooth-logger")} />;
    case "composite-logger":
      return <CompositeLoggerView onClose={() => handleTabClose("composite-logger")} />;
    case "och-status":
      return <OutputChannelStatus />;
    case "console":
      return (
        <EcuConsole
          ecuType={ecuType}
          isConnected={status.state === "Connected"}
          dfuCommandName={iniCapabilities?.dfu_command_name ?? null}
        />
      );
    case "lua-console":
      return (
        <LuaConsole
          isConnected={status.state === "Connected"}
          luaScriptConstant={iniCapabilities?.lua_script_constant ?? null}
        />
      );
    default:
      return null;
  }
}
