import { useState, useEffect, useMemo, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { open } from "@tauri-apps/plugin-dialog";
import { ThemeProvider, useTheme, ThemeName } from "./themes";
import {
  TunerLayout,
  MenuItem as TunerMenuItem,
  ToolbarItem,
  SidebarNode,
  StatusItem,
  Tab,
  TableEditor,
  TableData as TunerTableData,
  StatusIndicator,
  LoggingIndicator,
  SaveDialog,
  LoadDialog,
  BurnDialog,
  NewTuneDialog,
  SettingsDialog,
  AboutDialog,
  ConnectionDialog,
  AutoTuneLive,
  DataLogView,
} from "./components/tuner-ui";
import TsDashboard from "./components/dashboards/TsDashboard";
import DialogRenderer, { DialogDefinition as RendererDialogDef } from "./components/dialogs/DialogRenderer";
import HelpViewer, { HelpTopicData } from "./components/dialogs/HelpViewer";
import SignatureMismatchDialog, { SignatureMismatchInfo } from "./components/dialogs/SignatureMismatchDialog";
import TuneMismatchDialog, { TuneMismatchInfo } from "./components/dialogs/TuneMismatchDialog";
import TuneComparisonDialog from "./components/dialogs/TuneComparisonDialog";
import RestorePointsDialog from "./components/dialogs/RestorePointsDialog";
import ImportProjectWizard from "./components/dialogs/ImportProjectWizard";
import ErrorDetailsDialog, { useErrorDialog } from "./components/dialogs/ErrorDetailsDialog";
import { PluginPanel } from "./plugin";
import { useLoading } from "./components/LoadingContext";
import { useToast } from "./components/ToastContext";
import "./styles";

// Backend types
interface ConnectionStatus {
  state: "Connected" | "Connecting" | "Disconnected" | "Error";
  signature: string | null;
  has_definition: boolean;
  ini_name?: string | null;
}

interface ConnectResult {
  signature: string;
  mismatch_info: SignatureMismatchInfo | null;
}

interface SyncResult {
  success: boolean;
  pages_synced: number;
  pages_failed: number;
  total_pages: number;
  errors: string[];
}

interface SyncStatus {
  pages_synced: number;
  pages_failed: number;
  total_pages: number;
  errors: string[];
}

interface CurrentProject {
  name: string;
  path: string;
  signature: string;
  has_tune: boolean;
  tune_modified: boolean;
  connection: {
    port: string | null;
    baud_rate: number;
  };
}

interface ProjectInfo {
  name: string;
  path: string;
  signature: string;
  modified: string;
}

interface IniEntry {
  id: string;
  name: string;
  signature: string;
  path: string;
}

interface BackendTableData {
  name: string;
  title: string;
  x_axis_name?: string;
  y_axis_name?: string;
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  x_output_channel?: string | null;
  y_output_channel?: string | null;
}

interface BackendCurveData {
  name: string;
  title: string;
  x_bins: number[];
  y_bins: number[];
  x_label: string;
  y_label: string;
  x_axis?: [number, number, number] | null;
  y_axis?: [number, number, number] | null;
  x_output_channel?: string | null;
  gauge?: string | null;
}

interface BackendMenu {
  name: string;
  title: string;
  items: BackendMenuItem[];
}

interface BackendMenuItem {
  type: "SubMenu" | "Table" | "Dialog" | "Separator" | "Std" | "Help";
  label?: string;
  target?: string;
  condition?: string;
  items?: BackendMenuItem[];
}

// Protocol defaults fetched from loaded INI
interface ProtocolDefaults {
  default_baud_rate: number;
  inter_write_delay: number;
  delay_after_port_open: number;
  message_envelope_format?: string | null;
  page_activation_delay: number;
  timeout_ms: number;
}


// PortEditor configuration from backend
interface PortEditorConfig {
  name: string;
  label: string;
  enable_condition?: string;
}

// Tab content types
interface TabContent {
  type: "dashboard" | "table" | "curve" | "dialog" | "portEditor" | "settings" | "project" | "autotune" | "datalog";
  data?: TunerTableData | RendererDialogDef | PortEditorConfig | string;
}

function AppContent() {
  const { theme, setTheme } = useTheme();
  const { showLoading, hideLoading } = useLoading();
  const { showToast } = useToast();
  const { isOpen: errorDialogOpen, errorInfo, showError, hideError } = useErrorDialog();

  // Project state
  const [currentProject, setCurrentProject] = useState<CurrentProject | null>(null);
  const [availableProjects, setAvailableProjects] = useState<ProjectInfo[]>([]);
  const [repositoryInis, setRepositoryInis] = useState<IniEntry[]>([]);
  const [projectDialogOpen, setProjectDialogOpen] = useState(false);
  const [openProjectDialogOpen, setOpenProjectDialogOpen] = useState(false);

  // Connection state
  const [status, setStatus] = useState<ConnectionStatus>({
    state: "Disconnected",
    signature: null,
    has_definition: false,
  });
  const [ports, setPorts] = useState<string[]>([]);
  const [selectedPort, setSelectedPort] = useState("");
  const [baudRate, setBaudRate] = useState(115200);
  const [timeoutMs, setTimeoutMs] = useState(2000);

  // INI-derived defaults
  const [iniDefaults, setIniDefaults] = useState<ProtocolDefaults | null>(null);
  const [baudUserSet, setBaudUserSet] = useState(false);
  const [timeoutUserSet, setTimeoutUserSet] = useState(false);

  // Wrappers that mark user-changed state
  const handleBaudChange = (b: number) => { setBaudRate(b); setBaudUserSet(true); };
  const handleTimeoutChange = (t: number) => { setTimeoutMs(t); setTimeoutUserSet(true); };

  const applyIniDefaults = () => {
    if (!iniDefaults) return;
    if (iniDefaults.default_baud_rate && iniDefaults.default_baud_rate !== 0) {
      setBaudRate(iniDefaults.default_baud_rate);
      setBaudUserSet(true);
    }
    if (iniDefaults.timeout_ms && iniDefaults.timeout_ms !== 0) {
      setTimeoutMs(iniDefaults.timeout_ms);
      setTimeoutUserSet(true);
    }
  };

  // Fetch INI protocol defaults when a definition is loaded
  useEffect(() => {
    if (!status.has_definition) return;
    // Only fetch defaults when running inside Tauri
    const inTauri = !!(window as any).__TAURI_INTERNALS__;
    if (!inTauri) return;

    (async () => {
      try {
        const proto = await invoke<ProtocolDefaults>('get_protocol_defaults');
        setIniDefaults(proto);

        // Auto-apply only if the user hasn't already changed values and they're still the app defaults
        if (!baudUserSet && baudRate === 115200 && proto.default_baud_rate && proto.default_baud_rate !== 0) {
          setBaudRate(proto.default_baud_rate);
        }
        if (!timeoutUserSet && timeoutMs === 2000 && proto.timeout_ms && proto.timeout_ms !== 0) {
          setTimeoutMs(proto.timeout_ms);
        }
      } catch (e) {
        // Not fatal - no definition loaded or call failed
        console.warn('get_protocol_defaults failed:', e);
      }
      
      // Fetch status bar channel defaults from INI FrontPage
      try {
        const defaults = await invoke<string[]>('get_status_bar_defaults');
        if (defaults && defaults.length > 0) {
          setStatusBarChannels(defaults);
        }
      } catch (e) {
        console.warn('get_status_bar_defaults failed:', e);
      }
    })();
  }, [status.has_definition]);

  // Menu/tree state
  const [backendMenus, setBackendMenus] = useState<BackendMenu[]>([]);
  const [constantValues, setConstantValues] = useState<Record<string, number>>({});

  // Realtime data
  const [realtimeData, setRealtimeData] = useState<Record<string, number>>({});
  const [isLogging, setIsLogging] = useState(false);
  const [logDuration, setLogDuration] = useState("");
  
  // Status bar channel configuration - fetched from INI FrontPage or defaults
  const [statusBarChannels, setStatusBarChannels] = useState<string[]>([]);

  // Tabs state - starts empty when no project is loaded
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [tabContents, setTabContents] = useState<Record<string, TabContent>>({});

  // Sidebar state
  const [sidebarVisible, setSidebarVisible] = useState(true);

  // Dialog state
  const [saveDialogOpen, setSaveDialogOpen] = useState(false);
  const [loadDialogOpen, setLoadDialogOpen] = useState(false);
  const [burnDialogOpen, setBurnDialogOpen] = useState(false);
  const [newTuneDialogOpen, setNewTuneDialogOpen] = useState(false);
  const [settingsDialogOpen, setSettingsDialogOpen] = useState(false);
  const [aboutDialogOpen, setAboutDialogOpen] = useState(false);
  const [connectionDialogOpen, setConnectionDialogOpen] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [syncProgress, setSyncProgress] = useState<{ current: number; total: number; percent: number } | null>(null);
  const [helpTopic, setHelpTopic] = useState<HelpTopicData | null>(null);
  
  // Signature mismatch dialog state
  const [signatureMismatchOpen, setSignatureMismatchOpen] = useState(false);
  const [signatureMismatchInfo, setSignatureMismatchInfo] = useState<SignatureMismatchInfo | null>(null);
  
  // Tune mismatch dialog state
  const [tuneMismatchOpen, setTuneMismatchOpen] = useState(false);
  const [tuneMismatchInfo, setTuneMismatchInfo] = useState<TuneMismatchInfo | null>(null);
  
  // Tune comparison dialog state
  const [tuneComparisonOpen, setTuneComparisonOpen] = useState(false);
  
  // Restore points dialog state
  const [restorePointsOpen, setRestorePointsOpen] = useState(false);
  
  // Import project wizard state
  const [importProjectOpen, setImportProjectOpen] = useState(false);
  
  // Plugin panel state
  const [pluginPanelOpen, setPluginPanelOpen] = useState(false);
  
  // Sync status tracking (for partial sync warning)
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);

  // Settings state
  const [unitsSystem, setUnitsSystem] = useState<'metric'|'imperial'>('metric');
  const [autoBurnOnClose, setAutoBurnOnClose] = useState(false);
  // Legacy dashboard settings (removed with TabbedDashboard, may be re-added later)
  // const [indicatorColumnCount, setIndicatorColumnCount] = useState<number | 'auto'>('auto');
  // const [indicatorFillEmpty, setIndicatorFillEmpty] = useState(false);
  // const [indicatorTextFit, setIndicatorTextFit] = useState<'scale' | 'wrap'>('scale');

  // Tauri check
  const [isTauri, setIsTauri] = useState(true);

  // Check if running in Tauri
  useEffect(() => {
    const inTauri = !!(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    setIsTauri(inTauri);
    if (!inTauri) {
      console.warn("Running in browser mode. Use `npm run tauri dev` for full functionality.");
    }
  }, []);

  // Initial data fetch - initialize INI repository and check for existing project
  useEffect(() => {
    if (isTauri) {
      initializeApp();
      const statusInterval = setInterval(checkStatus, 1000);
      return () => clearInterval(statusInterval);
    }
  }, [isTauri]);

  async function initializeApp() {
    showLoading("Initializing LibreTune...");
    try {
      // Initialize INI repository
      await invoke("init_ini_repository");
      
      // Load repository INIs
      const inis = await invoke<IniEntry[]>("list_repository_inis");
      setRepositoryInis(inis);
      
      // Load available projects
      const projects = await invoke<ProjectInfo[]>("list_projects");
      setAvailableProjects(projects);
      
      // Load settings
      try {
        const settings = await invoke<{ 
          units_system?: string; 
          auto_burn_on_close?: boolean;
          indicator_column_count?: string;
          indicator_fill_empty?: boolean;
          indicator_text_fit?: string;
        }>("get_settings");
        if (settings.units_system) setUnitsSystem(settings.units_system as 'metric' | 'imperial');
        if (settings.auto_burn_on_close !== undefined) setAutoBurnOnClose(settings.auto_burn_on_close);
        // Legacy dashboard settings (removed with TabbedDashboard)
        // if (settings.indicator_column_count) { ... }
        // if (settings.indicator_fill_empty !== undefined) { ... }
        // if (settings.indicator_text_fit) { ... }
      } catch (e) {
        console.warn("Failed to load settings:", e);
      }
      
      // Check if there's already a project open (from previous session)
      const project = await invoke<CurrentProject | null>("get_current_project");
      if (project) {
        setCurrentProject(project);
        try {
          // Fetch menus for the project
          const values = await fetchConstants();
          await fetchMenuTree(values);
          
          // Initialize dashboard tab
          setTabs([{ id: "dashboard", title: "Dashboard", icon: "dashboard", closable: false }]);
          setTabContents({ dashboard: { type: "dashboard" } });
          setActiveTabId("dashboard");
        } catch (menuError) {
          console.error("Failed to load menus:", menuError);
          showToast("Menu loading failed. Some features may be unavailable.", "warning");
        }
      }
      
      // Refresh serial ports
      const p = await invoke<string[]>("get_serial_ports");
      setPorts(p);
      if (p.length > 0 && !selectedPort) setSelectedPort(p[0]);
    } catch (e) {
      console.error("Failed to initialize app:", e);
      showToast("Failed to initialize application: " + e, "error");
    } finally {
      hideLoading();
    }
  }

  // Listen for signature mismatch events from backend
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    
    (async () => {
      try {
        unlisten = await listen<SignatureMismatchInfo>("signature:mismatch", (event) => {
          console.log("Signature mismatch detected:", event.payload);
          setSignatureMismatchInfo(event.payload);
          setSignatureMismatchOpen(true);
        });
      } catch (e) {
        console.error("Failed to listen for signature:mismatch events:", e);
      }
    })();
    
    return () => {
      if (unlisten) unlisten();
    };
  }, []);
  
  // Listen for INI change events (requires re-sync)
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    
    (async () => {
      try {
        unlisten = await listen<string>("ini:changed", async (event) => {
          console.log("INI changed:", event.payload);
          if (event.payload === "resync_required" && status.state === "Connected") {
            // Re-sync ECU data with new INI (uses resilient sync)
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
    
    return () => {
      if (unlisten) unlisten();
    };
  }, [status.state, showLoading, hideLoading]);
  
  // Listen for tune mismatch events (after ECU sync)
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    
    (async () => {
      try {
        unlisten = await listen<TuneMismatchInfo>("tune:mismatch", (event) => {
          console.log("Tune mismatch detected:", event.payload);
          setTuneMismatchInfo(event.payload);
          setTuneMismatchOpen(true);
        });
      } catch (e) {
        console.error("Failed to listen for tune:mismatch events:", e);
      }
    })();
    
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Listen for tune loaded events to refresh table data
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    
    (async () => {
      try {
        unlisten = await listen<string>("tune:loaded", async (event) => {
          console.log("Tune loaded from:", event.payload);
          // Refresh ALL open tables when tune is loaded (INI updated or tune file loaded)
          // Small delay to ensure state is current
          await new Promise(resolve => setTimeout(resolve, 50));
          
          // Get current tabs and tabContents
          const currentTabs = tabs;
          const currentTabContents = tabContents;
          const tablesToRefresh: string[] = [];
          const curvesToRefresh: string[] = [];
          
          for (const tab of currentTabs) {
            const tabContent = currentTabContents[tab.id];
            if (tabContent && tabContent.type === "table") {
              tablesToRefresh.push(tab.id);
            } else if (tabContent && tabContent.type === "curve") {
              curvesToRefresh.push(tab.id);
            }
          }
          
          const totalToRefresh = tablesToRefresh.length + curvesToRefresh.length;
          if (totalToRefresh > 0) {
            console.log(`[tune:loaded] Refreshing ${tablesToRefresh.length} table(s) and ${curvesToRefresh.length} curve(s)`);
            const updatedTabs = { ...currentTabContents };
            
            // Refresh all tables and curves in parallel
            await Promise.all([
              ...tablesToRefresh.map(async (tabId) => {
                try {
                  const data = await invoke<BackendTableData>("get_table_data", { tableName: tabId });
                  const tableData: TunerTableData = {
                    name: data.name,
                    xAxis: data.x_bins,
                    yAxis: data.y_bins,
                    zValues: data.z_values,
                    xLabel: data.x_axis_name || "X",
                    yLabel: data.y_axis_name || "Y",
                    xOutputChannel: data.x_output_channel ?? undefined,
                    yOutputChannel: data.y_output_channel ?? undefined,
                  };
                  updatedTabs[tabId] = { type: "table", data: tableData };
                  console.log(`[tune:loaded] ✓ Refreshed table '${tabId}': ${data.z_values.length} values`);
                } catch (e) {
                  console.error(`[tune:loaded] ✗ Failed to refresh table '${tabId}':`, e);
                }
              }),
              ...curvesToRefresh.map(async (tabId) => {
                try {
                  const data = await invoke<BackendCurveData>("get_curve_data", { curveName: tabId });
                  const tableData: TunerTableData = {
                    name: data.name,
                    xAxis: data.x_bins,
                    yAxis: [0],
                    zValues: [data.y_bins],
                    xLabel: data.x_label,
                    yLabel: data.y_label,
                  };
                  updatedTabs[tabId] = { type: "curve", data: tableData };
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
  }, [tabs, tabContents]); // Depend on tabs and tabContents to access current state

  // Refresh table/curve data when a table or curve tab is activated
  useEffect(() => {
    if (!activeTabId) return;
    
    // Check if the active tab is a table or curve using functional update
    setTabContents((prev) => {
      const tabContent = prev[activeTabId];
      if (tabContent && tabContent.type === "table") {
        // Refresh the active table to ensure it has fresh data
        invoke<BackendTableData>("get_table_data", { tableName: activeTabId })
          .then((data) => {
            const tableData: TunerTableData = {
              name: data.name,
              xAxis: data.x_bins,
              yAxis: data.y_bins,
              zValues: data.z_values,
              xLabel: data.x_axis_name || "X",
              yLabel: data.y_axis_name || "Y",
              xOutputChannel: data.x_output_channel ?? undefined,
              yOutputChannel: data.y_output_channel ?? undefined,
            };
            setTabContents((prevTabContents) => ({
              ...prevTabContents,
              [activeTabId]: { type: "table", data: tableData },
            }));
            console.log(`[tab:activated] Refreshed table '${activeTabId}': ${data.z_values.length} values`);
          })
          .catch((e) => {
            console.error(`[tab:activated] Failed to refresh table '${activeTabId}':`, e);
          });
      } else if (tabContent && tabContent.type === "curve") {
        // Refresh the active curve to ensure it has fresh data
        invoke<BackendCurveData>("get_curve_data", { curveName: activeTabId })
          .then((data) => {
            const tableData: TunerTableData = {
              name: data.name,
              xAxis: data.x_bins,
              yAxis: [0],
              zValues: [data.y_bins],
              xLabel: data.x_label,
              yLabel: data.y_label,
            };
            setTabContents((prevTabContents) => ({
              ...prevTabContents,
              [activeTabId]: { type: "curve", data: tableData },
            }));
            console.log(`[tab:activated] Refreshed curve '${activeTabId}': ${data.x_bins.length} points`);
          })
          .catch((e) => {
            console.error(`[tab:activated] Failed to refresh curve '${activeTabId}':`, e);
          });
      }
      return prev; // Return unchanged, update happens in promise
    });
  }, [activeTabId]); // Only depend on activeTabId to avoid infinite loops

  // Realtime streaming
  useEffect(() => {
    let unlistenUpdate: UnlistenFn | null = null;
    let unlistenErr: UnlistenFn | null = null;

    if (status.state === "Connected" && status.has_definition) {
      (async () => {
        try {
          await invoke("start_realtime_stream", { intervalMs: 100 });
        } catch (e) {
          console.warn("Realtime stream failed, falling back to polling:", e);
          const pollInterval = setInterval(fetchRealtimeData, 100);
          unlistenUpdate = () => clearInterval(pollInterval);
        }

        try {
          unlistenUpdate = await listen("realtime:update", (event) => {
            setRealtimeData(event.payload as Record<string, number>);
          });
          unlistenErr = await listen("realtime:error", (event) => {
            console.error("Realtime error:", event.payload);
          });
        } catch (e) {
          console.error("Failed to listen for realtime events:", e);
        }
      })();
    }

    return () => {
      if (unlistenUpdate) unlistenUpdate();
      if (unlistenErr) unlistenErr();
      try {
        invoke("stop_realtime_stream");
      } catch {
        /* ignore */
      }
    };
  }, [status.state, status.has_definition]);

  // Poll logging status when recording
  useEffect(() => {
    if (!isLogging) return;
    
    const interval = setInterval(async () => {
      try {
        const loggingStatus = await invoke<{ is_recording: boolean; entry_count: number; duration_ms: number }>('get_logging_status');
        setIsLogging(loggingStatus.is_recording);
        
        // Format duration as mm:ss
        const seconds = Math.floor(loggingStatus.duration_ms / 1000);
        const mins = Math.floor(seconds / 60);
        const secs = seconds % 60;
        setLogDuration(`${mins}:${secs.toString().padStart(2, '0')}`);
      } catch (err) {
        console.error('Failed to get logging status:', err);
      }
    }, 500);
    
    return () => clearInterval(interval);
  }, [isLogging]);

  // Load menus when definition is loaded
  useEffect(() => {
    if (status.has_definition) {
      fetchConstants().then((values) => {
        fetchMenuTree(values);
      });
    }
  }, [status.has_definition]);

  // Listen for demo mode or definition changes and refresh UI accordingly
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
              // Start realtime streaming for demo
              try { await invoke('start_realtime_stream', { intervalMs: 100 }); } catch (e) { /* ignore */ }
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

    return () => {
      if (unlistenDemo) unlistenDemo();
    };
  }, [isTauri]);

  // Global keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Check for Ctrl/Cmd key
      const isCtrl = e.ctrlKey || e.metaKey;
      
      if (isCtrl && !e.shiftKey) {
        switch (e.key.toLowerCase()) {
          case 'n':
            e.preventDefault();
            setNewTuneDialogOpen(true);
            break;
          case 'o':
            e.preventDefault();
            setLoadDialogOpen(true);
            break;
          case 's':
            e.preventDefault();
            setSaveDialogOpen(true);
            break;
          case 'b':
            e.preventDefault();
            if (status.state === "Connected") {
              setBurnDialogOpen(true);
            }
            break;
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [status.state]);

  // API functions
  async function checkStatus() {
    try {
      const s = await invoke<ConnectionStatus>("get_connection_status");
      setStatus(s);
    } catch (e) {
      console.error(e);
    }
  }

  async function fetchRealtimeData() {
    try {
      setRealtimeData(await invoke<Record<string, number>>("get_realtime_data"));
    } catch (e) {
      console.error(e);
    }
  }

  async function fetchConstants() {
    try {
      const vals = await invoke<Record<string, number>>("get_all_constant_values");
      setConstantValues(vals);
      return vals;
    } catch (e) {
      console.error(e);
      return {};
    }
  }

  async function fetchMenuTree(context?: Record<string, number>) {
    try {
      const tree = await invoke<BackendMenu[]>("get_menu_tree", {
        filterContext: context || constantValues,
      });
      setBackendMenus(tree);
    } catch (e) {
      console.error(e);
    }
  }

  // Sync ECU data with resilient error handling
  // Returns SyncResult and updates syncStatus state
  async function doSync(): Promise<SyncResult | null> {
    setSyncing(true);
    setSyncProgress({ current: 0, total: 0, percent: 0 });
    
    // Listen for sync progress events
    const unlisten = await listen<{ current_page: number; total_pages: number; bytes_read: number; total_bytes: number; complete: boolean; failed_page: boolean }>(
      "sync:progress",
      (event) => {
        const { bytes_read, total_bytes, complete } = event.payload;
        const percent = total_bytes > 0 ? Math.round((bytes_read / total_bytes) * 100) : 0;
        setSyncProgress({ current: bytes_read, total: total_bytes, percent });
        if (complete) {
          setSyncing(false);
          setSyncProgress(null);
        }
      }
    );
    
    try {
      const result = await invoke<SyncResult>("sync_ecu_data");
      
      // Store sync status for status bar indicator
      setSyncStatus({
        pages_synced: result.pages_synced,
        pages_failed: result.pages_failed,
        total_pages: result.total_pages,
        errors: result.errors,
      });
      
      // If partial sync, log errors but don't show scary error dialog
      if (result.pages_failed > 0) {
        console.warn(`Partial sync: ${result.pages_synced}/${result.total_pages} pages succeeded`);
        result.errors.forEach(err => console.warn("Sync error:", err));
      }
      
      // Compare tunes after successful sync
      if (result.pages_synced > 0) {
        try {
          const differs = await invoke<boolean>("compare_project_and_ecu_tunes");
          if (differs) {
            setTuneComparisonOpen(true);
          }
        } catch (e) {
          console.error("Failed to compare tunes:", e);
          // Don't block on comparison failure
        }
      }
      
      return result;
    } catch (e) {
      console.error("Sync failed completely:", e);
      return null;
    } finally {
      unlisten();
      setSyncing(false);
      setSyncProgress(null);
    }
  }

  async function connect() {
    setConnecting(true);
    setSyncProgress(null);
    setSyncStatus(null);
    try {
      // Sanity-check selected port is still available; refresh list if necessary
      if (!ports.includes(selectedPort)) {
        await refreshPorts();
      }

      // If still not present, pick first available and notify user
      if (!ports.includes(selectedPort)) {
        if (ports.length > 0) {
          const old = selectedPort;
          setSelectedPort(ports[0]);
          showToast(`Selected port '${old}' is not available; using '${ports[0]}' instead.`, "warning");
        } else {
          throw new Error('No serial ports available');
        }
      }

      // Connect and get mismatch info directly (no async race)
      const result = await invoke<ConnectResult>("connect_to_ecu", { portName: selectedPort, baudRate, timeoutMs });
      await checkStatus();
      
      // If there's a signature mismatch, show dialog and DON'T auto-sync
      // User must choose to continue or select a different INI first
      if (result.mismatch_info) {
        console.log("Signature mismatch detected:", result.mismatch_info);
        setSignatureMismatchInfo(result.mismatch_info);
        setSignatureMismatchOpen(true);
        // Don't sync yet - wait for user decision
        return;
      }
      
      // If connected and has definition (and no mismatch), sync ECU data
      const newStatus = await invoke<ConnectionStatus>("get_connection_status");
      if (newStatus.state === "Connected" && newStatus.has_definition) {
        await doSync();
      }
    } catch (e) {
      // IMPORTANT: Always check status after connection attempt, even on error
      // This ensures the UI shows the correct disconnected state
      await checkStatus();
      showToast("Connection failed: " + e, "error");
    } finally {
      setConnecting(false);
      setSyncing(false);
    }
  }

  async function disconnect() {
    try {
      await invoke("disconnect_ecu");
      await checkStatus();
    } catch (e) {
      console.error(e);
    }
  }

  async function refreshPorts() {
    try {
      const p = await invoke<string[]>("get_serial_ports");
      setPorts(p);

      if (p.length > 0) {
        // Prefer explicit ttyACM0 if present, otherwise pick first available
        const acm0 = p.find((x) => x.endsWith("ttyACM0"));
        const preferred = acm0 || p[0];

        // If user hasn't chosen a port yet, or current selection is missing, use preferred
        if (!selectedPort || !p.includes(selectedPort)) {
          setSelectedPort(preferred);
        }
      }
    } catch (e) {
      console.error("Failed to refresh ports:", e);
    }
  }

  // Helper to format error for display
  function formatError(e: unknown): { message: string; details: string } {
    const errorStr = String(e);
    // Check for panic messages (Rust panics often contain "panicked" or stack traces)
    if (errorStr.includes("panicked") || errorStr.includes("overflow") || errorStr.includes("thread")) {
      return {
        message: "An internal error occurred while processing the tune file. This may indicate an incompatibility between the INI definition and the tune file.",
        details: errorStr,
      };
    }
    // Check for parse errors
    if (errorStr.includes("parse") || errorStr.includes("Parse") || errorStr.includes("invalid")) {
      return {
        message: "The tune file could not be parsed. It may be corrupted or use an unsupported format.",
        details: errorStr,
      };
    }
    // Default error format
    return {
      message: errorStr,
      details: "",
    };
  }

  // Project management functions
  async function createProject(name: string, iniId: string, tunePath?: string) {
    try {
      const project = await invoke<CurrentProject>("create_project", { 
        name, 
        iniId,
        tunePath: tunePath || null 
      });
      
      // Close dialog IMMEDIATELY after project is created (before any other async calls)
      setProjectDialogOpen(false);
      setCurrentProject(project);
      
      // Show loading spinner while we fetch menus and initialize
      showLoading("Loading project...");
      
      try {
        // Refresh menus for the new project
        const values = await fetchConstants();
        await fetchMenuTree(values);
        
        // Initialize dashboard tab
        setTabs([{ id: "dashboard", title: "Dashboard", icon: "dashboard", closable: false }]);
        setTabContents({ dashboard: { type: "dashboard" } });
        setActiveTabId("dashboard");
        
        // Refresh projects list
        const projects = await invoke<ProjectInfo[]>("list_projects");
        setAvailableProjects(projects);
      } catch (menuError) {
        console.error("Failed to load menus:", menuError);
        showToast("Project created but menu loading failed. Some features may be unavailable.", "warning");
      } finally {
        hideLoading();
      }
    } catch (e) {
      const { message, details } = formatError(e);
      if (details) {
        // Complex error - show detailed dialog for bug reporting
        showError("Failed to Create Project", message, details);
      } else {
        // Simple error - use toast
        showToast("Failed to create project: " + message, "error");
      }
    }
  }

  async function openProject(path: string) {
    // Close dialog immediately
    setOpenProjectDialogOpen(false);
    showLoading("Loading project...");
    
    try {
      const project = await invoke<CurrentProject>("open_project", { path });
      setCurrentProject(project);
      
      // Update port selection from project settings
      if (project.connection.port) {
        setSelectedPort(project.connection.port);
      }
      setBaudRate(project.connection.baud_rate || 115200);
      
      try {
        // Refresh menus for the project
        const values = await fetchConstants();
        await fetchMenuTree(values);
        
        // Reset tabs to dashboard
        setTabs([{ id: "dashboard", title: "Dashboard", icon: "dashboard", closable: false }]);
        setTabContents({ dashboard: { type: "dashboard" } });
        setActiveTabId("dashboard");
      } catch (menuError) {
        console.error("Failed to load menus:", menuError);
        showToast("Project opened but menu loading failed. Some features may be unavailable.", "warning");
      }
    } catch (e) {
      const { message, details } = formatError(e);
      if (details) {
        showError("Failed to Open Project", message, details);
      } else {
        showToast("Failed to open project: " + message, "error");
      }
    } finally {
      hideLoading();
    }
  }

  async function closeProject() {
    try {
      await invoke("close_project");
      setCurrentProject(null);
      
      // Clear menus and reset to no-project state
      setBackendMenus([]);
      setTabs([]);
      setTabContents({});
      setActiveTabId(null);
    } catch (e) {
      showToast("Failed to close project: " + e, "error");
    }
  }

  async function handleCreateRestorePoint() {
    try {
      const result = await invoke<{ filename: string; size: number; timestamp: string }>("create_restore_point");
      showToast(`Restore point created: ${result.filename}`, "success");
    } catch (e) {
      showToast("Failed to create restore point: " + e, "error");
    }
  }

  async function importIniToRepository() {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "INI Definition", extensions: ["ini"] }],
      });
      if (selected && typeof selected === "string") {
        const entry = await invoke<IniEntry>("import_ini", { sourcePath: selected });
        setRepositoryInis([...repositoryInis, entry]);
      }
    } catch (e) {
      console.error(e);
      showToast("Failed to import INI: " + e, "error");
    }
  }

  // Open a table or dialog in a new tab
  const openTarget = useCallback(
    async (name: string, title?: string) => {
      console.log("[openTarget] Opening:", name, "title:", title);
      
      // Check if already open
      const existingTab = tabs.find((t) => t.id === name);
      if (existingTab) {
        setActiveTabId(name);
        return;
      }

      // Handle special built-in views
      if (name === "autotune") {
        const newTab: Tab = { id: "autotune", title: "AutoTune Live", icon: "autotune" };
        setTabs([...tabs, newTab]);
        setTabContents({ ...tabContents, autotune: { type: "autotune", data: "veTable1" } });
        setActiveTabId("autotune");
        return;
      }

      if (name === "datalog") {
        const newTab: Tab = { id: "datalog", title: "Data Logging", icon: "datalog" };
        setTabs([...tabs, newTab]);
        setTabContents({ ...tabContents, datalog: { type: "datalog" } });
        setActiveTabId("datalog");
        return;
      }

      // Try table first
      let tableErr: unknown = null;
      try {
        console.log("[openTarget] Trying as table:", name);
        // Always fetch fresh data from backend (reads from cache or ECU)
        const data = await invoke<BackendTableData>("get_table_data", { tableName: name });
        const tableData: TunerTableData = {
          name: data.name,
          xAxis: data.x_bins,
          yAxis: data.y_bins,
          zValues: data.z_values,
          xLabel: data.x_axis_name || "X",
          yLabel: data.y_axis_name || "Y",
          xOutputChannel: data.x_output_channel ?? undefined,
          yOutputChannel: data.y_output_channel ?? undefined,
        };
        
        console.log(`[openTarget] Loaded table '${name}': ${data.z_values.length} values, ${data.x_bins.length}x${data.y_bins.length} size`);

        // Format title as "Menu Label (ini_name)" when menu label is available
        const displayTitle = title && title !== name
          ? `${title} (${name})`
          : data.title || name;

        const newTab: Tab = {
          id: name,
          title: displayTitle,
          icon: "table",
        };
        setTabs([...tabs, newTab]);
        setTabContents({ ...tabContents, [name]: { type: "table", data: tableData } });
        setActiveTabId(name);
        return;
      } catch (err) {
        tableErr = err;
        console.log("[openTarget] Not a table:", err);
      }

      // Try curve second
      let curveErr: unknown = null;
      try {
        console.log("[openTarget] Trying as curve:", name);
        const data = await invoke<BackendCurveData>("get_curve_data", { curveName: name });
        // Convert curve to table format for TableEditor (1D mode)
        const tableData: TunerTableData = {
          name: data.name,
          xAxis: data.x_bins,
          yAxis: [0],  // Single row for 1D curve
          zValues: [data.y_bins],  // Y values as single row
          xLabel: data.x_label,
          yLabel: data.y_label,
        };
        
        console.log(`[openTarget] Loaded curve '${name}': ${data.x_bins.length} points`);

        // Format title as "Menu Label (ini_name)" when menu label is available
        const displayTitle = title && title !== name
          ? `${title} (${name})`
          : data.title || name;

        const newTab: Tab = {
          id: name,
          title: displayTitle,
          icon: "curve",
        };
        setTabs([...tabs, newTab]);
        setTabContents({ ...tabContents, [name]: { type: "curve", data: tableData } });
        setActiveTabId(name);
        return;
      } catch (err) {
        curveErr = err;
        console.log("[openTarget] Not a curve:", err);
      }

      // Try dialog third
      let dialogErr: unknown = null;
      try {
        console.log("[openTarget] Trying as dialog:", name);
        const def = await invoke<RendererDialogDef>("get_dialog_definition", { name });
        console.log("[openTarget] Dialog found:", def);
        
        // Format title as "Menu Label (ini_name)" when menu label is available
        const displayTitle = title && title !== name
          ? `${title} (${name})`
          : def.title || name;

        const newTab: Tab = {
          id: name,
          title: displayTitle,
          icon: "dialog",
        };
        setTabs([...tabs, newTab]);
        setTabContents({ ...tabContents, [name]: { type: "dialog", data: def } });
        setActiveTabId(name);
        return;
      } catch (err) {
        dialogErr = err;
        console.log("[openTarget] Not a dialog:", err);
      }

      // Try portEditor fourth (for std_port_edit and similar)
      try {
        console.log("[openTarget] Trying as portEditor:", name);
        const portEditor = await invoke<{ name: string; label: string; enable_condition?: string }>("get_port_editor", { name });
        console.log("[openTarget] PortEditor found:", portEditor);
        
        // Format title as "Menu Label (ini_name)" when menu label is available
        const displayTitle = title && title !== name
          ? `${title} (${name})`
          : portEditor.label || name;

        const newTab: Tab = {
          id: name,
          title: displayTitle,
          icon: "dialog",
        };
        setTabs([...tabs, newTab]);
        setTabContents({ ...tabContents, [name]: { type: "portEditor", data: portEditor } });
        setActiveTabId(name);
        return;
      } catch (portErr) {
        // All four failed - show user feedback
        console.error("[openTarget] Failed to open target:", name, 
          "table error:", tableErr, 
          "curve error:", curveErr, 
          "dialog error:", dialogErr,
          "portEditor error:", portErr);
        showToast(`Could not open "${title || name}" - not found as table, curve, dialog, or port editor`, "warning");
      }
    },
    [tabs, tabContents, showToast]
  );

  // Handle standard built-in targets (std_*)
  const handleStdTarget = useCallback(
    (target: string, label: string) => {
      console.log("[handleStdTarget]", target, label);
      
      switch (target) {
        case "std_realtime":
          // Open the realtime dashboard - create tab if it doesn't exist
          setTabs(prev => {
            if (prev.find(t => t.id === "dashboard")) return prev;
            return [{ id: "dashboard", title: "Dashboard", icon: "dashboard", closable: false }, ...prev];
          });
          setTabContents(prev => {
            if (prev.dashboard) return prev;
            return { ...prev, dashboard: { type: "dashboard" } };
          });
          setActiveTabId("dashboard");
          break;
        case "std_ms2gentherm":
        case "std_thermfactor":
          // Thermistor wizard - could open a specific dialog or tab
          console.log("Thermistor wizard not yet implemented:", target);
          break;
        case "std_separator":
          // Separator - no action needed
          break;
        default:
          console.log("Unknown std target:", target);
          // Try to open as a dialog as fallback
          openTarget(target, label);
      }
    },
    [openTarget]
  );

  // Open help topic in a viewer
  const openHelpTopic = useCallback(
    async (topicName: string, title: string) => {
      console.log("[openHelpTopic]", topicName, title);
      
      try {
        const topic = await invoke<HelpTopicData>("get_help_topic", { name: topicName });
        console.log("[openHelpTopic] Got help topic:", topic);
        
        // If there's a web URL and no text content, open directly in browser
        if (topic.web_url && (!topic.text_lines || topic.text_lines.length === 0)) {
          window.open(topic.web_url, "_blank");
          return;
        }
        
        // Otherwise, show the help viewer modal
        setHelpTopic(topic);
      } catch (err) {
        console.error("[openHelpTopic] Failed to get help topic:", topicName, err);
      }
    },
    []
  );

  // Tab handlers
  const handleTabSelect = useCallback((tabId: string) => {
    setActiveTabId(tabId);
  }, []);

  const handleTabClose = useCallback(
    (tabId: string) => {
      const newTabs = tabs.filter((t) => t.id !== tabId);
      const newContents = { ...tabContents };
      delete newContents[tabId];

      setTabs(newTabs);
      setTabContents(newContents);

      if (activeTabId === tabId) {
        setActiveTabId(newTabs[newTabs.length - 1]?.id || "dashboard");
      }
    },
    [tabs, tabContents, activeTabId]
  );

  const handleTabReorder = useCallback((newTabs: Tab[]) => {
    setTabs(newTabs);
  }, []);

  // Pop out a tab to its own window
  const handleTabPopout = useCallback(
    async (tabId: string) => {
      const content = tabContents[tabId];
      const tab = tabs.find((t) => t.id === tabId);
      if (!content || !tab) return;

      // Store data in localStorage for the pop-out window to retrieve
      const storageKey = `popout-${tabId}`;
      localStorage.setItem(storageKey, JSON.stringify({
        data: content.data,
      }));

      // Create the pop-out window
      const label = `popout-${tabId.replace(/[^a-zA-Z0-9]/g, '_')}`;
      
      // Build URL for pop-out window
      // Use current window's origin to ensure it works in both dev and production
      const currentOrigin = window.location.origin;
      const hashParams = `#/popout?tabId=${encodeURIComponent(tabId)}&type=${encodeURIComponent(content.type)}&title=${encodeURIComponent(tab.title)}`;
      const url = `${currentOrigin}/${hashParams}`;
      
      console.log('[handleTabPopout] Creating window with URL:', url);
      console.log('[handleTabPopout] Current origin:', currentOrigin);

      try {
        const webview = new WebviewWindow(label, {
          url,
          title: tab.title,
          width: 900,
          height: 700,
          center: true,
          decorations: true,
          // Enable devtools for debugging
          devtools: true,
        });

        // Wait for window to be created
        await webview.once('tauri://created', () => {
          console.log('Pop-out window created:', label, 'url:', url);
        });

        // Log errors for debugging
        webview.once('tauri://error', (e) => {
          console.error('Pop-out window error:', e);
        });

        // Remove tab from main window
        handleTabClose(tabId);
      } catch (e) {
        console.error('Failed to create pop-out window:', e);
        showToast('Failed to pop out tab: ' + e, 'error');
        // Clean up localStorage
        localStorage.removeItem(storageKey);
      }
    },
    [tabs, tabContents, handleTabClose, showToast]
  );

  // Listen for dock events from pop-out windows
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    (async () => {
      try {
        unlisten = await listen<{
          tabId: string;
          type: TabContent['type'];
          title: string;
          data: TabContent['data'];
        }>('tab:dock', (event) => {
          const { tabId, type, title, data } = event.payload;
          console.log('Tab docking back:', tabId);

          // Re-add the tab
          setTabs((prev) => {
            if (prev.find((t) => t.id === tabId)) return prev;
            return [...prev, { id: tabId, title, icon: type === 'table' || type === 'curve' ? 'table' : type }];
          });
          setTabContents((prev) => ({
            ...prev,
            [tabId]: { type, data },
          }));
          setActiveTabId(tabId);
        });
      } catch (e) {
        console.error('Failed to listen for tab:dock events:', e);
      }
    })();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Listen for table updates from pop-out windows
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    (async () => {
      try {
        unlisten = await listen<{
          tabId: string;
          type: TabContent['type'];
          data: TabContent['data'];
        }>('table:updated', (event) => {
          const { tabId, type, data } = event.payload;
          // Update our local state if we have this tab
          setTabContents((prev) => {
            if (!prev[tabId]) return prev;
            return {
              ...prev,
              [tabId]: { type, data },
            };
          });
        });
      } catch (e) {
        console.error('Failed to listen for table:updated events:', e);
      }
    })();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Convert backend menus to menu bar format
  const menuItems: TunerMenuItem[] = useMemo(() => {
    // File menu changes based on whether a project is open
    const fileMenuItems: TunerMenuItem["items"] = currentProject
      ? [
          // Project open - show full menu
          { id: "new-project", label: "&New Project...", onClick: () => setProjectDialogOpen(true) },
          { id: "open-project", label: "&Open Project...", onClick: () => setOpenProjectDialogOpen(true) },
          { id: "import-project", label: "&Import TS Project...", onClick: () => setImportProjectOpen(true) },
          { id: "close-project", label: "&Close Project", onClick: closeProject },
          { id: "sep1", label: "", separator: true },
          { id: "save", label: "&Save Tune\tCtrl+S", onClick: () => setSaveDialogOpen(true) },
          { id: "saveas", label: "Save Tune &As...", onClick: () => setSaveDialogOpen(true) },
          { id: "load", label: "&Load Tune...\tCtrl+O", onClick: () => setLoadDialogOpen(true) },
          { id: "sep2", label: "", separator: true },
          { id: "create-restore", label: "Create &Restore Point", onClick: handleCreateRestorePoint },
          { id: "restore-points", label: "Restore &Points...", onClick: () => setRestorePointsOpen(true) },
          { id: "sep3", label: "", separator: true },
          { id: "burn", label: "&Burn to ECU\tCtrl+B", onClick: () => setBurnDialogOpen(true), disabled: status.state !== "Connected" },
          { id: "sep4", label: "", separator: true },
          { id: "exit", label: "E&xit", onClick: () => window.close() },
        ]
      : [
          // No project open - limited menu
          { id: "new-project", label: "&New Project...\tCtrl+N", onClick: () => setProjectDialogOpen(true) },
          { id: "open-project", label: "&Open Project...\tCtrl+O", onClick: () => setOpenProjectDialogOpen(true) },
          { id: "import-project", label: "&Import TS Project...", onClick: () => setImportProjectOpen(true) },
          { id: "sep1", label: "", separator: true },
          { id: "import-ini", label: "&Import ECU Definition...", onClick: importIniToRepository },
          { id: "sep2", label: "", separator: true },
          { id: "exit", label: "E&xit", onClick: () => window.close() },
        ];

    const fileMenu: TunerMenuItem = {
      id: "file",
      label: "&File",
      items: fileMenuItems,
    };

    // View menu is always available
    const viewMenu: TunerMenuItem = {
      id: "view",
      label: "&View",
      items: [
        { id: "sidebar", label: "Toggle &Sidebar", onClick: () => setSidebarVisible(!sidebarVisible) },
        { id: "sep1", label: "", separator: true },
        {
          id: "theme",
          label: "&Theme",
          items: [
            { id: "dark", label: "Dark", checked: theme === "dark", onClick: () => setTheme("dark") },
            { id: "light", label: "Light", checked: theme === "light", onClick: () => setTheme("light") },
            { id: "midnight", label: "Midnight", checked: theme === "midnight", onClick: () => setTheme("midnight") },
            { id: "carbon", label: "Carbon", checked: theme === "carbon", onClick: () => setTheme("carbon") },
          ],
        },
      ],
    };

    // Build tuning menus from backend
    // Helper function to recursively convert backend menu items to TunerMenuItem
    const convertMenuItems = (items: BackendMenuItem[], prefix: string): TunerMenuItem["items"] => {
      return items
        .filter((item) => item.type !== "Separator" || item.label)
        .map((item, idx) => {
          if (item.type === "Separator") {
            return { id: `${prefix}-sep-${idx}`, label: "", separator: true };
          }
          if (item.type === "SubMenu" && item.items && item.items.length > 0) {
            // SubMenu with children - recursively convert
            return {
              id: `${prefix}-submenu-${idx}`,
              label: item.label || "",
              items: convertMenuItems(item.items, `${prefix}-${idx}`),
            };
          }
          if (item.type === "Std") {
            // Standard built-in targets like std_realtime, std_ms2gentherm, etc.
            return {
              id: item.target || `${prefix}-std-${idx}`,
              label: item.label || "",
              onClick: () => handleStdTarget(item.target || "", item.label || ""),
            };
          }
          if (item.type === "Help") {
            // Help topic - open help viewer
            return {
              id: item.target || `${prefix}-help-${idx}`,
              label: item.label || "",
              onClick: () => openHelpTopic(item.target || "", item.label || ""),
            };
          }
          // Default: Table or Dialog
          return {
            id: item.target || `${prefix}-item-${idx}`,
            label: item.label || "",
            onClick: () => item.target && openTarget(item.target, item.label),
          };
        });
    };

    const tuningMenus: TunerMenuItem[] = backendMenus.map((menu) => ({
      id: menu.name,
      label: menu.title.replace(/^&/, ""),
      items: convertMenuItems(menu.items, menu.name),
    }));

    const toolsMenu: TunerMenuItem = {
      id: "tools",
      label: "&Tools",
      items: [
        { id: "autotune", label: "&AutoTune Live", onClick: () => openTarget("autotune", "AutoTune Live"), disabled: !currentProject },
        { id: "datalog", label: "&Data Logging", onClick: () => openTarget("datalog", "Data Logging"), disabled: !currentProject },
        { id: "sep1", label: "", separator: true },
        { id: "plugins", label: "&Plugins...", onClick: () => setPluginPanelOpen(true) },
        { id: "sep2", label: "", separator: true },
        { id: "connection", label: "&ECU Connection...", onClick: () => setConnectionDialogOpen(true) },
        { id: "settings", label: "&Settings...", onClick: () => setSettingsDialogOpen(true) },
      ],
    };

    const helpMenu: TunerMenuItem = {
      id: "help",
      label: "&Help",
      items: [
        { id: "docs", label: "&Documentation", onClick: () => window.open("https://libretune.org/docs", "_blank") },
        { id: "about", label: "&About LibreTune", onClick: () => setAboutDialogOpen(true) },
      ],
    };

    // Only show tuning menus if a project is open
    if (currentProject) {
      return [fileMenu, viewMenu, ...tuningMenus, toolsMenu, helpMenu];
    } else {
      return [fileMenu, viewMenu, helpMenu];
    }
  }, [backendMenus, theme, sidebarVisible, status.state, openTarget, handleStdTarget, openHelpTopic, currentProject]);

  // Toolbar items
  const toolbarItems: ToolbarItem[] = useMemo(
    () => [
      { id: "open", icon: "open", tooltip: "Open Tune", onClick: () => setLoadDialogOpen(true) },
      { id: "save", icon: "save", tooltip: "Save Tune", onClick: () => setSaveDialogOpen(true), disabled: !status.has_definition },
      { id: "burn", icon: "burn", tooltip: "Burn to ECU", onClick: () => setBurnDialogOpen(true), disabled: status.state !== "Connected" },
      { id: "sep1", icon: "", tooltip: "", separator: true },
      {
        id: "connect",
        icon: status.state === "Connected" ? "disconnect" : "connect",
        tooltip: status.state === "Connected" ? "Disconnect" : "Connect to ECU",
        active: status.state === "Connected",
        onClick: () => setConnectionDialogOpen(true),
      },
      { id: "realtime", icon: "realtime", tooltip: "Realtime Dashboard", onClick: () => setActiveTabId("dashboard") },
      { id: "sep2", icon: "", tooltip: "", separator: true },
      {
        id: "log-start",
        icon: isLogging ? "log-stop" : "log-start",
        tooltip: isLogging ? "Stop Logging" : "Start Logging",
        active: isLogging,
        onClick: async () => {
          try {
            if (isLogging) {
              await invoke('stop_logging');
              setIsLogging(false);
            } else {
              await invoke('start_logging', { sampleRate: 10 });
              setIsLogging(true);
            }
          } catch (err) {
            console.error('Logging toggle failed:', err);
          }
        },
      },
      { id: "sep3", icon: "", tooltip: "", separator: true },
      { id: "settings", icon: "settings", tooltip: "Settings", onClick: () => setSettingsDialogOpen(true) },
    ],
    [status, isLogging]
  );

  // Build sidebar tree from menus - recursively handle SubMenus (e.g., LUA, GDI groups)
  const buildSidebarItems = useCallback((items: BackendMenuItem[], prefix: string): (SidebarNode & { itemType?: string })[] => {
    return items
      .filter((item) => item.type !== "Separator")
      .map((item, idx) => {
        if (item.type === "SubMenu" && item.items && item.items.length > 0) {
          // Recursively build children for SubMenu
          return {
            id: `${prefix}-submenu-${idx}`,
            label: item.label || "",
            type: "folder" as const,
            children: buildSidebarItems(item.items, `${prefix}-${idx}`),
          };
        }
        // Leaf item - Table, Dialog, Std, or Help
        // Map item type to sidebar node type
        let nodeType: string = "dialog";
        if (item.type === "Table") {
          nodeType = "table";
        } else if (item.type === "Help") {
          nodeType = "help";
        }
        return {
          id: item.target || `${prefix}-${idx}`,
          label: item.label || "",
          type: nodeType as "table" | "dialog" | "help",
          itemType: item.type, // Store original type for click handling
        };
      });
  }, []);

  const sidebarItems: SidebarNode[] = useMemo(() => {
    return backendMenus.map((menu) => ({
      id: menu.name,
      label: menu.title.replace(/^&/, ""),
      type: "folder" as const,
      children: buildSidebarItems(menu.items, menu.name),
    }));
  }, [backendMenus, buildSidebarItems]);

  const handleSidebarItemSelect = useCallback(
    (item: SidebarNode & { itemType?: string }) => {
      console.log('[App] handleSidebarItemSelect called', { id: item.id, label: item.label, type: item.type, itemType: (item as any).itemType });
      if (item.type === "folder") {
        // Folder clicked - expansion handled by Sidebar component
        console.log('[App] Early return - item.type is folder');
        return;
      }
      // Handle based on the original item type
      if (item.itemType === "Std") {
        console.log('[App] Calling handleStdTarget');
        handleStdTarget(item.id, item.label);
      } else if (item.itemType === "Help") {
        console.log('[App] Calling openHelpTopic');
        openHelpTopic(item.id, item.label);
      } else {
        // Table or Dialog
        console.log('[App] Calling openTarget for Table/Dialog');
        openTarget(item.id, item.label);
      }
    },
    [openTarget, handleStdTarget, openHelpTopic]
  );

  // Status bar items - dynamically shows channels from INI FrontPage or defaults
  const statusItems: StatusItem[] = useMemo(() => {
    const items: StatusItem[] = [];

    // Show partial sync warning if any pages failed
    if (syncStatus && syncStatus.pages_failed > 0) {
      items.push({
        id: "sync-warning",
        content: (
          <span 
            className="sync-warning-indicator" 
            title={`Some ECU pages could not be read. This may cause display issues or missing data.\n\nErrors:\n${syncStatus.errors.join('\n')}`}
            style={{ 
              color: '#f59e0b', 
              cursor: 'help',
              display: 'flex',
              alignItems: 'center',
              gap: '4px'
            }}
          >
            ⚠ Partial sync ({syncStatus.pages_synced}/{syncStatus.total_pages})
          </span>
        ),
      });
    }

    if (status.state === "Connected" && statusBarChannels.length > 0) {
      // Add indicators for each configured status bar channel
      for (const channel of statusBarChannels) {
        const value = realtimeData[channel];
        // Format value based on magnitude - integers for large values, 1 decimal for small
        const formatted = value !== undefined 
          ? (Math.abs(value) >= 100 ? value.toFixed(0) : value.toFixed(1))
          : "--";
        items.push({
          id: `status-${channel}`,
          content: <StatusIndicator label={channel} value={formatted} />,
        });
      }
    }

    items.push({
      id: "logging",
      content: <LoggingIndicator isLogging={isLogging} duration={logDuration} />,
      align: "right",
    });

    return items;
  }, [status.state, realtimeData, statusBarChannels, isLogging, logDuration, syncStatus]);

  // Render tab content
  const renderTabContent = () => {
    // If no project is open, show the welcome/no project view
    if (!currentProject) {
      return <NoProjectView 
        projects={availableProjects}
        onNewProject={() => setProjectDialogOpen(true)}
        onOpenProject={(path) => openProject(path)}
        onBrowseProject={() => setOpenProjectDialogOpen(true)}
      />;
    }
    
    // No active tab selected
    if (!activeTabId) return null;
    
    const content = tabContents[activeTabId];
    if (!content) return null;

    switch (content.type) {
      case "dashboard":
        return <TsDashboard 
          realtimeData={realtimeData} 
        />;
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
            realtimeData={realtimeData}
          />
        );
      case "curve":
        return (
          <TableEditor
            data={content.data as TunerTableData}
            onChange={(newData) => {
              if (activeTabId) {
                setTabContents({
                  ...tabContents,
                  [activeTabId]: { type: "curve", data: newData },
                });
              }
            }}
            onBurn={() => setBurnDialogOpen(true)}
            realtimeData={realtimeData}
          />
        );
      case "dialog":
        // Find the active tab to get its formatted title
        const activeTab = tabs.find(t => t.id === activeTabId);
        return (
          <DialogRenderer
            definition={content.data as RendererDialogDef}
            onBack={() => activeTabId && handleTabClose(activeTabId)}
            openTable={(tableName) => openTarget(tableName)}
            context={constantValues}
            displayTitle={activeTab?.title}
            onOptimisticUpdate={(name, value) => {
              // Immediately update the context so sibling fields re-evaluate their conditions
              setConstantValues(prev => ({ ...prev, [name]: value }));
            }}
            onUpdate={async () => {
              // Refresh constants and menu tree when constants are updated
              // This ensures menu visibility conditions are re-evaluated
              const values = await fetchConstants();
              await fetchMenuTree(values);
              // Update context for dialog fields
              setConstantValues(values);
            }}
          />
        );
      case "portEditor":
        return (
          <div className="port-editor-placeholder" style={{ padding: 24 }}>
            <h2>{(content.data as PortEditorConfig)?.label || "Port Editor"}</h2>
            <p>Port editor for: {(content.data as PortEditorConfig)?.name || "unknown"}</p>
            <p style={{ color: 'var(--text-muted)', marginTop: 8 }}>
              <em>Port editor UI is not yet implemented. This feature allows configuring ECU pin assignments.</em>
            </p>
          </div>
        );
      case "settings":
        return <SettingsView />;
      case "autotune":
        return (
          <AutoTuneLive 
            tableName={content.data as string || "veTable1"} 
            onClose={() => handleTabClose("autotune")} 
          />
        );
      case "datalog":
        return <DataLogView realtimeData={realtimeData} />;
      default:
        return null;
    }
  };

  return (
    <>
      <TunerLayout
        menuItems={menuItems}
        toolbarItems={toolbarItems}
        tabs={tabs}
        activeTabId={activeTabId}
        onTabSelect={handleTabSelect}
        onTabClose={handleTabClose}
        onTabReorder={handleTabReorder}
        onTabPopout={handleTabPopout}
        sidebarItems={sidebarItems}
        sidebarVisible={sidebarVisible}
        onSidebarToggle={() => setSidebarVisible(!sidebarVisible)}
        onSidebarItemSelect={handleSidebarItemSelect}
        statusItems={statusItems}
        connected={status.state === "Connected"}
        ecuName={(status.state === "Connected" ? status.signature : (status.ini_name ? status.ini_name : undefined)) as string | undefined}
        realtimeData={realtimeData}
        unitsSystem={unitsSystem}
      >
        {renderTabContent()}
      </TunerLayout>

      {/* Dialogs */}
      <SaveDialog
        isOpen={saveDialogOpen}
        onClose={() => setSaveDialogOpen(false)}
        autoBurnOnClose={autoBurnOnClose}
      />
      <LoadDialog 
        isOpen={loadDialogOpen} 
        onClose={() => setLoadDialogOpen(false)} 
      />
      <BurnDialog 
        isOpen={burnDialogOpen} 
        onClose={() => setBurnDialogOpen(false)} 
        connected={status.state === "Connected"}
      />
      <NewTuneDialog 
        isOpen={newTuneDialogOpen} 
        onClose={() => setNewTuneDialogOpen(false)} 
      />
      <SettingsDialog
        isOpen={settingsDialogOpen}
        onClose={() => setSettingsDialogOpen(false)}
        theme={theme}
        onThemeChange={(t) => setTheme(t as ThemeName)}
        currentProject={currentProject}
        onSettingsChange={(settings) => {
          if (settings.units) setUnitsSystem(settings.units as 'metric' | 'imperial');
          if (settings.autoBurnOnClose !== undefined) setAutoBurnOnClose(settings.autoBurnOnClose);
          if (settings.demoMode !== undefined) setStatus(s => ({ ...s, demo_mode: settings.demoMode }));
          // Legacy dashboard settings (removed with TabbedDashboard)
          // if (settings.indicatorColumnCount !== undefined) { ... }
          // if (settings.indicatorFillEmpty !== undefined) { ... }
          // if (settings.indicatorTextFit) { ... }
        }}
      />
      <AboutDialog 
        isOpen={aboutDialogOpen} 
        onClose={() => setAboutDialogOpen(false)} 
      />
      <ConnectionDialog 
        isOpen={connectionDialogOpen}
        onClose={() => setConnectionDialogOpen(false)}
        ports={ports}
        selectedPort={selectedPort}
        baudRate={baudRate}
        timeoutMs={timeoutMs}
        connected={status.state === "Connected"}
        connecting={connecting || syncing}
        onPortChange={setSelectedPort}
        onBaudChange={handleBaudChange}
        onTimeoutChange={handleTimeoutChange}
        onConnect={connect}
        onDisconnect={disconnect}
        onRefreshPorts={refreshPorts}
        statusMessage={syncing && syncProgress ? `Syncing ECU data... ${syncProgress.percent}%` : undefined}
        iniDefaults={iniDefaults ?? undefined}
        onApplyIniDefaults={applyIniDefaults}
      />
      
      {/* Project Dialogs */}
      <NewProjectDialog
        isOpen={projectDialogOpen}
        onClose={() => setProjectDialogOpen(false)}
        inis={repositoryInis}
        onImportIni={importIniToRepository}
        onCreate={createProject}
      />
      <OpenProjectDialog
        isOpen={openProjectDialogOpen}
        onClose={() => setOpenProjectDialogOpen(false)}
        projects={availableProjects}
        onOpen={openProject}
      />
      
      {/* Tune Comparison Dialog */}
      <TuneComparisonDialog
        isOpen={tuneComparisonOpen}
        onClose={() => setTuneComparisonOpen(false)}
        onUseProjectTune={async () => {
          // Project tune has been written to ECU, refresh UI
          await checkStatus();
        }}
        onUseEcuTune={async () => {
          // ECU tune has been saved to project, refresh UI
          await checkStatus();
        }}
      />
      
      {/* Signature Mismatch Dialog */}
      <SignatureMismatchDialog
        isOpen={signatureMismatchOpen}
        mismatchInfo={signatureMismatchInfo}
        onClose={() => {
          setSignatureMismatchOpen(false);
          setSignatureMismatchInfo(null);
        }}
        onSelectIni={async (path) => {
          console.log("Selected INI:", path);
          setSignatureMismatchOpen(false);
          setSignatureMismatchInfo(null);
          // Re-fetch menus and constants for the new INI
          const values = await fetchConstants();
          fetchMenuTree(values);
          // Sync with the new INI
          await doSync();
        }}
        onContinue={async () => {
          console.log("Continuing with mismatched INI - syncing anyway");
          setSignatureMismatchOpen(false);
          // User explicitly chose to continue - sync even though INI doesn't match
          await doSync();
        }}
      />
      
      {/* Help Viewer */}
      {helpTopic && (
        <HelpViewer
          topic={helpTopic}
          onClose={() => setHelpTopic(null)}
        />
      )}
      
      {/* Tune Mismatch Dialog */}
      <TuneMismatchDialog
        isOpen={tuneMismatchOpen}
        mismatchInfo={tuneMismatchInfo}
        onClose={() => {
          setTuneMismatchOpen(false);
          setTuneMismatchInfo(null);
        }}
        onUseProject={async () => {
          // Refresh menus and constants after loading project tune
          const values = await fetchConstants();
          await fetchMenuTree(values);
        }}
        onUseECU={async () => {
          // ECU tune is already loaded, just refresh UI
          const values = await fetchConstants();
          await fetchMenuTree(values);
        }}
      />
      
      {/* Error Details Dialog - for bug reporting */}
      <ErrorDetailsDialog
        isOpen={errorDialogOpen}
        onClose={hideError}
        title={errorInfo.title}
        message={errorInfo.message}
        details={errorInfo.details}
      />
      
      {/* Restore Points Dialog */}
      <RestorePointsDialog
        isOpen={restorePointsOpen}
        onClose={() => setRestorePointsOpen(false)}
        tuneModified={currentProject?.tune_modified || false}
        onRestorePointLoaded={async () => {
          // Refresh UI after loading restore point
          const values = await fetchConstants();
          await fetchMenuTree(values);
          showToast("Restore point loaded successfully", "success");
        }}
      />
      
      {/* Import Project Wizard */}
      <ImportProjectWizard
        isOpen={importProjectOpen}
        onClose={() => setImportProjectOpen(false)}
        onImportComplete={async (projectPath) => {
          showToast("Project imported successfully", "success");
          // Refresh project list
          const projects = await invoke<ProjectInfo[]>("list_projects");
          setAvailableProjects(projects);
          // Open the imported project
          try {
            const project = await invoke<CurrentProject>("open_project", { path: projectPath });
            setCurrentProject(project);
            // Fetch menus for the project
            const values = await fetchConstants();
            await fetchMenuTree(values);
            // Initialize dashboard tab
            setTabs([{ id: "dashboard", title: "Dashboard", icon: "dashboard", closable: false }]);
            setTabContents({ dashboard: { type: "dashboard" } });
            setActiveTabId("dashboard");
          } catch (e) {
            console.error("Failed to open imported project:", e);
            showToast("Project imported but failed to open: " + e, "error");
          }
        }}
      />
      
      {/* Plugin Panel Dialog */}
      {pluginPanelOpen && (
        <div className="dialog-overlay" onClick={() => setPluginPanelOpen(false)}>
          <div 
            className="dialog-content plugin-dialog" 
            onClick={(e) => e.stopPropagation()}
            style={{ width: '900px', maxWidth: '95vw', height: '600px', maxHeight: '85vh' }}
          >
            <PluginPanel onClose={() => setPluginPanelOpen(false)} />
          </div>
        </div>
      )}
    </>
  );
}

// Settings view
function SettingsView() {
  const { theme, setTheme } = useTheme();
  const { showToast } = useToast();
  const [demoMode, setDemoMode] = useState(false);
  const [demoLoading, setDemoLoading] = useState(false);

  // Check demo mode status on mount
  useEffect(() => {
    invoke<boolean>("get_demo_mode").then(setDemoMode).catch(console.error);
  }, []);

  const handleDemoToggle = async () => {
    setDemoLoading(true);
    try {
      const newValue = !demoMode;
      await invoke("set_demo_mode", { enabled: newValue });
      setDemoMode(newValue);
      
      if (newValue) {
        // Start realtime streaming when demo mode is enabled
        await invoke("start_realtime_stream", { intervalMs: 100 });
      } else {
        // Stop streaming when demo mode is disabled
        await invoke("stop_realtime_stream");
      }
    } catch (err) {
      console.error("Failed to toggle demo mode:", err);
      showToast(`Failed to toggle demo mode: ${err}`, "error");
    } finally {
      setDemoLoading(false);
    }
  };

  return (
    <div style={{ padding: 20 }}>
      <h2 style={{ marginBottom: 20 }}>Settings</h2>
      
      {/* Demo Mode Section */}
      <div style={{ 
        marginBottom: 24, 
        padding: 16, 
        background: demoMode ? 'rgba(255, 152, 0, 0.1)' : 'var(--bg-surface)', 
        border: `1px solid ${demoMode ? '#ff9800' : 'var(--border)'}`,
        borderRadius: 8 
      }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
          <label style={{ fontWeight: 600, fontSize: 14 }}>
            🎮 Demo Mode (Simulated ECU)
          </label>
          <button
            onClick={handleDemoToggle}
            disabled={demoLoading}
            style={{
              padding: '6px 16px',
              background: demoMode ? '#ff9800' : 'var(--bg-elevated)',
              color: demoMode ? 'white' : 'var(--text-primary)',
              border: `1px solid ${demoMode ? '#e65100' : 'var(--border)'}`,
              borderRadius: 4,
              cursor: demoLoading ? 'wait' : 'pointer',
              fontWeight: 500,
            }}
          >
            {demoLoading ? 'Loading...' : demoMode ? 'Disable' : 'Enable'}
          </button>
        </div>
        <p style={{ 
          color: demoMode ? '#ffb74d' : 'var(--text-muted)', 
          fontSize: 12, 
          margin: 0,
          lineHeight: 1.5
        }}>
          ⚠️ This generates <strong>fake sensor data</strong> for UI testing. 
          You are <strong>NOT connected to a real ECU</strong>. 
          The simulated engine idles at ~850 RPM with occasional throttle blips.
        </p>
      </div>

      {/* Theme Section */}
      <div style={{ marginBottom: 16 }}>
        <label style={{ display: "block", marginBottom: 8 }}>Theme</label>
        <select
          value={theme}
          onChange={(e) => setTheme(e.target.value as ThemeName)}
          style={{ padding: 8, minWidth: 200 }}
        >
          <option value="dark">Dark</option>
          <option value="light">Light</option>
          <option value="midnight">Midnight</option>
          <option value="carbon">Carbon</option>
        </select>
      </div>
    </div>
  );
}

// No Project View - shown when no project is open (like TS startup)
function NoProjectView({
  projects,
  onNewProject,
  onOpenProject,
  onBrowseProject,
}: {
  projects: ProjectInfo[];
  onNewProject: () => void;
  onOpenProject: (path: string) => void;
  onBrowseProject: () => void;
}) {
  return (
    <div style={{ 
      display: "flex", 
      flexDirection: "column", 
      alignItems: "center", 
      justifyContent: "center",
      height: "100%",
      padding: 40,
      textAlign: "center",
    }}>
      <div style={{ marginBottom: 32 }}>
        <h1 style={{ fontSize: 32, marginBottom: 8, color: "var(--text-primary)" }}>
          LibreTune
        </h1>
        <p style={{ color: "var(--text-muted)", fontSize: 14 }}>
          Open-source ECU tuning software
        </p>
      </div>
      
      <div style={{ display: "flex", gap: 16, marginBottom: 48 }}>
        <button
          onClick={onNewProject}
          style={{
            padding: "16px 32px",
            fontSize: 16,
            background: "var(--primary)",
            color: "white",
            border: "none",
            borderRadius: 8,
            cursor: "pointer",
          }}
        >
          New Project
        </button>
        <button
          onClick={onBrowseProject}
          style={{
            padding: "16px 32px",
            fontSize: 16,
            background: "var(--bg-elevated)",
            color: "var(--text-primary)",
            border: "1px solid var(--border-default)",
            borderRadius: 8,
            cursor: "pointer",
          }}
        >
          Open Project
        </button>
      </div>
      
      {projects.length > 0 && (
        <div style={{ maxWidth: 500, width: "100%" }}>
          <h3 style={{ marginBottom: 16, color: "var(--text-secondary)" }}>Recent Projects</h3>
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {projects.slice(0, 5).map((project) => (
              <div
                key={project.path}
                onClick={() => onOpenProject(project.path)}
                style={{
                  padding: 16,
                  background: "var(--bg-elevated)",
                  borderRadius: 8,
                  cursor: "pointer",
                  textAlign: "left",
                  border: "1px solid var(--border-default)",
                }}
              >
                <div style={{ fontWeight: 600, marginBottom: 4 }}>{project.name}</div>
                <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{project.signature}</div>
                <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4 }}>
                  Last modified: {new Date(project.modified).toLocaleDateString()}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// New Project Dialog
function NewProjectDialog({
  isOpen,
  onClose,
  inis,
  onImportIni,
  onCreate,
}: {
  isOpen: boolean;
  onClose: () => void;
  inis: IniEntry[];
  onImportIni: () => void;
  onCreate: (name: string, iniId: string, tunePath?: string) => void;
}) {
  const [projectName, setProjectName] = useState("");
  const [selectedIni, setSelectedIni] = useState<string>("");
  const [tunePath, setTunePath] = useState<string>("");
  const [tuneFileName, setTuneFileName] = useState<string>("");
  
  async function browseTune() {
    try {
      const path = await open({
        multiple: false,
        filters: [
          { name: "Tune Files", extensions: ["xml", "msq"] },
          { name: "LibreTune Tune", extensions: ["xml"] },
          { name: "TunerStudio MSQ", extensions: ["msq"] },
          { name: "All Files", extensions: ["*"] },
        ],
      });
      if (path && typeof path === "string") {
        setTunePath(path);
        // Extract just the filename for display
        const parts = path.split(/[\\/]/);
        setTuneFileName(parts[parts.length - 1]);
      }
    } catch (e) {
      console.error("Error browsing for tune:", e);
    }
  }
  
  if (!isOpen) return null;
  
  return (
    <div style={{
      position: "fixed",
      inset: 0,
      background: "rgba(0,0,0,0.6)",
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      zIndex: 1000,
    }}>
      <div style={{
        background: "var(--bg-surface)",
        borderRadius: 12,
        padding: 24,
        minWidth: 500,
        maxHeight: "80vh",
        overflow: "auto",
      }}>
        <h2 style={{ marginBottom: 24 }}>New Project</h2>
        
        <div style={{ marginBottom: 20 }}>
          <label style={{ display: "block", marginBottom: 8, fontWeight: 500 }}>
            Project Name
          </label>
          <input
            type="text"
            value={projectName}
            onChange={(e) => setProjectName(e.target.value)}
            placeholder="My Engine Tune"
            style={{
              width: "100%",
              padding: 12,
              borderRadius: 6,
              border: "1px solid var(--border-default)",
              background: "var(--bg-input)",
              color: "var(--text-primary)",
            }}
          />
        </div>
        
        <div style={{ marginBottom: 20 }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
            <label style={{ fontWeight: 500 }}>ECU Definition</label>
            <button onClick={onImportIni} style={{ padding: "4px 12px", fontSize: 13 }}>
              Import INI...
            </button>
          </div>
          
          {inis.length === 0 ? (
            <div style={{ padding: 20, textAlign: "center", color: "var(--text-muted)", background: "var(--bg-elevated)", borderRadius: 6 }}>
              No ECU definitions imported yet.<br/>
              Click "Import INI..." to add an ECU definition file.
            </div>
          ) : (
            <div style={{ maxHeight: 200, overflow: "auto", border: "1px solid var(--border-default)", borderRadius: 6 }}>
              {inis.map((ini) => (
                <div
                  key={ini.id}
                  onClick={() => setSelectedIni(ini.id)}
                  style={{
                    padding: 12,
                    cursor: "pointer",
                    background: selectedIni === ini.id ? "var(--primary)" : "transparent",
                    color: selectedIni === ini.id ? "white" : "var(--text-primary)",
                    borderBottom: "1px solid var(--border-default)",
                  }}
                >
                  <div style={{ fontWeight: 500 }}>{ini.name}</div>
                  <div style={{ fontSize: 12, opacity: 0.8 }}>{ini.signature}</div>
                </div>
              ))}
            </div>
          )}
        </div>
        
        <div style={{ marginBottom: 20 }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
            <label style={{ fontWeight: 500 }}>Import Existing Tune (Optional)</label>
            <button onClick={browseTune} style={{ padding: "4px 12px", fontSize: 13 }}>
              Browse...
            </button>
          </div>
          <div style={{ 
            padding: 12, 
            background: "var(--bg-elevated)", 
            borderRadius: 6,
            border: "1px solid var(--border-default)",
            color: tunePath ? "var(--text-primary)" : "var(--text-muted)",
            fontSize: 13,
          }}>
            {tunePath ? (
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span>{tuneFileName}</span>
                <button 
                  onClick={() => { setTunePath(""); setTuneFileName(""); }}
                  style={{ padding: "2px 8px", fontSize: 12 }}
                >
                  Clear
                </button>
              </div>
            ) : (
              <span>Start with a blank tune, or import an existing .xml or .msq file</span>
            )}
          </div>
        </div>
        
        <div style={{ display: "flex", gap: 12, justifyContent: "flex-end" }}>
          <button onClick={onClose} style={{ padding: "10px 20px" }}>
            Cancel
          </button>
          <button
            onClick={() => onCreate(projectName, selectedIni, tunePath || undefined)}
            disabled={!projectName.trim() || !selectedIni}
            style={{
              padding: "10px 20px",
              background: (!projectName.trim() || !selectedIni) ? "var(--bg-disabled)" : "var(--primary)",
              color: "white",
              border: "none",
              borderRadius: 6,
              cursor: (!projectName.trim() || !selectedIni) ? "not-allowed" : "pointer",
            }}
          >
            Create Project
          </button>
        </div>
      </div>
    </div>
  );
}

// Open Project Dialog
function OpenProjectDialog({
  isOpen,
  onClose,
  projects,
  onOpen,
}: {
  isOpen: boolean;
  onClose: () => void;
  projects: ProjectInfo[];
  onOpen: (path: string) => void;
}) {
  if (!isOpen) return null;
  
  return (
    <div style={{
      position: "fixed",
      inset: 0,
      background: "rgba(0,0,0,0.6)",
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      zIndex: 1000,
    }}>
      <div style={{
        background: "var(--bg-surface)",
        borderRadius: 12,
        padding: 24,
        minWidth: 500,
        maxHeight: "80vh",
        overflow: "auto",
      }}>
        <h2 style={{ marginBottom: 24 }}>Open Project</h2>
        
        {projects.length === 0 ? (
          <div style={{ padding: 40, textAlign: "center", color: "var(--text-muted)" }}>
            No projects found.<br/>
            Create a new project to get started.
          </div>
        ) : (
          <div style={{ maxHeight: 400, overflow: "auto" }}>
            {projects.map((project) => (
              <div
                key={project.path}
                onClick={() => onOpen(project.path)}
                style={{
                  padding: 16,
                  cursor: "pointer",
                  borderRadius: 8,
                  marginBottom: 8,
                  background: "var(--bg-elevated)",
                  border: "1px solid var(--border-default)",
                }}
              >
                <div style={{ fontWeight: 600, marginBottom: 4 }}>{project.name}</div>
                <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{project.signature}</div>
                <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4 }}>
                  {project.path}
                </div>
              </div>
            ))}
          </div>
        )}
        
        <div style={{ display: "flex", gap: 12, justifyContent: "flex-end", marginTop: 20 }}>
          <button onClick={onClose} style={{ padding: "10px 20px" }}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}

// Main app with theme provider
function App() {
  return (
    <ThemeProvider>
      <AppContent />
    </ThemeProvider>
  );
}

export default App;
