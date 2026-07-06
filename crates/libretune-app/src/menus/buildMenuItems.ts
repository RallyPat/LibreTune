import type { TFunction } from "i18next";
import { invoke } from "@tauri-apps/api/core";
import type { MenuItem as TunerMenuItem, Tab } from "../components/tuner-ui";
import { THEME_INFO, ThemeName } from "../themes";
import type {
  BackendMenu,
  BackendMenuItem,
  CurrentProject,
  ConnectionStatus,
  IniCapabilities,
  TabContent,
} from "../types/app";

export interface BuildMenuItemsDeps {
  t: TFunction;
  currentProject: CurrentProject | null;
  status: ConnectionStatus;
  ecuType: string;
  iniCapabilities: IniCapabilities | null;
  backendMenus: BackendMenu[] | null;
  theme: ThemeName;
  sidebarVisible: boolean;
  tabs: Tab[];
  // Callbacks (closures from App)
  openTarget: (target: string, label?: string) => void;
  handleStdTarget: (target: string, label: string) => void;
  openHelpTopic: (topic: string, label: string) => void;
  showToast: (msg: string, kind?: "info" | "success" | "error" | "warning") => void;
  closeProject: () => void;
  handleCreateRestorePoint: () => void;
  // Setters
  setNewProjectDialogOpen: (open: boolean) => void;
  setImportProjectOpen: (open: boolean) => void;
  setSaveDialogOpen: (open: boolean) => void;
  setLoadDialogOpen: (open: boolean) => void;
  setBurnDialogOpen: (open: boolean) => void;
  setRestorePointsOpen: (open: boolean) => void;
  setTuneHistoryOpen: (open: boolean) => void;
  setSettingsDialogOpen: (open: boolean) => void;
  setMathChannelsDialogOpen: (open: boolean) => void;
  setBaseMapDialogOpen: (open: boolean) => void;
  setTableComparisonOpen: (open: boolean) => void;
  setTuneFileDiffOpen: (open: boolean) => void;
  setDynoOverlayOpen: (open: boolean) => void;
  setPluginPanelOpen: (open: boolean) => void;
  setConnectionDialogOpen: (open: boolean) => void;
  setUserManualOpen: (open: boolean) => void;
  setUserManualSection: (section: string | undefined) => void;
  setAboutDialogOpen: (open: boolean) => void;
  setSidebarVisible: (visible: boolean) => void;
  setTheme: (theme: ThemeName) => void;
  setTabs: React.Dispatch<React.SetStateAction<Tab[]>>;
  setTabContents: React.Dispatch<React.SetStateAction<Record<string, TabContent>>>;
  setActiveTabId: (id: string) => void;
}

