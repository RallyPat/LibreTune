import { useState, useEffect, useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ThemeProvider, useTheme } from "./themes";
import { initializeHotkeyManager } from "./services/hotkeyService";
import { useRealtimeStore } from "./stores/realtimeStore";
import { LANGUAGE_STORAGE_KEY } from "./i18n/languages";
import {
  TunerLayout,
  MenuItem as TunerMenuItem,
  ToolbarItem,
  SidebarNode,
  StatusItem,
  Tab,
  LoggingIndicator,
} from "./components/tuner-ui";
import { SimpleGaugeInfo } from "./components/curves/CurveEditor";
import type { DialogDefinition as RendererDialogDef } from "./components/dialogs/DialogRenderer";
import { HelpTopicData } from "./components/dialogs/HelpViewer";
import { SignatureMismatchInfo } from "./components/dialogs/SignatureMismatchDialog";
import { TuneMismatchInfo } from "./components/dialogs/TuneMismatchDialog";
import { BaseMapResult } from "./components/dialogs/BaseMapDialog";
import { useErrorDialog } from "./components/dialogs/ErrorDetailsDialog";
import ErrorBoundary from "./components/common/ErrorBoundary";
import { DialogOverlays } from "./components/DialogOverlays";
import { PinConfig } from "./components/hardware/PortEditor";
import { useLoading } from "./components/LoadingContext";
import { useToast } from "./components/ToastContext";
import { formatError } from "./utils/formatError";
import { ensureRealtimeListener } from "./services/realtimeListener";
import { buildSidebarItems } from "./utils/buildSidebarItems";
import { TabContentRouter } from "./components/TabContentRouter";
import { buildMenuItems } from "./menus/buildMenuItems";
import { buildToolbarItems } from "./menus/buildToolbarItems";
import { openTargetImpl } from "./services/openTarget";
import {
  type ConnectionStatus,
  type ConnectResult,
  type SyncResult,
  type SyncStatus,
  type CurrentProject,
  type IniCapabilities,
  type ProjectInfo,
  type IniEntry,
  type BackendTableData,
  type BackendCurveData,
  type ChannelInfo,
  type BackendMenu,
  type ProtocolDefaults,
  type TabContent,
  toTunerTableData,
  toCurveData,
} from "./types/app";
import "./styles";

