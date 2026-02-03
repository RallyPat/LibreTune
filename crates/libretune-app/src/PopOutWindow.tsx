/**
 * PopOutWindow - Standalone window for popped-out tabs
 * 
 * Renders a single tab's content in its own window with:
 * - Dock-back button to return tab to main window
 * - Realtime data streaming for live updates
 * - Bidirectional sync for table/curve edits
 */

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, emit, UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ThemeProvider } from './themes';
import { TableEditor, TableData as TunerTableData, AutoTune, DataLogView } from './components/tuner-ui';
import CurveEditor, { CurveData, SimpleGaugeInfo } from './components/curves/CurveEditor';
import TsDashboard from './components/dashboards/TsDashboard';
import DialogRenderer, { DialogDefinition as RendererDialogDef } from './components/dialogs/DialogRenderer';
import { useRealtimeStore } from './stores/realtimeStore';
import { ArrowLeftToLine, X } from 'lucide-react';
import './styles';
import './PopOutWindow.css';

// Backend response types for invoke calls
interface BackendTableData {
  name: string;
  x_bins: number[];
  y_bins: number[];
  z_values: number[][];
  x_axis_name?: string;
  y_axis_name?: string;
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

interface PopOutData {
  tabId: string;
  type: 'dashboard' | 'table' | 'curve' | 'dialog' | 'settings' | 'autotune' | 'datalog';
  title: string;
  data?: TunerTableData | CurveData | RendererDialogDef | string;
  gauge?: SimpleGaugeInfo | null;
}

export default function PopOutWindow() {
  const [popOutData, setPopOutData] = useState<PopOutData | null>(null);
  const [constantValues, setConstantValues] = useState<Record<string, number>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Debug: log on mount
  useEffect(() => {
    console.log('[PopOutWindow] Component mounted');
    console.log('[PopOutWindow] hash:', window.location.hash);
    console.log('[PopOutWindow] href:', window.location.href);
  }, []);

  // Parse URL params and load data
  useEffect(() => {
    console.log('[PopOutWindow] Parsing URL params...');
    const hash = window.location.hash;
    const params = new URLSearchParams(hash.replace('#/popout?', ''));
    const tabId = params.get('tabId');
    const type = params.get('type') as PopOutData['type'];
    const title = params.get('title') || tabId || 'Pop-out';

    console.log('[PopOutWindow] Parsed:', { tabId, type, title });

    if (!tabId || !type) {
      console.error('[PopOutWindow] Invalid params - tabId:', tabId, 'type:', type);
      setError(`Invalid pop-out parameters: tabId=${tabId}, type=${type}, hash=${hash}`);
      setLoading(false);
      return;
    }

    // Load data from localStorage
    const storageKey = `popout-${tabId}`;
    const storedData = localStorage.getItem(storageKey);
    
    console.log('[PopOutWindow] Storage key:', storageKey);
    console.log('[PopOutWindow] Stored data exists:', !!storedData);
    
    if (storedData) {
      try {
        const parsed = JSON.parse(storedData);
        console.log('[PopOutWindow] Parsed data:', parsed);
        console.log('[PopOutWindow] Data field:', parsed.data);
        setPopOutData({
          tabId,
          type,
          title,
          data: parsed.data,
        });
        // Clean up localStorage
        localStorage.removeItem(storageKey);
      } catch (e) {
        console.error('Failed to parse pop-out data:', e);
        setError('Failed to load tab data');
        setLoading(false);
        return;
      }
    } else {
      console.log('[PopOutWindow] No stored data, will need to fetch for type:', type);
      // No stored data - set up with just the ID for types that fetch their own data
      setPopOutData({ tabId, type, title });
    }

    // Set window title
    getCurrentWindow().setTitle(title).catch(console.error);

    setLoading(false);
  }, []);

  // Listen for realtime updates and update Zustand store
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    (async () => {
      try {
        unlisten = await listen<Record<string, number>>('realtime:update', (event) => {
          useRealtimeStore.getState().updateChannels(event.payload);
        });
      } catch (e) {
        console.error('Failed to listen for realtime updates:', e);
      }
    })();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Fetch dialog definition if type is dialog and data is missing
  useEffect(() => {
    if (!popOutData || popOutData.type !== 'dialog' || popOutData.data) return;

    console.log('[PopOutWindow] Fetching dialog definition for:', popOutData.tabId);

    (async () => {
      try {
        const definition = await invoke<RendererDialogDef>('get_dialog_definition', { 
          name: popOutData.tabId 
        });
        console.log('[PopOutWindow] Fetched dialog definition:', definition);
        setPopOutData(prev => prev ? { ...prev, data: definition } : null);
      } catch (e) {
        console.error('[PopOutWindow] Failed to fetch dialog definition:', e);
        setError(`Failed to load dialog: ${e}`);
      }
    })();
  }, [popOutData?.tabId, popOutData?.type, popOutData?.data]);

  // Fetch table/curve data if type is table or curve and data is missing
  useEffect(() => {
    if (!popOutData || (popOutData.type !== 'table' && popOutData.type !== 'curve') || popOutData.data) return;

    console.log('[PopOutWindow] Fetching table/curve data for:', popOutData.tabId);

    (async () => {
      try {
        if (popOutData.type === 'table') {
          const data = await invoke<BackendTableData>('get_table_data', { tableName: popOutData.tabId });
          const tableData: TunerTableData = {
            name: data.name,
            xAxis: data.x_bins,
            yAxis: data.y_bins,
            zValues: data.z_values,
            xLabel: data.x_axis_name || 'X',
            yLabel: data.y_axis_name || 'Y',
            xOutputChannel: data.x_output_channel ?? undefined,
            yOutputChannel: data.y_output_channel ?? undefined,
          };
          console.log('[PopOutWindow] Fetched table data:', tableData);
          setPopOutData(prev => prev ? { ...prev, data: tableData } : null);
        } else {
          const data = await invoke<BackendCurveData>('get_curve_data', { curveName: popOutData.tabId });
          const curveData: CurveData = {
            name: data.name,
            title: data.title,
            x_bins: data.x_bins,
            y_bins: data.y_bins,
            x_label: data.x_label,
            y_label: data.y_label,
            x_axis: data.x_axis ?? null,
            y_axis: data.y_axis ?? null,
            x_output_channel: data.x_output_channel ?? null,
            gauge: data.gauge ?? null,
          };

          let gaugeInfo: SimpleGaugeInfo | null = null;
          if (data.gauge) {
            try {
              gaugeInfo = await invoke<SimpleGaugeInfo>('get_gauge_config', { gaugeName: data.gauge });
            } catch (gaugeErr) {
              console.warn(`[PopOutWindow] Failed to load gauge '${data.gauge}':`, gaugeErr);
            }
          }

          console.log('[PopOutWindow] Fetched curve data:', curveData);
          setPopOutData(prev => prev ? { ...prev, data: curveData, gauge: gaugeInfo } : null);
        }
      } catch (e) {
        console.error('[PopOutWindow] Failed to fetch table/curve data:', e);
        setError(`Failed to load ${popOutData.type}: ${e}`);
      }
    })();
  }, [popOutData?.tabId, popOutData?.type, popOutData?.data]);

  // Listen for table updates from main window
  useEffect(() => {
    if (!popOutData) return;

    let unlisten: UnlistenFn | null = null;

    (async () => {
      try {
        unlisten = await listen<{ tabId: string; data: unknown }>('table:updated', (event) => {
          if (event.payload.tabId === popOutData.tabId) {
            setPopOutData(prev => prev ? { ...prev, data: event.payload.data as TunerTableData } : null);
          }
        });
      } catch (e) {
        console.error('Failed to listen for table updates:', e);
      }
    })();

    return () => {
      if (unlisten) unlisten();
    };
  }, [popOutData?.tabId]);

  // Fetch constants for dialog context
  useEffect(() => {
    if (popOutData?.type === 'dialog') {
      invoke<Record<string, number>>('get_all_constant_values')
        .then(setConstantValues)
        .catch(console.error);
    }
  }, [popOutData?.type]);

  // Handle data changes and sync back to main window
  const handleDataChange = useCallback((newData: TunerTableData) => {
    if (!popOutData) return;

    setPopOutData(prev => prev ? { ...prev, data: newData } : null);

    // Emit to main window
    emit('table:updated', {
      tabId: popOutData.tabId,
      type: popOutData.type,
      data: newData,
    }).catch(console.error);
  }, [popOutData]);

  const handleCurveChange = useCallback((yBins: number[]) => {
    if (!popOutData || popOutData.type !== 'curve' || !popOutData.data) return;
    const curveData = popOutData.data as CurveData;
    const updatedData = { ...curveData, y_bins: yBins };
    setPopOutData(prev => prev ? { ...prev, data: updatedData } : null);
    emit('table:updated', {
      tabId: popOutData.tabId,
      type: 'curve',
      data: updatedData,
    }).catch(console.error);
  }, [popOutData]);

  // Dock back to main window
  const handleDockBack = useCallback(async () => {
    if (!popOutData) return;

    try {
      // Emit dock event with current data
      await emit('tab:dock', {
        tabId: popOutData.tabId,
        type: popOutData.type,
        title: popOutData.title,
        data: popOutData.data,
      });

      // Close this window
      await getCurrentWindow().close();
    } catch (e) {
      console.error('Failed to dock back:', e);
    }
  }, [popOutData]);

  // Handle close button
  const handleClose = useCallback(async () => {
    await getCurrentWindow().close();
  }, []);

  // Render content based on type
  const renderContent = () => {
    if (!popOutData) return null;

    switch (popOutData.type) {
      case 'dashboard':
        return <TsDashboard />;
      
      case 'table':
        // Show loading while fetching data
        if (!popOutData.data) {
          return (
            <div className="popout-loading">
              <p>Loading {popOutData.type}...</p>
            </div>
          );
        }
        return (
          <TableEditor
            data={popOutData.data as TunerTableData}
            onChange={handleDataChange}
            onBurn={() => {
              // Trigger burn dialog in main window
              emit('action:burn', {}).catch(console.error);
            }}
          />
        );
      case 'curve':
        if (!popOutData.data) {
          return (
            <div className="popout-loading">
              <p>Loading curve...</p>
            </div>
          );
        }
        return (
          <CurveEditor
            data={popOutData.data as CurveData}
            embedded={false}
            simpleGaugeInfo={popOutData.gauge ?? undefined}
            onValuesChange={handleCurveChange}
            onBack={handleClose}
          />
        );
      
      case 'dialog':
        // Show loading while fetching dialog definition
        if (!popOutData.data) {
          return (
            <div className="popout-loading">
              <p>Loading dialog...</p>
            </div>
          );
        }
        return (
          <DialogRenderer
            definition={popOutData.data as RendererDialogDef}
            onBack={handleClose}
            openTable={(tableName) => {
              // Tell main window to open the table
              emit('action:openTable', { tableName }).catch(console.error);
            }}
            context={constantValues}
            displayTitle={popOutData.title}
            onUpdate={async () => {
              // Refresh constants
              const values = await invoke<Record<string, number>>('get_all_constant_values');
              setConstantValues(values);
            }}
          />
        );
      
      case 'autotune':
        return (
          <AutoTune
            tableName={(popOutData.data as string) || 'veTable1'}
            onClose={handleClose}
          />
        );
      
      case 'datalog':
        return <DataLogView />;
      
      default:
        return <div className="popout-unsupported">Unsupported content type</div>;
    }
  };

  if (loading) {
    return (
      <ThemeProvider>
        <div className="popout-window popout-loading">
          <p>Loading...</p>
        </div>
      </ThemeProvider>
    );
  }

  if (error) {
    return (
      <ThemeProvider>
        <div className="popout-window popout-error">
          <p>{error}</p>
          <button onClick={handleClose}>Close</button>
        </div>
      </ThemeProvider>
    );
  }

  return (
    <ThemeProvider>
      <div className="popout-window">
        {/* Header bar with dock-back button */}
        <div className="popout-header">
          <button 
            className="popout-dock-btn"
            onClick={handleDockBack}
            title="Dock back to main window"
          >
            <ArrowLeftToLine size={16} />
            <span>Dock</span>
          </button>
          <h1 className="popout-title">{popOutData?.title}</h1>
          <button 
            className="popout-close-btn"
            onClick={handleClose}
            title="Close window"
          >
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="popout-content">
          {renderContent()}
        </div>
      </div>
    </ThemeProvider>
  );
}
