import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import {
    Activity,
    Search,
    ChevronRight,
    Settings,
    Database,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import TabbedDashboard from "./components/dashboards/TabbedDashboard";
import TableEditor2D from "./components/tables/TableEditor2D";
import TableEditor3D from "./components/tables/TableEditor3D";
import DialogRenderer, { DialogDefinition } from "./components/dialogs/DialogRenderer";
import { ConnectionStatus } from "./components/layout/Header";
import Sidebar, { Menu, MenuItem } from "./components/layout/Sidebar";
import Overlays, { OverlayState } from "./components/layout/Overlays";
import MenuBar from "./components/layout/MenuBar";
import StatusBar from "./components/layout/StatusBar";
import "./App.css";
import "./components/layout/MenuBar.css";
import "./components/layout/StatusBar.css";
import "./components/realtime/AutoTuneLive.css";
import "./components/dashboards/TabbedDashboard.css";
import "./components/tables/TableComponents.css";
import "./components/tables/TableEditor2D.css";
import "./components/tables/TableEditor3D.css";

interface TableInfo {
  name: string;
  title: string;
}

interface TableData {
  name: string;
  title: string;
  x_axis_name?: string;
  y_axis_name?: string;
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
}

function App() {
  console.log("App component rendering...");
  const [ports, setPorts] = useState<string[]>([]);
  const [availableInis, setAvailableInis] = useState<string[]>([]);
  const [status, setStatus] = useState<ConnectionStatus>({ state: "Disconnected", signature: null, has_definition: false });
  const [selectedPort, setSelectedPort] = useState("");
  const [baudRate, setBaudRate] = useState(115200);
  const [activeTab, setActiveTab] = useState("dashboard"); // dashboard, tuning, project, settings
  const [realtimeData, setRealtimeData] = useState<Record<string, number>>({});
  const [tables, setTables] = useState<TableInfo[]>([]);
  const [activeTable, setActiveTable] = useState<TableData | null>(null);
  const [activeDialog, setActiveDialog] = useState<DialogDefinition | null>(null);
  const [tableSearch, setTableSearch] = useState("");
  const [standaloneTable, setStandaloneTable] = useState<string | null>(null);
  const [menuTree, setMenuTree] = useState<Menu[]>([]);
  const [expandedMenus, setExpandedMenus] = useState<Record<string, boolean>>({});
  const [menuSearch, setMenuSearch] = useState("");
  const [iniError, setIniError] = useState<string | null>(null);
  const [isLoadingInis, setIsLoadingInis] = useState(false);
  const [isTauri, setIsTauri] = useState(true);
  const [constantValues, setConstantValues] = useState<Record<string, number>>({});

  const [overlayState, setOverlayState] = useState<OverlayState>({
    showVeAnalyze: false,
    showPerformanceDialog: false,
    showActionsPanel: false,
    showSaveDialog: false,
    showLoadDialog: false,
    showBurnDialog: false,
    showNewProjectDialog: false,
    showBrowseProjectsDialog: false,
    showRebinDialog: false,
    showCellEditDialog: false,
    cellEditValue: 0,
  });
  const [is3DView, setIs3DView] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [hotkeyHints, setHotkeyHints] = useState<Record<string, string>>({});

  const closeOverlay = (key: keyof OverlayState) => {
    setOverlayState(prev => ({ ...prev, [key]: false }));
  };

  const openOverlay = (key: keyof OverlayState) => {
    setOverlayState(prev => ({ ...prev, [key]: true }));
  };

  const evalContext = useMemo(() => ({ ...constantValues, ...realtimeData }), [constantValues, realtimeData]);

  const filteredMenuTree = useMemo(() => {
    if (!menuSearch) return menuTree;
    const search = menuSearch.toLowerCase();
    return menuTree.map((menu: Menu) => {
      // Find items that match
      const matchingItems = menu.items.filter((item: MenuItem) => {
        if (item.type === 'Separator') return false;
        const label = (item.label || "").toLowerCase();
        const target = (item.target || "").toLowerCase();
        return label.includes(search) || target.includes(search);
      });

      if (menu.title.toLowerCase().includes(search) || matchingItems.length > 0) {
        return { ...menu, items: matchingItems.length > 0 ? matchingItems : menu.items };
      }
      return null;
    }).filter(Boolean) as Menu[];
  }, [menuTree, menuSearch]);

  useEffect(() => {
    // Check if we are running inside Tauri
    const inTauri = !!(window as any).__TAURI_INTERNALS__;
    setIsTauri(inTauri);
    if (!inTauri) {
      setIniError("Running in browser mode. Backend features (INI loading, ECU connection) are disabled. Use `npm run tauri dev` to run as a desktop app.");
    }
  }, []);
  
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const tableId = params.get("table");
    if (tableId) setStandaloneTable(tableId);
  }, []);
  
  // Hotkey handling
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Prevent hotkey conflicts in input fields
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      
      switch (e.key) {
        case "F11":
          e.preventDefault();
          setIsFullscreen(prev => !prev);
          break;
        case "Escape":
          if (activeTable || activeDialog) {
            setActiveTable(null);
            setActiveDialog(null);
          } else if (isFullscreen) {
            setIsFullscreen(false);
          }
          break;
        case "F":
          if (e.ctrlKey) {
            e.preventDefault();
            setIsFullscreen(prev => !prev);
          }
          break;
        case "s":
          if (e.ctrlKey) {
            e.preventDefault();
            openOverlay('showSaveDialog');
          } else if (activeTable && !e.altKey) {
            e.preventDefault();
            setHotkeyHints({ ...hotkeyHints, hint: "Smooth table..." });
          }
          break;
        case "/":
          if (activeTable && !e.ctrlKey) {
            e.preventDefault();
            setHotkeyHints({ ...hotkeyHints, hint: "Interpolate..." });
          }
          break;
        case "=":
          if (activeTable && !e.ctrlKey) {
            e.preventDefault();
            setHotkeyHints({ ...hotkeyHints, hint: "Set Equal..." });
          }
          break;
        case "*":
          if (activeTable && !e.ctrlKey) {
            e.preventDefault();
            setHotkeyHints({ ...hotkeyHints, hint: "Scale..." });
          }
          break;
        case "3":
          if (activeTable && !e.ctrlKey) {
            e.preventDefault();
            setIs3DView(!is3DView);
          }
          break;
        case "r":
          if (!e.ctrlKey) {
            e.preventDefault();
            openOverlay('showRebinDialog');
          }
          break;
          case "e":
            if (!e.ctrlKey) {
              e.preventDefault();
              openOverlay('showCellEditDialog');
            }
            break;
        case "l":
          if (e.ctrlKey) {
            e.preventDefault();
            openOverlay('showLoadDialog');
          }
          break;
        case "b":
          if (e.ctrlKey) {
            e.preventDefault();
            openOverlay('showBurnDialog');
          }
          break;
        default:
          break;
      }
    };
    
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeTable, activeDialog, isFullscreen]);

  useEffect(() => {
    if (standaloneTable) openMenuTarget(standaloneTable);
  }, [standaloneTable]);

  useEffect(() => {
    // Only run once isTauri is determined
    if (isTauri) {
      refreshData();
      const statusInterval = setInterval(checkStatus, 1000);
      return () => clearInterval(statusInterval);
    }
  }, [isTauri]);

  useEffect(() => {
    let unlistenUpdate: UnlistenFn | null = null;
    let unlistenErr: UnlistenFn | null = null;

    if (status.state === "Connected" && status.has_definition) {
      (async () => {
        try {
          await invoke("start_realtime_stream", { intervalMs: 100 });
        } catch (e) {
          console.warn("Failed to start realtime stream, falling back to polling:", e);
          // Fallback: polling
          const dataInterval = setInterval(fetchRealtimeData, 100) as unknown as number;
          // Clean up polling on stop
          unlistenUpdate = () => clearInterval(dataInterval);
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
      try { invoke("stop_realtime_stream"); } catch (e) { /* ignore */ }
    };
  }, [status.state, status.has_definition]);

  useEffect(() => {
    if (status.has_definition) {
      fetchConstants().then(values => {
        fetchMenuTree(values);
        fetchTables();
      });
    }
  }, [status.has_definition]);

  async function refreshData() {
    if (!isTauri) return;

    // Parallelize to avoid one failure blocking everything
    Promise.all([
      invoke<string[]>("get_serial_ports").then(p => {
        setPorts(p);
        if (p.length > 0 && !selectedPort) setSelectedPort(p[0]);
      }).catch(e => console.error("Serial ports check failed:", e)),

      (async () => {
        setIsLoadingInis(true);
        setIniError(null);
        try {
          const inis = await invoke<string[]>("get_available_inis");
          setAvailableInis(inis);

          // Auto-load last INI if none currently loaded
          if (!status.has_definition) {
            const lastPath = await invoke<string | null>("auto_load_last_ini");
            if (lastPath) {
              console.log("Auto-loading:", lastPath);
              await invoke("load_ini", { path: lastPath });
              await checkStatus();
            }
          }
        } catch (e) {
          setIniError(String(e));
          console.error("INI scanning failed:", e);
        } finally {
          setIsLoadingInis(false);
        }
      })()
    ]);
  }

  async function checkStatus() {
    try {
      const s = await invoke<ConnectionStatus>("get_connection_status");
      setStatus(s);
    } catch (e) { console.error(e); }
  }

  async function fetchRealtimeData() {
    try {
      setRealtimeData(await invoke<Record<string, number>>("get_realtime_data"));
    } catch (e) { console.error(e); }
  }

  async function fetchTables() {
    try {
      setTables(await invoke<TableInfo[]>("get_tables"));
    } catch (e) { console.error(e); }
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
      const tree = await invoke<Menu[]>("get_menu_tree", { filterContext: context || constantValues });
      setMenuTree(tree);
    } catch (e) { console.error(e); }
  }

  async function openMenuTarget(name: string) {
    // Try Table first, then Dialog
    try {
      const data = await invoke<TableData>("get_table_data", { tableName: name });
      setActiveTable(data);
      setActiveDialog(null);
      return;
    } catch (e) { }

    try {
      const def = await invoke<DialogDefinition>("get_dialog_definition", { name });
      setActiveDialog(def);
      setActiveTable(null);
    } catch (e) {
      console.error("Failed to open target:", name, e);
    }
  }


  async function loadIni(path: string) {
    try {
      await invoke("load_ini", { path });
      await checkStatus();
      if (activeTab === "project") setActiveTab("dashboard");
    } catch (e) { alert("Failed to load INI: " + e); }
  }

  async function browseIni() {
    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'INI Definition',
          extensions: ['ini']
        }]
      });
      if (selected && typeof selected === 'string') {
        await loadIni(selected);
      }
    } catch (e) {
      console.error(e);
      alert("Failed to open file dialog: " + e);
    }
  }

  async function connect() {
    try {
      await invoke("connect_to_ecu", { portName: selectedPort, baudRate });
      checkStatus();
    } catch (e) { alert(e); }
  }

  async function disconnect() {
    try {
      await invoke("disconnect_ecu");
      checkStatus();
      setActiveTable(null);
    } catch (e) { console.error(e); }
  }

  if (standaloneTable && activeTable) {
    if (is3DView) {
      return (
        <TableEditor3D
          title={activeTable.title}
          x_bins={activeTable.x_bins}
          y_bins={activeTable.y_bins}
          z_values={activeTable.z_values}
          onBack={() => { setActiveTable(null); setIs3DView(false); }}
        />
      );
    }
    return (
      <TableEditor2D
        title={activeTable.title}
        table_name={activeTable.name}
        x_axis_name={activeTable.x_axis_name || "RPM"}
        y_axis_name={activeTable.y_axis_name || "MAP"}
        x_bins={activeTable.x_bins}
        y_bins={activeTable.y_bins}
        z_values={activeTable.z_values}
        onBack={() => setActiveTable(null)}
        realtimeData={realtimeData}
      />
    );
  }

  const renderContent = () => {
    if (activeTable) {
      if (is3DView) {
        return (
          <TableEditor3D
            title={activeTable.title}
            x_bins={activeTable.x_bins}
            y_bins={activeTable.y_bins}
            z_values={activeTable.z_values}
            onBack={() => { setActiveTable(null); setIs3DView(false); }}
          />
        );
      }
      return (
        <TableEditor2D
          title={activeTable.title}
          table_name={activeTable.name}
          x_axis_name={activeTable.x_axis_name || "RPM"}
          y_axis_name={activeTable.y_axis_name || "MAP"}
          x_bins={activeTable.x_bins}
          y_bins={activeTable.y_bins}
          z_values={activeTable.z_values}
          onBack={() => setActiveTable(null)}
          realtimeData={realtimeData}
        />
      );
    }

    if (activeDialog) {
      return (
        <DialogRenderer
          definition={activeDialog}
          onBack={() => setActiveDialog(null)}
          openTable={openMenuTarget}
          context={evalContext}
          onUpdate={() => {
            fetchConstants().then(values => fetchMenuTree(values));
          }}
        />
      );
    }

    switch (activeTab) {
      case "dashboard":
        return <TabbedDashboard realtimeData={realtimeData} />;
      case "tuning":
        return (
          <div className="view-transition">
            <h1 className="content-title">Tuning Tables</h1>
            <div className="search-box">
              <Search className="search-icon" size={20} />
              <input
                placeholder="Search tables (VE, Ignition, etc)..."
                value={tableSearch}
                onChange={e => setTableSearch(e.target.value)}
              />
            </div>
            <div className="table-browser-grid">
              {tables.filter(t => t.title.toLowerCase().includes(tableSearch.toLowerCase())).map(t => (
                <div key={t.name} className="modern-table-card" onClick={() => openMenuTarget(t.name)}>
                  <div className="table-info">
                    <h4>{t.title}</h4>
                    <span>{t.name}</span>
                  </div>
                  <ChevronRight size={20} color="var(--text-muted)" />
                </div>
              ))}
              {tables.length === 0 && (
                <div className="glass-card" style={{ gridColumn: '1 / -1', textAlign: 'center' }}>
                  <p style={{ color: 'var(--text-muted)' }}>No tables found. Load an INI in Project settings.</p>
                </div>
              )}
            </div>
          </div>
        );
      case "project":
        return (
          <div className="view-transition">
            <h1 className="content-title">Project Management</h1>
            <div className="glass-card">
              <div className="card-header">
                <h3 className="card-title"><Activity size={20} /> ECU Connection</h3>
              </div>
              <div style={{ display: 'flex', gap: '1rem', alignItems: 'center' }}>
                <select value={selectedPort} onChange={e => setSelectedPort(e.target.value)} style={{ flex: 1 }}>
                  {ports.map(p => <option key={p} value={p}>{p}</option>)}
                </select>
                <select value={baudRate} onChange={e => setBaudRate(Number(e.target.value))}>
                  {[115200, 57600, 38400, 9600].map(b => <option key={b} value={b}>{b}</option>)}
                </select>
                {status.state === "Connected" ? (
                  <button className="primary-btn" style={{ background: 'var(--error)' }} onClick={disconnect}>Disconnect</button>
                ) : (
                  <button className="primary-btn" onClick={connect} disabled={!selectedPort}>Connect</button>
                )}
              </div>
            </div>

            <div className="glass-card">
              <div className="card-header">
                <h3 className="card-title"><Database size={20} /> INI Definitions</h3>
                <button className="secondary-btn" onClick={browseIni} style={{ padding: '0.4rem 0.8rem', fontSize: '0.8rem' }}>
                  Browse & Import...
                </button>
              </div>
              <div className="table-browser-grid">
                {isLoadingInis && <p style={{ color: 'var(--text-muted)' }}>Scanning definitions directory...</p>}

                {iniError && (
                  <div className="glass-card" style={{ gridColumn: '1 / -1', border: '1px solid var(--error)', background: 'hsla(0, 70%, 50%, 0.1)' }}>
                    <p style={{ color: 'var(--error)' }}>Error scanning INIs: {iniError}</p>
                    <button className="primary-btn" style={{ marginTop: '1rem' }} onClick={refreshData}>Retry</button>
                  </div>
                )}

                {availableInis.length === 0 && !isLoadingInis && !iniError && (
                  <p style={{ color: 'var(--text-muted)' }}>No INI files found in definitions directory.</p>
                )}

                {availableInis.map(ini => (
                  <div key={ini} className="modern-table-card" onClick={() => loadIni(ini)}>
                    <div className="table-info">
                      <h4>{ini}</h4>
                      <span style={{ color: status.has_definition && (status.signature && ini.includes(status.signature)) ? 'var(--success)' : 'var(--text-muted)' }}>
                        {status.has_definition && (status.signature && ini.includes(status.signature)) ? 'ACTIVE' : 'AVAILABLE'}
                      </span>
                    </div>
                    <ChevronRight size={20} color="var(--text-muted)" />
                  </div>
                ))}
              </div>
            </div>
          </div>
        );
      case "settings":
        return (
          <div className="view-transition">
            <h1 className="content-title">Settings</h1>
            <div className="glass-card">
              <h3 className="card-title"><Settings size={20} /> Preferences</h3>
              <p style={{ color: 'var(--text-muted)', marginTop: '1rem' }}>Interface and communication settings will appear here.</p>
            </div>
          </div>
        );
      default:
        return null;
    }
  };

  const handleTabChange = (tab: string) => {
    setActiveTab(tab);
    setActiveTable(null);
  };

  // Get current page name for status bar
  const getCurrentPageName = (): string => {
    if (activeTable) return activeTable.title || activeTable.name || '';
    if (activeDialog) return activeDialog.title || activeDialog.name || '';
    return activeTab;
  };

  return (
    <div className="app-shell">
      <MenuBar
        menuTree={menuTree}
        onMenuSelect={openMenuTarget}
        onSave={() => openOverlay('showSaveDialog')}
        onLoad={() => openOverlay('showLoadDialog')}
        onBurn={() => openOverlay('showBurnDialog')}
        onNewProject={() => openOverlay('showNewProjectDialog')}
        onBrowseProjects={() => openOverlay('showBrowseProjectsDialog')}
        onSettings={() => setActiveTab('settings')}
        onAutoTune={() => openOverlay('showVeAnalyze')}
        onPerformance={() => openOverlay('showPerformanceDialog')}
        onActions={() => openOverlay('showActionsPanel')}
      />

      <Sidebar
        activeTab={activeTab}
        menuTree={menuTree}
        filteredMenuTree={filteredMenuTree}
        expandedMenus={expandedMenus}
        menuSearch={menuSearch}
        onTabChange={handleTabChange}
        onMenuSearch={setMenuSearch}
        onMenuExpand={(name) => setExpandedMenus(prev => ({ ...prev, [name]: !prev[name] }))}
        onMenuTargetOpen={openMenuTarget}
        onAutoTune={() => openOverlay('showVeAnalyze')}
        onPerformance={() => openOverlay('showPerformanceDialog')}
        onActions={() => openOverlay('showActionsPanel')}
      />

      <main className="content-area">
        {renderContent()}
      </main>

      <StatusBar
        ecuStatus={status.state.toLowerCase() as 'connected' | 'connecting' | 'error' | 'disconnected'}
        ecuSignature={status.signature || undefined}
        currentPage={getCurrentPageName()}
        realtimeData={realtimeData}
      />

      <Overlays
        state={overlayState}
        onClose={closeOverlay}
        onCellEditValueChange={(value) => setOverlayState(prev => ({ ...prev, cellEditValue: value }))}
      />
    </div>
  );
}

export default App;