function AppContent() {
  const { theme, setTheme } = useTheme();
  const { t } = useTranslation('menu');
  const { showLoading, hideLoading } = useLoading();
  const { showToast } = useToast();
  const { isOpen: errorDialogOpen, errorInfo, showError, hideError } = useErrorDialog();

  // Project state
  const [currentProject, setCurrentProject] = useState<CurrentProject | null>(null);
  const [availableProjects, setAvailableProjects] = useState<ProjectInfo[]>([]);
  const [repositoryInis, setRepositoryInis] = useState<IniEntry[]>([]);
  const [newProjectDialogOpen, setNewProjectDialogOpen] = useState(false);
  const [baseMapDialogOpen, setBaseMapDialogOpen] = useState(false);

  // Connection state
  const [status, setStatus] = useState<ConnectionStatus>({
    state: "Disconnected",
    signature: null,
    has_definition: false,
  });
  const [ecuType, setEcuType] = useState<string>("Unknown");
  const [ports, setPorts] = useState<string[]>([]);
  const [selectedPort, setSelectedPort] = useState("");
  const [baudRate, setBaudRate] = useState(115200);
  const [timeoutMs, setTimeoutMs] = useState(2000);
  const [connectionType, setConnectionType] = useState<'Serial' | 'Tcp'>("Serial");
  const [tcpHost, setTcpHost] = useState("localhost");
  const [tcpPort, setTcpPort] = useState(29001);

  // INI-derived defaults
  const [iniDefaults, setIniDefaults] = useState<ProtocolDefaults | null>(null);
  const [iniCapabilities, setIniCapabilities] = useState<IniCapabilities | null>(null);
  const [baudUserSet, setBaudUserSet] = useState(false);
  const [timeoutUserSet, setTimeoutUserSet] = useState(false);

  const [portEditorAssignments, setPortEditorAssignments] = useState<Record<string, PinConfig[]>>({});

  // Runtime packet mode defaults
  const [defaultRuntimePacketMode, setDefaultRuntimePacketMode] = useState<'Auto'|'ForceBurst'|'ForceOCH'|'Disabled'>('Auto');
  const [connectionRuntimePacketMode, setConnectionRuntimePacketMode] = useState<'Auto'|'ForceBurst'|'ForceOCH'|'Disabled'>('Auto');

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
    if (!status.has_definition) {
      setIniCapabilities(null);
      return;
    }
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

      // Fetch channel metadata from INI for labels/units
      try {
        const channels = await invoke<ChannelInfo[]>('get_available_channels');
        const map: Record<string, ChannelInfo> = {};
        channels.forEach((ch) => {
          map[ch.name] = ch;
        });
        setChannelInfoMap(map);
      } catch (e) {
        console.warn('get_available_channels failed:', e);
      }

      // Fetch INI-driven capability map for feature gating
      try {
        const caps = await invoke<IniCapabilities>('get_ini_capabilities');
        setIniCapabilities(caps);
      } catch (e) {
        console.warn('get_ini_capabilities failed:', e);
        setIniCapabilities(null);
      }
    })();
  }, [status.has_definition]);

  // Menu/tree state
  const [backendMenus, setBackendMenus] = useState<BackendMenu[]>([]);
  const [constantValues, setConstantValues] = useState<Record<string, number>>({});
  const [searchIndex, setSearchIndex] = useState<Record<string, string[]>>({});

  // Status bar channel configuration - fetched from INI FrontPage or defaults
  const [statusBarChannels, setStatusBarChannels] = useState<string[]>([]);
  const [channelInfoMap, setChannelInfoMap] = useState<Record<string, ChannelInfo>>({});

  // Realtime data - now managed by Zustand store for efficient per-channel subscriptions
  // Components use useChannelValue() or useChannels() hooks to subscribe to specific channels
  // NOTE: App.tsx does NOT subscribe to realtime channels — the StatusBar handles its own
  // subscriptions via individual RealtimeChannelCell components to avoid re-rendering
  // this 3500-line component at 20 Hz.
  const [isLogging, setIsLogging] = useState(false);
  const [logDuration, setLogDuration] = useState("");

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
  const [mathChannelsDialogOpen, setMathChannelsDialogOpen] = useState(false);
  const [aboutDialogOpen, setAboutDialogOpen] = useState(false);
  const [connectionDialogOpen, setConnectionDialogOpen] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [syncProgress, setSyncProgress] = useState<{ current: number; total: number; percent: number } | null>(null);
  const [helpTopic, setHelpTopic] = useState<HelpTopicData | null>(null);
  const [userManualOpen, setUserManualOpen] = useState(false);
  const [userManualSection, setUserManualSection] = useState<string | undefined>(undefined);
  
  // Signature mismatch dialog state
  const [signatureMismatchOpen, setSignatureMismatchOpen] = useState(false);
  const [signatureMismatchInfo, setSignatureMismatchInfo] = useState<SignatureMismatchInfo | null>(null);
  
  // Tune mismatch dialog state
  const [tuneMismatchOpen, setTuneMismatchOpen] = useState(false);
  const [tuneMismatchInfo, setTuneMismatchInfo] = useState<TuneMismatchInfo | null>(null);
  
  // Tune comparison dialog state
  const [tuneComparisonOpen, setTuneComparisonOpen] = useState(false);
  
  // Table comparison dialog state
  const [tableComparisonOpen, setTableComparisonOpen] = useState(false);
  
  // Performance calculator dialog state
  const [performanceDialogOpen, setPerformanceDialogOpen] = useState(false);
  
  // Restore points dialog state
  const [restorePointsOpen, setRestorePointsOpen] = useState(false);
  
  // Tune history panel state (git versioning)
  const [tuneHistoryOpen, setTuneHistoryOpen] = useState(false);
  
  // Import project wizard state
  const [importProjectOpen, setImportProjectOpen] = useState(false);
  
  // Migration report dialog state (shown when loading a tune from different INI version)
  const [migrationReportOpen, setMigrationReportOpen] = useState(false);
  
  // Tune file diff dialog state (cross-file comparison + merge)
  const [tuneFileDiffOpen, setTuneFileDiffOpen] = useState(false);
  
  // Dyno overlay dialog state
  const [dynoOverlayOpen, setDynoOverlayOpen] = useState(false);
  
  // WASM Plugin panel state
  const [pluginPanelOpen, setPluginPanelOpen] = useState(false);
  
  // Sync status tracking (for partial sync warning)
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);

  // Onboarding state
  const [onboardingOpen, setOnboardingOpen] = useState(false);

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
    // Expose debug helper to browser console
    (window as any).__libretune_debug = async () => {
      try {
        const result = await invoke<string>('debug_single_realtime_read');
        console.log('[DEBUG] Single realtime read result:\n' + result);
        return result;
      } catch (e) {
        console.error('[DEBUG] debug_single_realtime_read failed:', e);
        return e;
      }
    };
  }, []);

  // Initial data fetch - initialize INI repository and check for existing project
  useEffect(() => {
    if (isTauri) {
      initializeApp();
      const statusInterval = setInterval(checkStatus, 1000);
      return () => clearInterval(statusInterval);
    }
  }, [isTauri]);

  // Update window title with project name
  useEffect(() => {
    const base = "LibreTune";
    if (currentProject) {
      const title = `${currentProject.name} — ${base}`;
      document.title = title;
      // Also update Tauri window title
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
      let settings: any = {};
      try {
        settings = await invoke("get_settings");
        if (settings.units_system) setUnitsSystem(settings.units_system as 'metric' | 'imperial');
        if (settings.auto_burn_on_close !== undefined) setAutoBurnOnClose(settings.auto_burn_on_close);
        if (settings.status_bar_channels) setStatusBarChannels(settings.status_bar_channels);
        // Honor saved UI language preference (mirror to localStorage so the
        // i18n LanguageDetector picks it up on next app start, and switch live now).
        if (settings.language && typeof settings.language === 'string') {
          try {
            const stored = localStorage.getItem(LANGUAGE_STORAGE_KEY);
            if (stored !== settings.language) {
              localStorage.setItem(LANGUAGE_STORAGE_KEY, settings.language);
            }
          } catch { /* ignore */ }
          // Lazy-load the i18n instance to avoid pulling i18next into the
          // initial app bundle path.
          void import('./i18n').then(({ default: i18n }) => {
            if (i18n.language !== settings.language) {
              void i18n.changeLanguage(settings.language);
            }
          });
        }
      } catch (e) {
        console.warn("Failed to load settings:", e);
      }
      
      // Load custom hotkey bindings
      try {
        await initializeHotkeyManager();
      } catch (e) {
        console.warn("Failed to load hotkey bindings:", e);
      }
      
      // Check if onboarding has been completed
      try {
        const onboardingCompleted = await invoke<boolean>("is_onboarding_completed");
        if (!onboardingCompleted) {
          setOnboardingOpen(true);
        }
      } catch (e) {
        console.warn("Failed to check onboarding status:", e);
        // If we can't check, show onboarding (safer default)
        setOnboardingOpen(true);
      }
      
      // Check if there's already a project open (backend memory)
      let project = await invoke<CurrentProject | null>("get_current_project");

      // If no project in memory, try restoring last opened project from settings
      if (!project && settings.last_project_path) {
        try {
          console.log("Restoring last project:", settings.last_project_path);
          project = await invoke<CurrentProject>("open_project", { path: settings.last_project_path });
        } catch (e) {
          console.warn("Failed to restore last project:", e);
        }
      }
      
      if (project) {
        setCurrentProject(project);
        try {
          // Fetch menus for the project
          const values = await fetchConstants();
          await fetchMenuTree(values);
          
          let restored = false;
          // Dashboard base is always needed
          const dashTab = { id: "dashboard", title: "Dashboard", icon: "dashboard", closable: false };

          if (settings.last_active_tab && settings.last_active_tab !== "dashboard") {
             const tabId = settings.last_active_tab;
             
             // Define known tools mapping for quick persistence restoration
             const TOOLS: Record<string, { title: string, icon: string, type: TabContent['type'] }> = {
               "console": { title: "ECU Console", icon: "terminal", type: "console" },
               "datalog": { title: "Data Logging", icon: "datalog", type: "datalog" },
               "autotune": { title: "AutoTune", icon: "autotune", type: "autotune" },
               "tooth-logger": { title: "Tooth Logger", icon: "scope", type: "tooth-logger" },
               "composite-logger": { title: "Composite Logger", icon: "scope", type: "composite-logger" },
               "och-status": { title: "Output Channel Status", icon: "dashboard", type: "och-status" },
               "lua-console": { title: "Lua Console", icon: "terminal", type: "lua-console" },
               "settings": { title: "Settings", icon: "settings", type: "settings" },
             };

             try {
                if (TOOLS[tabId]) {
                   // Case 1: Known Tool request
                   const tool = TOOLS[tabId];
                   console.log(`Restoring tool tab: ${tabId}`);
                   setTabs([dashTab, { id: tabId, title: tool.title, icon: tool.icon }]);
                   setTabContents({ 
                     dashboard: { type: "dashboard" },
                     [tabId]: { type: tool.type, data: tabId === "autotune" ? "" : undefined }
                   });
                   setActiveTabId(tabId);
                   restored = true;
                } else {
                   // Case 2: Content (Table, Curve, Dialog, PortEditor)
                   // Remove "table:" prefix if present (legacy support)
                   const targetName = tabId.startsWith("table:") ? tabId.replace("table:", "") : tabId;
                   console.log(`Restoring content tab: ${targetName}`);

                   // Strategy: Try Table -> Curve -> Dialog -> PortEditor
                   try {
                     // 1. Try Table
                     const data = await invoke<BackendTableData>("get_table_data", { tableName: targetName });
                     const tableData = toTunerTableData(data);
                     
                     setTabs([dashTab, { id: targetName, title: data.title || targetName, icon: "table" }]);
                     setTabContents({ 
                       dashboard: { type: "dashboard" },
                       [targetName]: { type: "table", data: tableData }
                     });
                     setActiveTabId(targetName);
                     restored = true;
                   } catch {
                     // 2. Try Curve
                     try {
                       const data = await invoke<BackendCurveData>("get_curve_data", { curveName: targetName });
                       const curveData = toCurveData(data);
                       let gaugeInfo: SimpleGaugeInfo | null = null;
                       if (data.gauge) {
                         try {
                           gaugeInfo = await invoke<SimpleGaugeInfo>("get_gauge_config", { gaugeName: data.gauge });
                         } catch (e) { console.warn("Gauge load failed", e); }
                       }
                       
                       setTabs([dashTab, { id: targetName, title: data.title || targetName, icon: "curve" }]);
                       setTabContents({ 
                         dashboard: { type: "dashboard" },
                         [targetName]: { type: "curve", data: curveData, gauge: gaugeInfo }
                       });
                       setActiveTabId(targetName);
                       restored = true;
                     } catch {
                       // 3. Try Dialog
                       try {
                         const def = await invoke<RendererDialogDef>("get_dialog_definition", { name: targetName });
                         setTabs([dashTab, { id: targetName, title: def.title || targetName, icon: "dialog" }]);
                         setTabContents({ 
                           dashboard: { type: "dashboard" },
                           [targetName]: { type: "dialog", data: def }
                         });
                         setActiveTabId(targetName);
                         restored = true;
                       } catch {
                          // 4. Try PortEditor
                          try {
                            const portEditor = await invoke<{ name: string; label: string }>("get_port_editor", { name: targetName });
                            const assignments = await invoke("get_port_editor_assignments", { name: portEditor.name }).catch(() => []);
                            setPortEditorAssignments(prev => ({ ...prev, [portEditor.name]: assignments as any }));

                            setTabs([dashTab, { id: targetName, title: portEditor.label || targetName, icon: "dialog" }]);
                            setTabContents({ 
                              dashboard: { type: "dashboard" },
                              [targetName]: { type: "portEditor", data: portEditor }
                            });
                            setActiveTabId(targetName);
                            restored = true;
                          } catch {
                            console.warn(`Could not restore tab '${tabId}' - content not found.`);
                          }
                       }
                     }
                   }
                }
             } catch (restoreErr) {
                console.warn("Failed to restore tab:", restoreErr);
             }
          }

          if (!restored) {
            setTabs([dashTab]);
            setTabContents({ dashboard: { type: "dashboard" } });
            setActiveTabId("dashboard");
          }
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

  // Listen for migration needed events from backend
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    
    (async () => {
      try {
        unlisten = await listen("tune:migration_needed", () => {
          console.log("Tune migration needed - opening dialog");
          setMigrationReportOpen(true);
        });
      } catch (e) {
        console.error("Failed to listen for tune:migration_needed events:", e);
      }
    })();
    
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Listen for frontend reconnect requests (dispatched by e.g., controller command flow)
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    (async () => {
      try {
        unlisten = await listen<any>("reconnect:request", async (event) => {
          console.log('Reconnect requested from:', event.payload);

          // Dev-only debug & telemetry hook
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

    return () => {
      if (unlisten) unlisten();
    };
  }, [connecting, selectedPort, baudRate, timeoutMs, connectionRuntimePacketMode, defaultRuntimePacketMode]);
  
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
  
  // Listen for definition:loaded event to ensure INI is ready before table operations
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    
    (async () => {
      try {
        unlisten = await listen<{ signature: string; tables: number; curves: number; dialogs: number; constants: number }>(
          'definition:loaded',
          (event) => {
            console.log('[App] definition:loaded event:', event.payload);
            // Force status refresh to update has_definition
            checkStatus();
          }
        );
      } catch (e) {
        console.error("Failed to listen for definition:loaded events:", e);
      }
    })();
    
    return () => {
      if (unlisten) unlisten();
    };
  }, []);
  
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
        // Refresh the active curve to ensure it has fresh data
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
      return prev; // Return unchanged, update happens in promise
    });
  }, [activeTabId]); // Only depend on activeTabId to avoid infinite loops

  // Realtime streaming - updates go directly to Zustand store (no React state change cascade)
  //
  // Architecture: The Tauri event listener is registered ONCE at module level
  // (ensureRealtimeListener) and never unregistered. This eliminates all race
  // conditions that plagued the previous approach where listen()/unlisten() was
  // done inside useEffect — React 18 StrictMode's mount→cleanup→mount cycle
  // caused two concurrent stop_realtime_stream IPC calls that non-deterministically
  // killed the freshly started stream.
  //
  // The effect's only job is to start the backend stream (which always replaces any
  // existing task) and clear channels on cleanup.
  useEffect(() => {
    console.log(`[Stream effect] status.state=${status.state} has_definition=${status.has_definition}`);
    let pollIntervalHandle: NodeJS.Timeout | null = null;
    let heartbeatHandle: NodeJS.Timeout | null = null;
    let cancelled = false;

    if (status.state === "Connected" && status.has_definition) {
      console.log("[Stream effect] Condition met, starting IIFE");
      (async () => {
        // Ensure the global event listener is registered (no-op if already done)
        console.log("[Stream effect] Registering listener...");
        await ensureRealtimeListener();
        if (cancelled) { console.log("[Stream effect] cancelled after listener"); return; }

        // Start the backend stream. start_realtime_stream always aborts any
        // existing task before creating a new one, so concurrent calls from
        // StrictMode double-mount are harmless — the last one wins.
        try {
          console.log("[Stream effect] Calling start_realtime_stream...");
          await invoke("start_realtime_stream", { intervalMs: 50 });
          console.log("[Stream effect] Stream started successfully");
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
              } catch (error) {
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

        // Heartbeat: if no store update arrives for 2 seconds, restart the stream.
        // Add cooldown to prevent thundering herd (max one restart per 10 seconds).
        let lastRestartTime = 0;
        heartbeatHandle = setInterval(() => {
          if (cancelled) return;
          const lastUpdate = useRealtimeStore.getState().lastUpdateTime;
          const now = Date.now();
          if (lastUpdate > 0 && now - lastUpdate > 2000 && now - lastRestartTime > 10000) {
            console.warn("[Heartbeat] No realtime update for 2s, restarting stream (cooldown 10s)");
            lastRestartTime = now;
            invoke("start_realtime_stream", { intervalMs: 50 }).catch(() => {});
          }
        }, 2000);
      })();
    }

    return () => {
      console.log("[Stream effect] CLEANUP running");
      cancelled = true;
      // Do NOT unregister the event listener — it lives at module level.
      // Do NOT call stop_realtime_stream — it races with the next mount's start.
      if (pollIntervalHandle) clearInterval(pollIntervalHandle);
      if (heartbeatHandle) clearInterval(heartbeatHandle);
      useRealtimeStore.getState().clearChannels();
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
            setNewProjectDialogOpen(true);
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
      
      // Also fetch ECU type if connected
      if (s.state === "Connected") {
        try {
          const type = await invoke<string>("get_ecu_type");
          setEcuType(type);
        } catch (err) {
          console.warn("Failed to get ECU type:", err);
          setEcuType("Unknown");
        }
      } else {
        setEcuType("Unknown");
      }
    } catch (e) {
      console.error(e);
    }
  }

  async function fetchRealtimeData() {
    try {
      const data = await invoke<Record<string, number>>("get_realtime_data");
      useRealtimeStore.getState().updateChannels(data);
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
      
      // Also fetch searchable index for deep search
      try {
        const index = await invoke<Record<string, string[]>>("get_searchable_index");
        setSearchIndex(index);
      } catch (e) {
        console.error("Failed to fetch search index:", e);
      }
    } catch (e) {
      console.error(e);
    }
  }

  // Note: Curves are accessed via their parent dialogs in the menu tree, not via a catchall folder.
  // The get_curves backend command still exists for search functionality.

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
      // if (result.pages_synced > 0) {
      //   try {
      //     const differs = await invoke<boolean>("compare_project_and_ecu_tunes");
      //     if (differs) {
      //       setTuneComparisonOpen(true);
      //     }
      //   } catch (e) {
      //     console.error("Failed to compare tunes:", e);
      //     // Don't block on comparison failure
      //   }
      // }
      
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
      let runtimeMode = connectionRuntimePacketMode || defaultRuntimePacketMode;

      // If runtime mode is Auto, try to detect best mode from INI capabilities
      if (runtimeMode === 'Auto') {
        try {
          // Attempt to query backend capabilities directly. If a definition isn't loaded
          // the command will error and we'll fall back to a safe default.
          const caps = await invoke<{ supports_och: boolean }>('get_protocol_capabilities');
          runtimeMode = caps && caps.supports_och ? 'ForceOCH' : 'ForceBurst';
        } catch (e) {
          console.warn('Runtime mode auto-detect failed, defaulting to ForceBurst:', e);
          runtimeMode = 'ForceBurst';
        }
      }

      const result = await invoke<ConnectResult>("connect_to_ecu", { 
        portName: selectedPort, 
        baudRate, 
        timeoutMs, 
        runtimePacketMode: runtimeMode,
        connectionType,
        tcpHost,
        tcpPort
      });
      await checkStatus();
      
      // If there's a signature mismatch, behave based on severity
      if (result.mismatch_info) {
        const mi = result.mismatch_info;
        setSignatureMismatchInfo(mi);
        if (mi.match_type === 'mismatch') {
          // Block automatic sync for full mismatches and require explicit user decision
          console.log("Signature mismatch detected:", mi);
          setSignatureMismatchOpen(true);
          return;
        } else {
          // Partial match: advisory only — warn user but continue to sync
          showToast(
            `Connected: ECU signature partially matches the loaded INI (ECU: ${mi.ecu_signature}). Proceeding with sync.`,
            "warning"
          );
          // continue with sync
        }
      }
      
      // If connected and has definition (and no mismatch), sync ECU data
      const newStatus = await invoke<ConnectionStatus>("get_connection_status");
      if (newStatus.state === "Connected" && newStatus.has_definition) {
        await doSync();
        
        // Save the successful connection port to project config
        if (currentProject) {
          try {
            await invoke("update_project_connection", {
              port: selectedPort,
              baudRate: baudRate,
            });
            console.log("Saved connection settings to project");
          } catch (saveError) {
            console.error("Failed to save connection settings:", saveError);
            // Don't show error to user as connection was successful
          }
        }
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
      await invoke("stop_realtime_stream").catch(() => {});
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

  async function initializeDefaultTabs() {
    let caps = iniCapabilities;
    if (!caps) {
      try {
        caps = await invoke<IniCapabilities>('get_ini_capabilities');
        setIniCapabilities(caps);
      } catch (e) {
        console.warn('get_ini_capabilities failed during tab init:', e);
      }
    }

    const allowDashboard = caps?.has_frontpage || caps?.has_gauges;
    if (allowDashboard) {
      setTabs([{ id: "dashboard", title: "Dashboard", icon: "dashboard", closable: false }]);
      setTabContents({ dashboard: { type: "dashboard" } });
      setActiveTabId("dashboard");
    } else {
      setTabs([]);
      setTabContents({});
      setActiveTabId(null);
    }
  }

  // Project management functions
  async function createProject(name: string, iniId: string): Promise<boolean> {
    try {
      // Close previous project if one is open
      if (currentProject) {
        try {
          await invoke("close_project");
          setCurrentProject(null);
          setBackendMenus([]);
          setTabs([]);
          setTabContents({});
          setActiveTabId(null);
        } catch (closeErr) {
          console.warn("Failed to close previous project:", closeErr);
        }
      }

      const project = await invoke<CurrentProject>("create_project", { 
        name, 
        iniId,
        tunePath: null 
      });
      
      setCurrentProject(project);
      
      // Show loading spinner while we fetch menus and initialize
      showLoading("Loading project...");
      
      try {
        // Refresh menus for the new project
        const values = await fetchConstants();
        await fetchMenuTree(values);

        await initializeDefaultTabs();
        
        // Refresh projects list
        const projects = await invoke<ProjectInfo[]>("list_projects");
        setAvailableProjects(projects);
        
        showToast(`Project "${name}" created successfully`, "success");
      } catch (menuError) {
        console.error("Failed to load menus:", menuError);
        showToast("Project created but menu loading failed. Some features may be unavailable.", "warning");
      } finally {
        hideLoading();
      }
      return true;
    } catch (e) {
      const { message, details } = formatError(e);
      if (details) {
        showError("Failed to Create Project", message, details);
      } else {
        showToast("Failed to create project: " + message, "error");
      }
      return false;
    }
  }

  /** Import an existing tune file (.msq) into the currently open project */
  async function handleImportTuneIntoProject(tunePath: string) {
    if (!currentProject) return;
    try {
      showLoading("Loading tune file...");
      await invoke("load_tune", { path: tunePath });
      // Refresh constants so UI reflects the loaded tune
      await fetchConstants();
      showToast("Tune file loaded successfully", "success");
    } catch (e) {
      showToast("Failed to load tune: " + e, "error");
    } finally {
      hideLoading();
    }
  }

  async function openProject(path: string) {
    // Close any open dialogs
    setNewProjectDialogOpen(false);
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

        await initializeDefaultTabs();
      } catch (menuError) {
        console.error("Failed to load menus:", menuError);
        showToast("Project opened but menu loading failed. Some features may be unavailable.", "warning");
      }
      
      // Auto-connect if enabled and port is set
      if (project.connection.auto_connect && project.connection.port) {
        hideLoading(); // Hide the project loading first
        showToast("Auto-connecting to ECU...", "info");
        // Small delay to let the UI update
        setTimeout(async () => {
          try {
            await connect();
          } catch (e) {
            console.error("Auto-connect failed:", e);
            // Don't show error toast as connect() already does that
          }
        }, 500);
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

  async function handleDeleteProject(projectName: string) {
    try {
      await invoke("delete_project", { projectName });
      showToast(`Project "${projectName}" deleted`, "success");
      // Refresh project list
      const projects = await invoke<ProjectInfo[]>("list_projects");
      setAvailableProjects(projects);
    } catch (e) {
      showToast("Failed to delete project: " + e, "error");
    }
  }

  async function handleBaseMapApply(baseMap: BaseMapResult) {
    try {
      if (currentProject) {
        const result = await invoke<{ applied: string[]; errors: string[] }>("apply_base_map", {
          baseMap: baseMap,
        });
        if (result.applied.length > 0) {
          showToast(`Base map applied: ${result.applied.join(", ")}`, "success");
          // Refresh table data so UI shows new values
          await fetchConstants();
        } else {
          showToast("No matching tables found in the loaded INI — base map could not be applied", "warning");
        }
        if (result.errors.length > 0) {
          showToast(`Base map warnings: ${result.errors.join("; ")}`, "warning");
          console.warn("Base map apply errors:", result.errors);
        }
      } else {
        showToast("Please create a project first before generating a base map", "info");
      }
      setBaseMapDialogOpen(false);
    } catch (e) {
      showToast("Failed to apply base map: " + e, "error");
    }
  }

  // Open a table or dialog in a new tab
  const openTarget = useCallback(
    async (name: string, title?: string, highlightTerm?: string) => {
      await openTargetImpl(
        {
          tabs, tabContents, iniCapabilities,
          setTabs, setTabContents, setActiveTabId,
          setPortEditorAssignments, showToast,
        },
        name, title, highlightTerm,
      );
    },
    [tabs, tabContents, iniCapabilities, showToast]
  );

  // Handle standard built-in targets (std_*)
  const handleStdTarget = useCallback(
    (target: string, label: string) => {
      console.log("[handleStdTarget]", target, label);
      
      switch (target) {
        case "std_realtime":
          if (!iniCapabilities?.has_frontpage && !iniCapabilities?.has_gauges) {
            showToast("Realtime dashboard is not available for this ECU definition.", "warning");
            return;
          }
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
        case "std_port_edit":
          if (!iniCapabilities?.has_port_editors) {
            showToast("Port Editor is not available for this ECU definition.", "warning");
            return;
          }
          // Open port editor with matching name from INI [PortEditor] section
          openTarget(target, label);
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
    [iniCapabilities, openTarget, showToast]
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

  // Call a stub backend command - shows "coming soon" toast on expected error
  // Tab handlers
  const handleTabSelect = useCallback((tabId: string) => {
    setActiveTabId(tabId);
  }, []);

  const handleTabClose = useCallback(
    (tabId: string) => {
      // Don't close tabs marked as non-closable (e.g. Dashboard)
      const tab = tabs.find((t) => t.id === tabId);
      if (tab && tab.closable === false) return;

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
  const menuItems: TunerMenuItem[] = useMemo(() => buildMenuItems({
    t, currentProject, status, ecuType, iniCapabilities, backendMenus, theme,
    sidebarVisible, tabs, openTarget, handleStdTarget, openHelpTopic, showToast,
    closeProject, handleCreateRestorePoint,
    setNewProjectDialogOpen, setImportProjectOpen, setSaveDialogOpen, setLoadDialogOpen,
    setBurnDialogOpen, setRestorePointsOpen, setTuneHistoryOpen, setSettingsDialogOpen,
    setMathChannelsDialogOpen, setBaseMapDialogOpen, setTableComparisonOpen,
    setTuneFileDiffOpen, setDynoOverlayOpen, setPluginPanelOpen, setConnectionDialogOpen,
    setUserManualOpen, setUserManualSection, setAboutDialogOpen, setSidebarVisible,
    setTheme, setTabs, setTabContents, setActiveTabId,
  }), [backendMenus, theme, sidebarVisible, status.state, ecuType, iniCapabilities, openTarget, handleStdTarget, openHelpTopic, currentProject, showToast, t, tabs]);

  // Toolbar items
  const toolbarItems: ToolbarItem[] = useMemo(() => buildToolbarItems({
    status, iniCapabilities, isLogging, connectionRuntimePacketMode, defaultRuntimePacketMode,
    setLoadDialogOpen, setSaveDialogOpen, setBurnDialogOpen, setConnectionDialogOpen,
    setSettingsDialogOpen, setActiveTabId, setIsLogging,
  }), [status, isLogging, iniCapabilities, connectionRuntimePacketMode, defaultRuntimePacketMode]);

  const sidebarItems: SidebarNode[] = useMemo(() => {
    // Build menu-based sidebar items from INI menus
    // Curves are accessed via their parent dialogs (e.g., Fuel > Injection configuration)
    const menuItems: SidebarNode[] = (backendMenus ?? []).map((menu) => ({
      id: menu.name,
      label: menu.title.replace(/^&/, ""),
      type: "folder" as const,
      children: buildSidebarItems(menu.items, menu.name),
    }));
    
    return menuItems;
  }, [backendMenus]);

  const handleSidebarItemSelect = useCallback(
    (item: SidebarNode & { itemType?: string }, highlightTerm?: string) => {
      console.log('[App] handleSidebarItemSelect called', { id: item.id, label: item.label, type: item.type, itemType: (item as any).itemType, highlightTerm });
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
        // Table or Dialog - pass highlightTerm for search result highlighting
        console.log('[App] Calling openTarget for Table/Dialog');
        openTarget(item.id, item.label, highlightTerm);
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
      // Realtime channel indicators are now rendered by StatusBar internally
      // (each RealtimeChannelCell subscribes to a single channel)
      // Only static items like sync warnings are added here
    }

    items.push({
      id: "logging",
      content: <LoggingIndicator isLogging={isLogging} duration={logDuration} />,
      align: "right",
    });

    return items;
  }, [status.state, statusBarChannels, isLogging, logDuration, syncStatus]);


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
        searchIndex={searchIndex}
        statusItems={statusItems}
        connected={status.state === "Connected"}
        ecuName={(status.state === "Connected" ? status.signature : (status.ini_name ? status.ini_name : undefined)) as string | undefined}
        projectName={currentProject?.name}
        unitsSystem={unitsSystem}
        realtimeChannels={statusBarChannels}
        channelInfoMap={channelInfoMap}
      >
        <ErrorBoundary>
          <TabContentRouter
            currentProject={currentProject}
            availableProjects={availableProjects}
            status={status}
            ecuType={ecuType}
            activeTabId={activeTabId}
            tabs={tabs}
            tabContents={tabContents}
            setTabContents={setTabContents}
            openProject={openProject}
            setNewProjectDialogOpen={setNewProjectDialogOpen}
            setConnectionDialogOpen={setConnectionDialogOpen}
            setImportProjectOpen={setImportProjectOpen}
            handleDeleteProject={handleDeleteProject}
            setBurnDialogOpen={setBurnDialogOpen}
            handleTabClose={handleTabClose}
            openTarget={openTarget}
            fetchConstants={fetchConstants}
            fetchMenuTree={fetchMenuTree}
            setConstantValues={setConstantValues}
            constantValues={constantValues}
            portEditorAssignments={portEditorAssignments}
            setPortEditorAssignments={setPortEditorAssignments}
            showToast={showToast}
          />
        </ErrorBoundary>
      </TunerLayout>

      <DialogOverlays
        status={status}
        currentProject={currentProject}
        theme={theme}
        setTheme={setTheme}
        showToast={showToast}
        saveDialogOpen={saveDialogOpen}
        setSaveDialogOpen={setSaveDialogOpen}
        autoBurnOnClose={autoBurnOnClose}
        loadDialogOpen={loadDialogOpen}
        setLoadDialogOpen={setLoadDialogOpen}
        burnDialogOpen={burnDialogOpen}
        setBurnDialogOpen={setBurnDialogOpen}
        newTuneDialogOpen={newTuneDialogOpen}
        setNewTuneDialogOpen={setNewTuneDialogOpen}
        settingsDialogOpen={settingsDialogOpen}
        setSettingsDialogOpen={setSettingsDialogOpen}
        setUnitsSystem={setUnitsSystem}
        setAutoBurnOnClose={setAutoBurnOnClose}
        setStatus={setStatus}
        setStatusBarChannels={setStatusBarChannels}
        setDefaultRuntimePacketMode={setDefaultRuntimePacketMode}
        mathChannelsDialogOpen={mathChannelsDialogOpen}
        setMathChannelsDialogOpen={setMathChannelsDialogOpen}
        aboutDialogOpen={aboutDialogOpen}
        setAboutDialogOpen={setAboutDialogOpen}
        connectionDialogOpen={connectionDialogOpen}
        setConnectionDialogOpen={setConnectionDialogOpen}
        ports={ports}
        selectedPort={selectedPort}
        baudRate={baudRate}
        timeoutMs={timeoutMs}
        connectionType={connectionType}
        setConnectionType={setConnectionType}
        tcpHost={tcpHost}
        setTcpHost={setTcpHost}
        tcpPort={tcpPort}
        setTcpPort={setTcpPort}
        setSelectedPort={setSelectedPort}
        handleBaudChange={handleBaudChange}
        handleTimeoutChange={handleTimeoutChange}
        connect={connect}
        disconnect={disconnect}
        refreshPorts={refreshPorts}
        connecting={connecting}
        syncing={syncing}
        syncProgress={syncProgress}
        iniDefaults={iniDefaults}
        applyIniDefaults={applyIniDefaults}
        connectionRuntimePacketMode={connectionRuntimePacketMode}
        setConnectionRuntimePacketMode={setConnectionRuntimePacketMode}
        newProjectDialogOpen={newProjectDialogOpen}
        setNewProjectDialogOpen={setNewProjectDialogOpen}
        repositoryInis={repositoryInis}
        setRepositoryInis={setRepositoryInis}
        createProject={createProject}
        handleImportTuneIntoProject={handleImportTuneIntoProject}
        baseMapDialogOpen={baseMapDialogOpen}
        setBaseMapDialogOpen={setBaseMapDialogOpen}
        handleBaseMapApply={handleBaseMapApply}
        tuneComparisonOpen={tuneComparisonOpen}
        setTuneComparisonOpen={setTuneComparisonOpen}
        checkStatus={checkStatus}
        tableComparisonOpen={tableComparisonOpen}
        setTableComparisonOpen={setTableComparisonOpen}
        tuneFileDiffOpen={tuneFileDiffOpen}
        setTuneFileDiffOpen={setTuneFileDiffOpen}
        dynoOverlayOpen={dynoOverlayOpen}
        setDynoOverlayOpen={setDynoOverlayOpen}
        performanceDialogOpen={performanceDialogOpen}
        setPerformanceDialogOpen={setPerformanceDialogOpen}
        signatureMismatchOpen={signatureMismatchOpen}
        signatureMismatchInfo={signatureMismatchInfo}
        setSignatureMismatchOpen={setSignatureMismatchOpen}
        setSignatureMismatchInfo={setSignatureMismatchInfo}
        fetchConstants={fetchConstants}
        fetchMenuTree={fetchMenuTree}
        doSync={doSync}
        helpTopic={helpTopic}
        setHelpTopic={setHelpTopic}
        userManualOpen={userManualOpen}
        setUserManualOpen={setUserManualOpen}
        userManualSection={userManualSection}
        setUserManualSection={setUserManualSection}
        tuneMismatchOpen={tuneMismatchOpen}
        tuneMismatchInfo={tuneMismatchInfo}
        setTuneMismatchOpen={setTuneMismatchOpen}
        setTuneMismatchInfo={setTuneMismatchInfo}
        errorDialogOpen={errorDialogOpen}
        errorInfo={errorInfo}
        hideError={hideError}
        restorePointsOpen={restorePointsOpen}
        setRestorePointsOpen={setRestorePointsOpen}
        tuneHistoryOpen={tuneHistoryOpen}
        setTuneHistoryOpen={setTuneHistoryOpen}
        importProjectOpen={importProjectOpen}
        setImportProjectOpen={setImportProjectOpen}
        setAvailableProjects={setAvailableProjects}
        setCurrentProject={setCurrentProject}
        setTabs={setTabs}
        setTabContents={setTabContents}
        setActiveTabId={setActiveTabId}
        migrationReportOpen={migrationReportOpen}
        setMigrationReportOpen={setMigrationReportOpen}
        onboardingOpen={onboardingOpen}
        setOnboardingOpen={setOnboardingOpen}
        pluginPanelOpen={pluginPanelOpen}
        setPluginPanelOpen={setPluginPanelOpen}
      />
    </>
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