export function buildMenuItems(deps: BuildMenuItemsDeps): TunerMenuItem[] {
  const {
    t, currentProject, status, ecuType, iniCapabilities, backendMenus, theme,
    sidebarVisible, tabs, openTarget, handleStdTarget, openHelpTopic, showToast,
    closeProject, handleCreateRestorePoint,
    setNewProjectDialogOpen, setImportProjectOpen, setSaveDialogOpen, setLoadDialogOpen,
    setBurnDialogOpen, setRestorePointsOpen, setTuneHistoryOpen, setSettingsDialogOpen,
    setMathChannelsDialogOpen, setBaseMapDialogOpen, setTableComparisonOpen,
    setTuneFileDiffOpen, setDynoOverlayOpen, setPluginPanelOpen, setConnectionDialogOpen,
    setUserManualOpen, setUserManualSection, setAboutDialogOpen, setSidebarVisible,
    setTheme, setTabs, setTabContents, setActiveTabId,
  } = deps;

  const fileMenuItems: TunerMenuItem["items"] = currentProject
    ? [
        { id: "new-project", label: "&New Project...\tCtrl+N", onClick: () => setNewProjectDialogOpen(true) },
        { id: "import-project", label: "&Import TS Project...", onClick: () => setImportProjectOpen(true) },
        { id: "close-project", label: "&Close Project", onClick: closeProject },
        { id: "sep1", label: "", separator: true },
        { id: "save", label: "&Save Tune\tCtrl+S", onClick: () => setSaveDialogOpen(true) },
        { id: "saveas", label: "Save Tune &As...", onClick: () => setSaveDialogOpen(true) },
        { id: "load", label: "&Load Tune...", onClick: () => setLoadDialogOpen(true) },
        { id: "sep2", label: "", separator: true },
        { id: "create-restore", label: "Create &Restore Point", onClick: handleCreateRestorePoint },
        { id: "restore-points", label: "Restore &Points...", onClick: () => setRestorePointsOpen(true) },
        { id: "tune-history", label: "Tune &History...", onClick: () => setTuneHistoryOpen(true) },
        { id: "sep3", label: "", separator: true },
        { id: "burn", label: "&Burn to ECU\tCtrl+B", onClick: () => setBurnDialogOpen(true), disabled: status.state !== "Connected" },
        { id: "sep4", label: "", separator: true },
        { id: "exit", label: "E&xit", onClick: () => window.close() },
      ]
    : [
        { id: "new-project", label: "&New Project...\tCtrl+N", onClick: () => setNewProjectDialogOpen(true) },
        { id: "import-project", label: "&Import TS Project...", onClick: () => setImportProjectOpen(true) },
        { id: "sep1", label: "", separator: true },
        { id: "settings", label: "&Settings...", onClick: () => setSettingsDialogOpen(true) },
        { id: "sep2", label: "", separator: true },
        { id: "exit", label: "E&xit", onClick: () => window.close() },
      ];

  const fileMenu: TunerMenuItem = { id: "file", label: t('file.title'), items: fileMenuItems };

  const viewMenu: TunerMenuItem = {
    id: "view",
    label: t('view.title'),
    items: [
      { id: "dashboard", label: "&Dashboard", onClick: () => {
        if (!tabs.find(tab => tab.id === "dashboard")) {
          setTabs(prev => [{ id: "dashboard", title: "Dashboard", icon: "dashboard", closable: false }, ...prev]);
          setTabContents(prev => ({ ...prev, dashboard: { type: "dashboard" } }));
        }
        setActiveTabId("dashboard");
      }},
      { id: "sidebar", label: "Toggle &Sidebar", onClick: () => setSidebarVisible(!sidebarVisible) },
      { id: "sep1", label: "", separator: true },
      {
        id: "theme",
        label: "&Theme",
        items: Object.entries(THEME_INFO).map(([key, info]) => ({
          id: key,
          label: info.label,
          checked: theme === key,
          onClick: () => setTheme(key as ThemeName),
        })),
      },
    ],
  };

  const editMenu: TunerMenuItem = {
    id: "edit",
    label: t('edit.title'),
    items: [
      { id: "undo", label: "&Undo\tCtrl+Z", onClick: () => showToast("Undo - use table-specific controls", "info"), disabled: !currentProject },
      { id: "redo", label: "&Redo\tCtrl+Y", onClick: () => showToast("Redo - use table-specific controls", "info"), disabled: !currentProject },
      { id: "sep1", label: "", separator: true },
      { id: "cut", label: "Cu&t\tCtrl+X", onClick: () => showToast("Cut - select cells in table first", "info"), disabled: !currentProject },
      { id: "copy", label: "&Copy\tCtrl+C", onClick: () => showToast("Copy - select cells in table first", "info"), disabled: !currentProject },
      { id: "paste", label: "&Paste\tCtrl+V", onClick: () => showToast("Paste - select cells in table first", "info"), disabled: !currentProject },
      { id: "sep2", label: "", separator: true },
      { id: "reset-defaults", label: "Reset to &Defaults", onClick: async () => {
        try {
          const count = await invoke<number>("reset_tune_to_defaults");
          showToast(`Reset ${count} values to defaults`, "success");
        } catch (err) {
          showToast(`Reset failed: ${err}`, "error");
        }
      }, disabled: !currentProject },
    ],
  };

  const convertMenuItems = (items: BackendMenuItem[], prefix: string): TunerMenuItem["items"] => {
    return items
      .filter((item) => item.type !== "Separator" || item.label)
      .map((item, idx) => {
        if (item.type === "Separator") {
          return { id: `${prefix}-sep-${idx}`, label: "", separator: true };
        }
        if (item.type === "SubMenu" && item.items && item.items.length > 0) {
          return {
            id: `${prefix}-submenu-${idx}`,
            label: item.label || "",
            items: convertMenuItems(item.items, `${prefix}-${idx}`),
          };
        }
        if (item.type === "Std") {
          return {
            id: item.target || `${prefix}-std-${idx}`,
            label: item.label || "",
            onClick: () => handleStdTarget(item.target || "", item.label || ""),
          };
        }
        if (item.type === "Help") {
          return {
            id: item.target || `${prefix}-help-${idx}`,
            label: item.label || "",
            onClick: () => openHelpTopic(item.target || "", item.label || ""),
          };
        }
        return {
          id: item.target || `${prefix}-item-${idx}`,
          label: item.label || "",
          onClick: () => item.target && openTarget(item.target, item.label),
        };
      });
  };

  const tuningMenus: TunerMenuItem[] = (backendMenus ?? []).map((menu) => ({
    id: menu.name,
    label: menu.title.replace(/^&/, ""),
    items: convertMenuItems(menu.items, menu.name),
  }));

  const toolItems: TunerMenuItem["items"] = [];
  const caps = iniCapabilities;

  toolItems.push({ id: "autotune", label: "&AutoTune", onClick: () => openTarget("autotune", "AutoTune"), disabled: !currentProject });
  if (caps?.has_datalog_entries || caps?.has_output_channels) {
    toolItems.push({ id: "datalog", label: "&Data Logging", onClick: () => openTarget("datalog", "Data Logging"), disabled: !currentProject });
    toolItems.push({ id: "och-status", label: "&Output Channel Status", onClick: () => openTarget("och-status", "Output Channel Status"), disabled: !currentProject });
  }
  if (caps?.has_logger_definitions) {
    if (toolItems.length > 0) toolItems.push({ id: "sep1", label: "", separator: true });
    toolItems.push(
      { id: "tooth-logger", label: "&Tooth Logger", onClick: () => openTarget("tooth-logger", "Tooth Logger"), disabled: !currentProject },
      { id: "composite-logger", label: "&Composite Logger", onClick: () => openTarget("composite-logger", "Composite Logger"), disabled: !currentProject }
    );
  }
  if (caps?.supports_console) {
    if (toolItems.length > 0) toolItems.push({ id: "sep2", label: "", separator: true });
    toolItems.push({
      id: "console",
      label: "&ECU Console",
      onClick: () => openTarget("console", `Console - ${ecuType}`),
      disabled: !currentProject || status.state !== "Connected",
    });
    if (caps.lua_script_constant) {
      toolItems.push({
        id: "lua-console",
        label: "ECU &Lua Editor",
        onClick: () => openTarget("lua-console", "ECU Lua Editor"),
        disabled: !currentProject,
      });
    }
    if (caps.dfu_command_name) {
      toolItems.push({
        id: "enter-dfu",
        label: "Enter &DFU Mode…",
        onClick: () => {
          window.dispatchEvent(new CustomEvent('controller-command:prompt', {
            detail: {
              commandName: caps.dfu_command_name,
              label: 'Enter DFU Mode',
              description: 'Reset the ECU into DFU (firmware update) mode. The connection will drop — use STM32CubeProgrammer, dfu-util, or your board\'s flash tool to update firmware.',
            },
          }));
        },
        disabled: !currentProject || status.state !== "Connected",
      });
    }
  }
  if (toolItems.length > 0) toolItems.push({ id: "sep3", label: "", separator: true });
  toolItems.push({ id: "compare-tables", label: "Table &Compare", onClick: () => setTableComparisonOpen(true), disabled: !currentProject });
  toolItems.push({ id: "tune-file-diff", label: "Tune File &Diff...", onClick: () => setTuneFileDiffOpen(true), disabled: !currentProject });
  toolItems.push({ id: "dyno-overlay", label: "D&yno Data...", onClick: () => setDynoOverlayOpen(true) });
  toolItems.push({ id: "math-channels", label: "&Math Channels...", onClick: () => setMathChannelsDialogOpen(true), disabled: !currentProject });
  toolItems.push({ id: "base-map", label: "Generate &Base Map...", onClick: () => setBaseMapDialogOpen(true), disabled: !currentProject });
  toolItems.push({ id: "sep4", label: "", separator: true });
  toolItems.push(
    { id: "plugins", label: "&Plugins...", onClick: () => setPluginPanelOpen(true) },
    { id: "connection", label: "&ECU Connection...", onClick: () => setConnectionDialogOpen(true) },
    { id: "settings", label: "&Settings...", onClick: () => setSettingsDialogOpen(true) }
  );

  const toolsMenu: TunerMenuItem = { id: "tools", label: t('tools.title'), items: toolItems };

  const helpMenu: TunerMenuItem = {
    id: "help",
    label: t('help.title'),
    items: [
      { id: "docs", label: t('help.userManual'), onClick: () => setUserManualOpen(true) },
      { id: "shortcuts", label: t('help.keyboardShortcuts'), onClick: () => {
        setUserManualSection('reference/shortcuts');
        setUserManualOpen(true);
      }},
      { id: "sep1", label: "", separator: true },
      { id: "about", label: t('help.about'), onClick: () => setAboutDialogOpen(true) },
    ],
  };

  if (currentProject) {
    return [fileMenu, editMenu, viewMenu, ...tuningMenus, toolsMenu, helpMenu];
  }
  return [fileMenu, viewMenu, helpMenu];
}
