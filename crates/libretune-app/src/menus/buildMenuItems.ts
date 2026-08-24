import type { TFunction } from "i18next";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
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
  tuneModified: boolean;
  status: ConnectionStatus;
  ecuType: string;
  iniCapabilities: IniCapabilities | null;
  backendMenus: BackendMenu[] | null;
  theme: ThemeName;
  sidebarVisible: boolean;
  /** When false, ECU-derived (INI) tuning menus are hidden from the menu bar
   *  (they remain available in the sidebar). */
  showEcuMenus: boolean;
  tabs: Tab[];
  // Callbacks (closures from App)
  openTarget: (target: string, label?: string) => void;
  handleStdTarget: (target: string, label: string) => void;
  openHelpTopic: (topic: string, label: string) => void;
  showToast: (msg: string, kind?: "info" | "success" | "error" | "warning") => void;
  closeProject: () => void;
  handleCreateRestorePoint: () => void;
  // Setters
  setConnectEcuWizardOpen: (open: boolean) => void;
  setImportProjectOpen: (open: boolean) => void;
  setSaveDialogOpen: (open: boolean) => void;
  setLoadDialogOpen: (open: boolean) => void;
  setOnlineIniDialogOpen: (open: boolean) => void;
  setBurnDialogOpen: (open: boolean) => void;
  refreshTuneModified?: () => void | Promise<void>;
  setFirmwareUpdateDialogOpen: (open: boolean) => void;
  setRestorePointsOpen: (open: boolean) => void;
  setTuneHistoryOpen: (open: boolean) => void;
  setSettingsDialogOpen: (open: boolean) => void;
  setMathChannelsDialogOpen: (open: boolean) => void;
  setAfrDelayTestOpen: (open: boolean) => void;
  setBaseMapDialogOpen: (open: boolean) => void;
  setTableComparisonOpen: (open: boolean) => void;
  setTuneFileDiffOpen: (open: boolean) => void;
  setDynoOverlayOpen: (open: boolean) => void;
  setPluginPanelOpen: (open: boolean) => void;
  /** AI assistant panel visibility (for the menu's checked state). */
  agentPanelVisible: boolean;
  setAgentPanelVisible: (visible: boolean) => void;
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

function quitApp(): void {
  getCurrentWindow()
    .close()
    .catch((err) => console.error("[quitApp] Failed to close window:", err));
}

export function buildMenuItems(deps: BuildMenuItemsDeps): TunerMenuItem[] {
  const {
    t, currentProject, tuneModified, status, ecuType, iniCapabilities, backendMenus, theme,
    sidebarVisible, showEcuMenus, tabs, openTarget, handleStdTarget, openHelpTopic, showToast,
    closeProject, handleCreateRestorePoint,
    setConnectEcuWizardOpen, setImportProjectOpen, setSaveDialogOpen, setLoadDialogOpen,
    setOnlineIniDialogOpen,
    setBurnDialogOpen, setFirmwareUpdateDialogOpen, setRestorePointsOpen, setTuneHistoryOpen, setSettingsDialogOpen,
    setMathChannelsDialogOpen, setAfrDelayTestOpen, setBaseMapDialogOpen, setTableComparisonOpen,
    setTuneFileDiffOpen, setDynoOverlayOpen, setPluginPanelOpen, agentPanelVisible, setAgentPanelVisible, setConnectionDialogOpen,
    setUserManualOpen, setUserManualSection, setAboutDialogOpen, setSidebarVisible,
    setTheme, setTabs, setTabContents, setActiveTabId,
  } = deps;

  const fileMenuItems: TunerMenuItem["items"] = currentProject
    ? [
        { id: "connect-ecu-wizard", label: "Connect ECU / New Project…", onClick: () => setConnectEcuWizardOpen(true) },
        { id: "import-project", label: t('file.importProject'), onClick: () => setImportProjectOpen(true) },
        { id: "search-ini-online", label: "Search for INI Online…", onClick: () => setOnlineIniDialogOpen(true) },
        { id: "close-project", label: t('file.closeProject'), onClick: closeProject },
        { id: "sep1", label: "", separator: true },
        { id: "save", label: t('file.saveTune'), onClick: () => setSaveDialogOpen(true) },
        { id: "saveas", label: t('file.saveTuneAs'), onClick: () => setSaveDialogOpen(true) },
        { id: "load", label: t('file.loadTune'), onClick: () => setLoadDialogOpen(true) },
        { id: "sep2", label: "", separator: true },
        { id: "create-restore", label: t('file.createRestorePoint'), onClick: handleCreateRestorePoint },
        { id: "restore-points", label: t('file.restorePoints'), onClick: () => setRestorePointsOpen(true) },
        { id: "tune-history", label: t('file.tuneHistory'), onClick: () => setTuneHistoryOpen(true) },
        { id: "sep3", label: "", separator: true },
        { id: "burn", label: t('file.burnToEcu'), onClick: () => setBurnDialogOpen(true), disabled: status.state !== "Connected" || !tuneModified },
        { id: "sep4", label: "", separator: true },
        { id: "exit", label: t('file.exit'), onClick: quitApp },
      ]
    : [
        { id: "connect-ecu-wizard", label: "Connect ECU / New Project…", onClick: () => setConnectEcuWizardOpen(true) },
        { id: "import-project", label: t('file.importProject'), onClick: () => setImportProjectOpen(true) },
        { id: "search-ini-online", label: "Search for INI Online…", onClick: () => setOnlineIniDialogOpen(true) },
        { id: "sep1", label: "", separator: true },
        { id: "settings", label: t('file.settings'), onClick: () => setSettingsDialogOpen(true) },
        { id: "sep2", label: "", separator: true },
        { id: "exit", label: t('file.exit'), onClick: quitApp },
      ];

  const fileMenu: TunerMenuItem = { id: "file", label: t('file.title'), items: fileMenuItems };

  const viewMenu: TunerMenuItem = {
    id: "view",
    label: t('view.title'),
    items: [
      { id: "dashboard", label: t('view.dashboard'), onClick: () => {
        if (!tabs.find(tab => tab.id === "dashboard")) {
          // Tab title is intentionally a plain string (no '&' mnemonic);
          // see i18n note: menu labels embed '&' for the menu renderer only.
          setTabs(prev => [{ id: "dashboard", title: "Dashboard", icon: "dashboard", closable: false }, ...prev]);
          setTabContents(prev => ({ ...prev, dashboard: { type: "dashboard" } }));
        }
        setActiveTabId("dashboard");
      }},
      { id: "sidebar", label: t('view.toggleSidebar'), onClick: () => setSidebarVisible(!sidebarVisible) },
      { id: "sep1", label: "", separator: true },
      {
        id: "theme",
        label: t('view.theme'),
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
      { id: "undo", label: t('edit.undo'), onClick: () => showToast("Undo - use table-specific controls", "info"), disabled: !currentProject },
      { id: "redo", label: t('edit.redo'), onClick: () => showToast("Redo - use table-specific controls", "info"), disabled: !currentProject },
      { id: "sep1", label: "", separator: true },
      { id: "cut", label: t('edit.cut'), onClick: () => showToast("Cut - select cells in table first", "info"), disabled: !currentProject },
      { id: "copy", label: t('edit.copy'), onClick: () => showToast("Copy - select cells in table first", "info"), disabled: !currentProject },
      { id: "paste", label: t('edit.paste'), onClick: () => showToast("Paste - select cells in table first", "info"), disabled: !currentProject },
      { id: "sep2", label: "", separator: true },
      { id: "reset-defaults", label: t('edit.resetToDefaults'), onClick: async () => {
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
            disabled: item.enabled === false,
            items: convertMenuItems(item.items, `${prefix}-${idx}`),
          };
        }
        if (item.type === "Std") {
          return {
            id: item.target || `${prefix}-std-${idx}`,
            label: item.label || "",
            disabled: item.enabled === false,
            onClick: () => handleStdTarget(item.target || "", item.label || ""),
          };
        }
        if (item.type === "Help") {
          return {
            id: item.target || `${prefix}-help-${idx}`,
            label: item.label || "",
            disabled: item.enabled === false,
            onClick: () => openHelpTopic(item.target || "", item.label || ""),
          };
        }
        return {
          id: item.target || `${prefix}-item-${idx}`,
          label: item.label || "",
          disabled: item.enabled === false,
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

  toolItems.push({ id: "autotune", label: t('tools.autotune'), onClick: () => openTarget("autotune", "AutoTune"), disabled: !currentProject });
  if (caps?.has_datalog_entries || caps?.has_output_channels) {
    toolItems.push({ id: "datalog", label: t('tools.dataLogging'), onClick: () => openTarget("datalog", "Data Logging"), disabled: !currentProject });
    toolItems.push({ id: "datalog-viewer", label: "Datalog Viewer…", onClick: () => openTarget("datalog-viewer", "Datalog Viewer"), disabled: !currentProject });
    toolItems.push({ id: "och-status", label: t('tools.outputChannelStatus'), onClick: () => openTarget("och-status", "Output Channel Status"), disabled: !currentProject });
  }
  if (caps?.has_logger_definitions) {
    if (toolItems.length > 0) toolItems.push({ id: "sep1", label: "", separator: true });
    toolItems.push(
      { id: "tooth-logger", label: t('tools.toothLogger'), onClick: () => openTarget("tooth-logger", "Tooth Logger"), disabled: !currentProject },
      { id: "composite-logger", label: t('tools.compositeLogger'), onClick: () => openTarget("composite-logger", "Composite Logger"), disabled: !currentProject }
    );
  }
  if (caps?.supports_console) {
    if (toolItems.length > 0) toolItems.push({ id: "sep2", label: "", separator: true });
    toolItems.push({
      id: "console",
      label: t('tools.ecuConsole'),
      onClick: () => openTarget("console", `Console - ${ecuType}`),
      disabled: !currentProject || status.state !== "Connected",
    });
    if (caps.lua_script_constant) {
      toolItems.push({
        id: "lua-console",
        label: t('tools.ecuLuaEditor'),
        onClick: () => openTarget("lua-console", "ECU Lua Editor"),
        disabled: !currentProject,
      });
    }
    if (caps.dfu_command_name || caps.openblt_command_name) {
      toolItems.push({
        id: "firmware-update",
        label: t('tools.updateFirmware'),
        onClick: () => setFirmwareUpdateDialogOpen(true),
        disabled: !currentProject || status.state !== "Connected",
      });
    }
    if (caps.dfu_command_name) {
      toolItems.push({
        id: "enter-dfu",
        label: t('tools.enterDfuMode'),
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
  toolItems.push({ id: "compare-tables", label: t('tools.tableCompare'), onClick: () => setTableComparisonOpen(true), disabled: !currentProject });
  toolItems.push({ id: "tune-file-diff", label: t('tools.tuneFileDiff'), onClick: () => setTuneFileDiffOpen(true), disabled: !currentProject });
  toolItems.push({ id: "dyno-overlay", label: t('tools.dynoData'), onClick: () => setDynoOverlayOpen(true) });
  toolItems.push({ id: "math-channels", label: t('tools.mathChannels'), onClick: () => setMathChannelsDialogOpen(true), disabled: !currentProject });
  // Requires a live ECU as well as a project; the command reports clearly
  // if reqFuel cannot be read, so the menu only guards on the project.
  toolItems.push({ id: "afr-delay-test", label: "AFR Delay Test…", onClick: () => setAfrDelayTestOpen(true), disabled: !currentProject });
  toolItems.push({ id: "base-map", label: t('tools.generateBaseMap'), onClick: () => setBaseMapDialogOpen(true), disabled: !currentProject });
  toolItems.push({ id: "sep4", label: "", separator: true });
  toolItems.push(
    { id: "plugins", label: t('tools.plugins'), onClick: () => setPluginPanelOpen(true) },
    { id: "ai-assistant", label: t('tools.aiAssistant'), checked: agentPanelVisible, onClick: () => setAgentPanelVisible(!agentPanelVisible) },
    { id: "connection", label: t('tools.ecuConnection'), onClick: () => setConnectionDialogOpen(true) },
    { id: "settings", label: t('file.settings'), onClick: () => setSettingsDialogOpen(true) }
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
    // Group the top-level menus into three zones separated by vertical
    // dividers: app-action menus (translated, stable positions) | ECU
    // tuning menus (INI-native, untranslated) | Help (rightmost).
    // Omit the ECU zone entirely when there are no tuning menus to show
    // (empty INI menu tree or the user disabled them in settings), so no
    // dangling divider appears before Help.
    const ecuMenus = showEcuMenus ? tuningMenus : [];
    const topBar: TunerMenuItem[] = [fileMenu, editMenu, viewMenu, toolsMenu];
    if (ecuMenus.length > 0) {
      topBar.push(
        { id: "menubar-sep-ecu", label: "", separator: true },
        ...ecuMenus,
        { id: "menubar-sep-help", label: "", separator: true },
      );
    }
    topBar.push(helpMenu);
    return topBar;
  }
  return [fileMenu, viewMenu, helpMenu];
}
